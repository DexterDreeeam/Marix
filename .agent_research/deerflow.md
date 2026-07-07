# DeerFlow 2.x External Agent System Research

> Target repository: bytedance/deer-flow — https://github.com/bytedance/deer-flow  
> Research date: 2026-06-22  
> Scope: public `main` branch source, README, config samples, contracts, backend/app, backend/packages/harness, frontend/src, skills, tests, docker/provisioner/nginx.  
> Limitations: static source review only; no clone, deployment, or test execution. Some repository metadata is based on user-provided facts. DeerFlow 2.x differs materially from earlier v1 Deep Research material; this note uses the public 2.x source as the source of truth.

## 1. Sources and activity

| Item | Observation |
|---|---|
| Repository | `bytedance/deer-flow` |
| License | MIT, per user-provided metadata |
| Created | 2025-05-07, per user-provided metadata |
| Pushed / Updated | 2026-06-22, per user-provided metadata |
| Stars | ~73k, per user-provided metadata |
| Default branch | `main` |
| Version | `backend/pyproject.toml` and `backend/packages/harness/pyproject.toml` show 2.1.0 |
| Positioning | README describes DeerFlow 2.0 as a long-horizon SuperAgent harness; backend is a LangGraph-based AI agent backend with sandbox execution capabilities |

Primary source paths inspected:

- `README.md`
- `backend/langgraph.json`
- `backend/pyproject.toml`
- `backend/packages/harness/pyproject.toml`
- `frontend/package.json`
- `config.example.yaml`
- `extensions_config.example.json`
- `contracts/subagent_status_contract.json`
- `backend/app/gateway/*`
- `backend/packages/harness/deerflow/*`
- `frontend/src/*`
- `skills/*`
- `tests/*`
- `docker/*`

## 2. Technology stack and system nature

DeerFlow 2.x is best understood as a deployable **SuperAgent harness**, not a single fixed research graph.

| Layer | Technology / module |
|---|---|
| Agent runtime | LangGraph + LangChain `create_agent` |
| Backend / Gateway | Python 3.12+, FastAPI, LangGraph-compatible API |
| Harness package | `deerflow-harness`: agents, middleware, tools, sandbox, models, MCP, skills, runtime |
| Frontend | Next.js 16, React 19, pnpm, LangGraph SDK, React Query, Radix, CodeMirror, streamdown |
| Sandbox | LocalSandboxProvider, AIO sandbox, Docker / Apple Container, Kubernetes provisioner |
| Persistence | LangGraph checkpointer plus DeerFlow application DB; SQLite/Postgres/memory |
| Integrations | MCP, ACP agents, IM channels, OAuth/OIDC auth, Langfuse/LangSmith tracing |
| Skills | Markdown skill packages, frontmatter metadata, slash activation, security scan, optional self-evolution |

High-level architecture:

```text
Gateway / Frontend / Channel
  -> run manager + stream bridge
  -> LangGraph lead_agent
  -> middleware chain
  -> model call
  -> tools / MCP / ACP / sandbox / skills / subagents
  -> state checkpoint + stream + run events
```

## 3. Entrypoints and modules

| Module | Core paths | Purpose |
|---|---|---|
| LangGraph graph | `backend/langgraph.json` | Declares `lead_agent: deerflow.agents:make_lead_agent`, auth, and checkpointer |
| Gateway | `backend/app/gateway/app.py` | FastAPI app, routers, middleware, lifespan startup |
| Run lifecycle | `backend/app/gateway/services.py`, `runtime/runs/worker.py`, `runtime/runs/manager.py` | Run creation, config/context merge, background execution, SSE, cancel/rollback |
| Lead agent | `agents/lead_agent/agent.py` | Model resolution, tool assembly, skills, subagents, middleware |
| Prompt | `agents/lead_agent/prompt.py` | Static prompt, skill metadata, subagent instructions, confidentiality boundary |
| State | `agents/thread_state.py` | ThreadState and reducers |
| Tools | `tools/tools.py`, `sandbox/tools.py`, `tools/builtins/*` | Built-ins, config tools, MCP, ACP, task tool |
| Models | `models/factory.py`, `models/*provider*.py` | Provider resolution and patched adapters |
| Sandbox | `sandbox/*` | Sandbox abstraction and local provider |
| Skills | `skills/*`, `tools/skill_manage_tool.py` | Skill parsing, install, security, self-evolution |
| MCP | `mcp/*` | MCP loading, OAuth, session pool, path rewrite |
| Frontend | `frontend/src/app/workspace/*`, `frontend/src/core/*`, `frontend/src/components/workspace/*` | Chat UI, run API, uploads, artifacts, subtasks |
| Contracts | `contracts/subagent_status_contract.json` | Shared backend/frontend subagent status contract |
| Docker | `docker/docker-compose*.yaml`, `docker/nginx/nginx.conf`, `docker/provisioner/*` | Deployment topology, routing, K8s sandbox provisioner |

