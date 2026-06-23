# OpenHands Coding Agent Research

## 1. Source, activity, technical stack, and nature

Primary material: the completed external `coding-agent-research` output, cross-checked against upstream repositories and raw source paths listed below.

| Item | Details |
|---|---|
| Main repository | https://github.com/OpenHands/OpenHands |
| Core SDK repository | https://github.com/OpenHands/software-agent-sdk |
| Main languages | Python, TypeScript |
| Stack | FastAPI, Socket.IO/SSE, Docker/Kubernetes, LiteLLM, FastMCP, SQLAlchemy, Redis, React/Vite |
| Activity evidence | GitHub API showed a recent push on 2026-06-22; latest release `1.8.0` on 2026-06-10 |
| License signal | `pyproject.toml` declares MIT; GitHub API license field was `NOASSERTION` |

OpenHands now looks like a product, server, and frontend shell around a separate agent SDK. The main repository depends on `openhands-sdk==1.29.0`, `openhands-agent-server==1.29.0`, and `openhands-tools==1.29.0`, while the reusable loop, conversation state, tools, and model plumbing live mostly in `OpenHands/software-agent-sdk`.

## 2. Entry points and modules

Main repository shape:

```text
openhands/
  app_server/          # FastAPI application services: conversation, sandbox, event, settings, MCP
  server/              # compatibility entry; app.py points to app_server.app
  db/                  # database models and migrations
frontend/              # React/Vite frontend
openhands-ui/          # UI package
skills/                # built-in skills
tests/unit/            # unit tests
```

SDK shape:

```text
openhands-sdk/
  openhands/sdk/agent/          # AgentBase, Agent, ACPAgent
  openhands/sdk/conversation/   # Conversation, LocalConversation, RemoteConversation, EventLog
  openhands/sdk/context/        # prompts, condenser, skills, context construction
  openhands/sdk/llm/            # LLM wrapper, model registry, LiteLLM calls
  openhands/sdk/tool/           # tool schemas, specs, client tools
openhands-tools/
  openhands/tools/              # terminal, apply_patch, task_tracker, browser, file editor
openhands-agent-server/
  openhands/agent_server/       # remote agent server and event service
```

Key application paths include `openhands/app_server/app.py`, `app_conversation/live_status_app_conversation_service.py`, `event/event_service.py`, `sandbox/process_sandbox_service.py`, `sandbox/sandbox_spec_service.py`, and `mcp/mcp_router.py`.

## 3. Agent loop

The loop is centered in `LocalConversation` and `Agent`:

```text
Conversation.send_message()
  -> append MessageEvent
Conversation.run()/arun()
  -> lazily load plugins, skills, MCP, hooks
  -> agent.init_state()
  -> while execution status is not FINISHED/PAUSED/STUCK/ERROR:
       - stuck_detector.is_stuck()
       - execute pending confirmed actions if any
       - agent.step()/astep()
           - prepare_llm_messages(state.view, condenser, llm)
           - make_llm_completion(..., tools=tools_map)
           - classify_response()
           - TOOL_CALLS -> ActionEvent -> tool executor -> ObservationEvent
           - CONTENT/EMPTY/REASONING_ONLY -> appropriate state update
       - enforce budget, max iteration, confirmation, and stop hooks
```

Important behavior:

- `LocalConversation.run()` has a synchronous loop and `arun()` has an asynchronous loop.
- `ConversationState.execution_status` tracks `IDLE`, `RUNNING`, `PAUSED`, `FINISHED`, `STUCK`, `ERROR`, and `WAITING_FOR_CONFIRMATION`.
- `StuckDetector` catches repeated errors, tool crashes, monologues, and repeated action/observation patterns.
- `max_iteration_per_run` defaults to 500 and `max_budget_per_run` enforces spending limits.
- Tool calls can be executed concurrently through `ParallelToolExecutor`.

## 4. Planner / executor

OpenHands does not expose one separate planner class. Planning and execution are shaped by prompts, tool availability, subagents, confirmation policy, and hooks.

| Mechanism | Role |
|---|---|
| System prompt | Encodes task style, constraints, and behavior |
| `ThinkTool` | Gives the model an explicit planning step |
| `FinishTool` | Provides a structured completion contract |
| Tool set | Determines what the executor can actually do |
| `delegate` tool and subagents | Allow task decomposition |
| Confirmation policy | Moves risky actions into `WAITING_FOR_CONFIRMATION` |
| Stop hooks | Can block completion and inject feedback |

