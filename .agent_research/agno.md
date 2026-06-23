# Agno Framework Agent Research

## 1. Sources and activity

| Item | Detail |
|---|---|
| Repository | https://github.com/agno-agi/agno |
| Default branch | `main` |
| Main language | Python |
| License | Apache-2.0 |
| Nature | Build, run, and manage agent platforms |
| Activity | GitHub API: `pushed_at=2026-06-22T15:11:04Z`; latest release `v2.6.18` on `2026-06-18` |

Primary research inputs:

- https://github.com/agno-agi/agno
- https://github.com/agno-agi/agno/releases/tag/v2.6.18
- `libs/agno/pyproject.toml`
- `libs/agno/agno/agent/agent.py`
- `libs/agno/agno/team/team.py`
- `libs/agno/agno/workflow/*`
- `libs/agno/agno/tools/*`
- `libs/agno/agno/models/*`
- `libs/agno/agno/db/*`
- `libs/agno/agno/os/*`

## 2. Technology stack and nature

The core package is under `libs/agno`.

Important core dependencies include pydantic, pydantic-settings, httpx, typer, rich, pyyaml, python-dotenv, docstring-parser, and gitpython.

AgentOS/API extras include FastAPI, uvicorn, SQLAlchemy, PyJWT, OpenTelemetry, OpenInference, croniter, and pytz.

| Path | Role |
|---|---|
| `agno/agent` | Single-agent runtime |
| `agno/team` | Multi-agent/team runtime |
| `agno/workflow` | Pipeline workflow runtime |
| `agno/tools` | Toolkit, Function, and decorator tooling |
| `agno/models` | Provider abstraction |
| `agno/memory` | Long-term memory |
| `agno/db` | Storage abstraction |
| `agno/run` | Run output, events, and requirements |
| `agno/os` | AgentOS FastAPI control plane |
| `agno/tracing` | Tracing and spans |
| `agno/skills` | Skill system |
| `agno/registry` | Runtime object registry |
| `agno/scheduler` | Schedules |

Agno is broad: it combines agent runtime, team modes, workflow steps, model registry, tool execution, storage abstraction, tracing, approval state, schedules, and an AgentOS control plane.

## 3. Entry points and modules

| Entry | Path | Role |
|---|---|---|
| `Agent` | `libs/agno/agno/agent/agent.py` | Single-agent configuration and execution |
| `Team` | `libs/agno/agno/team/team.py` | Multi-agent/team orchestration |
| `Workflow` | `libs/agno/agno/workflow/workflow.py` | Pipeline workflow |
| `AgentOS` | `libs/agno/agno/os/app.py` | FastAPI control plane |
| `Model` | `libs/agno/agno/models/base.py` | Provider abstraction |
| `Toolkit` / `Function` | `libs/agno/agno/tools/toolkit.py`, `function.py` | Tool abstraction |

## 4. Runtime, agent, team, and graph execution

### Agent

`Agent` is highly configurable and covers model/fallback, session state, memory, database, history, knowledge, skills, tools, hooks, reasoning, structured output, streaming, events, and telemetry.

Typical execution:

1. Read or create the session.
2. Merge metadata and session state.
3. Resolve dependencies.
4. Run pre-hooks.
5. Resolve explicit tools, default tools, skills, and knowledge tools.
6. Build system/user/model messages.
7. Optionally update memory, learning, and culture.
8. Run reasoning.
9. Invoke the model.
10. Process tool calls.
11. Process structured output and follow-ups.
12. Run post-hooks.
13. Update metrics and session summary.
14. Persist session and run output.

### Team

Core paths:

- `libs/agno/agno/team/team.py`
- `libs/agno/agno/team/mode.py`

`TeamMode` includes:

- `coordinate`: leader delegates work and synthesizes results.
- `route`: route to expert members.
- `broadcast`: concurrently broadcast to all members.
- `tasks`: leader decomposes a goal and repeatedly delegates.

### Graph runtime

Agno's native core is not a graph runtime. Graph checkpoint and time-travel functionality mainly comes through the LangGraph adapter:

- `libs/agno/agno/agents/langgraph/agent.py`

