# AutoGPT Framework Agent Research

## 1. Sources and activity

| Item | Detail |
|---|---|
| Repository | https://github.com/Significant-Gravitas/AutoGPT |
| Default branch | `master` |
| Main languages | Python + TypeScript |
| Current main line | `autogpt_platform/`: low-code graphical AI workflow/agent platform |
| Historical line | `classic/`: AutoGPT Classic, Forge, benchmark, and classic frontend |
| License notes | GitHub API reports `NOASSERTION`; README states `autogpt_platform/` uses Polyform Shield License and most other areas are MIT |
| Activity | GitHub API: `pushed_at=2026-06-22T15:19:08Z`; latest release `autogpt-platform-beta-v0.6.64` on `2026-06-18` |

Primary research inputs:

- https://github.com/Significant-Gravitas/AutoGPT
- https://github.com/Significant-Gravitas/AutoGPT/releases/tag/autogpt-platform-beta-v0.6.64
- `autogpt_platform/backend/pyproject.toml`
- `autogpt_platform/frontend/package.json`
- `autogpt_platform/backend/schema.prisma`
- `classic/original_autogpt/autogpt/agents/agent.py`
- `classic/forge/forge/agent/base.py`

## 2. Technology stack and nature

AutoGPT currently contains two distinct product lines:

| Area | Role |
|---|---|
| `autogpt_platform/backend` | FastAPI backend, graph execution, scheduler, executor, Copilot, and block runtime |
| `autogpt_platform/frontend` | Next.js / React / TypeScript graphical builder UI |
| `classic/original_autogpt` | Original autonomous agent loop |
| `classic/forge` | Forge agent component pipeline |
| `classic/benchmark` | Benchmark and evaluation tooling |

The platform backend depends on FastAPI, Prisma Python, PostgreSQL, pgvector, Redis Cluster, RabbitMQ, APScheduler, Prometheus, Sentry, Supabase/Auth, Stripe, FalkorDB, Graphiti, ClamAV, and provider SDKs for OpenAI, Anthropic, Groq, Ollama, OpenRouter, and related providers.

The frontend uses Next.js 15, React, TypeScript, pnpm, `@xyflow/react`, TanStack Query, Vitest, Playwright, and Storybook. The architecture is platform-oriented rather than a small local agent library: persistence, queueing, credentials, marketplace metadata, UI schemas, billing, and observability are part of the core shape.

## 3. Entry points and modules

| Entry | Path | Role |
|---|---|---|
| All-in-one app | `autogpt_platform/backend/backend/app.py` | Starts REST, WebSocket, executor, scheduler, notification, database manager, and Copilot services |
| REST API | `backend/rest.py`, `backend/api/rest_api.py` | FastAPI app, routers, middleware, and Prometheus instrumentation |
| Executor | `backend/exec.py`, `backend/executor/manager.py` | Consumes graph execution messages and runs node/block runtime |
| Scheduler | `backend/scheduler.py`, `backend/executor/scheduler.py` | APScheduler-backed workflow engine |
| WebSocket | `backend/ws.py`, `backend/api/ws_api.py` | Pushes execution events to clients |
| Copilot executor | `backend/copilot/executor/` | Background Copilot turn execution |
| Classic CLI | `classic/original_autogpt/autogpt/app/main.py` | Classic agent CLI loop |
| Classic agent | `classic/original_autogpt/autogpt/agents/agent.py` | Propose/execute autonomous loop |
| Forge agent | `classic/forge/forge/agent/base.py` | Component pipeline and sub-agent context |

## 4. Runtime, agent, team, and graph execution

The platform core is a durable graph runtime, not a single in-memory agent loop:

1. API, Copilot, or scheduler calls `add_graph_execution`.
2. The runtime validates graph structure, input, credentials, and node masks.
3. It creates database records such as `AgentGraphExecution` and `AgentNodeExecution`.
4. It publishes an execution message to RabbitMQ.
5. `ExecutionManager` consumes the message.
6. Redis locks prevent duplicated execution by multiple executors.
7. `ExecutionProcessor` executes node blocks.
8. Node output is written back to the database.
9. `_enqueue_next_nodes` propagates results through `AgentNodeLink` edges.
10. The graph reaches states such as `COMPLETED`, `FAILED`, `TERMINATED`, or `REVIEW`.

