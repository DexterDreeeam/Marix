# AutoGPT Framework Agent 研究

## 1. 来源与活跃度

| 项 | 内容 |
|---|---|
| 仓库 | https://github.com/Significant-Gravitas/AutoGPT |
| 默认分支 | `master` |
| 主语言 | Python + TypeScript |
| 当前主线 | `autogpt_platform/`：低代码图式 AI workflow/agent 平台 |
| 历史线 | `classic/`：AutoGPT Classic、Forge、benchmark、classic frontend |
| License 说明 | GitHub API 为 `NOASSERTION`；README 说明 `autogpt_platform/` 为 Polyform Shield License，其余多为 MIT |
| 近期活跃 | GitHub API：`pushed_at=2026-06-22T15:19:08Z`；latest release `autogpt-platform-beta-v0.6.64`，发布于 `2026-06-18` |

主要研究输入：

- https://github.com/Significant-Gravitas/AutoGPT
- https://github.com/Significant-Gravitas/AutoGPT/releases/tag/autogpt-platform-beta-v0.6.64
- `autogpt_platform/backend/pyproject.toml`
- `autogpt_platform/frontend/package.json`
- `autogpt_platform/backend/schema.prisma`
- `classic/original_autogpt/autogpt/agents/agent.py`
- `classic/forge/forge/agent/base.py`

## 2. 技术栈与性质

AutoGPT 当前是两条产品线并存：

| 区域 | 作用 |
|---|---|
| `autogpt_platform/backend` | FastAPI 后端、graph execution、scheduler、executor、Copilot、block runtime |
| `autogpt_platform/frontend` | Next.js / React / TypeScript 图式 builder UI |
| `classic/original_autogpt` | 早期 autonomous agent loop |
| `classic/forge` | Forge agent component pipeline |
| `classic/benchmark` | benchmark 与 eval 能力 |

Platform backend 依赖 FastAPI、Prisma Python、PostgreSQL、pgvector、Redis Cluster、RabbitMQ、APScheduler、Prometheus、Sentry、Supabase/Auth、Stripe、FalkorDB、Graphiti、ClamAV，以及 OpenAI、Anthropic、Groq、Ollama、OpenRouter 等 provider SDK。

Frontend 使用 Next.js 15、React、TypeScript、pnpm、`@xyflow/react`、TanStack Query、Vitest、Playwright、Storybook。整体性质更接近平台化 agent runtime，而不是小型本地 agent 库：持久化、队列、凭据、marketplace 元数据、UI schema、计费和 observability 都进入核心设计。

## 3. 入口与模块

| 入口 | 路径 | 说明 |
|---|---|---|
| all-in-one app | `autogpt_platform/backend/backend/app.py` | 启动 REST、WebSocket、executor、scheduler、notification、database manager、Copilot 等服务 |
| REST API | `backend/rest.py`, `backend/api/rest_api.py` | FastAPI app、router、中间件、Prometheus instrumentation |
| executor | `backend/exec.py`, `backend/executor/manager.py` | 消费 graph execution 消息并运行 node/block runtime |
| scheduler | `backend/scheduler.py`, `backend/executor/scheduler.py` | APScheduler-backed workflow engine |
| WebSocket | `backend/ws.py`, `backend/api/ws_api.py` | 推送 execution events |
| Copilot executor | `backend/copilot/executor/` | Copilot turn 后台执行 |
| Classic CLI | `classic/original_autogpt/autogpt/app/main.py` | Classic agent CLI loop |
| Classic agent | `classic/original_autogpt/autogpt/agents/agent.py` | propose/execute autonomous loop |
| Forge agent | `classic/forge/forge/agent/base.py` | component pipeline 与 sub-agent context |

## 4. Runtime、Agent、Team 与 Graph 执行

Platform 核心不是单个内存 agent loop，而是持久化 graph runtime：