## 4. Agent loop and LangGraph execution

Execution flow:

1. The frontend or an IM channel calls the LangGraph-compatible API or Gateway custom API.
2. Gateway normalizes input/config/context and injects authenticated user context.
3. `RunManager` creates a run; `StreamBridge` prepares an SSE event log.
4. `runtime/runs/worker.py::run_agent`:
   - marks the run as running;
   - captures the pre-run checkpoint for rollback;
   - builds runtime context with `thread_id`, `run_id`, `app_config`, and journal;
   - calls `make_lead_agent`;
   - attaches checkpointer/store;
   - runs `agent.astream(...)`.
5. The LangChain agent loop:
   - middleware prepares messages/context/uploads/memory/sandbox;
   - the model emits an AI message or tool calls;
   - tools run in sandbox/MCP/ACP/subagent runtimes;
   - observations update LangGraph state;
   - the loop continues until completion or a limit is hit.
6. The worker maps LangGraph stream chunks to SSE events.
7. On completion, it flushes journal events, persists token/message summaries, updates thread title/status, sends an end sentinel, and later cleans the stream buffer.

Supported stream modes include `values`, `updates`, `checkpoints`, `tasks`, `debug`, `messages`, and `custom`. The public `events` mode is skipped in the Gateway because the Python public API cannot simultaneously provide `astream_events()` and values snapshots.

Failure handling:

| Scenario | Handling |
|---|---|
| LLM/provider failure | LLM error middleware and worker status updates |
| Tool exception | Converted into error ToolMessage by `ToolErrorHandlingMiddleware` |
| Sandbox failure | Lazy acquisition errors become tool errors |
| Cancellation | Run abort event supports interrupt or rollback |
| Rollback | Worker restores the pre-run checkpoint snapshot when possible |
| Reconnect | `MemoryStreamBridge` supports bounded replay with `Last-Event-ID` |
| Multi-worker | Production compose defaults to one Gateway worker because run/stream state is in-process |

## 5. Planner / researcher / coder / reporter / subagent roles

DeerFlow 2.x does not hard-code a planner-researcher-coder-reporter graph. The responsibilities are implemented as harness capabilities:

| Traditional role | DeerFlow 2.x mechanism |
|---|---|
| Planner | Plan mode via `TodoMiddleware`; prompt enforces clarify -> plan -> act |
| Researcher | Web tools, MCP tools, research skills, `general-purpose` subagent |
| Coder | Sandbox file tools, bash tool, ACP agents, custom subagents |
| Reporter | `present_files`, outputs artifacts, Markdown/HTML/PPT/chart skills |
| Subagent | `task` tool launches isolated subagent runs |
| Orchestrator | Lead agent decomposes, delegates, invokes tools, and synthesizes |

Subagent design:

- The `task` tool launches a `SubagentExecutor` and polls for completion.
- Custom events include `task_started`, `task_running`, `task_completed`, `task_failed`, `task_cancelled`, and `task_timed_out`.
- Subagents inherit sandbox/thread/model/tool/user context but have isolated messages.
- Recursive `task`, `ask_clarification`, and `present_files` are disabled.
- Built-ins:
  - `general-purpose`: complex multi-step work, default max_turns 150.
  - `bash`: command/file-tool specialist, default max_turns 60; hidden when host bash is disabled in local sandbox mode.

## 6. Tool abstraction

Tool sources:

1. Configured tools in `config.example.yaml`: web search/fetch, image search, file tools, bash.
2. Built-ins: `present_files`, `ask_clarification`, `view_image`, `task`, `skill_manage`.
3. Cached MCP tools.
4. ACP agent invocation tool.

Important patterns:

- `get_available_tools` filters by tool group, model capability, subagent flag, and sandbox policy.
- Tool names are deduplicated; config tools have priority.
- Host bash is hidden by default when LocalSandboxProvider is active.
- MCP schemas can be deferred through `tool_search`:
  - prompt lists names only;
  - the model fetches full schemas on demand;
  - promotions are stored in ThreadState;
  - a catalog hash prevents stale promotions from exposing different tools.
- Tool output budget middleware externalizes oversized tool results to disk and returns a compact preview plus file reference.

## 7. Model and provider adapters

`models/factory.py::create_chat_model` dynamically loads the model class from the config `use` path and centralizes:

- thinking mode;
- reasoning effort;
- vision support;
- provider-specific enable/disable payloads;
- OpenAI-compatible `stream_usage`;
- generous default `stream_chunk_timeout` for reasoning models.

Provider-specific adapters:

| Provider | Adaptation |
|---|---|
| OpenAI-compatible | Responses API, stream usage, thinking payloads |
| Claude | Claude Code OAuth token, Bearer auth, billing header, prompt caching, thinking budget |
| Codex | Codex CLI credential and ChatGPT Codex Responses API |
| vLLM | Preserves reasoning fields across tool-call turns |
| Gemini via OpenAI gateway | Preserves `thought_signature` |
| MiMo / StepFun / MiniMax / DeepSeek | Patched reasoning/message quirks |
| MindIE | Conservative timeout/retry handling |

Takeaway: provider quirks are isolated in the model layer instead of leaking into the agent loop.

## 8. Context building

Context comes from layered middleware:

1. **Static system prompt**
   - Kept stable for prefix cache.
   - User input is wrapped as untrusted data.
   - Internal prompt/skill/subagent/system tags are confidential.

2. **Dynamic context**
   - Date is injected by `DynamicContextMiddleware`.
   - Memory is injected as hidden HumanMessage, not system authority.

3. **Uploads**
   - `UploadsMiddleware` injects `<uploaded_files>` into the latest HumanMessage.
   - Converted Markdown outlines/previews guide `read_file` and `grep`.

4. **Skills**
   - Base prompt lists metadata only.
   - `/skill-name` slash activation injects full `SKILL.md`.
   - Summarization preserves recently loaded skills.

5. **Summarization**
   - Triggered by token/message/fraction thresholds.
   - Preserves recent history or token fraction.

6. **Plan mode**
   - Adds todo-management rules and tool.

7. **Vision**
   - Only enabled when the selected model supports vision.

## 9. Files, reports, and artifacts

Virtual path contract:

- `/mnt/user-data/workspace`
- `/mnt/user-data/uploads`
- `/mnt/user-data/outputs`
- `/mnt/skills`
- `/mnt/acp-workspace`
- custom mounts

Artifact flow:

- User-visible outputs should be written under `/mnt/user-data/outputs`.
- `present_files` only accepts outputs paths.
- Artifact API serves `/api/threads/{thread_id}/artifacts/{path}`.
- Active content types such as HTML/XHTML/SVG are forced to download as attachments.
- `.skill` archive preview is size-limited.
- IM channels only deliver outputs artifacts.

Uploads are thread/user scoped and protected by filename normalization, traversal checks, no-follow symlink writes, file count and size limits, and disabled-by-default host-side document conversion.

## 10. Sandbox, command execution, permissions, and safety

Sandbox interface:

- `execute_command`
- `read_file`
- `download_file`
- `list_dir`
- `write_file`
- `glob`
- `grep`
- `update_file`

Providers:

| Provider | Notes |
|---|---|
| LocalSandboxProvider | Virtual-path mapping to per-thread host dirs; not a security boundary |
| AIO sandbox | Docker / Apple Container execution isolation |
| Provisioner mode | Kubernetes Pod + NodePort + mounted user-data/skills |
| Custom mounts | Configurable host-to-container paths with read-only support |

