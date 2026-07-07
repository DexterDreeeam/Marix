# CrewAI Framework Agent Research

## 1. Sources and activity

| Item | Detail |
|---|---|
| Repository | https://github.com/crewAIInc/crewAI |
| Default branch | `main` |
| Main language | Python |
| License | MIT |
| Nature | Multi-agent and role-playing autonomous agent orchestration framework |
| Activity | GitHub API: `pushed_at=2026-06-22T16:26:03Z`; latest release `1.14.7` on `2026-06-11` |

Primary research inputs:

- https://github.com/crewAIInc/crewAI
- https://github.com/crewAIInc/crewAI/releases/tag/1.14.7
- `lib/crewai/src/crewai/__init__.py`
- `lib/crewai/src/crewai/agent/core.py`
- `lib/crewai/src/crewai/crew.py`
- `lib/crewai/src/crewai/flow/runtime/__init__.py`
- `lib/crewai/src/crewai/tools/*`
- `lib/crewai/src/crewai/state/*`
- `.github/workflows/*`

## 2. Technology stack and nature

CrewAI is a Python monorepo with a clear separation between runtime, tools, CLI, and supporting packages.

| Path | Role |
|---|---|
| `lib/crewai` | Core agent, crew, and flow runtime |
| `lib/crewai-tools` | Tool ecosystem and MCP integration |
| `lib/cli` | `crewai` CLI |
| `lib/crewai-core` | Low-level CLI, token, and locking helpers |
| `lib/crewai-files` | File processing |
| `lib/devtools` | Developer tooling |

Important technologies include Python `>=3.10,<3.14`, Pydantic v2, Click CLI, uv workspace, OpenTelemetry tracing, MCP SDK, LanceDB, Qdrant Edge, ChromaDB, pytest, ruff, mypy, bandit, pip-audit, and CodeQL.

Architecturally, CrewAI combines a high-level team abstraction (`Crew`) for role/task collaboration with a lower-level event-driven workflow runtime (`Flow`). This makes it useful as both an agent team framework and a deterministic workflow layer.

## 3. Entry points and modules

The Python API exported from `crewai.__init__` includes `Agent`, `Crew`, `Task`, `Flow`, `LLM`, `BaseLLM`, `Knowledge`, `Memory`, `CheckpointConfig`, `Process`, `CrewOutput`, and `TaskOutput`.

The CLI entry point is:

- `crewai = "crewai_cli.cli:crewai"`

Common CLI commands include:

- `crewai create crew|flow`
- `crewai run`
- `crewai train`
- `crewai test`
- `crewai replay`
- `crewai memory`
- `crewai deploy`
- `crewai tool`

## 4. Runtime, agent, team, and graph execution

### Agent

Core path: `lib/crewai/src/crewai/agent/core.py`.

`Agent` covers role, goal, backstory, LLM, function-calling LLM, tools, memory, knowledge, planning, reasoning, guardrails, skills, MCP, A2A, and executor class. A typical execution prepares the task prompt, injects knowledge and memory context, resolves tools, emits agent/LLM/tool events, calls the executor, handles context length, retries, failures, saves last messages, and cleans up MCP clients.

### Crew

Core path: `lib/crewai/src/crewai/crew.py`.

`Crew` supports `Process.sequential` and `Process.hierarchical`. Sequential mode executes tasks in order. Hierarchical mode creates or uses a manager agent and delegation tools to coordinate sub-tasks.

### Flow

Core paths:

- `lib/crewai/src/crewai/flow/runtime/__init__.py`
- `lib/crewai/src/crewai/flow/dsl/*`
- `lib/crewai/src/crewai/flow/flow_definition.py`

`Flow` is an event-driven graph runtime with `@start`, `@listen`, `@router`, `or_`, `and_`, `@human_feedback`, dict/Pydantic/JSON-schema state, parallel starts/listeners, checkpoint restore, and pending human-feedback resume.

## 5. Tools and model adapters

Core tool paths:

- `lib/crewai/src/crewai/tools/base_tool.py`
- `lib/crewai/src/crewai/tools/structured_tool.py`
- `lib/crewai/src/crewai/tools/tool_usage.py`

| Module | Role |
|---|---|
| `BaseTool` | Pydantic tool model, args schema, result schema, cache, and usage limit |
| `CrewStructuredTool` | Wraps callables as structured tools with sync/async execution and validation |
| `ToolUsage` | Tool selection, parsing, execution, cache, retries, events, and raw result tracking |

MCP integration lives at `lib/crewai/src/crewai/mcp/tool_resolver.py` and supports stdio, HTTP, SSE, HTTPS MCP URLs, AMP refs, tool filters, schema cache, timeout, and retry.

Model/provider paths:

- `lib/crewai/src/crewai/llm.py`
- `lib/crewai/src/crewai/llms/base_llm.py`
- `lib/crewai/src/crewai/llms/providers/*`

Native providers include OpenAI, Anthropic/Claude, Azure OpenAI, Gemini/Google, Bedrock/AWS, Snowflake, OpenAI-compatible, OpenRouter, DeepSeek, Ollama, Hosted vLLM, Cerebras, and DashScope. If no native provider matches, CrewAI lazy-loads LiteLLM as a fallback. `BaseLLM` emits call started/completed/failed, stream chunk, thinking, and usage tracking events.

## 6. Memory, state, checkpoint, and storage

### Unified memory

