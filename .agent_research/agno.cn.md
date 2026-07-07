# Agno Framework Agent 研究

## 1. 来源与活跃度

| 项 | 内容 |
|---|---|
| 仓库 | https://github.com/agno-agi/agno |
| 默认分支 | `main` |
| 主语言 | Python |
| License | Apache-2.0 |
| 性质 | Build, run, and manage agent platforms |
| 近期活跃 | GitHub API：`pushed_at=2026-06-22T15:11:04Z`；latest release `v2.6.18`，发布于 `2026-06-18` |

主要研究输入：

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

## 2. 技术栈与性质

核心包位于 `libs/agno`。

核心依赖包括 pydantic、pydantic-settings、httpx、typer、rich、pyyaml、python-dotenv、docstring-parser、gitpython。

AgentOS/API extras 包括 FastAPI、uvicorn、SQLAlchemy、PyJWT、OpenTelemetry、OpenInference、croniter、pytz。

| 路径 | 作用 |
|---|---|
| `agno/agent` | 单 agent runtime |
| `agno/team` | 多 agent/team runtime |
| `agno/workflow` | pipeline workflow runtime |
| `agno/tools` | Toolkit、Function、decorator 工具层 |
| `agno/models` | provider abstraction |
| `agno/memory` | long-term memory |
| `agno/db` | storage abstraction |
| `agno/run` | run output、events、requirements |
| `agno/os` | AgentOS FastAPI control plane |
| `agno/tracing` | tracing / spans |
| `agno/skills` | skill system |
| `agno/registry` | runtime object registry |
| `agno/scheduler` | schedules |

Agno 覆盖面很宽：它把 agent runtime、team modes、workflow steps、model registry、tool execution、storage abstraction、tracing、approval state、schedules、AgentOS control plane 放在同一体系中。

## 3. 入口与模块

| 入口 | 路径 | 作用 |
|---|---|---|
| `Agent` | `libs/agno/agno/agent/agent.py` | 单 agent 配置与运行 |
| `Team` | `libs/agno/agno/team/team.py` | 多 agent/team 编排 |
| `Workflow` | `libs/agno/agno/workflow/workflow.py` | pipeline workflow |
| `AgentOS` | `libs/agno/agno/os/app.py` | FastAPI control plane |
| `Model` | `libs/agno/agno/models/base.py` | provider 抽象 |
| `Toolkit` / `Function` | `libs/agno/agno/tools/toolkit.py`, `function.py` | 工具抽象 |

## 4. Runtime、Agent、Team 与 Graph 执行

### Agent

`Agent` 是高度配置化主体，覆盖 model/fallback、session state、memory、db、history、knowledge、skills、tools、hooks、reasoning、structured output、streaming、events、telemetry。

典型执行流程：

1. 读取或创建 session。
2. 合并 metadata / session state。
3. 解析 dependencies。
4. 执行 pre-hooks。
5. 解析显式 tools、default tools、skills、knowledge tools。
6. 构造 system/user/model messages。
7. 可选更新 memory、learning、culture。
8. 调用 reasoning。
9. 调用模型。
10. 处理 tool calls。
11. 处理 structured output / followups。
12. 执行 post-hooks。
13. 更新 metrics 与 session summary。
14. 持久化 session 与 run output。

### Team

核心路径：

- `libs/agno/agno/team/team.py`
- `libs/agno/agno/team/mode.py`

`TeamMode` 包括：

- `coordinate`：leader 分派任务并综合结果。
- `route`：路由到专家成员。
- `broadcast`：并发广播给所有成员。
- `tasks`：leader 拆解目标并循环委派。

### Graph runtime

Agno 原生核心不是 graph runtime。Graph checkpoint 和 time travel 主要通过 LangGraph adapter：

- `libs/agno/agno/agents/langgraph/agent.py`

`LangGraphAgent` 包装 compiled graph，支持 `get_state_history`、`get_state`、`update_state`、`replay`、`fork`。

## 5. 工具与模型适配

工具核心路径：

- `libs/agno/agno/tools/toolkit.py`
- `libs/agno/agno/tools/function.py`
- `libs/agno/agno/tools/decorator.py`

| 模块 | 作用 |
|---|---|
| `Toolkit` | 管理工具函数、include/exclude、sync/async variants、连接生命周期、缓存 |
| `Function` | Pydantic tool schema/runtime model，含 hooks、HITL flags、cache |
| `@tool` | 将 Python 函数包装为 Agno `Function` |
| `FunctionCall` | 执行模型生成的 tool call，支持 sync/async/generator/cache/hooks |

工具来源包括显式传入的 tools、Toolkit、Function、callable、provider built-in tools、default memory/history/knowledge tools、skills access tools、MCP tools。

模型核心路径：

- `libs/agno/agno/models/base.py`
- `libs/agno/agno/models/response.py`
- `libs/agno/agno/models/utils.py`
- `libs/agno/agno/models/fallback.py`

`Model` 支持 sync/async invocation、streaming、tool call formatting/execution loop、retry/exponential backoff、context-window/rate-limit error 分类、response cache、structured output、provider response delta 解析。

Provider registry 显式列出 OpenAI、Anthropic、Bedrock、Azure、Gemini、Groq、Cohere、Cerebras、Ollama、OpenRouter、Portkey、Mistral、Meta、IBM、DeepSeek、Nvidia、Together 等。Fallback 可按 general error、rate limit、context overflow 选择替代模型。

## 6. Memory、State、Checkpoint 与 Storage

### Session state