Local sandbox safety:

- Host bash is disabled by default.
- Even when enabled, command paths are conservatively validated.
- File tools enforce path families:
  - `/mnt/user-data/*`: read/write.
  - `/mnt/skills/*`: read-only.
  - `/mnt/acp-workspace/*`: read-only.
  - custom mounts follow mount policy.
- Host paths are masked in user-visible output.

Other safety boundaries:

| Area | Mechanism |
|---|---|
| Auth | Fail-closed `AuthMiddleware` |
| CSRF | Double Submit Cookie on state-changing requests |
| CORS | Explicit `GATEWAY_CORS_ORIGINS` |
| Owner isolation | Permission decorators plus repository filtering |
| LangGraph compatibility | JWT + CSRF + metadata user filtering |
| Guardrails | Built-in allowlist, OAP provider, or custom provider |
| Provider safety stop | Suppresses unreliable tool calls after safety termination |
| Uploads | Traversal, filename, symlink, size, and conversion controls |
| Skills | Safe archive extraction plus LLM security scanner |
| Runtime loops | Loop detection, token budget, output budget |

## 11. Memory, state, checkpoint, and storage

ThreadState extends LangChain AgentState with:

- `sandbox`
- `thread_data`
- `title`
- `artifacts`
- `todos`
- `uploaded_files`
- `viewed_images`
- `promoted`

Reducers are explicit for sandbox conflict detection, artifact dedupe, todo replacement, and deferred-tool promotion isolation.

Checkpointer/storage:

- `runtime/checkpointer/async_provider.py` supports memory/sqlite/postgres.
- Legacy `checkpointer` config takes precedence over unified `database`.
- Unified `database` drives both LangGraph state and app data.
- SQLite uses WAL and foreign keys.
- Postgres uses pooled connections and keepalive.

Run events:

- `memory`: no persistence.
- `db`: SQL-backed events with user filtering, trace truncation, per-thread seq, Postgres advisory locks.
- `jsonl`: lightweight single-process persistence; not safe for multi-process monotonic seq.

Memory:

- File-backed by default and scoped by user/agent.
- Shape includes `user`, `history`, and `facts`.
- Updated asynchronously through a debounced queue and LLM summarization.
- Upload-event memories are stripped because uploads are session-scoped.
- Injection is token-budgeted and can use tiktoken or network-free char estimation.

## 12. Message gateway and integrations

Gateway routers cover models, MCP, memory, skills, artifacts, uploads, threads, agents, suggestions, channels, assistants compatibility, auth, feedback, thread runs, and stateless runs.

Supported IM channels:

- Telegram
- Slack
- Discord
- Feishu/Lark
- DingTalk
- WeChat
- WeCom

Channel flow:

```text
External IM platform
  -> Channel adapter
  -> MessageBus inbound queue
  -> ChannelManager
  -> Gateway / LangGraph-compatible run API
  -> stream / wait result
  -> OutboundMessage
  -> Channel adapter send / file upload
```

Key behaviors:

- Per-user channel binding is supported.
- `require_bound_identity` can block unbound external users.
- Inbound dedupe only activates when stable workspace/team/guild namespace exists.
- Channel workers call Gateway with internal auth and CSRF.
- Inbound files and agent outputs are aligned to the same user/thread storage bucket.
- Only outputs artifacts are delivered.

## 13. Event stream, logging, and observability

| Module | Role |
|---|---|
| `MemoryStreamBridge` | Per-run bounded event log, SSE replay, heartbeat, end sentinel |
| `RunJournal` | LangChain callback capture for run/LLM/tool/token events |
| `RunManager` | In-memory registry plus optional persistent RunStore |
| Tracing | LangSmith/Langfuse root graph callbacks |
| Token usage | Per-caller and per-model accounting |
| Frontend | Subtask cards, token usage UI, artifact preview, stream rendering |

Run statuses:

- `pending`
- `running`
- `success`
- `error`
- `timeout`
- `interrupted`

Important deployment caveat: the production compose file defaults to one Gateway worker because run cancellation, SSE reconnect, request dedupe, and IM channel state are in-process.

## 14. Testing strategy