Important Prisma models include `AgentGraph`, `AgentNode`, `AgentNodeLink`, `AgentBlock`, `AgentGraphExecution`, `AgentNodeExecution`, `AgentNodeExecutionInputOutput`, and `AgentNodeExecutionKeyValueData`.

The Classic line keeps the older autonomous loop: CLI loads configuration, workspace, and agent state; the agent calls `propose_action()` to collect directives, commands, and messages; prompt strategy builds the prompt; the LLM returns an action proposal; permission manager checks command permission; the tool is executed; action/result enters history; the loop continues until finish, interruption, or error.

Forge adds sub-agent execution context with maximum depth, maximum sub-agents, budget, inherited deny rules, workspace isolation, and cancellation.

## 5. Tools and model adapters

The platform tool abstraction is the `Block` system. A block carries input schema, output schema, credentials schema, webhook configuration, cost model, review gates, timeout, input/output validation, UI form metadata, marketplace metadata, and documentation metadata.

Core paths:

- `autogpt_platform/backend/backend/blocks/_base.py`
- `autogpt_platform/backend/backend/blocks/__init__.py`
- `autogpt_platform/backend/backend/sdk/registry.py`
- `autogpt_platform/backend/backend/sdk/builder.py`
- `autogpt_platform/backend/backend/sdk/provider.py`

Block categories include Standard, Input, Output, Note, Webhook, Agent, AI, Human In The Loop, and MCP Tool. The key architectural point is that AutoGPT binds tool behavior, UI shape, credentials, billing, audit, and runtime validation to the same schema-bearing block model.

Model/provider adaptation is centralized around:

- `autogpt_platform/backend/backend/blocks/llm.py`
- `autogpt_platform/backend/backend/util/llm/providers.py`
- `autogpt_platform/backend/backend/copilot/config.py`
- `classic/forge/forge/llm/providers/multi.py`

The platform seam covers OpenAI, Anthropic, Groq, Ollama, OpenRouter, Llama API, AIML API, and OpenAI-compatible providers. It normalizes content, tokens, cache tokens, tool calls, reasoning, USD cost, and raw response. Classic/Forge `MultiProvider` routes by model name to providers such as Anthropic, Groq, Llamafile, and OpenAI.

## 6. Memory, state, checkpoint, and storage

The platform source of truth is PostgreSQL through Prisma. It stores graph definitions, graph executions, node executions, node input/output, pending human review, chat sessions/messages, workspace files, cost logs, embeddings, and search index data.

Copilot session state uses the database as source of truth while Redis provides session cache, queues, locks, and in-flight tool call buffers. `ChatSession` has `idle`, `queued`, and `running` states. `ChatMessage` uses sequence numbers to preserve order within a session. Metadata JSON is used to reduce migration churn.

Graphiti memory lives under `backend/copilot/graphiti/*` and uses Graphiti plus FalkorDB. It stores structured facts, preferences, rules, findings, plans, events, and procedures. Retrieved warm context is injected into Copilot context.

Storage objects in `schema.prisma` include users and permissions (`User`, `APIKey`, OAuth models), graph models, execution models, HITL (`PendingHumanReview`), chat (`ChatSession`, `ChatMessage`), workspace (`UserWorkspace`, `UserWorkspaceFile`), marketplace/library (`LibraryAgent`, `StoreListing`, `StoreListingVersion`), search/memory (`UnifiedContentEmbedding`, pgvector, tsvector), and billing/observability (`PlatformCostLog`, credit transaction tables). Runtime also depends on Redis, RabbitMQ, FalkorDB, Supabase, and ClamAV.

## 7. Workflow orchestration

The platform scheduler is part of the workflow runtime:

- APScheduler `BackgroundScheduler`
- SQLAlchemy job store
- Graph execution schedules
- Copilot turn schedules
- Notification batch jobs
- Cleanup jobs
- OAuth cleanup
- Embedding coverage jobs
- Graphiti community rebuild
- Background Copilot jobs

A useful implementation detail is explicit handling of APScheduler versus Unix cron day-of-week semantics, avoiding mismatches between `0=Sunday` and APScheduler `0=Monday`.

## 8. Human-in-the-loop

HITL is modeled as execution state, not just a UI callback. Relevant paths:

- `autogpt_platform/backend/backend/blocks/human_in_the_loop.py`
- `autogpt_platform/backend/backend/blocks/helpers/review.py`
- `autogpt_platform/backend/backend/data/human_review.py`
- `autogpt_platform/backend/backend/api/features/executions/review/routes.py`

A node can enter `REVIEW`. Review data is persisted to `PendingHumanReview` with states such as `WAITING`, `APPROVED`, and `REJECTED`. Payloads can be editable and include review message, `wasEdited`, `processed`, and `reviewedAt`. After user approval, graph execution resumes when all pending reviews are complete.

## 9. Events, logging, and observability

Core paths:

- `autogpt_platform/backend/backend/data/event_bus.py`
- `autogpt_platform/backend/backend/api/ws_api.py`
- `autogpt_platform/backend/backend/api/conn_manager.py`
- `autogpt_platform/backend/backend/data/execution.py`

Capabilities include Redis sharded pub/sub, per-execution channels, per-graph channels, WebSocket `/ws`, token authentication, graph/node event fan-out, Prometheus FastAPI instrumentation, executor metrics and gauges, Sentry exception capture, `PlatformCostLog`, execution statistics, correctness scores, and activity status.

## 10. Tests and validation

Platform backend validation uses pytest, pytest-asyncio, pytest-cov, pytest-snapshot, pyright, ruff, and black. Platform frontend validation uses Vitest, Playwright, Storybook, TypeScript checks, and Next build. Classic validation includes `forge/tests`, `original_autogpt/tests`, and markers such as `slow`, `integration`, and `requires_agent`.

## 11. Core source paths

| Concern | Paths |
|---|---|
| Platform app/API | `autogpt_platform/backend/backend/app.py`, `backend/rest.py`, `backend/api/rest_api.py` |
| Execution runtime | `backend/exec.py`, `backend/executor/manager.py`, `backend/data/execution.py` |
| Scheduler | `backend/scheduler.py`, `backend/executor/scheduler.py` |
| Block/tool abstraction | `backend/blocks/_base.py`, `backend/sdk/registry.py`, `backend/sdk/builder.py`, `backend/sdk/provider.py` |
| Model providers | `backend/blocks/llm.py`, `backend/util/llm/providers.py`, `classic/forge/forge/llm/providers/multi.py` |
| HITL | `backend/blocks/human_in_the_loop.py`, `backend/data/human_review.py`, `backend/api/features/executions/review/routes.py` |
| Events/WS | `backend/data/event_bus.py`, `backend/api/ws_api.py`, `backend/api/conn_manager.py` |
| Storage schema | `autogpt_platform/backend/schema.prisma` |
| Classic agent | `classic/original_autogpt/autogpt/agents/agent.py` |
| Forge agent | `classic/forge/forge/agent/base.py` |

## 12. Reusable lessons for Marix

1. Use schema-bearing runtime units so tools, UI metadata, validation, credentials, billing, and marketplace metadata stay aligned.
2. Persist graph and node execution state for recovery, audit, debugging, billing, and event replay.
3. Model HITL as durable execution state for long-running approval workflows.
4. Keep model provider differences behind a dedicated seam that normalizes usage, cost, reasoning, and tool-call metadata.
5. Give Redis, RabbitMQ, and the database clear roles: locks/cache/pubsub, queueing, and source of truth.
6. Integrate MCP into the existing graph/block model instead of creating a parallel runtime.
7. Reuse the Classic permission pattern: approve once, approve always, deny, and deny with feedback.
8. Clean server-side context tags to reduce prompt/context spoofing risk.

## 13. Risks and anti-patterns

1. The platform dependency stack is heavy and self-hosting is complex.
2. Platform and Classic pursue different architecture goals; patterns should not be mixed without clear boundaries.
3. Dynamic blocks, MCP, and credentials create SSRF, privilege escalation, and secret leakage risks.
4. Multi-provider adapters grow many branches and can be expensive to maintain.
5. Graphical workflow debugging becomes hard when persistence, queues, and UI state interact.
6. `autogpt_platform/` licensing is not appropriate for direct implementation reuse; only architecture patterns should be borrowed.
7. DB, Redis, RabbitMQ, WebSocket, and FalkorDB all participate in state flow, so consistency design is critical.
