# LangGraph Framework Agent 研究

## 1. 来源与活跃度

| 项 | 内容 |
|---|---|
| 仓库 | https://github.com/langchain-ai/langgraph |
| 默认分支 | `main` |
| 主语言 | Python |
| License | MIT |
| 性质 | 面向长期运行、有状态 agent/workflow 的低层 orchestration runtime |
| 近期活跃 | GitHub API：`pushed_at=2026-06-21T01:21:56Z`；latest release `1.2.6`，发布于 `2026-06-18` |

主要研究输入：

- https://github.com/langchain-ai/langgraph
- https://github.com/langchain-ai/langgraph/releases/tag/1.2.6
- `libs/langgraph/langgraph/graph/state.py`
- `libs/langgraph/langgraph/pregel/main.py`
- `libs/langgraph/langgraph/pregel/_runner.py`
- `libs/langgraph/langgraph/pregel/_loop.py`
- `libs/prebuilt/langgraph/prebuilt/tool_node.py`
- `libs/checkpoint/*`
- `libs/sdk-py/*`
- `libs/cli/*`

## 2. 技术栈与性质

| 包/目录 | 作用 |
|---|---|
| `libs/langgraph` | 核心 Python runtime：Graph API、Pregel runtime、streaming、types |
| `libs/prebuilt` | `ToolNode`、`create_react_agent` 等预构建组件 |
| `libs/checkpoint` | checkpoint、store、cache 基础接口与内存实现 |
| `libs/checkpoint-postgres` | Postgres checkpointer/store |
| `libs/checkpoint-sqlite` | SQLite checkpointer/store/cache |
| `libs/sdk-py` | Assistants、threads、runs、cron、store API SDK |
| `libs/cli` | `langgraph` CLI，本地 server、构建、部署配置 |
| `libs/sdk-js` | JS/TS 迁移说明，实际实现迁移至 `langgraphjs` |

核心依赖包括 LangChain Core、Pydantic、httpx、websockets、psycopg、aiosqlite、sqlite-vec、orjson、ormsgpack。LangGraph 的定位是 runtime substrate，而不是完整高层 agent 产品：它定义 state、graph execution、durability、checkpointing、streaming 和集成缝隙。

## 3. 入口与模块

主要用户入口：

- `langgraph.graph.StateGraph`
- `START`
- `END`
- `MessagesState`
- `add_messages`

核心路径：

- `libs/langgraph/langgraph/graph/__init__.py`
- `libs/langgraph/langgraph/graph/state.py`

`StateGraph` 是 builder；`compile()` 后产物为 `CompiledStateGraph`，继承 Pregel runtime。

## 4. Runtime、Agent、Team 与 Graph 执行

LangGraph 核心是 graph runtime，而不是内置高层 agent/team 产品。

核心模型：

- 节点是函数或 Runnable。
- State 可使用 TypedDict、dataclass、Pydantic。
- 每个 state key 可配置 reducer。
- edge/conditional edge 控制流程。
- compiled graph 使用 Pregel 模型执行。

Pregel 执行模型：

1. Plan：选择活跃 actor/node 集合。
2. Execute：并行执行当前 step 的节点。
3. Update：写入 channel，下一 step 可见。
4. 循环直到无活跃节点或终止。

核心路径：

- `libs/langgraph/langgraph/pregel/main.py`
- `libs/langgraph/langgraph/pregel/_runner.py`
- `libs/langgraph/langgraph/pregel/_loop.py`

多 agent/team 通常通过 subgraph 作为 node、`Command.PARENT` 从子图跳回父图、`Send` 做 fan-out/map-reduce、`RemoteGraph` 将远端 assistant/graph 当作本地图节点。

## 5. 工具与模型适配

核心路径：`libs/prebuilt/langgraph/prebuilt/tool_node.py`。

`ToolNode` 支持 `langchain_core.tools.BaseTool`、普通函数转 tool、message-list/state-dict/direct-tool-call 输入、sync executor map、async `asyncio.gather`、`InjectedState`、`InjectedStore`、`ToolRuntime`、`handle_tool_errors`、`wrap_tool_call` / `awrap_tool_call`。

关键设计是 state、store、runtime 可以注入工具，但不暴露给 LLM tool schema。

LangGraph 不维护自己的模型 provider 层，主要委托 LangChain：`BaseChatModel`、`LanguageModelLike`、`RunnableBinding`、`init_chat_model("provider:model")`、`.bind_tools(...)`、`.with_structured_output(...)`。

`create_react_agent` 支持静态模型、字符串模型标识、动态模型函数 `(state, runtime) -> model`，但 prebuilt agent API 有向 LangChain agents 迁移的压力，长期依赖需谨慎。

## 6. Memory、State、Checkpoint 与 Storage

