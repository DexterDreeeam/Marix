# Microsoft AutoGen Framework Agent 研究

## 1. 来源与活跃度

| 项 | 内容 |
|---|---|
| 仓库 | https://github.com/microsoft/autogen |
| 默认分支 | `main` |
| 主语言 | Python，另有 .NET/C#、protobuf/gRPC、Studio Web/API |
| GitHub license | API 显示 `CC-BY-4.0`；各子包需分别查看 |
| 当前状态 | README 标记 Maintenance Mode，推荐新用户转向 Microsoft Agent Framework |
| 近期活跃 | GitHub API：`pushed_at=2026-04-15T11:59:09Z`；latest release `python-v0.7.5`，发布于 `2025-09-30` |

主要研究输入：

- https://github.com/microsoft/autogen
- https://github.com/microsoft/autogen/releases/tag/python-v0.7.5
- `python/packages/autogen-core`
- `python/packages/autogen-agentchat`
- `python/packages/autogen-ext`
- `python/packages/autogen-studio`
- `dotnet/src`
- `protos`

## 2. 技术栈与性质

| 路径 | 作用 |
|---|---|
| `python/packages/autogen-core` | 低层 runtime、agent protocol、message passing、tool/model/memory 抽象、telemetry |
| `python/packages/autogen-agentchat` | 高层对话 agent/team API |
| `python/packages/autogen-ext` | Provider、MCP、code executor、memory backend、cache、gRPC runtime |
| `python/packages/autogen-studio` | AutoGen Studio GUI/API |
| `python/packages/agbench` | benchmark/evaluation |
| `python/packages/magentic-one-cli` | Magentic-One CLI |
| `dotnet/src` | .NET 版本与 provider |
| `protos` | gRPC / CloudEvent 协议 |

Python 包版本线为 `autogen-core==0.7.5`、`autogen-agentchat==0.7.5`、`autogen-ext==0.7.5`，目标 Python `>=3.10`。

AutoGen 是 protocol-first 架构，分离 core runtime、AgentChat 高层 teams、extensions 和 Studio。它对 message-based runtime abstraction、workbench/tool container、state save/load、event telemetry 有参考价值，但 maintenance mode 让它不适合作为新的长期唯一依赖。

## 3. 入口与模块

CLI 入口：

- Studio CLI：`autogenstudio = "autogenstudio.cli:run"`
- Magentic-One CLI：`m1 = "magentic_one_cli._m1:main"`

核心模块：

| 模块 | 路径 |
|---|---|
| Agent protocol | `autogen-core/src/autogen_core/_agent.py` |
| Runtime protocol | `autogen-core/src/autogen_core/_agent_runtime.py` |
| SingleThreaded runtime | `autogen-core/src/autogen_core/_single_threaded_agent_runtime.py` |
| AssistantAgent | `autogen-agentchat/src/autogen_agentchat/agents/_assistant_agent.py` |
| UserProxyAgent | `autogen-agentchat/src/autogen_agentchat/agents/_user_proxy_agent.py` |
| GroupChat | `teams/_group_chat/*` |
| GraphFlow | `teams/_group_chat/_graph/_digraph_group_chat.py` |
| Magentic-One | `teams/_group_chat/_magentic_one/*` |
| Tool abstraction | `autogen-core/src/autogen_core/tools/*` |
| Model client | `autogen-core/src/autogen_core/models/_model_client.py` |

## 4. Runtime、Agent、Team 与 Graph 执行

### Core runtime

AutoGen 将 agent 与 runtime 解耦：

- `Agent` 处理消息、保存/加载状态、关闭。
- `AgentRuntime` 负责 send/publish、factory 注册、subscription、state save/load。
- `SingleThreadedAgentRuntime` 用 asyncio queue 处理 direct/publish/response envelope，支持 intervention handler、tracing、异常记录。

### AssistantAgent loop

典型流程：

