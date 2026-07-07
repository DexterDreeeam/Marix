# OpenCode Coding Agent Research

## 1. Source, activity, technical stack, and nature

Primary material: the completed external `coding-agent-research` output and upstream sources.

| Item | Details |
|---|---|
| User-mentioned repository | https://github.com/sst/opencode |
| Current authoritative repository | https://github.com/anomalyco/opencode |
| Verification | GitHub API returned or redirected `sst/opencode` to `anomalyco/opencode` |
| Default branch | `dev` |
| Main language | TypeScript |
| Stack | Bun, TypeScript, Effect, AI SDK, Hono, Drizzle SQLite, SolidJS, OpenTUI, Electron, MCP |
| Activity evidence | Recent push on 2026-06-22; latest release `v1.17.9` on 2026-06-21 |
| License | MIT |

OpenCode is a TypeScript agent platform with a strong event model, CLI/server/TUI/web/desktop surfaces, plugin hooks, MCP lifecycle support, and Effect-based typed runtime boundaries.

## 2. Entry points and modules

Top-level package shape:

```text
packages/
  opencode/       # CLI, session, tools, config, server runtime glue
  core/           # agent config, permission, session event, database, filesystem
  llm/            # LLM request/schema/tool/provider/protocol
  plugin/         # plugin SDK and hooks
  sdk/js/         # generated JS SDK
  server/         # HTTP API
  tui/            # TUI
  console/        # OpenTUI/Solid console app
  app/ web/ ui/   # web UI packages
  desktop/        # Electron desktop
  containers/     # container-related package
```

The CLI entry is `packages/opencode/src/index.ts`, implemented with `yargs`. Commands include `run`, `serve`, `web`, `attach`, `mcp`, `agent`, `models`, `session`, `plugin`, `db`, and `acp`.

The root package uses `bun@1.3.14`, `effect 4.0.0-beta.83`, `ai 6.0.168`, `hono`, `drizzle-orm`, `diff`, `@modelcontextprotocol/sdk`, `solid-js`, `@opentui/*`, and `typescript 5.8.2`.

## 3. Agent loop

The core loop is distributed around `packages/opencode/src/session/processor.ts` and related session files. `SessionProcessor.create()` maps an LLM stream into session messages, message parts, tool state, and events.

```text
create assistant message
  -> snapshot.track()
  -> build ProcessorContext
  -> llm.stream(...)
  -> handleEvent(LLMEvent):
       - reasoning-start/delta/end
       - text-start/delta/end
       - tool-input-start/delta/end
       - tool-call
       - tool-result
       - usage/finish/error
  -> session.updatePart()
  -> EventV2Bridge.publish(SessionEvent.*)
  -> tool call state pending/running/completed/error
  -> compaction / retry / stop / continue
```

`DOOM_LOOP_THRESHOLD = 3` protects against repeated abnormal loops. The runtime treats streaming text, reasoning, tool input, tool calls, and usage as first-class state transitions.

## 4. Planner / executor

OpenCode expresses planning and execution through agent modes, permissions, and runtime components.

| Agent | Role |
|---|---|
| `build` | Default execution/modification agent |
| `plan` | Read-only or planning-oriented agent |
| `general` | General subagent / delegated task agent |

`packages/core/src/agent.ts` defines `AgentV2.Info` with `id`, `model`, `request`, `system`, `description`, `mode`, `hidden`, `color`, `steps`, and `permissions`.

The executor is a composition of session processor, LLM stream client, tool runtime, permission service, plugin hooks, and EventV2. This avoids one monolithic executor class but requires strict event and permission contracts.

## 5. Tool abstraction

Core paths:

- `packages/llm/src/tool.ts`
- `packages/core/src/tool/*`
- `packages/opencode/src/tool/*`

`Tool.make()` supports two forms:

| Mode | Description |
|---|---|
| Typed | Effect `Schema` for parameters and success output with decode/encode |
| Dynamic | JSON Schema, useful for MCP and plugin-provided tools |

