# Microsoft AutoGen Framework Agent Research

## 1. Sources and activity

| Item | Detail |
|---|---|
| Repository | https://github.com/microsoft/autogen |
| Default branch | `main` |
| Main languages | Python, plus .NET/C#, protobuf/gRPC, Studio Web/API |
| GitHub license | API reports `CC-BY-4.0`; each subpackage should be checked separately |
| Current status | README marks the project as Maintenance Mode and recommends Microsoft Agent Framework for new users |
| Activity | GitHub API: `pushed_at=2026-04-15T11:59:09Z`; latest release `python-v0.7.5` on `2025-09-30` |

Primary research inputs:

- https://github.com/microsoft/autogen
- https://github.com/microsoft/autogen/releases/tag/python-v0.7.5
- `python/packages/autogen-core`
- `python/packages/autogen-agentchat`
- `python/packages/autogen-ext`
- `python/packages/autogen-studio`
- `dotnet/src`
- `protos`

## 2. Technology stack and nature

| Path | Role |
|---|---|
| `python/packages/autogen-core` | Low-level runtime, agent protocol, message passing, tool/model/memory abstractions, telemetry |
| `python/packages/autogen-agentchat` | High-level conversational agent/team API |
| `python/packages/autogen-ext` | Providers, MCP, code executor, memory backend, cache, and gRPC runtime |
| `python/packages/autogen-studio` | AutoGen Studio GUI/API |
| `python/packages/agbench` | Benchmark and evaluation |
| `python/packages/magentic-one-cli` | Magentic-One CLI |
| `dotnet/src` | .NET implementation and providers |
| `protos` | gRPC and CloudEvent protocols |

The Python package line is `autogen-core==0.7.5`, `autogen-agentchat==0.7.5`, and `autogen-ext==0.7.5`, targeting Python `>=3.10`.

AutoGen is protocol-first. It separates core runtime, AgentChat high-level teams, extensions, and Studio. Its design is useful for message-based runtime abstractions, workbench/tool containers, state save/load, and event telemetry, but maintenance mode makes it less suitable as a new long-term dependency.

## 3. Entry points and modules

CLI entries:

- Studio CLI: `autogenstudio = "autogenstudio.cli:run"`
- Magentic-One CLI: `m1 = "magentic_one_cli._m1:main"`

Core modules:

| Module | Path |
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

## 4. Runtime, agent, team, and graph execution

### Core runtime

AutoGen decouples agents from runtime:

- `Agent` handles messages, state save/load, and close.
- `AgentRuntime` handles send/publish, factory registration, subscription, and state save/load.
- `SingleThreadedAgentRuntime` uses an asyncio queue for direct/publish/response envelopes and supports intervention handlers, tracing, and exception recording.

### AssistantAgent loop

Typical flow:

1. New messages enter the model context.
2. Memory is queried and injected into context.
3. Tools are collected from workbenches and handoff definitions.
4. `ChatCompletionClient.create` or `create_stream` is called.
5. Plain text returns a final response.
6. Function calls emit tool request events.
7. The workbench executes tools; multiple tool calls are concurrent by default.
8. Tool execution events are emitted.
9. `max_tool_iterations`, handoff, and `reflect_on_tool_use` decide whether to continue or finish.

### Team and GroupChat

`BaseGroupChat` registers participant containers and a group chat manager, then drives multi-agent conversations through topic subscriptions.

Built-in team/workflow styles include RoundRobin, Selector, Swarm, MagenticOneGroupChat, and GraphFlow.

### GraphFlow

GraphFlow is experimental directed-graph workflow support. It handles DAGs, sequential nodes, parallel fan-out, joins, conditional edges, and loops. Loops must have an exit condition, termination condition, or maximum turns.

## 5. Tools and model adapters

Tool core paths:

- `autogen-core/src/autogen_core/tools/_base.py`
- `autogen-core/src/autogen_core/tools/_workbench.py`
- `autogen-ext/src/autogen_ext/tools/mcp/_workbench.py`

| Layer | Role |
|---|---|
| `ToolSchema` / `Tool` protocol | name, description, JSON schema, `run_json`, state save/load |
| `BaseTool` / `BaseStreamTool` | Pydantic args/return-type based tools |
| `Workbench` | Lifecycle container for dynamic tool sets; supports list/call/start/stop/reset/save/load |
| `McpWorkbench` | Wraps MCP server tools, resources, prompts, and sampling host |

`FunctionTool` wraps Python functions as tools, but restoration from configuration can involve source/import dynamic execution and therefore requires trusted configuration.

The model abstraction is `ChatCompletionClient` with `create`, `create_stream`, `close`, `usage`, `count_tokens`, `remaining_tokens`, and `model_info`. `ModelInfo` declares vision, function-calling, JSON output, model family, structured output, and multiple-system-message support.

