# Cline Coding Agent Research

## 1. Source, activity, technical stack, and nature

Primary material: the completed external `coding-agent-research` output, cross-checked against upstream repository files.

| Item | Details |
|---|---|
| Repository | https://github.com/cline/cline |
| Main language | TypeScript |
| Stack | Bun, Node.js, TypeScript, VS Code WebView, OpenTUI, WebSocket, SQLite/file storage, AI SDK, multi-provider LLM support |
| Activity evidence | GitHub API showed a recent push on 2026-06-22; latest release `cli-v3.0.29` on 2026-06-20 |
| License | Apache-2.0 |

Cline has moved toward an SDK and monorepo architecture. The main split is `shared -> llms -> agents -> core -> apps`: `@cline/agents` is intentionally stateless, while `@cline/core` owns sessions, persistence, tools, compaction, hub, and runtime hosting.

## 2. Entry points and modules

Current structure:

```text
sdk/
  ARCHITECTURE.md
  packages/
    shared/       # types, tools, hooks, storage, prompts, logging, remote config
    llms/         # provider gateway, model catalog, AI SDK providers
    agents/       # stateless agent loop
    core/         # session lifecycle, runtime host, storage, plugins, cron, hub
    sdk/          # SDK packaging
apps/
  cli/            # Bun CLI, OpenTUI, headless execution
  vscode/         # VS Code extension
  cline-hub/      # hub and webview surface
  examples/
```

`package.json` requires `bun@1.3.13`, Node `>=22`, and exposes `build:sdk`, `test:e2e`, and `test:unit` scripts. Workspaces include `sdk/packages/*`, `apps/*`, and related packages.

## 3. Agent loop

The core loop is `AgentRuntime.execute()` in `sdk/packages/agents/src/agent-runtime.ts`.

```text
AgentRuntime.run()/continue()
  -> ensureInitialized()
       - register tools
       - setup plugins and hooks
  -> status=running, runId=createUID
  -> beforeRun hooks
  -> append user message
  -> while iteration < maxIterations:
       - emit turn-started
       - generateAssistantMessage()
           - beforeModel hooks
           - model.stream/chat
           - afterModel hooks
       - append assistant message
       - extract tool-call parts
       - if no tool call:
           - completion guard / reminder
           - finish run
       - executeToolCalls()
           - beforeTool hooks
           - tool policy
           - execute tools sequentially or in parallel
           - afterTool hooks
       - append tool results to messages
       - finish if completion tool succeeds
  -> afterRun hooks
```

Event types include `run-started`, `turn-started`, `message-added`, `assistant-message`, `tool-start`, `tool-finish`, `turn-finished`, `run-finished`, usage events, and content update events.

## 4. Planner / executor

Cline does not hard-code a planner/executor class split. It uses mode, prompt, tool policy, and lifecycle tools to create planning and execution behavior.

| Mechanism | Role |
|---|---|
| Plan/build mode | Changes available tools, prompt, and approval policy |
| Completion tool | `submit_and_exit`-style tool marks a run complete |
| `maxIterations` | Guards against infinite loops |
| Hooks | `beforeModel`, `beforeTool`, `afterTool`, and similar hooks can alter runtime behavior |
| Tool policies | Merge global `*` policy with per-tool allow/ask/deny decisions |
| Subagent/team primitives | SDK can model delegated or team-style execution |

The key separation is that `@cline/agents` remains stateless while `@cline/core` handles state, session orchestration, default tools, compaction, plugins, and hub integration.

## 5. Tool abstraction

The tool creation API lives in `sdk/packages/shared/src/tools/create.ts`.

| Field | Role |
|---|---|
| `name` | Tool identifier exposed to the model |
| `description` | Model-facing behavior description |
| `inputSchema` | JSON Schema or Zod schema |
| `execute(input, context)` | Runtime implementation |
| `lifecycle.completesRun` | Whether success terminates the agent run |
| `timeoutMs` | Default timeout, normally 30 seconds |
| `retryable` / `maxRetries` | Retry contract, defaulting to retryable with up to 3 retries |

Registration normalizes JSON Schema, supports Zod-to-JSON-Schema conversion, requires object input schemas, and passes execution context with signal and session data. Built-in tool areas include `read_file`, `write_to_file`, `edit_file`, `apply_patch`, `bash`, `search_files`, `fetch_web`, `list_code_definition_names`, and a completion tool.

## 6. Model / provider adaptation

Provider layering:

```text
@cline/llms
  -> createGateway(providerConfigs)
  -> gateway.createAgentModel({ providerId, modelId })
@cline/agents
  -> depends only on the AgentModel interface
@cline/core
  -> provider settings, telemetry, runtime wiring
```

Supported providers include Anthropic, OpenAI, Google/Gemini, AWS Bedrock, Azure, Vertex, OpenRouter, Ollama, LM Studio, and OpenAI-compatible endpoints. `agent-runtime.ts` supports streaming, reasoning/text/tool-call parts, token usage, cache read/write tokens, provider finish reasons, abort, and cancellation.

## 7. Context construction

`@cline/core` owns context strategy; `@cline/agents` exposes preparation seams. Context sources include initial runtime messages, system prompt, project rules from `.cline` / `.clinerules` / managed rules, materialized skills, hook and extension transformations, tool results, and core-owned context compaction.

`SDK/ARCHITECTURE.md` explicitly places context compaction in `core`, not in `agents`. That boundary keeps the loop reusable and testable while allowing applications to choose their own memory and compression policies.