Tool fields include `description`, `parameters`, `success`, `execute`, `toModelOutput`, `toStructuredOutput`, `_decode`, `_encode`, and `_definition`. Outputs can be structured, model-facing text, or legacy results.

Built-in examples include `bash` in `packages/core/src/tool/bash.ts`, `write` in `packages/opencode/src/tool/write.ts`, read/edit tools under `packages/opencode/src/tool/*`, MCP tools in `packages/opencode/src/mcp/index.ts`, and web/search-style tools.

## 6. Model / provider adaptation

Important paths:

- `packages/llm/src/provider.ts`
- `packages/llm/src/llm.ts`
- `packages/llm/src/route/*`
- `packages/llm/src/providers/*`

`Provider.Definition` includes `id`, `model(factory)`, and optional `apis`. `LLM.request()` normalizes system, prompt, messages, tools, tool choice, generation settings, provider options, and HTTP options.

`generateObject()` implements cross-provider structured output by forcing a synthetic tool call rather than relying on each provider's native JSON mode. Provider support comes from AI SDK and internal routes/protocols, covering common providers such as OpenAI, Anthropic, Google, Bedrock, Groq, and xAI.

## 7. Context construction

Context sources include `AGENTS.md` and config, `AgentV2.Info.system`, session messages, snapshots, LSP diagnostics after writes, plugin hooks such as `experimental.chat.messages.transform` and `experimental.chat.system.transform`, compaction events, active location, and worktree information.

Relevant paths:

- `packages/core/src/session/message.ts`
- `packages/core/src/session/event.ts`
- `packages/opencode/src/session/summary.ts`
- `packages/opencode/src/session/overflow.ts`

The design treats context as both persisted session state and a transformable stream boundary. Plugin transforms can affect messages and system prompts, while compaction records remain in the session event timeline.

## 8. File editing and diff

The write tool is `packages/opencode/src/tool/write.ts`.

```text
resolve file path
  -> assertExternalDirectoryEffect()
  -> read existing content and BOM
  -> createTwoFilesPatch()
  -> ctx.ask(permission="edit", metadata.diff)
  -> fs.writeWithDirs()
  -> format.file()
  -> publish FileSystem.Event.Edited
  -> publish Watcher.Event.Updated
  -> LSP touchFile + diagnostics
  -> return diagnostics/output
```

Important traits: BOM preservation, diff preview in permission metadata, write-after-formatting, write-after-LSP diagnostics, and approval for external directory access.

## 9. Command execution, sandbox, and permissions

Bash tool path: `packages/core/src/tool/bash.ts`.

Parameters include `command`, `workdir`, `timeout`, and `description`. Defaults include a two-minute timeout, ten-minute maximum timeout, 1 MB capture for stdout and stderr, POSIX `/bin/sh`, Windows `COMSPEC`/`cmd.exe`, detached process group, and `AppProcess.run()` for timeout/output control.

Permission model:

- `PermissionV2.Rule` has `action`, `resource`, and `effect`.
- Effects are `allow`, `deny`, and `ask`.
- Bash calls `permission.assert({ action: "bash", resources: [command] })` before execution.
- External directories require an `external_directory` permission assertion.

The bash description explicitly states that commands use the host user's filesystem, process, and network authority. Containers exist as a package area, but the core bash tool is not a hard sandbox by default.

## 10. Memory and state persistence

OpenCode is heavily evented and database-backed.

| Path / concept | Role |
|---|---|
| `packages/core/src/session/event.ts` | durable and ephemeral session events |
| `packages/core/src/database/` | database layer |
| `drizzle-orm` | SQLite schema/ORM |
| `SessionEvent.*` | step, text, tool, reasoning, compaction, retry events |
| snapshot | file-system snapshot per session step |
| message parts | text, reasoning, tool input, and tool result parts |