The test suite covers many safety and runtime boundaries:

| Area | Example coverage |
|---|---|
| Gateway/auth | auth, CSRF, owner isolation |
| Runtime/runs | run manager, rollback, lifecycle e2e |
| Storage | checkpointer, SQLite persistence, event stores |
| Sandbox/tools | local sandbox, path security, write size guard |
| Subagents | executor, task tool, status contract |
| MCP | config, OAuth, session pool, sync wrapper |
| Skills | parser, loader, installer, scanner, skill manage |
| Models | factory and provider patches |
| Channels | Telegram, Slack, Discord, DingTalk, WeChat, WeCom |
| Frontend | Playwright e2e for chat, artifacts, subtasks, threads, channels |
| Contracts | Shared `contracts/subagent_status_contract.json` |

The shared contract fixture is especially useful: it prevents backend/frontend drift for subagent status interpretation.

## 15. Plugins, skills, MCP, and extension points

### Skills

A skill is a Markdown package with `SKILL.md`, YAML frontmatter, and optional support files.

Frontmatter fields:

- `name`
- `description`
- optional `license`
- optional `allowed-tools`

Loading strategy:

- Metadata only in the base prompt.
- Full skill content via slash activation.
- Skill `allowed-tools` constrains the tool set.
- Public and custom skills are separated.
- Self-evolution is disabled by default; when enabled, `skill_manage` can create/edit/patch/delete skills.

Security:

- Reject absolute/traversal archive paths.
- Skip symlinks.
- Enforce decompressed size limit.
- Reject nested `SKILL.md`.
- LLM scanner returns `allow|warn|block`.
- Scanner failures or unparseable output block writes.

### MCP

`extensions_config.example.json` supports:

- `stdio`, `sse`, and `http` servers.
- OAuth.
- Custom interceptors.
- MCP tools with name prefixing.

Design details:

- Stdio MCP sessions are pooled by `(server_name, user_id:thread_id)`.
- Pool size is LRU-capped at 256.
- Stdio cwd and temp dirs are pinned under the thread workspace.
- Local file references are rewritten to `/mnt/user-data/...` only when they exist inside the current thread user-data tree.
- OAuth headers can be injected for discovery and per-tool calls.

### ACP agents

`acp_agents` can declare ACP-compatible external agents. DeerFlow exposes an `invoke_acp_agent` tool. ACP workspace files live under `/mnt/acp-workspace`; user-facing deliverables must be copied into `/mnt/user-data/outputs`.

## 16. Core source path list

| Path | Purpose |
|---|---|
| `README.md` | 2.0 positioning and deployment/security notes |
| `backend/langgraph.json` | Graph, auth, checkpointer entries |
| `config.example.yaml` | Canonical model/tool/sandbox/subagent/skill/memory/database/channel config |
| `extensions_config.example.json` | MCP and extensions config |
| `backend/app/gateway/app.py` | FastAPI application |
| `backend/app/gateway/services.py` | Run lifecycle service |
| `backend/app/gateway/auth_middleware.py` | Fail-closed auth |
| `backend/app/gateway/csrf_middleware.py` | CSRF middleware |
| `backend/app/gateway/routers/uploads.py` | Upload handling |
| `backend/app/gateway/routers/artifacts.py` | Artifact serving |
| `backend/packages/harness/deerflow/agents/lead_agent/agent.py` | Lead agent factory |
| `backend/packages/harness/deerflow/agents/lead_agent/prompt.py` | Prompt, skills, subagents |
| `backend/packages/harness/deerflow/agents/thread_state.py` | State schema and reducers |
| `backend/packages/harness/deerflow/agents/middlewares/*` | Runtime middleware |
| `backend/packages/harness/deerflow/tools/tools.py` | Tool registry |
| `backend/packages/harness/deerflow/tools/builtins/tool_search.py` | Deferred MCP schema loading |
| `backend/packages/harness/deerflow/tools/builtins/task_tool.py` | Subagent task tool |
| `backend/packages/harness/deerflow/subagents/*` | Subagent registry and executor |
| `backend/packages/harness/deerflow/models/*` | Model factory and adapters |
| `backend/packages/harness/deerflow/sandbox/*` | Sandbox abstraction and tools |
| `backend/packages/harness/deerflow/mcp/*` | MCP client/session/OAuth/tools |
| `backend/packages/harness/deerflow/skills/*` | Skill system |
| `backend/packages/harness/deerflow/runtime/*` | Runs, checkpointer, events, stream bridge, journal |
| `contracts/subagent_status_contract.json` | Backend/frontend contract |
| `frontend/src/core/api/api-client.ts` | LangGraph SDK client and CSRF |
| `frontend/src/core/tasks/subtask-result.ts` | Subagent status parsing |
| `frontend/src/components/workspace/messages/subtask-card.tsx` | Subtask UI |
| `frontend/src/components/workspace/input-box.tsx` | Model/mode/skill/upload UI |
| `docker/docker-compose.yaml` | Production topology |
| `docker/nginx/nginx.conf` | Routing and SSE support |
| `docker/provisioner/*` | Kubernetes sandbox provisioner |