## 8. File editing and diff

Editing capabilities sit in core tool executors and UI layers.

| Area | Behavior |
|---|---|
| Write/edit/apply_patch | Host-side tool executor modifies files |
| Diff preview | VS Code and WebView surfaces show proposed changes |
| Undo | User commands can restore prior edits |
| Output limits | Tool output is trimmed before re-entering context |
| Approval | Write operations usually require explicit approval or an allow policy |

Relevant paths are `sdk/packages/core/src/extensions/tools/`, `sdk/packages/shared/src/diff/`, and VS Code diff UI files under `apps/vscode/src/...`.

## 9. Command execution, sandbox, and permissions

The bash executor path is `sdk/packages/core/src/extensions/tools/executors/bash.ts`.

| Item | Behavior |
|---|---|
| Execution | Node.js `spawn` |
| Shell | Unix uses `$SHELL`/bash; Windows has PowerShell/cmd logic |
| Timeout | Default 30 seconds |
| Output | Rolling collector preserves head and tail and truncates the middle |
| Cancel | `AbortSignal` |
| Process tree | Unix kills process group; Windows uses process-tree termination logic |
| cwd/env | Provided by runtime session |

Cline has no default hard sandbox for local mode. Commands run with host workspace authority and rely on approval flow, tool policy, plan mode, auto-approve settings, local hub discovery tokens, and remote runtime boundaries.

## 10. Memory and state persistence

`@cline/core` owns session lifecycle, storage and persistence, config watching/loading, durable cron queue, hub sessions/events/approvals/schedules, and usage telemetry.

| Persisted area | Notes |
|---|---|
| Session messages | Managed by core storage adapters |
| Cron | `packages/core/src/cron/` with SQLite `cron.db` |
| Settings | Core settings facade and file watcher |
| Rules, skills, hooks, plugins | File-system watchers and reconciliation |
| Hub discovery | Owner-only discovery record plus auth token |
| Usage | Root and aggregate usage buckets |

## 11. Event stream, logging, and audit

`AgentRuntimeEvent` flows to host UI and hub transports. Events cover text and reasoning deltas, tool start/finish, message-added, assistant-message, run-finished, task completion telemetry, and usage/cost buckets.

Logging is abstracted through `BasicLogger` with `debug`, `log`, and `error`. CLI uses a Pino adapter, VS Code uses an OutputChannel, and telemetry sinks can mirror telemetry into the logger. This creates a clean separation between runtime events and environment-specific presentation.

## 12. Testing strategy

Package scripts use Bun and Vitest, with `test:unit` and `test:e2e`. Key test areas include:

| Path | Focus |
|---|---|
| `sdk/packages/agents/src/agent-runtime.test.ts` | agent loop |
| `sdk/packages/core/src/ClineCore.test.ts` | core session orchestration |
| `sdk/packages/shared/src/tools/create.test.ts` | tool API and schema handling |
| `sdk/packages/shared/src/vcr.test.ts` | HTTP VCR/replay |
| `apps/cli/*e2e*` | CLI end-to-end behavior |

## 13. Plugins, MCP, and extension model

Cline extension points include plugins that register tools and hooks, lifecycle hooks (`beforeRun`, `afterRun`, `beforeModel`, `afterModel`, `beforeTool`, `afterTool`, `onEvent`), MCP tools/resources, file watchers for rules/workflows/skills/agents/hooks/plugins, durable cron automation through Markdown specs with YAML frontmatter, and remote config that materializes managed rules/workflows/skills into workspace-local `.cline/...` files.

## 14. Core source file paths

Recommended paths for architecture review:

- `sdk/ARCHITECTURE.md`
- `sdk/packages/agents/src/agent-runtime.ts`
- `sdk/packages/core/src/ClineCore.ts`
- `sdk/packages/core/src/runtime/host.ts`
- `sdk/packages/shared/src/agent.ts`
- `sdk/packages/shared/src/tools/create.ts`
- `sdk/packages/llms/src/providers.ts`
- `sdk/packages/core/src/extensions/tools/executors/bash.ts`
- `sdk/packages/shared/src/diff/`
- `apps/vscode/src/extension.ts`

## 15. Lessons for `{{proj}}`

1. Keep the agent loop stateless and move session/persistence into a higher layer.
2. Define a small, typed tool API that bundles schema, timeout, retry, and lifecycle completion.
3. Introduce a runtime host boundary so local, hub, and remote execution share one contract.
4. Use completion tools for reliable termination rather than interpreting natural language as done.
5. Load file-based rules, skills, hooks, and plugins through one watcher/reconciliation pipeline.
6. Make tool policies composable with global and per-tool defaults.
7. Treat hub/shared sessions as attachable runtime views, not as independent agent authorities.

## 16. Risks and anti-patterns

| Risk / anti-pattern | Why it matters |
|---|---|
| No default hard sandbox | Local shell executes with host permissions |
| High runtime requirements | Bun 1.3.13 and Node >=22 may not fit all enterprise environments |
| Rapidly evolving SDK | Hub, remote config, and monorepo boundaries may change quickly |
| Concurrent edits | Multiple sessions need locking or merge strategy to avoid file conflicts |
| Watcher complexity | File-based extension loading can suffer cache and consistency bugs |
| Approval UX tension | Too many prompts reduce automation; too few increase risk |
| Over-hooking | Hooks can make behavior hard to reason about if they mutate prompts, tools, and policy invisibly |