Agent、Team、Workflow 都支持 `session_state`、`add_session_state_to_context`、`enable_agentic_state`、`overwrite_db_session_state`、`cache_session`。Session state 可注入上下文，也可暴露给模型通过工具更新。

### Long-term memory

核心路径：`libs/agno/agno/memory/manager.py`。

`MemoryManager` 通过 DB-backed user-scoped memory store 生成、更新、删除、清理用户记忆。

### Checkpoint

Agno 原生 workflow 更偏 run output、paused state 与 session persistence。完整 graph checkpoint/time travel 主要通过 LangGraph adapter 提供。

### Storage

核心路径：`libs/agno/agno/db/base.py`。

`BaseDb` 覆盖 sessions、memories、metrics、eval runs、knowledge、traces/spans、components/configs/links、learnings、schedules/schedule runs、approvals。

存储适配包括 SQLite、Postgres、Async Postgres、Mongo、MySQL、Redis、Firestore、GCS JSON、Dynamo、JSON、in-memory、SingleStore、SurrealDB。

## 7. Workflow Orchestration

核心路径：

- `libs/agno/agno/workflow/workflow.py`
- `libs/agno/agno/workflow/step.py`
- `libs/agno/agno/workflow/types.py`
- `condition.py`
- `router.py`
- `parallel.py`
- `loop.py`
- `steps.py`

| 组件 | 作用 |
|---|---|
| `Step` | 包装 function、agent、team 或 nested workflow |
| `Steps` | 顺序步骤组 |
| `Loop` | 循环执行 |
| `Parallel` | 并行执行 |
| `Condition` | 条件分支，支持 callable、bool、CEL |
| `Router` | 路由选择 |
| nested `Workflow` | workflow 组合 |

Workflow 支持 sync、async、streaming、step events、executor events、pause、continue、cancel。关键限制是 `Parallel` 明确不支持 HITL pause。

## 8. Human-in-the-loop

### Tool-level HITL

`Function` / `@tool` 支持 `requires_confirmation`、`requires_user_input`、`external_execution`、`stop_after_tool_call`。触发后生成 `RunRequirement`，run 进入 paused 状态，等待 continue。

### Workflow-level HITL

`HumanReview` 支持 pre-execution confirmation、user input、output review、loop iteration review、reject/timeout/error 策略。

### Admin approval

核心路径：

- `libs/agno/agno/approval/decorator.py`
- `libs/agno/agno/run/approval.py`
- `libs/agno/agno/os/auth.py`

Agno 支持 pending approval record，并在 resolution 后继续 run。

## 9. 事件、日志与 Observability

核心路径：

- `libs/agno/agno/run/agent.py`
- `libs/agno/agno/run/team.py`
- `libs/agno/agno/run/workflow.py`
- `libs/agno/agno/metrics.py`
- `libs/agno/agno/tracing/setup.py`
- `libs/agno/agno/tracing/exporter.py`

Agent events 包括 `RunStarted`、`RunContent`、`RunCompleted`、`RunPaused`、`ToolCallStarted`、`ToolCallCompleted`、`ReasoningStarted`、`MemoryUpdateStarted`、`ModelRequestStarted`、`CompressionStarted`、`CustomEvent`。

Tracing 使用 OpenTelemetry 与 OpenInference。`DatabaseSpanExporter` 将 spans/traces 写入 Agno DB。

## 10. 测试与验证

测试目录：

- `libs/agno/tests/unit`
- `libs/agno/tests/integration`
- `libs/agno/tests/system`

CI 包括 `.github/workflows/test.yml` 的 ruff、mypy、pytest unit tests，以及 `.github/workflows/performance.yml` 与 LangGraph 做 performance comparison。System tests 使用多容器环境，覆盖 agents、teams、workflows、sessions、memory、knowledge、traces、evals、metrics、A2A、AG-UI、MCP、Slack 等 API routes。

## 11. 核心源码路径

| 关注点 | 路径 |
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

## 12. AgentOS 与 Control Plane

核心路径：`libs/agno/agno/os/app.py`。

AgentOS 是 FastAPI runtime/control plane，可注册 agents、teams、workflows、knowledge、interfaces、db、MCP server、scheduler、tracing、authorization/RBAC。Router 覆盖 agents、teams、workflows、approvals、components、database、evals、health、knowledge、memory、metrics、registry、schedules、sessions、traces。

安全能力包括 JWT authorization、scope/RBAC、optional user isolation、MCP host/origin protection、scheduler internal service token。

## 13. 对 Marix 的借鉴

1. Agent、Team、Workflow 统一围绕 run output、events、metrics、session persistence。
2. `Toolkit`、`Function`、decorator 工具层分工清晰。
3. HITL 是一等 runtime state，支持 pause、requirement、approval、continue。
4. 用集中 provider registry 管理模型适配。
5. control plane 可通过统一 API 暴露 agents、teams、workflows、memory、traces、approvals、schedules。
6. Skills 渐进式加载，避免一次性塞满上下文。
7. run events、metrics、OpenTelemetry spans、DB exporter 组合提供内建 observability。

## 14. 风险与反模式

1. `Agent`、`Team`、`Workflow` 配置面很宽，学习和组合状态成本高。
2. 核心文件较大，维护成本偏高。
3. `Parallel` 不支持 HITL pause，复杂并发审批需拆分。
4. 原生 graph/checkpoint 不如 LangGraph 完整，依赖 adapter。
5. extras 依赖面很大，供应链和版本冲突风险高。
6. Telemetry 默认策略在平台引入前需要明确隐私边界。
7. Registry 可恢复部分 runtime objects，但完整 serialization/rehydration 仍需谨慎。