1. API、Copilot 或 scheduler 调用 `add_graph_execution`。
2. Runtime 校验 graph 结构、输入、credentials、node mask。
3. 创建 `AgentGraphExecution`、`AgentNodeExecution` 等数据库记录。
4. 将 execution 消息发布到 RabbitMQ。
5. `ExecutionManager` 消费消息。
6. Redis lock 防止多个 executor 重复执行。
7. `ExecutionProcessor` 执行节点 block。
8. node 输出写回数据库。
9. `_enqueue_next_nodes` 根据 `AgentNodeLink` 传播到下游。
10. graph 进入 `COMPLETED`、`FAILED`、`TERMINATED`、`REVIEW` 等状态。

关键 Prisma 模型包括 `AgentGraph`、`AgentNode`、`AgentNodeLink`、`AgentBlock`、`AgentGraphExecution`、`AgentNodeExecution`、`AgentNodeExecutionInputOutput`、`AgentNodeExecutionKeyValueData`。

Classic 线仍保留早期 autonomous loop：CLI 读取配置、workspace、agent state；agent `propose_action()` 从 components 收集 directives、commands、messages；prompt strategy 构造 prompt；LLM 返回 action proposal；permission manager 检查命令权限；执行 tool；action/result 写入 history；循环直到 finish、用户中断或错误。

Forge 还支持 sub-agent execution context，包括 max depth、max sub-agents、预算、继承 deny rules、workspace 隔离和取消。

## 5. 工具与模型适配

Platform 的工具抽象是 `Block` 系统。Block 同时承载 input schema、output schema、credentials schema、webhook config、cost model、review gate、timeout、input/output validation、UI 表单元数据、marketplace 元数据和 documentation metadata。

核心路径：

- `autogpt_platform/backend/backend/blocks/_base.py`
- `autogpt_platform/backend/backend/blocks/__init__.py`
- `autogpt_platform/backend/backend/sdk/registry.py`
- `autogpt_platform/backend/backend/sdk/builder.py`
- `autogpt_platform/backend/backend/sdk/provider.py`

Block 类型包括 Standard、Input、Output、Note、Webhook、Agent、AI、Human In The Loop、MCP Tool。关键设计是把工具行为、UI 形态、凭据、计费、审计和运行时校验绑定到同一个带 schema 的 block 模型。

模型/provider 适配集中在：

- `autogpt_platform/backend/backend/blocks/llm.py`
- `autogpt_platform/backend/backend/util/llm/providers.py`
- `autogpt_platform/backend/backend/copilot/config.py`
- `classic/forge/forge/llm/providers/multi.py`

Platform provider seam 覆盖 OpenAI、Anthropic、Groq、Ollama、OpenRouter、Llama API、AIML API、OpenAI-compatible provider，并统一返回内容、tokens、cache tokens、tool calls、reasoning、cost USD、raw response 等。Classic/Forge 的 `MultiProvider` 按 model name 路由到 Anthropic、Groq、Llamafile、OpenAI 等 provider。

## 6. Memory、State、Checkpoint 与 Storage

Platform 的 source of truth 是 PostgreSQL + Prisma。它保存 graph definition、graph execution、node execution、node input/output、pending human review、chat session/message、workspace files、cost logs、embeddings 和 search index。

Copilot session state 以数据库为 source of truth，同时使用 Redis 做 session cache、queue、lock、in-flight tool call buffer。`ChatSession` 有 `idle`、`queued`、`running` 状态。`ChatMessage` 使用 sequence 保证会话内顺序。metadata JSON 用于降低迁移频率。

Graphiti memory 位于 `backend/copilot/graphiti/*`，使用 Graphiti + FalkorDB，存储 fact、preference、rule、finding、plan、event、procedure 等结构化记忆；检索到的 warm context 会注入 Copilot 上下文。

`schema.prisma` 中的存储对象包括用户与权限（`User`、`APIKey`、OAuth 模型）、graph 模型、execution 模型、HITL（`PendingHumanReview`）、chat（`ChatSession`、`ChatMessage`）、workspace（`UserWorkspace`、`UserWorkspaceFile`）、marketplace/library（`LibraryAgent`、`StoreListing`、`StoreListingVersion`）、search/memory（`UnifiedContentEmbedding`、pgvector、tsvector）以及 billing/observability（`PlatformCostLog`、credit transaction tables）。运行时还依赖 Redis、RabbitMQ、FalkorDB、Supabase、ClamAV。