## 17. Takeaways for `Marix`

1. **Build a harness, not just one workflow**
   - Separate agent loop, runtime, tool registry, sandbox, memory, and observability.

2. **Keep prompts static and inject dynamic context through middleware**
   - This improves caching and keeps authority boundaries clear.

3. **Use deferred tool schema loading for large tool ecosystems**
   - Especially useful when `Marix` integrates MCP or many plugins.

4. **Engineer subagents as runtime units**
   - Isolated context, timeouts, cancellation, status contracts, and token accounting should be first-class.

5. **Treat sandboxing as a separate security layer**
   - Local execution convenience must not be marketed as isolation.
   - User-visible outputs should have one canonical directory.

6. **Make run lifecycle observable**
   - A RunManager + StreamBridge + Journal pattern gives auditability, replay, and UI progress.

7. **Centralize provider quirks**
   - Thinking, reasoning, vision, tool-call replay, and streaming quirks belong in the provider layer.

8. **Use skill security and tool allowlists**
   - Agent-writable skills require archive guards, scanner fail-closed behavior, and `allowed-tools`.

9. **Be explicit about single-worker assumptions**
   - If `Marix` uses in-process run/stream state, multi-worker deployment needs shared infrastructure first.

## 18. Risks and anti-patterns

| Risk | DeerFlow observation | `Marix` caution |
|---|---|---|
| Local sandbox mistaken for isolation | Host bash disabled by default | Document local mode as convenience only |
| In-process state limits scaling | Run/stream/channel state is process-local | Add shared bridge before multi-worker deployment |
| Provider quirks grow quickly | Many patched providers | Maintain a provider test matrix |
| Skill self-evolution risk | Disabled by default and scanner-gated | Do not default-enable agent-written skills |
| MCP state leakage | Scoped by user/thread | Define MCP isolation keys explicitly |
| Active artifact content | HTML/SVG forced download | Do not inline untrusted generated HTML in app origin |
| Upload parser risk | Auto conversion disabled by default | Sandbox or opt-in host parsing |
| Prompt/context leakage | Confidentiality rules in prompt | Also enforce at UI/logging boundaries |
| Tool output explosion | Output budget externalizes large results | Build tool result budgeting early |
| Old architecture confusion | v1 role graph differs from 2.x harness | Always label architecture by version |

## 19. Summary module map

```text
[Frontend / IM Channel / SDK]
        |
        v
[FastAPI Gateway]
  - Auth / CSRF / owner isolation
  - runs / threads / uploads / artifacts / models / skills / MCP routers
        |
        v
[RunManager + StreamBridge + RunJournal]
        |
        v
[LangGraph lead_agent]
  - static prompt
  - dynamic context middleware
  - uploads / memory / summarization / title / token / loop / safety / guardrails
        |
        v
[Model Provider Factory]
        |
        v
[Tool Runtime]
  - file/bash/web/image
  - MCP deferred tools
  - ACP agents
  - skill_manage
  - task subagents
        |
        v
[Sandbox + Storage]
  - /mnt/user-data/workspace
  - /mnt/user-data/uploads
  - /mnt/user-data/outputs
  - checkpointer / database / run_events / memory
```
