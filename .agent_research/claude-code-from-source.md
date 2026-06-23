# Claude Code from Source Research

## 1. Source and activity

- Website: <https://claude-code-from-source.com/>
- GitHub repository: <https://github.com/alejandrobalderas/claude-code-from-source>
- Material nature: independent educational analysis, not official Anthropic documentation.
- The site states that it is based on TypeScript `sourcesContent` from early npm source maps and is intended as an architectural learning resource while avoiding redistribution of original Claude Code source.
- Chapters covered by the source research:
  - `/`
  - `/ch01-architecture/`
  - `/ch02-bootstrap/`
  - `/ch03-state/`
  - `/ch04-api-layer/`
  - `/ch05-agent-loop/`
  - `/ch06-tools/`
  - `/ch07-concurrency/`
  - `/ch08-sub-agents/`
  - `/ch09-fork-agents/`
  - `/ch10-coordination/`
  - `/ch11-memory/`
  - `/ch12-extensibility/`
  - `/ch15-mcp/`
  - `/ch17-performance/`
- Chapters not fully covered in the source research include terminal UI, input interaction, remote/cloud, and epilogue material.
- Repository metadata from the source research:
  - `created_at`: 2026-04-01
  - `pushed_at`: 2026-04-04
  - `updated_at`: 2026-06-22
  - approximately 2.2k stars
- Credibility limitation: this is secondary architecture analysis and does not represent current official Claude Code source or behavior.

## 2. Technical stack and project nature

The site describes Claude Code as a terminal-native coding agent with a TypeScript/Node-style architecture, centered on six abstractions:

| Abstraction | Description |
|---|---|
| Query loop | `query.ts` async generator shared by REPL, SDK, subagent, and headless print |
| Tool system | Self-describing tools with schema, permission, concurrency, rendering, and progress |
| Tasks | Background work and subagent state machine |
| State | Bootstrap singleton plus UI reactive store |
| Memory | File-based memory at project/user/team layers |
| Hooks | Lifecycle interceptors that can block, rewrite, inject context, or force continuation |

This material is most useful as a pattern catalog: agent loop shape, tool protocol, permission resolution, context management, subagent/fork strategy, MCP adaptation, and performance tuning.

## 3. Entrypoints and modules

The source material does not expose a maintained official source tree. It instead maps conceptual modules and chapters:

- `query.ts`
  - Unified async-generator loop for REPL, SDK, subagents, headless print, compaction, and internal queries.
- Tool system chapter
  - Tool definitions, validation, permission checks, execution pipeline, progress, rendering, and result budgeting.
- Bootstrap/state chapter
  - Separates infrastructure singleton state from UI reactive store state.
- API layer chapter
  - Client factory, provider routing, prompt cache stability, raw SSE streaming, non-streaming fallback.
- Subagent/fork chapters
  - Normal subagents, fork agents, prompt-cache-preserving parallel branches, and cleanup.
- Coordination chapter
  - Task types, lifecycle status, background/foreground transitions, output file and notification channels.
- Memory chapter
  - Markdown memory taxonomy, always-loaded index, relevance-selected detailed memory files.
- Extensibility/MCP chapters
  - Hooks, skills, MCP tool wrapping, OAuth, transport, and trust boundaries.
- Performance chapter
  - Startup fast paths, dynamic imports, prompt cache ordering, token budgeting, search indexing, streaming watchdogs.

## 4. Agent loop

The core claim is that Claude Code's agent loop is an async generator, not a callback tree or plain event emitter.

Key properties:

- The loop yields `Message/Event` values and returns a typed terminal reason.
- The generator form provides:
  - backpressure
  - cancellation
  - `yield*` composition
  - explicit termination reasons
- `query()` is shared by:
  - REPL
  - SDK
  - sub-agent
  - `--print`
  - compaction and internal queries
- On each continuation, the loop rebuilds a complete state object instead of mutating scattered local state, which improves testability and auditability.

Typical single-turn flow:

1. Manage context.
2. Call model and stream response.
3. Collect tool calls.
4. Execute tools.
5. Append tool results to message history.
6. If there are no tool calls, evaluate stop hooks, completion, and token budget.
7. End or continue according to a typed terminal or continuation reason.

Terminal states listed in the source material include:

- `completed`
- `model_error`
- `prompt_too_long`
- `aborted_streaming`
- `aborted_tools`
- `stop_hook_prevented`
- `hook_stopped`
- `max_turns`
- `blocking_limit`
- `image_error`

Continuation states include:

- `next_turn`
- `reactive_compact_retry`
- `max_output_tokens_recovery`
- `stop_hook_blocking`
- `token_budget_continuation`

## 5. Tool protocol, model adaptation, and concurrency

The tool system is one of the strongest architecture patterns in the material.

Important Tool interface fields:

- `call()`
- `inputSchema`
- `isConcurrencySafe(input)`
- `checkPermissions()`
- `validateInput()`

Key patterns:

1. Fail-closed defaults:
   - New tools are not concurrency-safe by default.
   - New tools are not read-only by default.
2. Input-dependent safety:
   - A Bash `ls` call may be safe to run concurrently, while `rm` is not.
3. A unified execution pipeline:
   - lookup
   - abort check
   - schema validation
   - semantic validation
   - speculative classifier
   - input backfill
   - PreToolUse hooks
   - permission resolution
   - denial handling
   - call execution
   - result budgeting
   - PostToolUse hooks
   - new messages
   - error classification
4. Result budgeting:
   - Per-tool output limit.
   - Aggregate conversation budget.
   - Large results persisted to disk with preview/path given back to the model.
5. Tool-result protocol safety:
   - Orphaned `tool_use` blocks are repaired with synthetic error `tool_result` messages to avoid next-turn API protocol errors.

Concurrency has two layers:

| Layer | Description |
|---|---|
| Batch orchestration | After a model response completes, tool calls are split into parallel or serial batches based on safety |
| Streaming executor | A complete streamed `tool_use` block can start speculative execution before the full model response ends |

Concurrency rules:

- Read/Grep-like tools can run in parallel.
- Edit, file mutation, and shell mutation are serialized.
- Bash safety is decided per input, not only by tool name.
- Results are yielded in model-request order, not completion order.
- Bash failures can trigger sibling abort cascades in the same batch.

Model/API adaptation patterns:

- `getAnthropicClient()` acts as a unified client factory.
- The query loop is provider-independent while the API layer handles direct API, Bedrock, Vertex, Foundry, and similar routes.
- Static prompt sections come first for cache stability; dynamic/user-specific sections come later.
- Volatile prompt sections are explicitly named and separated because they break cache reuse.
- Raw SSE streaming avoids repeated partial parsing of large tool JSON inputs.
- A streaming idle watchdog is separate from the HTTP request timeout.
- Non-streaming fallback is available for proxy/network issues, but cannot be applied blindly when speculative tool execution may already have side effects.
- Default output cap is around 8K, escalating to a larger cap after truncation to save context slots.

## 6. Context, state, and memory

### Context management

The material describes layered context handling:

1. Tool result budget.
2. Snip compact.
3. Microcompact.
4. Context collapse.
5. Auto-compact.

Principles:

- Prefer light deletion/truncation before heavy summary.
- Auto-compact needs a circuit breaker to avoid compact-fail-retry token burn.
- Recoverable errors are withheld internally first, so SDK consumers do not observe a failure that later recovers.

### State architecture

The state chapter describes two layers:

| Layer | Role |
|---|---|
| Bootstrap mutable singleton | cwd, session id, model config, cost, telemetry, prompt cache latch |
| UI reactive store | messages, input mode, tool approval, progress, tasks |

Reasons:

- Infrastructure state should not trigger React re-rendering.
- UI state needs to be reactive.
- Bootstrap state must be available before React and plugins.
- Getter/setter and side-effect bridges synchronize the layers.

### Memory

The memory chapter describes file-based memory:

- Markdown files instead of a vector database.
- Human-readable, human-editable, and version-controllable.
- Memory types:
  - user
  - feedback
  - project
  - reference
- `MEMORY.md` is the always-loaded index.
- Individual memory files are selected by an LLM side-query based on relevance.
- Stale memory includes age hints so the model is prompted to verify current code.

## 7. Permissions, sandbox, and security

The material lists seven permission modes:

| Mode | Behavior |
|---|---|
| `bypassPermissions` | Allow everything; internal/test use |
| `dontAsk` | Do not ask the user; often auto-denies prompt-like operations in background contexts |
| `auto` | Lightweight LLM classifier decides |
| `acceptEdits` | File edits auto-approved; other mutations ask |
| `default` | Standard interactive approval |
| `plan` | Read-only |
| `bubble` | Subagent permission request bubbles to parent |

Permission resolution chain:

1. PreToolUse hook decision.
2. allow/deny/ask rules.
3. tool-specific check.
4. permission mode default.
5. interactive prompt.
6. auto classifier.

Hooks:

- Skills are content/capability.
- Hooks are control flow/lifecycle.
- Important hook events include:
  - PreToolUse
  - PostToolUse
  - Stop
  - SessionStart
  - UserPromptSubmit
  - SubagentStart/SubagentStop
  - PreCompact/PostCompact
- Command hooks use exit codes:
  - 0 success
  - 2 blocking
  - other warning
- Hook configuration is snapshotted after the trust boundary to avoid TOCTOU changes after a repository is trusted.

Security takeaways:

- Workspace trust boundary is central.
- Hook snapshots reduce post-trust mutation risk.
- Subagents should bubble risky permissions.
- MCP skills should not execute inline shell.
- SSRF and DNS rebinding risks need connection-level validation.

The material does not present a simple container sandbox as the primary boundary; the more relevant boundary is layered permission, trust, hooks, and transport validation.

## 8. Subagents, fork agents, and tasks

### Normal subagent

The `runAgent` lifecycle described in the source material includes:

- model resolution
- agent ID
- context preparation
- stripping project instructions for read-only agents
- permission isolation
- tool resolution
- system prompt
- abort controller isolation
- hook registration
- skill preloading
- MCP initialization
- subagent context creation
- query loop
- cleanup

Key rules:

- Sync agents share the parent abort controller.
- Async agents use an independent abort controller.
- Async agents isolate UI app state but share the task state channel.
- Agent-specific hooks are cleaned up in `finally`.
- `runAgent` is an async generator, so cleanup must be explicit and reliable.

### Fork agent

Fork agents are built around prompt cache reuse:

- Child agents inherit the already-rendered parent system prompt.
- Child agents inherit the exact parent tool array.
- Child agents clone the parent conversation history.
- Child agents use the parent model/thinking config.
- Each child differs mainly in the final directive, preserving a byte-identical prefix.
- The Agent tool is kept to preserve tool array identity, while query source/message tags prevent recursive fork loops.

### Task coordination

Task types include:

- `local_bash`
- `local_agent`
- `remote_agent`
- `in_process_teammate`
- `local_workflow`
- `monitor_mcp`
- `dream`

Statuses include:

- `pending`
- `running`
- `completed`
- `failed`
- `killed`

Background tasks communicate through output files, offsets, notifications, and pending inboxes. A foreground agent can race into the background without losing history.

## 9. MCP and extensibility

The MCP chapter describes MCP as a JSON-RPC 2.0 tool discovery and invocation protocol:

- Client calls `tools/list` to get names, descriptions, and schemas.
- Client calls `tools/call` to execute.
- MCP tools are wrapped into the internal Tool interface.
- Tool names are normalized as `mcp__{serverName}__{toolName}`.
- Descriptions are truncated to prevent generated servers from injecting huge descriptions into context.
- MCP annotations include:
  - `readOnlyHint`
  - `destructiveHint`
- Transports include stdio, HTTP, SSE, WebSocket/IDE/internal transports.
- OAuth supports PKCE, discovery, token refresh, and error normalization.
- Connection states include connected, failed, needs-auth, pending, and disabled.
- Local and remote servers are connected in batches to avoid resource exhaustion.