`LangGraphAgent` wraps a compiled graph and supports `get_state_history`, `get_state`, `update_state`, `replay`, and `fork`.

## 5. Tools and model adapters

Tool core paths:

- `libs/agno/agno/tools/toolkit.py`
- `libs/agno/agno/tools/function.py`
- `libs/agno/agno/tools/decorator.py`

| Module | Role |
|---|---|
| `Toolkit` | Manages tool functions, include/exclude, sync/async variants, connection lifecycle, and cache |
| `Function` | Pydantic tool schema/runtime model with hooks, HITL flags, and cache |
| `@tool` | Wraps Python functions as Agno `Function` objects |
| `FunctionCall` | Executes model-generated tool calls with sync/async/generator/cache/hooks support |

Tool sources include explicit tools, Toolkit, Function, callable, provider built-in tools, default memory/history/knowledge tools, skill access tools, and MCP tools.

Model core paths:

- `libs/agno/agno/models/base.py`
- `libs/agno/agno/models/response.py`
- `libs/agno/agno/models/utils.py`
- `libs/agno/agno/models/fallback.py`

`Model` supports sync/async invocation, streaming, tool-call formatting and execution loop, retry/exponential backoff, context-window and rate-limit error classification, response cache, structured output, and provider response delta parsing.

The provider registry explicitly lists OpenAI, Anthropic, Bedrock, Azure, Gemini, Groq, Cohere, Cerebras, Ollama, OpenRouter, Portkey, Mistral, Meta, IBM, DeepSeek, Nvidia, Together, and related providers. Fallback can choose alternatives for general errors, rate limits, and context overflow.

## 6. Memory, state, checkpoint, and storage

### Session state

Agent, Team, and Workflow support `session_state`, `add_session_state_to_context`, `enable_agentic_state`, `overwrite_db_session_state`, and `cache_session`. Session state can be injected into context or exposed to the model through tools that update state.

### Long-term memory

Core path: `libs/agno/agno/memory/manager.py`.

`MemoryManager` generates, updates, deletes, and clears user memories through a DB-backed user-scoped memory store.

### Checkpoint

Agno's native workflow emphasizes run output, paused state, and session persistence. Full graph checkpoint/time-travel is mainly provided through the LangGraph adapter.

### Storage

Core path: `libs/agno/agno/db/base.py`.

`BaseDb` covers sessions, memories, metrics, eval runs, knowledge, traces/spans, components/configs/links, learnings, schedules/schedule runs, and approvals.

Storage adapters include SQLite, Postgres, Async Postgres, Mongo, MySQL, Redis, Firestore, GCS JSON, Dynamo, JSON, in-memory, SingleStore, and SurrealDB.

## 7. Workflow orchestration

Core paths:

- `libs/agno/agno/workflow/workflow.py`
- `libs/agno/agno/workflow/step.py`
- `libs/agno/agno/workflow/types.py`
- `condition.py`
- `router.py`
- `parallel.py`
- `loop.py`
- `steps.py`

| Component | Role |
|---|---|
| `Step` | Wraps function, agent, team, or nested workflow |
| `Steps` | Sequential step group |
| `Loop` | Repeated execution |
| `Parallel` | Parallel execution |
| `Condition` | Conditional branch with callable, bool, or CEL |
| `Router` | Route selection |
| nested `Workflow` | Workflow composition |

Workflow supports sync, async, streaming, step events, executor events, pause, continue, and cancel. A key limitation is that `Parallel` explicitly does not support HITL pause.

## 8. Human-in-the-loop

### Tool-level HITL

`Function` / `@tool` support `requires_confirmation`, `requires_user_input`, `external_execution`, and `stop_after_tool_call`. When triggered, they create `RunRequirement`, pause the run, and wait for continue.

### Workflow-level HITL

`HumanReview` supports pre-execution confirmation, user input, output review, loop iteration review, and reject/timeout/error strategies.

### Admin approval

Core paths:

- `libs/agno/agno/approval/decorator.py`
- `libs/agno/agno/run/approval.py`
- `libs/agno/agno/os/auth.py`