1. 新消息进入 model context。
2. 查询 memory 并注入 context。
3. 从 workbench 和 handoff 定义收集 tools。
4. 调用 `ChatCompletionClient.create` 或 `create_stream`。
5. 普通文本返回最终响应。
6. function calls 发出 tool request event。
7. workbench 执行工具；多 tool call 默认并发。
8. 产生 tool execution event。
9. 根据 `max_tool_iterations`、handoff、`reflect_on_tool_use` 决定继续或结束。

### Team 与 GroupChat

`BaseGroupChat` 注册 participant container 和 group chat manager，通过 topic subscription 驱动多 agent 对话。

内置 team/workflow 包括 RoundRobin、Selector、Swarm、MagenticOneGroupChat、GraphFlow。

### GraphFlow

GraphFlow 是 experimental 的有向图 workflow，支持 DAG、顺序节点、并行 fan-out、join、条件边和 loop。循环必须有退出条件、termination condition 或 max turns。

## 5. 工具与模型适配

工具核心路径：

- `autogen-core/src/autogen_core/tools/_base.py`
- `autogen-core/src/autogen_core/tools/_workbench.py`
- `autogen-ext/src/autogen_ext/tools/mcp/_workbench.py`

| 层 | 作用 |
|---|---|
| `ToolSchema` / `Tool` protocol | name、description、JSON schema、`run_json`、state save/load |
| `BaseTool` / `BaseStreamTool` | 基于 Pydantic args/return type 的工具 |
| `Workbench` | 动态工具集合生命周期容器，支持 list/call/start/stop/reset/save/load |
| `McpWorkbench` | MCP server tools、resources、prompts、sampling host 封装 |

`FunctionTool` 能把 Python function 包成工具，但从配置恢复时可能涉及源码/import 动态执行，需要可信配置边界。

模型抽象是 `ChatCompletionClient`，能力包括 `create`、`create_stream`、`close`、`usage`、`count_tokens`、`remaining_tokens`、`model_info`。`ModelInfo` 声明 vision、function calling、JSON output、model family、structured output、multiple system messages 支持。

`autogen-ext` provider 覆盖 OpenAI/Azure OpenAI、Anthropic、Ollama、Azure AI、Gemini、Semantic Kernel、llama.cpp 等。.NET 侧也有 OpenAI、Anthropic、Gemini、Ollama、Mistral、SemanticKernel 等适配。

## 6. Memory、State、Checkpoint 与 Storage

AutoGen 没有统一 checkpoint service，主要通过分层 `save_state/load_state` 组合：

| 类型 | 机制 |
|---|---|
| Model context | `ChatCompletionContext` 保存 LLM message history |
| Memory | `Memory.query/add/update_context/clear/close` |
| Agent state | `AssistantAgentState` 保存 `llm_context` |
| Team state | `TeamState` 保存每个 agent/manager state |
| Manager state | `message_thread`, `current_turn` |
| Graph/Magentic-One state | active nodes、task、facts、plan、round、stall |
| Storage backend | Studio SQLModel/SQLAlchemy/Alembic；ext memory/cache 有 ChromaDB、mem0、Redis、diskcache |

Storage 总结：

| 场景 | 实现 |
|---|---|
| Agent/team resume | `save_state/load_state` 返回 JSON-like mapping，由应用自行持久化 |
| Studio | SQLModel / SQLAlchemy / Alembic |
| Memory backend | ListMemory、ChromaDB、mem0、Redis 等 |
| Cache | InMemory、diskcache、redis |
| Code execution | local/docker executor workdir 与 container 生命周期 |

## 7. Workflow Orchestration

AutoGen 的 workflow 是多 pattern，而不是单 DSL：

1. `AgentTool`：把 agent 包成 tool。
2. GroupChat：manager-driven 多 agent 对话。
3. SelectorGroupChat：模型/规则选择下一 speaker。
4. Swarm：handoff-driven agent 交接。
5. GraphFlow：有向图、条件边、并行、join、loop。
6. Magentic-One：ledger-based 规划、进度追踪、re-plan。
7. Studio：GUI 配置与运行 team/workflow。

