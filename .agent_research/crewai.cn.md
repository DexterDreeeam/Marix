# CrewAI Framework Agent 研究

## 1. 来源与活跃度

| 项 | 内容 |
|---|---|
| 仓库 | https://github.com/crewAIInc/crewAI |
| 默认分支 | `main` |
| 主语言 | Python |
| License | MIT |
| 性质 | 多 agent / role-playing autonomous agent orchestration framework |
| 近期活跃 | GitHub API：`pushed_at=2026-06-22T16:26:03Z`；latest release `1.14.7`，发布于 `2026-06-11` |

主要研究输入：

- https://github.com/crewAIInc/crewAI
- https://github.com/crewAIInc/crewAI/releases/tag/1.14.7
- `lib/crewai/src/crewai/__init__.py`
- `lib/crewai/src/crewai/agent/core.py`
- `lib/crewai/src/crewai/crew.py`
- `lib/crewai/src/crewai/flow/runtime/__init__.py`
- `lib/crewai/src/crewai/tools/*`
- `lib/crewai/src/crewai/state/*`
- `.github/workflows/*`

## 2. 技术栈与性质

CrewAI 是 Python monorepo，并将 runtime、tools、CLI 与支撑包分层：

| 路径 | 作用 |
|---|---|
| `lib/crewai` | 核心 agent、crew、flow runtime |
| `lib/crewai-tools` | 工具体系与 MCP 适配 |
| `lib/cli` | `crewai` CLI |
| `lib/crewai-core` | CLI、token、lock 等底层能力 |
| `lib/crewai-files` | 文件处理 |
| `lib/devtools` | 开发工具 |

关键技术包括 Python `>=3.10,<3.14`、Pydantic v2、Click CLI、uv workspace、OpenTelemetry tracing、MCP SDK、LanceDB、Qdrant Edge、ChromaDB、pytest、ruff、mypy、bandit、pip-audit、CodeQL。

架构上，CrewAI 同时提供高层团队抽象 `Crew` 和较低层的事件驱动 workflow runtime `Flow`：前者适合 role/task 协作，后者适合确定性流程编排。

## 3. 入口与模块

`crewai.__init__` 导出的 Python API 包括 `Agent`、`Crew`、`Task`、`Flow`、`LLM`、`BaseLLM`、`Knowledge`、`Memory`、`CheckpointConfig`、`Process`、`CrewOutput`、`TaskOutput`。

CLI 入口：

- `crewai = "crewai_cli.cli:crewai"`

常用 CLI：

- `crewai create crew|flow`
- `crewai run`
- `crewai train`
- `crewai test`
- `crewai replay`
- `crewai memory`
- `crewai deploy`
- `crewai tool`

## 4. Runtime、Agent、Team 与 Graph 执行

### Agent

核心路径：`lib/crewai/src/crewai/agent/core.py`。

`Agent` 覆盖 role、goal、backstory、LLM、function calling LLM、tools、memory、knowledge、planning、reasoning、guardrail、skills、MCP、A2A、executor class。典型执行流程是准备 task prompt，注入 knowledge/memory context，整理 tools，发出 agent/LLM/tool 事件，调用 executor，处理 context length、retry、失败事件，保存 last messages，清理 MCP client。

### Crew

核心路径：`lib/crewai/src/crewai/crew.py`。

`Crew` 支持 `Process.sequential` 和 `Process.hierarchical`。顺序模式按 task 列表执行；层级模式创建或使用 manager agent，通过 delegation tools 管理子任务。

### Flow

核心路径：

- `lib/crewai/src/crewai/flow/runtime/__init__.py`
- `lib/crewai/src/crewai/flow/dsl/*`
- `lib/crewai/src/crewai/flow/flow_definition.py`

`Flow` 是事件驱动 graph runtime，支持 `@start`、`@listen`、`@router`、`or_`、`and_`、`@human_feedback`、dict/Pydantic/JSON schema state、parallel starts/listeners、checkpoint restore、pending human feedback resume。

## 5. 工具与模型适配

工具核心路径：