`Agent.step()` is both the sampling boundary and the executor coordinator: it prepares messages, calls the model, parses tool calls, executes tools, and converts results back into events.

## 5. Tool abstraction

Core paths:

- `openhands-sdk/openhands/sdk/tool/schema.py`
- `openhands-sdk/openhands/sdk/tool/spec.py`
- `openhands-tools/openhands/tools/*`
- `openhands-tools/openhands/tools/apply_patch/definition.py`

Tools carry action and observation types, JSON Schema or Pydantic validation, MCP tool definition compatibility, annotations such as `readOnlyHint`, `destructiveHint`, and `idempotentHint`, confirmation/security metadata, and event-shaped execution results.

Built-in tool areas include terminal/bash, file editor, `apply_patch`, task tracker, browser presets, MCP tools, and dynamic client tools executed by a UI or external client through the event stream.

## 6. Model / provider adaptation

The main dependency set includes `litellm==1.84.1`, `openai==2.33.0`, `anthropic[vertex]`, `google-genai`, `google-cloud-aiplatform`, and `boto3`.

The SDK `LLM` wrapper owns model configuration, prompt cache keys, headers such as `x-litellm-session-id`, token and cost accounting, context-window error mapping, and condenser fallback. Provider-specific differences are mostly hidden behind LiteLLM and `make_llm_completion()` in agent utilities.

Key paths:

- `openhands-sdk/openhands/sdk/llm/llm.py`
- `openhands-sdk/openhands/sdk/llm/llm_registry.py`
- `openhands-sdk/openhands/sdk/agent/utils.py`

## 7. Context construction

Context is assembled from static and dynamic system prompts, `ConversationState.view`, workspace state, public/user/project/plugin skills, secret names and descriptions, MCP configuration merged from plugins or agent context, and a first-class condenser.

Important mechanisms:

- `prepare_llm_messages(state.view, condenser, llm)` is the context boundary before model calls.
- `LLMSummarizingCondenser` handles context pressure.
- `CondensationRequest` and `Condensation` events make compaction visible in the event log.
- Project skills and plugin skills are loaded lazily and merged into the active agent context.
- Secrets are represented by metadata in prompt context; raw secret values stay behind the registry boundary.

## 8. File editing and diff

The strongest editing primitive is `apply_patch` in `openhands-tools/openhands/tools/apply_patch/core.py`. It supports add, update, delete, and move operations with a parser, chunk model, and fuzz matching. The patch format is close to the OpenAI-style `apply_patch` contract, but the tool metadata marks destructive and non-idempotent behavior for confirmation and risk analysis.

At the app layer, OpenHands also contains selected repository, branch, and Git provider integration for GitHub, GitLab, Forgejo, and related flows. Editing is therefore not only a text mutation; it participates in repository selection, permission, and event logging.

## 9. Command execution, sandbox, and permissions

Sandbox services live in the app layer:

| Path | Role |
|---|---|
| `openhands/app_server/sandbox/sandbox_service.py` | sandbox CRUD abstraction |
| `openhands/app_server/sandbox/process_sandbox_service.py` | starts an agent server as a local process |
| `openhands/app_server/sandbox/sandbox_spec_service.py` | sandbox template including `ghcr.io/openhands/agent-server:1.29.0-python` |
| `openhands/app_server/sandbox/session_auth.py` | sandbox session authentication |

The safety boundary is the combination of agent server isolation, Docker/remote/process sandboxes, session API keys, per-conversation secret registries, confirmation policy, and security analysis. `Agent._requires_user_confirmation()` analyzes an `ActionEvent`, asks the security analyzer for risk, applies confirmation policy, and either executes immediately or queues the action for a later confirmed run.

## 10. Memory and state persistence

OpenHands is event-sourcing oriented.

| Path | Role |
|---|---|
| `openhands-sdk/openhands/sdk/conversation/state.py` | `ConversationState` |
| `openhands-sdk/openhands/sdk/conversation/event_store.py` | `EventLog` |
| `openhands-sdk/openhands/sdk/conversation/secret_registry.py` | secret persistence and redaction |
| `openhands/app_server/app_conversation/*` | application conversation metadata |
| `openhands/app_server/event/*` | event storage and API services |