| 层 | 说明 |
|---|---|
| State | TypedDict/dataclass/Pydantic，每个 key 可 reducer |
| Short-term memory | thread-scoped，通过 checkpointer 持久化 graph state |
| Checkpointer | `BaseCheckpointSaver`，支持 get/put/list/put_writes/delete/prune/copy_thread |
| InMemorySaver | debug/test 实现 |
| Long-term memory | `BaseStore`，跨 thread namespace key-value store，支持 semantic search、TTL |
| Cache | `BaseCache` 及 memory、redis、sqlite 实现 |

核心路径：

- `libs/checkpoint/langgraph/checkpoint/base/__init__.py`
- `libs/checkpoint/langgraph/checkpoint/memory/__init__.py`
- `libs/checkpoint/langgraph/store/base/__init__.py`

Checkpoint storage 包括 `InMemorySaver`、`PostgresSaver` / `AsyncPostgresSaver`、`SqliteSaver` / `AsyncSqliteSaver`。Store storage 包括 `InMemoryStore`、Postgres store、SQLite store。Postgres checkpointer 表包括 `checkpoints`、`checkpoint_blobs`、`checkpoint_writes`、`checkpoint_migrations`。SQLite store 支持 TTL 与向量表，依赖 `sqlite-vec`。

## 7. Workflow Orchestration

LangGraph 提供两种 API：

1. Graph API：`StateGraph`、node、edge、conditional edge、subgraph。
2. Functional API：`@entrypoint` / `@task`，把普通 Python workflow 包装为 durable workflow。

Runtime 控制包括 `RetryPolicy`、`TimeoutPolicy`、`CachePolicy`，以及 durability 模式 `sync`、`async`、`exit`。

Agent Server 部署通常包含 API server、queue worker、Postgres、Redis、stream/cancel signal 和 SDK clients。

## 8. Human-in-the-loop

核心机制是 `interrupt(value)` + `Command(resume=...)`。

特点：

- `interrupt` 暂停 graph。
- 需要 checkpointer 和 `thread_id`。
- 恢复时节点会从头重新执行到 interrupt 点。
- 支持多个 interrupt id 映射恢复。
- stream events 暴露 interrupts。

核心路径：`libs/langgraph/langgraph/types.py`，以及 LangGraph interrupts 文档。

## 9. 事件、日志与 Observability

Stream modes 包括：

- `values`
- `updates`
- `messages`
- `custom`
- `checkpoints`
- `tasks`
- `debug`

v3 event streaming 提供 typed projections：messages、values、subgraphs、interrupts、extensions。

核心路径：

- `libs/langgraph/langgraph/stream/_mux.py`
- `libs/langgraph/langgraph/stream/transformers.py`
- `libs/langgraph/langgraph/callbacks.py`

LangSmith 是官方推荐的 tracing、debugging、evaluation、deployment observability 方案。

## 10. 测试与验证

测试路径：

- `libs/langgraph/tests`
- `libs/prebuilt/tests`
- `libs/checkpoint/tests`
- `libs/checkpoint-postgres/tests`
- `libs/checkpoint-sqlite/tests`

CI/tooling 覆盖 Python 3.10-3.14、ruff、ty、codespell、syrupy snapshot、pytest-xdist、CLI integration tests、schema drift checks、strict msgpack tests。

## 11. 核心源码路径

| 关注点 | 路径 |
|---|---|
| Graph builder | `libs/langgraph/langgraph/graph/__init__.py`, `graph/state.py` |
| Pregel runtime | `pregel/main.py`, `pregel/_runner.py`, `pregel/_loop.py` |
| Tool execution | `libs/prebuilt/langgraph/prebuilt/tool_node.py` |
| Checkpoint base | `libs/checkpoint/langgraph/checkpoint/base/__init__.py` |
| Memory checkpointer | `libs/checkpoint/langgraph/checkpoint/memory/__init__.py` |
| Store base | `libs/checkpoint/langgraph/store/base/__init__.py` |
| Streaming | `libs/langgraph/langgraph/stream/_mux.py`, `stream/transformers.py`, `callbacks.py` |
| SDK/CLI | `libs/sdk-py/*`, `libs/cli/*` |

## 12. 对 {{proj}} 的借鉴

1. Builder/runtime 分层，让声明与执行独立演进。
2. Channel + reducer state model 支撑并行与恢复。
3. Checkpoint 与 Store 分离，避免短期状态和长期记忆混用。
4. 事件流投影层让 UI、SDK、observability 复用同一 runtime feed。
5. Tool runtime/state/store 注入时不暴露内部参数给模型可见 schema。
6. 显式 durability modes，让性能与可靠性取舍可配置。
7. RemoteGraph 支持分布式 agent 组合。

## 13. 风险与反模式

1. 低层框架复杂度高。
2. `create_react_agent` prebuilt API 有迁移风险。
3. 完整生产体验可能依赖 LangSmith/Agent Server。
4. v3 event streaming 仍可能演进。
5. checkpoint pruning/copy 在 DeltaChannel 场景需谨慎。
6. private state streaming 不自动脱敏。
7. 模型/tool permission/sandbox 不是核心库内置能力。