- `lib/crewai/src/crewai/tools/base_tool.py`
- `lib/crewai/src/crewai/tools/structured_tool.py`
- `lib/crewai/src/crewai/tools/tool_usage.py`

| 模块 | 作用 |
|---|---|
| `BaseTool` | Pydantic tool 模型、args schema、result schema、cache、usage limit |
| `CrewStructuredTool` | 将 callable 包装为结构化 tool，支持 sync/async 与参数校验 |
| `ToolUsage` | tool 选择、解析、执行、缓存、重试、事件、raw result 跟踪 |

MCP 集成位于 `lib/crewai/src/crewai/mcp/tool_resolver.py`，支持 stdio、HTTP、SSE、HTTPS MCP URL、AMP refs、tool filters、schema cache、timeout/retry。

模型/provider 路径：

- `lib/crewai/src/crewai/llm.py`
- `lib/crewai/src/crewai/llms/base_llm.py`
- `lib/crewai/src/crewai/llms/providers/*`

native providers 包括 OpenAI、Anthropic/Claude、Azure OpenAI、Gemini/Google、Bedrock/AWS、Snowflake、OpenAI-compatible、OpenRouter、DeepSeek、Ollama、Hosted vLLM、Cerebras、DashScope。若没有命中 native provider，则 lazy-load LiteLLM 作为 fallback。`BaseLLM` 层会产生 LLM call started/completed/failed、stream chunk、thinking、usage tracking 等事件。

## 6. Memory、State、Checkpoint 与 Storage

### Unified Memory

核心路径：

- `lib/crewai/src/crewai/memory/unified_memory.py`
- `lib/crewai/src/crewai/memory/storage/*`

Unified memory 默认 LanceDB，也可使用 Qdrant Edge。它支持 background writes，`recall()` 前 drain writes 形成读屏障；shallow recall 使用向量搜索，deep recall 走 `RecallFlow`。

### Knowledge 与 RAG

核心路径：

- `lib/crewai/src/crewai/knowledge/knowledge.py`
- `lib/crewai/src/crewai/knowledge/source/*`
- `lib/crewai/src/crewai/rag/*`

支持 string、docling、CSV、Excel、JSON、PDF、text file 等 source。

### Checkpoint 与 RuntimeState

核心路径：

- `lib/crewai/src/crewai/state/checkpoint_config.py`
- `lib/crewai/src/crewai/state/runtime.py`
- `lib/crewai/src/crewai/state/event_record.py`
- `lib/crewai/src/crewai/state/provider/json_provider.py`
- `lib/crewai/src/crewai/state/provider/sqlite_provider.py`

默认 checkpoint 位置为 `./.checkpoints`，provider 为 JSON，默认事件为 `task_completed`。也可使用 SQLite provider。`on_events=["*"]` 可记录所有事件。`EventRecord` 是事件图，记录 parent/child、trigger、previous/next、started/completed 等边。

Storage 总结：Memory 使用 LanceDB/Qdrant Edge；Knowledge/RAG 可用 ChromaDB、Qdrant 等；Checkpoint 使用 JSON/SQLite；Flow persistence 使用 SQLite，包含 `flow_states`、`pending_feedback`、WAL、lock store。

## 7. Workflow Orchestration

CrewAI 有两层编排：

1. `Crew`：agent team/task 顺序或层级编排。
2. `Flow`：事件驱动 graph workflow 编排。

`FlowDefinition` 支持 YAML/JSON 风格可序列化 workflow，action 类型包括 `code`、`tool`、`crew`、`agent`、`expression`、`script`、`each`。其中 `script` action 可执行 Python，需要开启 `CREWAI_ALLOW_FLOW_SCRIPT_EXECUTION`；源码说明它不是 sandbox，不可用于不可信定义。

## 8. Human-in-the-loop

### Task-level HITL

`Task.human_input=True` 时，executor 最终答案进入 human feedback 流程。默认 provider 是控制台交互循环：空反馈表示接受，否则继续反馈修正。

核心路径：

- `lib/crewai/src/crewai/task.py`
- `lib/crewai/src/crewai/core/providers/human_input.py`

### Flow-level HITL