`EventLog` persists events, `ConversationState.rebuild_view()` can rebuild current state, and `fork()` deep-copies event history into a new conversation. Session title, tags, stats, usage, and cost are kept in state or events. Secrets can be encrypted with a cipher and are redacted when serialized.

## 11. Event stream, logging, and audit

The event model covers `MessageEvent`, `SystemPromptEvent`, `ActionEvent`, `ObservationEvent`, `AgentErrorEvent`, `ConversationErrorEvent`, `CondensationRequest`, `PauseEvent`, and `InterruptEvent`. The app server exposes event APIs while the SDK provides persistent replay through `EventLog`.

Observability dependencies include OpenTelemetry and Laminar-related packages. The event structure gives OpenHands strong replay, audit, and UI-streaming foundations because every user message, model action, tool result, pause, interrupt, and compaction can be represented explicitly.

## 12. Testing strategy

The main repository uses `pytest`, `pytest-asyncio`, `pytest-xdist`, `pytest-playwright`, `ruff`, and `mypy`. SDK-side tests cover stuck detection, conversation behavior, and tool parsing. Important test areas are event/state rebuild, sandbox services, MCP integration, app server APIs, frontend e2e, context compression, stuck detection, and patch parser behavior.

## 13. Plugins, MCP, and extension model

OpenHands has a broad extension surface:

| Extension point | Behavior |
|---|---|
| `PluginSource` | GitHub, git, and local plugin sources |
| `Plugin.load` | Loads skills, MCP config, hooks, and agents |
| Hooks | session start, stop hook, user prompt submit, and tool hooks |
| Skills | user, project, public, and plugin skills |
| MCP | FastMCP client/proxy with streamable HTTP and related transports |
| File-based agents | `.agents/agents/*.md` and `.openhands/agents/*.md` |
| Client tools | UI or external clients can register and execute dynamic tools |

The important design is merge semantics: skills can override, MCP config can merge, and hooks append predictably.

## 14. Core source file paths

Recommended paths for architecture review:

- `openhands/app_server/app.py`
- `openhands/app_server/app_conversation/live_status_app_conversation_service.py`
- `openhands/app_server/event/event_service.py`
- `openhands/app_server/sandbox/process_sandbox_service.py`
- `openhands/app_server/mcp/mcp_router.py`
- `openhands-sdk/openhands/sdk/conversation/impl/local_conversation.py`
- `openhands-sdk/openhands/sdk/agent/agent.py`
- `openhands-sdk/openhands/sdk/agent/utils.py`
- `openhands-sdk/openhands/sdk/context/condenser.py`
- `openhands-tools/openhands/tools/apply_patch/core.py`

## 15. Lessons for `{{proj}}`

1. Use event-sourced conversation state so action, observation, message, pause, and compaction are replayable and auditable.
2. Keep sandbox topology outside the core loop; the loop should target a sandbox/service boundary, not hard-code Docker or process details.
3. Combine tool annotations, security analysis, and confirmation policy instead of relying on prompt-only safety.
4. Treat context condensation as a first-class loop recovery path, not an offline cleanup step.
5. Define explicit plugin merge semantics for skills, MCP config, hooks, and agent definitions.
6. Consider client-side tools for capabilities that must run in a UI or user device boundary.
7. Build stuck detection into the runtime from the beginning.

## 16. Risks and anti-patterns

| Risk / anti-pattern | Why it matters |
|---|---|
| Distributed architecture | Main repo, SDK, tools, and agent server increase onboarding and release coordination cost |
| Heavy dependencies | Docker, Kubernetes, Redis, Postgres, FastMCP, LiteLLM, Playwright, and frontend services are operationally expensive |
| LiteLLM pinning | Stability improves, but new model/API support can lag |
| Event consistency | Multiple event backends need identical replay, pagination, and recovery semantics |
| Layered permissions | Security analyzer, confirmation, hooks, sandbox, and secrets can be hard to debug together |
| Plugin supply chain | Plugins can bring hooks, MCP servers, skills, and agents, so signing and secret isolation matter |
| Sandbox leakage | Avoid mixing sandbox implementation details into the agent loop or treating sandbox presence as a substitute for policy checks |