## 7. Workflow Orchestration

Platform scheduler 是 workflow runtime 的组成部分：

- APScheduler `BackgroundScheduler`
- SQLAlchemy job store
- graph execution schedules
- Copilot turn schedules
- notification batch jobs
- cleanup jobs
- OAuth cleanup
- embedding coverage jobs
- Graphiti community rebuild
- background Copilot jobs

一个值得借鉴的细节是显式处理 APScheduler 与 Unix cron 的 day-of-week 语义差异，避免 `0=Sunday` 与 APScheduler `0=Monday` 错位。

## 8. Human-in-the-loop

HITL 被建模为 execution state，而不只是 UI callback。相关路径：

- `autogpt_platform/backend/backend/blocks/human_in_the_loop.py`
- `autogpt_platform/backend/backend/blocks/helpers/review.py`
- `autogpt_platform/backend/backend/data/human_review.py`
- `autogpt_platform/backend/backend/api/features/executions/review/routes.py`

节点可以进入 `REVIEW`。review 数据持久化到 `PendingHumanReview`，状态包括 `WAITING`、`APPROVED`、`REJECTED`。payload 可编辑，并包含 review message、`wasEdited`、`processed`、`reviewedAt`。用户审批后，系统在所有 pending review 完成时恢复 graph execution。

## 9. 事件、日志与 Observability

核心路径：

- `autogpt_platform/backend/backend/data/event_bus.py`
- `autogpt_platform/backend/backend/api/ws_api.py`
- `autogpt_platform/backend/backend/api/conn_manager.py`
- `autogpt_platform/backend/backend/data/execution.py`

能力包括 Redis sharded pub/sub、per-execution channel、per-graph channel、WebSocket `/ws`、token auth、graph/node event fan-out、Prometheus FastAPI instrumentation、executor metrics/gauges、Sentry exception capture、`PlatformCostLog`、execution stats、correctness score、activity status。

## 10. 测试与验证

Platform backend 使用 pytest、pytest-asyncio、pytest-cov、pytest-snapshot、pyright、ruff、black。Platform frontend 使用 Vitest、Playwright、Storybook、TypeScript check、Next build。Classic 覆盖 `forge/tests`、`original_autogpt/tests`，并使用 `slow`、`integration`、`requires_agent` 等 markers。

## 11. 核心源码路径

| 关注点 | 路径 |
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

## 12. 对 {{proj}} 的借鉴

1. 使用带 schema 的 runtime 单元，让工具、UI 元数据、校验、凭据、计费和 marketplace 元数据保持一致。
2. graph/node execution 全量持久化，支撑恢复、审计、debug、计费和事件回放。
3. 把 HITL 建模为持久 execution state，适合长流程审批。
4. 用独立 provider seam 归一化 usage、cost、reasoning、tool-call metadata 等模型差异。
5. 明确 Redis、RabbitMQ、DB 分工：lock/cache/pubsub、queue、source of truth。
6. 将 MCP 纳入现有 graph/block 体系，而不是另起一套 runtime。
7. 借鉴 Classic permission manager 的 approve once、approve always、deny、deny with feedback 模式。
8. 清洗服务端 context tags，降低 prompt/context spoofing 风险。

## 13. 风险与反模式

1. Platform 依赖栈重，self-host 复杂。
2. Platform 与 Classic 架构目标不同，引用时必须明确边界。
3. 动态 block、MCP、credentials 组合带来 SSRF、权限越界和凭据泄漏风险。
4. 多 provider 适配分支多，维护成本高。
5. 图式 workflow 在持久化、队列和 UI 状态交织后调试复杂。
6. `autogpt_platform/` license 不适合直接复用实现代码，只适合借鉴架构模式。
7. DB、Redis、RabbitMQ、WebSocket、FalkorDB 都参与状态链路，一致性设计要求高。