Core paths:

- `lib/crewai/src/crewai/memory/unified_memory.py`
- `lib/crewai/src/crewai/memory/storage/*`

Unified memory defaults to LanceDB and can use Qdrant Edge. It supports background writes, drains writes before `recall()` as a read barrier, uses vector search for shallow recall, and delegates deep recall to `RecallFlow`.

### Knowledge and RAG

Core paths:

- `lib/crewai/src/crewai/knowledge/knowledge.py`
- `lib/crewai/src/crewai/knowledge/source/*`
- `lib/crewai/src/crewai/rag/*`

Sources include strings, docling, CSV, Excel, JSON, PDF, text files, and related document inputs.

### Checkpoint and runtime state

Core paths:

- `lib/crewai/src/crewai/state/checkpoint_config.py`
- `lib/crewai/src/crewai/state/runtime.py`
- `lib/crewai/src/crewai/state/event_record.py`
- `lib/crewai/src/crewai/state/provider/json_provider.py`
- `lib/crewai/src/crewai/state/provider/sqlite_provider.py`

The default checkpoint location is `./.checkpoints`, default provider is JSON, and the default event is `task_completed`. SQLite is also available. `on_events=["*"]` records every event. `EventRecord` forms an event graph with parent/child, trigger, previous/next, started, and completed edges.

Storage summary: memory uses LanceDB/Qdrant Edge; knowledge/RAG can use ChromaDB and Qdrant; checkpoint uses JSON/SQLite; flow persistence uses SQLite tables such as `flow_states` and `pending_feedback`, plus WAL and lock store.

## 7. Workflow orchestration

CrewAI has two orchestration layers:

1. `Crew`: agent team orchestration over tasks, either sequential or hierarchical.
2. `Flow`: event-driven graph workflow orchestration.

`FlowDefinition` supports serializable YAML/JSON-style workflow definitions with action types such as `code`, `tool`, `crew`, `agent`, `expression`, `script`, and `each`. The `script` action can execute Python and requires `CREWAI_ALLOW_FLOW_SCRIPT_EXECUTION`; source notes state it is not sandboxed and must not be used for untrusted definitions.

## 8. Human-in-the-loop

### Task-level HITL

When `Task.human_input=True`, the executor sends the final answer into a human feedback loop. The default provider is an interactive console loop: empty feedback accepts the answer, and non-empty feedback continues the correction cycle.

Core paths:

- `lib/crewai/src/crewai/task.py`
- `lib/crewai/src/crewai/core/providers/human_input.py`

### Flow-level HITL

`@human_feedback` supports synchronous feedback, asynchronous pending feedback, `HumanFeedbackPending`, persisted pending context, `resume()` / `resume_async()`, LLM folding of feedback into an outcome for routing, and `learn=True` memory writes.

Core paths:

- `lib/crewai/src/crewai/flow/human_feedback.py`
- `lib/crewai/src/crewai/flow/async_feedback/types.py`

## 9. Events, logging, and observability

Core paths:

- `lib/crewai/src/crewai/events/event_bus.py`
- `lib/crewai/src/crewai/events/event_listener.py`
- `lib/crewai/src/crewai/events/types/*`
- `lib/crewai/src/crewai/events/listeners/tracing/*`

`EventBus` is a singleton. Synchronous handlers run in a thread pool, asynchronous handlers run on a background event loop, dependency-aware handler execution is supported, `emit()` sets parent/previous/trigger metadata, events are written into `RuntimeState`, `replay()` can replay events, and LLM stream chunks are handled synchronously to preserve order. Tracing uses OpenTelemetry.

## 10. Tests and validation

Root pytest configuration covers:

- `lib/crewai/tests`
- `lib/crewai-tools/tests`
- `lib/crewai-files/tests`
- `lib/cli/tests`
- `lib/crewai-core/tests`

Test traits include `--block-network`, pytest-xdist, 60-second timeout, importlib mode, temporary `CREWAI_STORAGE_DIR`, `CREWAI_TESTING=true`, cleanup of event handlers/runtime state, and VCR filtering of sensitive headers. CI includes tests, linter, type-checker, vulnerability scan, and CodeQL.

## 11. Core source paths

| Concern | Paths |
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

## 12. Reusable lessons for Marix

1. Keep team orchestration and deterministic graph workflow as separate but composable layers.
2. Treat events as state: EventBus writes into RuntimeState/EventRecord.
3. Provide rich extension seams for tools, MCP, skills, hooks, A2A, project decorators, and providers.
4. Persist pending HITL feedback so runs can resume safely.
5. Combine native providers with LiteLLM fallback to balance first-class quality and coverage.
6. Use memory backend protocols and factories to avoid locking the runtime to one vector store.
7. Pair CLI scaffolding with declarative configuration for reusable project templates.
8. Default tests to blocked network and isolated global state for deterministic agent runtime validation.

## 13. Risks and anti-patterns

1. The API surface is large and contains compatibility/deprecated paths.
2. `FlowDefinition` `script` action can execute untrusted code if enabled incorrectly.
3. Singleton event bus, global hooks, and contextvars stress concurrency and test isolation.
4. Dependencies are heavy.
5. Task-level HITL is CLI-oriented by default and needs a custom provider for production/Web usage.
6. Some platform abilities are tied to CrewAI AMP/deploy boundaries and need OSS verification.
7. Documentation can lag source code; source paths should remain the final authority.
