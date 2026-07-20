# engineer-of-source experience — Marix

## Workflow runtime

- The active hierarchy is Task → Intent → Step → Invocation/Relay. Intent owns plain Plan state; there is no active Plan actor or protocol namespace.
- Register every child actor in its task-wide registry before `start`. On termination, commit lifecycle result/status and complete the registry entry before notifying the owner; reversing either order creates lookup races.
- `Lifecycle` stores `ActorStatus<Result>` atomically. Parent updates carry the terminal child result; use registry result lookup only for dynamic reconstruction.
- Task uses a current-thread Tokio runtime driven by a dedicated standard thread. Spawn child actors through that shared runtime; blocking the task dispatcher prevents every nested actor from progressing.
- `TaskAccess` permits construction/insertion only. Keep lookup, completion, cancellation, lineage checks, and routing in `TaskRuntime`; retain its weak `SessionContext` link.

## Model and tool boundaries

- Deepseek is stream-only. Buffer SSE tool-call fragments by index, require a valid terminal `[DONE]`, validate complete JSON-object arguments, then emit one normalized `StepDraft`; never execute partial deltas.
- 2026-07-20: DeepSeek chat-completion payloads explicitly disable Thinking in the base payload because Thinking rejects `tool_choice`; keep forced tool relays on `tool_choice: "required"` and non-tool relays on JSON response format.
- Invocation validates the exact registered tool name and JSON arguments. Return validation and unknown-tool failures through normal typed results so parent actors cannot strand.
- Windows managed processes launch executables directly with explicit arguments. Validate UUID paths, reject symlinks, bind ownership to PID plus creation time, bound output pages, and retain the lock file.

## Transport and configuration

- Remoc setup must keep its connection future driven after returning channels. Abort guarded connection tasks on timeout/rejection; otherwise failed handshakes leak workers.
- Server channel listeners bind the IPv4 wildcard independently of `server.ip`; connectors resolve the configured address. Server-driven host registration sends session identity and tool query before publishing the channel.
- Install a remote telemetry sink only after TCP, remoc, and token acceptance succeed. Startup telemetry exhaustion is fatal rather than a silent local fallback.

## Repository invariants

- Keep public protocol re-exports synchronized between `protocol/lib.rs` and its mirrored `protocol/mod.rs`.