Durable events can rebuild state; ephemeral events represent stream deltas such as text, reasoning, and tool-input deltas. This split supports both replay and responsive streaming UI.

## 11. Event stream, logging, and audit

`packages/core/src/session/event.ts` defines events such as `Step.Started/Ended/Failed`, `Text.Started/Delta/Ended`, `Reasoning.Started/Delta/Ended`, `Tool.Input.Started/Delta/Ended`, `Tool.Called/Progress/Success/Failed`, `Shell.Started/Ended`, `Compaction.Started/Delta/Ended`, and `Retried`.

This model is useful for streaming UI, replay, audit, partial/durable event separation, and recovery of tool run status. It is one of OpenCode's strongest architectural features.

## 12. Testing strategy

Repository notes indicate that tests should not be run from the root. Package tests include CLI/server/MCP lifecycle/httpapi tests under `packages/opencode/test/*`, Playwright e2e dependencies, TypeScript typecheck, oxlint, and package-local provider/tool/session tests.

Important test areas: CLI MCP add, HTTP API MCP, MCP lifecycle, config, server API, session event model, tool execution, and permission behavior.

## 13. Plugins, MCP, and extension model

Plugin path: `packages/plugin/src/index.ts`. A plugin is shaped as:

```text
Plugin(input, options) => Promise<Hooks>
```

`PluginInput` provides SDK client, project, directory, worktree, workspace adapter registry, server URL, and Bun shell `$`.

Hooks include event subscription, config mutation, tool registration, provider auth, provider/model hooks, `chat.message`, `chat.params`, `chat.headers`, `permission.ask`, `command.execute.before`, `tool.execute.before/after`, `shell.env`, experimental chat message/system transforms, and experimental session compaction prompts.

MCP support in `packages/opencode/src/mcp/index.ts` covers stdio local MCP, streamable HTTP, SSE fallback, OAuth, tools, prompts, resources, `mcp.tools.changed` events, roots capability, and statuses such as connected, disabled, failed, needs_auth, and needs_client_registration.

## 14. Core source file paths

Recommended paths for architecture review:

- `packages/opencode/src/index.ts`
- `packages/core/src/agent.ts`
- `packages/opencode/src/session/processor.ts`
- `packages/core/src/session/event.ts`
- `packages/llm/src/tool.ts`
- `packages/llm/src/llm.ts`
- `packages/llm/src/provider.ts`
- `packages/core/src/tool/bash.ts`
- `packages/opencode/src/tool/write.ts`
- `packages/opencode/src/mcp/index.ts`
- `packages/plugin/src/index.ts`

## 15. Lessons for `Marix`

1. Separate durable events from ephemeral stream deltas to support both replay and responsive UI.
2. Use typed schema-based tools where possible and JSON Schema dynamic tools for MCP/plugins.
3. Put real diffs into permission metadata so approval is based on concrete file changes.
4. Feed LSP diagnostics back after writes so the agent sees edit side effects immediately.
5. Make plugin hooks precise across chat, tools, permissions, shell environment, and compaction.
6. Model MCP lifecycle fully, including auth, status, resources, prompts, tools, and roots.
7. Snapshot before LLM streaming to avoid losing state when provider/tool execution interleaves with edits.

## 16. Risks and anti-patterns

| Risk / anti-pattern | Why it matters |
|---|---|
| Effect learning curve | Advanced typed effects can slow ordinary TypeScript contributors |
| Bun binding | Enterprise deployment compatibility needs validation |
| Non-sandboxed shell | Default command authority is still the host |
| SQLite concurrency | Many agents or sessions may create lock contention |
| Powerful plugins | Deep hooks can rewrite behavior and need supply-chain audit |
| Complex event model | Durable/ephemeral dual tracks require strict replay tests |
| Default `dev` branch | Stability expectations must be checked against release policy |
| Over-transforming context | Plugin transforms can make prompts hard to audit if not logged clearly |