Agno supports pending approval records and continuing a run after resolution.

## 9. Events, logging, and observability

Core paths:

- `libs/agno/agno/run/agent.py`
- `libs/agno/agno/run/team.py`
- `libs/agno/agno/run/workflow.py`
- `libs/agno/agno/metrics.py`
- `libs/agno/agno/tracing/setup.py`
- `libs/agno/agno/tracing/exporter.py`

Agent events include `RunStarted`, `RunContent`, `RunCompleted`, `RunPaused`, `ToolCallStarted`, `ToolCallCompleted`, `ReasoningStarted`, `MemoryUpdateStarted`, `ModelRequestStarted`, `CompressionStarted`, and `CustomEvent`.

Tracing uses OpenTelemetry and OpenInference. `DatabaseSpanExporter` writes spans and traces to the Agno database.

## 10. Tests and validation

Test directories:

- `libs/agno/tests/unit`
- `libs/agno/tests/integration`
- `libs/agno/tests/system`

CI includes `.github/workflows/test.yml` for ruff, mypy, and pytest unit tests, plus `.github/workflows/performance.yml` for performance comparison with LangGraph. System tests use multi-container environments and cover agents, teams, workflows, sessions, memory, knowledge, traces, evals, metrics, A2A, AG-UI, MCP, Slack, and related API routes.

## 11. Core source paths

| Concern | Paths |
|---|---|
| Agent runtime | `libs/agno/agno/agent/agent.py` |
| Team runtime | `libs/agno/agno/team/team.py`, `team/mode.py` |
| Workflow runtime | `workflow/workflow.py`, `workflow/step.py`, `workflow/types.py`, `condition.py`, `router.py`, `parallel.py`, `loop.py`, `steps.py` |
| Tools | `tools/toolkit.py`, `tools/function.py`, `tools/decorator.py` |
| Model providers | `models/base.py`, `models/response.py`, `models/utils.py`, `models/fallback.py` |
| Memory | `memory/manager.py` |
| Database | `db/base.py` |
| AgentOS | `os/app.py`, `os/auth.py` |
| HITL/approval | `approval/decorator.py`, `run/approval.py` |
| Events/tracing | `run/agent.py`, `run/team.py`, `run/workflow.py`, `metrics.py`, `tracing/setup.py`, `tracing/exporter.py` |
| LangGraph adapter | `agents/langgraph/agent.py` |

## 12. AgentOS and control plane

Core path: `libs/agno/agno/os/app.py`.

AgentOS is a FastAPI runtime/control plane that can register agents, teams, workflows, knowledge, interfaces, database, MCP server, scheduler, tracing, and authorization/RBAC. Routers cover agents, teams, workflows, approvals, components, database, evals, health, knowledge, memory, metrics, registry, schedules, sessions, and traces.

Security capabilities include JWT authorization, scope/RBAC, optional user isolation, MCP host/origin protection, and scheduler internal service token.

## 13. Reusable lessons for {{proj}}

1. Align Agent, Team, and Workflow around shared run output, events, metrics, and session persistence.
2. Use clear layers for `Toolkit`, `Function`, and decorator-based tools.
3. Treat HITL as first-class runtime state with pause, requirement, approval, and continue.
4. Keep model adapters in a centralized provider registry.
5. A control plane can expose agents, teams, workflows, memory, traces, approvals, and schedules through one consistent API surface.
6. Load skills progressively to avoid filling context all at once.
7. Combine run events, metrics, OpenTelemetry spans, and a DB span exporter for built-in observability.

## 14. Risks and anti-patterns

1. `Agent`, `Team`, and `Workflow` have a very wide configuration surface, increasing learning and state-composition cost.
2. Core files are large, which increases maintenance cost.
3. `Parallel` does not support HITL pause; complex concurrent approvals need decomposition.
4. Native graph/checkpoint functionality is less complete than LangGraph and depends on an adapter.
5. Extras create a large dependency surface with supply-chain and version-conflict risk.
6. Telemetry defaults need explicit privacy boundaries before platform adoption.
7. Registry can recover some runtime objects, but full serialization/rehydration still requires care.
