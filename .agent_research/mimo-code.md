# Xiaomi MiMo Code / MiMo Agent External Source Research

## 1. Source and activity

- Official GitHub organization: <https://github.com/XiaomiMiMo>
- Official repository: <https://github.com/XiaomiMiMo/MiMo-Code>
- The repository description is `MiMo Code: Where Models and Agents Co-Evolve`.
- License: MIT.
- Default branch: `main`.
- Repository activity captured in the research material:
  - `created_at`: 2026-06-10
  - `pushed_at`: 2026-06-22
  - `updated_at`: 2026-06-22
- Recent commit themes include `fix(metrics)`, `feat(skill)`, `feat(checkpoint)`, and `feat(tool)`, which indicates that agent, tool, memory, and checkpoint features were still moving quickly.
- A separate, more authoritative official "MiMo Agent" repository was not identified. `XiaomiMiMo/MiMo-Code` is the best match for MiMo Code/MiMo Agent architecture research.

## 2. Technical stack and project nature

MiMo Code is a terminal-native AI coding assistant. The codebase keeps the OpenCode fork structure and adds MiMo-side memory, checkpointing, task/subagent orchestration, goal-driven loop behavior, compose, dream, and distill features.

| Layer | Technology |
|---|---|
| Runtime and package management | Bun, TypeScript ESM |
| Agent and LLM | Vercel AI SDK `ai`, multiple `@ai-sdk/*` providers |
| State and effects | Effect, Bus/SyncEvent, OpenTelemetry |
| CLI and TUI | yargs, SolidJS, OpenTUI |
| Storage | SQLite, Drizzle ORM, FTS5 |
| Tool protocol | Built-in tool registry, plugin tools, MCP SDK |
| Shell analysis | `tree-sitter-bash`, `tree-sitter-powershell` |
| Git and file changes | diff, snapshot, patch/revert, file watcher |

`packages/opencode/package.json` exposes the CLI package `@mimo-ai/cli`, with `mimo` as the binary. The development entry uses `bun run --conditions=browser ./src/index.ts`.

## 3. Entrypoints and modules

Primary entrypoints:

- `packages/opencode/src/index.ts`
  - yargs CLI entrypoint.
  - Registers commands such as `run`, `generate`, `serve`, `mcp`, `agent`, `models`, `session`, `plugin`, `github`, `pr`, and `db`.
  - Initializes logging, heap diagnostics, environment variables, SQLite migrations, and Claude session import.
- `packages/opencode/src/session/prompt.ts`
  - Main agent loop.
- `packages/opencode/src/session/processor.ts`
  - Consumes model stream events, executes tool calls, and updates message parts.
- `packages/opencode/src/session/llm.ts`
  - Builds system prompt, model messages, tools, provider request, and retry stream.
- `packages/opencode/src/tool/registry.ts`
  - Registers built-in, plugin, and custom tools.
- `packages/opencode/src/agent/agent.ts`
  - Defines primary agents, subagents, hidden/system agents, and built-in modes.

Core module map:

| Module | Key path | Role |
|---|---|---|
| Agent definitions | `agent/agent.ts` | `build`, `plan`, `compose`, `max`, `general`, `explore`, hidden agents |
| Agent loop | `session/prompt.ts` | Multi-turn model call, tool execution, overflow handling, goal/task gates |
| LLM call | `session/llm.ts` | Provider adaptation, system prompt, tools, `streamText`, retry |
| Stream processor | `session/processor.ts` | State machine for reasoning, text, and tool stream parts |
| Tool abstraction | `tool/tool.ts` | Tool schema, execution, recoverable errors, output truncation |
| Tool registry | `tool/registry.ts` | read/edit/write/bash/grep/glob/actor/memory/task/patch and plugins |
| Subagent/Actor | `tool/actor.ts`, `actor/spawn.ts` | spawn/run/status/wait/cancel/send |
| Memory | `memory/service.ts` | SQLite FTS/BM25 memory recall |
| Checkpoint | `session/checkpoint.ts` | Checkpoint writer, context rebuild, overflow recovery |
| Permission | `permission/evaluate.ts` | allow/ask/deny ruleset matching |
| MCP | `mcp/index.ts` | stdio, HTTP, SSE, OAuth MCP client |
| Task registry | `task/registry.ts` | create/list/start/done/block/abandon task state |
| Revert/diff | `session/revert.ts` | snapshot diff, restore, revert |