`@human_feedback` 支持 sync feedback、async feedback pending、`HumanFeedbackPending`、pending context 持久化、`resume()` / `resume_async()`、LLM 将反馈折叠为 outcome 并路由、`learn=True` 写入 memory。

核心路径：

- `lib/crewai/src/crewai/flow/human_feedback.py`
- `lib/crewai/src/crewai/flow/async_feedback/types.py`

## 9. 事件、日志与 Observability

核心路径：

- `lib/crewai/src/crewai/events/event_bus.py`
- `lib/crewai/src/crewai/events/event_listener.py`
- `lib/crewai/src/crewai/events/types/*`
- `lib/crewai/src/crewai/events/listeners/tracing/*`

`EventBus` 是 singleton event bus。sync handler 在线程池执行，async handler 在后台 event loop 执行；支持 dependency-aware handler execution；`emit()` 设置 parent/previous/trigger metadata；event 写入 `RuntimeState`；`replay()` 可重放事件；LLM stream chunk 同步处理以保证顺序。Tracing 使用 OpenTelemetry。

## 10. 测试与验证

根 pytest 配置覆盖：

- `lib/crewai/tests`
- `lib/crewai-tools/tests`
- `lib/crewai-files/tests`
- `lib/cli/tests`
- `lib/crewai-core/tests`

测试特点包括 `--block-network`、pytest-xdist、timeout 60、importlib mode、临时 `CREWAI_STORAGE_DIR`、`CREWAI_TESTING=true`、清理 event handlers/runtime state、VCR 过滤敏感 header。CI 包括 tests、linter、type-checker、vulnerability-scan、CodeQL。

## 11. 核心源码路径

| 关注点 | 路径 |
|---|---|
| Public API | `lib/crewai/src/crewai/__init__.py` |
| Agent runtime | `lib/crewai/src/crewai/agent/core.py` |
| Crew runtime | `lib/crewai/src/crewai/crew.py` |
| Flow runtime | `lib/crewai/src/crewai/flow/runtime/__init__.py`, `flow/dsl/*`, `flow/flow_definition.py` |
| Tools | `tools/base_tool.py`, `tools/structured_tool.py`, `tools/tool_usage.py` |
| MCP | `mcp/tool_resolver.py` |
| Model providers | `llm.py`, `llms/base_llm.py`, `llms/providers/*` |
| Memory/RAG | `memory/unified_memory.py`, `memory/storage/*`, `knowledge/knowledge.py`, `rag/*` |
| State/checkpoint | `state/checkpoint_config.py`, `state/runtime.py`, `state/event_record.py`, `state/provider/*` |
| HITL | `task.py`, `core/providers/human_input.py`, `flow/human_feedback.py`, `flow/async_feedback/types.py` |
| Events/tracing | `events/event_bus.py`, `events/event_listener.py`, `events/types/*`, `events/listeners/tracing/*` |

## 12. 对 Marix 的借鉴

1. 双层编排：team orchestration 与 deterministic graph workflow 分离但可组合。
2. 事件即状态：EventBus 写入 RuntimeState/EventRecord。
3. 扩展入口丰富：tools、MCP、skills、hooks、A2A、project decorators、providers。
4. HITL pending feedback 持久化，保证 run 可恢复。
5. native provider + LiteLLM fallback 兼顾一等适配质量与覆盖面。
6. Memory backend protocol + factory 避免 runtime 绑定单一向量库。
7. CLI scaffolding + declarative config 适合项目模板复用。
8. 测试默认断网并隔离全局状态，适合 agent runtime 的确定性验证。

## 13. 风险与反模式

1. API 面很大，兼容层和 deprecated 路径较多。
2. `FlowDefinition` 的 `script` action 若错误启用，存在执行不可信代码风险。
3. singleton event bus、global hooks、contextvars 对并发与测试隔离有压力。
4. 依赖较重。
5. Task-level HITL 默认偏 CLI，生产/Web 场景需要替换 provider。
6. 部分平台能力与 CrewAI AMP/deploy 边界相关，OSS 能力需确认。
7. 文档可能滞后源码，最终应以源码路径为准。