`autogen-ext` providers cover OpenAI/Azure OpenAI, Anthropic, Ollama, Azure AI, Gemini, Semantic Kernel, llama.cpp, and more. The .NET side includes OpenAI, Anthropic, Gemini, Ollama, Mistral, SemanticKernel, and related adapters.

## 6. Memory, state, checkpoint, and storage

AutoGen does not provide one unified checkpoint service. It composes layered `save_state/load_state` mechanisms:

| Type | Mechanism |
|---|---|
| Model context | `ChatCompletionContext` saves LLM message history |
| Memory | `Memory.query/add/update_context/clear/close` |
| Agent state | `AssistantAgentState` saves `llm_context` |
| Team state | `TeamState` saves each agent/manager state |
| Manager state | `message_thread`, `current_turn` |
| Graph/Magentic-One state | active nodes, task, facts, plan, round, stall |
| Storage backend | Studio SQLModel/SQLAlchemy/Alembic; ext memory/cache includes ChromaDB, mem0, Redis, diskcache |

Storage summary:

| Scenario | Implementation |
|---|---|
| Agent/team resume | `save_state/load_state` returns JSON-like mappings; applications persist them |
| Studio | SQLModel / SQLAlchemy / Alembic |
| Memory backend | ListMemory, ChromaDB, mem0, Redis, and others |
| Cache | InMemory, diskcache, redis |
| Code execution | Local/docker executor workdir and container lifecycle |

## 7. Workflow orchestration

AutoGen workflow is a set of patterns rather than a single DSL:

1. `AgentTool`: wrap an agent as a tool.
2. GroupChat: manager-driven multi-agent conversation.
3. SelectorGroupChat: model/rule chooses the next speaker.
4. Swarm: handoff-driven agent transfer.
5. GraphFlow: directed graph, conditional edges, parallel execution, join, loop.
6. Magentic-One: ledger-based planning, progress tracking, and re-planning.
7. Studio: GUI configuration and execution for teams/workflows.

## 8. Human-in-the-loop

The core HITL component is `UserProxyAgent`.

Capabilities:

- Receives user input through `input_func`.
- Emits `UserInputRequestedEvent` before input.
- Supports `CancellationToken`.
- Recommends stopping the team, saving state, waiting for the user, and resuming later for slow human responses instead of blocking a team indefinitely.

Related termination types include `HandoffTermination` and `SourceMatchTermination`.

## 9. Events, logging, and observability

AgentChat events include:

- `ToolCallRequestEvent`
- `ToolCallExecutionEvent`
- `MemoryQueryEvent`
- `ModelClientStreamingChunkEvent`
- `UserInputRequestedEvent`
- `SelectSpeakerEvent`
- `ThoughtEvent`

Core logging events include `LLMCallEvent`, `ToolCallEvent`, `MessageEvent`, `MessageDroppedEvent`, and `MessageHandlerExceptionEvent`.

Telemetry uses OpenTelemetry spans and structured LLM/tool/message events. Helpers include `trace_tool_span`, `trace_create_agent_span`, and `trace_invoke_agent_span`.

## 10. Tests and validation

The Python workspace uses uv, ruff, strict mypy, strict pyright, pytest, pytest-asyncio, pytest-cov, pytest-xdist, and gRPC proto generation/tests.

CI includes format, lint, mypy, docs-mypy, pyright, tests, gRPC tests, Windows/ext tests, and coverage artifacts.

## 11. Core source paths

| Concern | Paths |
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

## 12. Reusable lessons for Marix

1. Keep Core, AgentChat, Extensions, and Studio as separate layers.
2. Prefer protocols for Agent, Runtime, Tool, ModelClient, and Memory.
3. Cover tool, memory, streaming, user input, and speaker selection in the event model.
4. Use the Workbench pattern for MCP, multi-tool containers, and sandbox lifecycles.
5. Layered save/load supports long-running pause/resume without one monolithic checkpoint system.
6. Support both conversational orchestration and graph orchestration.
7. For HITL, stop/save/resume is safer than blocking long-running teams.
8. `ModelInfo` style capability declaration makes provider behavior explicit.
9. Strict CI is a good baseline for foundational runtime code.

## 13. Risks and anti-patterns

1. The project is in maintenance mode, so it should not be the only long-term dependency for new work.
2. Core/AgentChat/ext, Studio, and Magentic-One show version and maturity gaps.
3. Studio is not production-grade; authentication and security need separate work.
4. MCP trust boundaries are explicit; connect only trusted servers.
5. Local code execution is high risk and should prefer Docker sandboxing.
6. Dynamic config loading and `FunctionTool` restoration require a trusted boundary.
7. GraphFlow is experimental.
8. The default `SingleThreadedAgentRuntime` is better suited to local/prototype workloads.
9. Application code owns durable state persistence; there is no built-in unified checkpoint service.