## 4. Agent loop

MiMo Code's agent loop is centered on `session/prompt.ts`. The high-level execution flow is:

1. Load the compacted message slice for the current session.
2. Isolate context for the main agent or a subagent actor.
3. Inject system prompt, provider prompt, memory recall hints, project/session/global memory instructions, skills, environment details, and local instructions.
4. Build the LLM request prefix.
5. Call the model through `session/llm.ts` using the AI SDK stream.
6. Let `session/processor.ts` consume stream events:
   - reasoning start/delta/end
   - text start/delta/end
   - tool input delta
   - tool call
   - tool result or error
   - finish and step finish
7. If the model calls tools, execute the tools and append results back into message history.
8. If the model does not call tools, evaluate final answer, goal judge, task gate, stop conditions, invalid output retry, and text-repeat recovery.
9. On context overflow:
   - subagents use a per-actor compaction boundary;
   - the main agent first waits for the checkpoint writer and rebuilds context;
   - if checkpoint rebuild is unavailable, it falls back to LLM compaction.
10. Continue until completion, failure, abort, or another terminal condition.

Important design observations:

- Model output is classified explicitly as states such as `filtered`, `failed`, `think-only`, `invalid`, `final`, and `continue`.
- Overflow handling combines checkpoint, memory, compaction, and tail preservation instead of relying on one summarization step.
- Subagent context is separated from main context, reducing the risk that noisy tool results pollute the primary conversation.

## 5. Tool protocol and model adaptation

`tool/tool.ts` defines a shared `Tool.Def` abstraction with:

- `id`
- `description`
- Zod `parameters`
- `execute`
- `formatValidationError`
- shell parser and recovery hooks

`tool/registry.ts` registers built-in tools:

- shell: `bash`
- file operations: `read`, `edit`, `write`, `patch`
- search: `glob`, `grep`
- agent orchestration: `actor`
- network: `fetch`, `search`
- workflow: `question`, `planenter`, `planexit`
- state: `memory`, `history`, `task`, `workflow`
- plugin and custom tools

Model adaptation is concentrated in `session/llm.ts`:

- Uses Vercel AI SDK `streamText`.
- Supports providers such as Anthropic, OpenAI, Google, Bedrock, Groq, Mistral, OpenRouter, xAI, DeepInfra, GitLab, and others.
- Lets plugin hooks rewrite chat parameters, headers, and system prompt content.
- Injects a `_noop` dummy tool for compatibility with LiteLLM/GitHub Copilot-like protocols when history contains tool calls but the current request has no tools.
- Provides dedicated tool execution and approval handling for GitLab workflow models.
- Emits `Session.Event.RetryAttempt` around retry attempts.

## 6. Context, state, and memory

MiMo Code uses layered context management:

| Layer | Mechanism |
|---|---|
| Short-term context | Session messages and parts |
| Tool result control | Processor tracks tokens, cost, and changed files |
| Memory recall | SQLite FTS5, `memory_fts_idx`, BM25, snippets |
| Durable files | Project/session/task/global `MEMORY.md`, `checkpoint.md`, and notes |
| Checkpoint writer | Hidden subagent `checkpoint-writer` |
| Context rebuild | Checkpoint + memory + notes + preserved tail |
| Task progress | `tasks/<id>/progress.md` and related task files |

The checkpoint writer in `session/checkpoint.ts` is the key pattern. The main agent does not carry all durable memory maintenance itself. A hidden, specialized subagent updates structured checkpoints, which reduces pressure on the main loop context and makes rebuilds more deterministic.

## 7. Permissions, sandbox, and security

MiMo Code mainly provides permission controls rather than a strong container sandbox.

Important mechanisms:

- `permission/evaluate.ts` flattens rulesets, applies wildcard matching, and defaults to ask.
- Default rules allow common safe tools, ask for `doom_loop`, ask for external directories, ask before reading `.env` or `.env.*`, and allow `.env.example`.
- The `plan` agent is prevented from editing and is only allowed to write to plan paths.
- `bash.ts`:
  - Uses an approximately two-minute default timeout.
  - Uses tree-sitter to analyze bash and PowerShell command paths.
  - Asks before external-directory or bash-sensitive operations.
  - Supports abort/timeout and returns `<bash_metadata>`.