## 8. Human-in-the-loop

核心 HITL 组件是 `UserProxyAgent`。

能力：

- 通过 `input_func` 获取用户输入。
- 输入前发出 `UserInputRequestedEvent`。
- 支持 `CancellationToken`。
- 对慢速人工响应，推荐停止 team、保存状态、等待用户后恢复，而不是长期阻塞 team。

相关 termination 包括 `HandoffTermination`、`SourceMatchTermination`。

## 9. 事件、日志与 Observability

AgentChat events 包括：

- `ToolCallRequestEvent`
- `ToolCallExecutionEvent`
- `MemoryQueryEvent`
- `ModelClientStreamingChunkEvent`
- `UserInputRequestedEvent`
- `SelectSpeakerEvent`
- `ThoughtEvent`

Core logging events 包括 `LLMCallEvent`、`ToolCallEvent`、`MessageEvent`、`MessageDroppedEvent`、`MessageHandlerExceptionEvent`。

Telemetry 使用 OpenTelemetry span 和结构化 LLM/tool/message events，辅助函数包括 `trace_tool_span`、`trace_create_agent_span`、`trace_invoke_agent_span`。

## 10. 测试与验证

Python workspace 使用 uv、ruff、strict mypy、strict pyright、pytest、pytest-asyncio、pytest-cov、pytest-xdist、gRPC proto generation/test。

CI 包含 format、lint、mypy、docs-mypy、pyright、tests、gRPC tests、Windows/ext tests、coverage artifacts。

## 11. 核心源码路径

| 关注点 | 路径 |
|---|---|
| Core protocols | `autogen-core/src/autogen_core/_agent.py`, `_agent_runtime.py` |
| Local runtime | `autogen-core/src/autogen_core/_single_threaded_agent_runtime.py` |
| Assistant/User agents | `autogen-agentchat/src/autogen_agentchat/agents/_assistant_agent.py`, `_user_proxy_agent.py` |
| Teams | `autogen-agentchat/src/autogen_agentchat/teams/_group_chat/*` |
| GraphFlow | `teams/_group_chat/_graph/_digraph_group_chat.py` |
| Magentic-One | `teams/_group_chat/_magentic_one/*`, `python/packages/magentic-one-cli` |
| Tools/workbench | `autogen-core/src/autogen_core/tools/*`, `autogen-ext/src/autogen_ext/tools/mcp/_workbench.py` |
| Model client | `autogen-core/src/autogen_core/models/_model_client.py` |
| Studio/storage | `python/packages/autogen-studio` |
| Protocols | `protos` |

## 12. 对 Marix 的借鉴

1. Core、AgentChat、Extensions、Studio 分层清晰。
2. Agent、Runtime、Tool、ModelClient、Memory 都协议优先。
3. 事件模型覆盖 tool、memory、stream、user input、speaker selection。
4. Workbench 模式适合 MCP、多工具容器和 sandbox 生命周期。
5. 分层 save/load 支持长任务 pause/resume，而不需要一个单体 checkpoint service。
6. 同时支持 conversation orchestration 与 graph orchestration。
7. HITL 使用 stop/save/resume 比长期阻塞 team 更安全。
8. `ModelInfo` 式能力声明让 provider 行为显式化。
9. 严格 CI 对基础 runtime 代码有参考价值。

## 13. 风险与反模式

1. 项目已进入 maintenance mode，不适合作为新工作的长期唯一依赖。
2. Core/AgentChat/ext、Studio、Magentic-One 的版本与成熟度跨度明显。
3. Studio 非生产级，认证与安全需自行补齐。
4. MCP 信任边界明确，只应连接可信 server。
5. 本地代码执行风险高，应优先 Docker sandbox。
6. 动态配置加载和 `FunctionTool` 恢复需要可信边界。
7. GraphFlow experimental。
8. 默认 `SingleThreadedAgentRuntime` 更适合本地/原型工作负载。
9. 持久状态由应用自行负责，没有内建统一 checkpoint service。