Risks:

- MCP servers can incorrectly or maliciously mark destructive tools as read-only.
- Remote MCP involves OAuth, SSRF, timeout, and session-expiry risks.
- Tool descriptions and schemas are prompt-attack surfaces and need truncation and sanitization.

## 10. Events, logs, and observability

The material emphasizes event-shaped control flow more than a single logging subsystem:

- The query loop yields messages/events and returns typed terminal reasons.
- Tool execution emits progress, permission, denial, result, and error information through the shared pipeline.
- Background tasks communicate with output files, offsets, notifications, and pending inboxes.
- Streaming has an idle watchdog so stalled bodies are observable separately from request setup failures.
- Startup and performance work uses profiling checkpoints.
- API usage and token count are anchored to provider usage data when available.
- Telemetry/profiling is used to validate startup and context-performance improvements.

For {{proj}}, the transferable point is that events, terminal reasons, and task notifications should be typed and auditable, not just log lines.

## 11. Testing and validation

Validation methods described by the material:

- The query loop is tested through narrow `QueryDeps` injection with fake model, fake compactor, and UUID generator.
- Context and memory prompt design are adjusted through evals.
- Startup profiling uses multiple checkpoints.
- Performance optimization is based on telemetry and profiling.
- The site states that the manuscript was analyzed, written, reviewed, and audited by several groups of AI agents to avoid leftover original source.

Trust limitations:

- The material is unofficial.
- It is based on early source maps and does not represent current Claude Code behavior.
- The site is a narrative architecture analysis, not executable source.
- This research summarizes architecture patterns only and does not reproduce original Claude Code source.

## 12. Core chapters

Recommended chapter paths for follow-up:

- `https://claude-code-from-source.com/ch01-architecture/`
- `https://claude-code-from-source.com/ch02-bootstrap/`
- `https://claude-code-from-source.com/ch03-state/`
- `https://claude-code-from-source.com/ch04-api-layer/`
- `https://claude-code-from-source.com/ch05-agent-loop/`
- `https://claude-code-from-source.com/ch06-tools/`
- `https://claude-code-from-source.com/ch07-concurrency/`
- `https://claude-code-from-source.com/ch08-sub-agents/`
- `https://claude-code-from-source.com/ch09-fork-agents/`
- `https://claude-code-from-source.com/ch10-coordination/`
- `https://claude-code-from-source.com/ch11-memory/`
- `https://claude-code-from-source.com/ch12-extensibility/`
- `https://claude-code-from-source.com/ch15-mcp/`
- `https://claude-code-from-source.com/ch17-performance/`

## 13. Lessons for {{proj}}

1. Use an async-generator agent loop with typed terminal reasons.
2. Make tools self-describing: schema, permission, concurrency, safety, rendering, and budget belong to the tool definition.
3. Centralize permission modes instead of scattering checks inside individual tools.
4. Keep recoverable errors internal until recovery fails.
5. Layer context compaction from cheap to expensive and add a circuit breaker.
6. Treat subagents as recursive instances of the same query loop, not a special branch.
7. Combine parallel multi-agent work with prompt-cache optimization when forking.
8. Separate Hooks from Skills: content extension and control-flow extension should not be mixed.
9. Truncate, normalize, and audit MCP tool descriptions and schemas.
10. Start memory with human-editable files and an LLM relevance selector before introducing vector infrastructure.

## 14. Risks and anti-patterns

- Do not put all state into one reactive store; that couples UI and infrastructure.
- Do not scatter permission checks across tools; behavior becomes inconsistent.
- Do not allow unbounded retry or compaction loops; they burn API budget.
- Do not place dynamic tool descriptions in a stable prompt prefix; this breaks cache reuse.
- Do not let subagents self-approve dangerous operations; permissions should bubble or go to the parent/user.
- Do not treat memory as a fact database; memory can be stale and needs staleness cues.
- Do not trust MCP annotations as security proof; they are server-supplied claims.