- `edit.ts`:
  - Calls `assertWriteAllowed`.
  - Applies edit permission checks.
  - Records diff metadata.
  - Provides fuzzy replacement logic for whitespace, indentation, escapes, and context anchors.

Security risks:

- The agent can still execute local shell commands; practical safety depends on permission prompts and rules.
- Plugin/custom tools and MCP servers widen the trusted surface.
- A bypass mode or overly broad allow rule can weaken the boundary significantly.

## 8. Events, logs, and observability

- `session/processor.ts` publishes `Metrics.ToolCall`, `Metrics.ModelCall`, and step-finish metrics.
- `session/session.ts` writes sessions, messages, and parts into SQLite/SyncEvent.
- Bus and SyncEvent coordinate cross-module event flow.
- OpenTelemetry is used for model, tool, and performance observation.
- Snapshot, diff, and revert paths emit `Session.Event.Diff`.

The overall pattern is that model calls, tool calls, message state, and file diffs are observable as first-class events rather than ad hoc logs.

## 9. Testing and validation

The repository includes broad tests under `packages/opencode/test/`, including:

- actor lifecycle, spawn, waiter, and status behavior
- agent registry and allowlist behavior
- MCP lifecycle, OAuth, and headers
- permission abort, non-interactive mode, and disabled permissions
- memory FTS, reconcile, and paths
- plugin lifecycle and hooks
- provider conversion, errors, and model groups
- patch, revert, and diff behavior
- task, inbox, session, and history behavior
- TUI, plugin, and UI behavior

CI workflow files include:

- `.github/workflows/lint.yml`
- `.github/workflows/test.yml`
- `.github/workflows/typecheck.yml`

## 10. Core paths

Recommended paths for deeper follow-up:

- `packages/opencode/src/index.ts`
- `packages/opencode/src/agent/agent.ts`
- `packages/opencode/src/session/prompt.ts`
- `packages/opencode/src/session/processor.ts`
- `packages/opencode/src/session/llm.ts`
- `packages/opencode/src/session/checkpoint.ts`
- `packages/opencode/src/session/compaction.ts`
- `packages/opencode/src/session/max-mode.ts`
- `packages/opencode/src/session/goal.ts`
- `packages/opencode/src/tool/tool.ts`
- `packages/opencode/src/tool/registry.ts`
- `packages/opencode/src/tool/bash.ts`
- `packages/opencode/src/tool/edit.ts`
- `packages/opencode/src/tool/actor.ts`
- `packages/opencode/src/actor/spawn.ts`
- `packages/opencode/src/memory/service.ts`
- `packages/opencode/src/task/registry.ts`
- `packages/opencode/src/mcp/index.ts`
- `packages/opencode/src/permission/evaluate.ts`

## 11. Lessons for {{proj}}

1. Keep the main loop focused on orchestration; delegate durable context maintenance to a checkpoint writer.
2. Treat subagents/actors as first-class system objects rather than simple tool callbacks.
3. Preserve an explainable permission chain: default ask, plus scoped allow and deny rules.
4. Let tool definitions own schema, permission, execution, recovery, and output truncation.
5. Prefer rebuildable checkpoints over repeated full LLM summaries when context overflows.
6. Bind task registry state to actor lifecycle so background work can be resumed, queried, and cancelled.
7. Add engineering fallbacks for model protocol differences, such as dummy tools, retry streams, and tool-result repair.

## 12. Risks and anti-patterns

- The project was very new in mid-June 2026; architectural stability is not yet proven.
- The OpenCode-derived module graph is large and has a high learning cost.
- Strong local shell and file-edit capabilities are not equivalent to sandboxing.
- Hidden agents, checkpoints, memory, tasks, plugins, and MCP can interact in ways that are hard to debug.
- Multi-provider and plugin hooks improve flexibility but also increase unpredictable input surfaces.
- Avoid copying the whole architecture wholesale; extract only the checkpoint, permission, and actor lifecycle patterns that match {{proj}} boundaries.
