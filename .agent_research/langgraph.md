# LangGraph Framework Agent Research

## 1. Sources and activity

| Item | Detail |
|---|---|
| Repository | https://github.com/langchain-ai/langgraph |
| Default branch | `main` |
| Main language | Python |
| License | MIT |
| Nature | Low-level orchestration runtime for long-running, stateful agents and workflows |
| Activity | GitHub API: `pushed_at=2026-06-21T01:21:56Z`; latest release `1.2.6` on `2026-06-18` |

Primary research inputs:

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

## 2. Technology stack and nature

| Package/directory | Role |
|---|---|
| `libs/langgraph` | Core Python runtime: Graph API, Pregel runtime, streaming, and types |
| `libs/prebuilt` | Prebuilt components such as `ToolNode` and `create_react_agent` |
| `libs/checkpoint` | Checkpoint, store, and cache interfaces plus in-memory implementations |
| `libs/checkpoint-postgres` | Postgres checkpointer/store |
| `libs/checkpoint-sqlite` | SQLite checkpointer/store/cache |
| `libs/sdk-py` | Assistants, threads, runs, cron, and store API SDK |
| `libs/cli` | `langgraph` CLI for local server, build, and deployment configuration |
| `libs/sdk-js` | JS/TS migration notes; active implementation moved to `langgraphjs` |

Core dependencies include LangChain Core, Pydantic, httpx, websockets, psycopg, aiosqlite, sqlite-vec, orjson, and ormsgpack. LangGraph is intentionally a runtime substrate rather than a complete high-level agent product: it defines state, graph execution, durability, checkpointing, streaming, and integration seams.

## 3. Entry points and modules

Main user-facing entries:

- `langgraph.graph.StateGraph`
- `START`
- `END`
- `MessagesState`
- `add_messages`

Core paths:

- `libs/langgraph/langgraph/graph/__init__.py`
- `libs/langgraph/langgraph/graph/state.py`

`StateGraph` is the builder. After `compile()`, it produces `CompiledStateGraph`, which inherits from the Pregel runtime.

## 4. Runtime, agent, team, and graph execution

LangGraph is centered on graph runtime rather than a bundled agent/team product.

Core model:

- Nodes are functions or Runnables.
- State can be TypedDict, dataclass, or Pydantic.
- Each state key can define a reducer.
- Edges and conditional edges define control flow.
- Compiled graphs execute with a Pregel-style model.

Pregel execution model:

1. Plan: choose active actor/node set.
2. Execute: run all nodes for the current step, often in parallel.
3. Update: write to channels; writes become visible in the next step.
4. Repeat until no active node remains or termination is reached.

Core paths:

- `libs/langgraph/langgraph/pregel/main.py`
- `libs/langgraph/langgraph/pregel/_runner.py`
- `libs/langgraph/langgraph/pregel/_loop.py`

Multi-agent/team patterns are usually built through subgraphs as nodes, `Command.PARENT` to jump from a subgraph to its parent, `Send` for fan-out/map-reduce, and `RemoteGraph` to treat a remote assistant/graph as a local node.

## 5. Tools and model adapters

Core path: `libs/prebuilt/langgraph/prebuilt/tool_node.py`.

`ToolNode` supports `langchain_core.tools.BaseTool`, plain functions converted to tools, message-list/state-dict/direct-tool-call inputs, synchronous executor map, asynchronous `asyncio.gather`, `InjectedState`, `InjectedStore`, `ToolRuntime`, `handle_tool_errors`, and `wrap_tool_call` / `awrap_tool_call`.

The key design is that state, store, and runtime can be injected into tools without exposing those internals to the LLM tool schema.

LangGraph does not maintain its own model provider layer. It delegates to LangChain primitives such as `BaseChatModel`, `LanguageModelLike`, `RunnableBinding`, `init_chat_model("provider:model")`, `.bind_tools(...)`, and `.with_structured_output(...)`.

`create_react_agent` supports static models, string model identifiers, and dynamic model functions `(state, runtime) -> model`, but prebuilt agent APIs have migration pressure toward LangChain agents, so long-term dependencies should be chosen carefully.

## 6. Memory, state, checkpoint, and storage

| Layer | Description |
|---|---|
| State | TypedDict/dataclass/Pydantic; each key can define a reducer |
| Short-term memory | Thread-scoped graph state persisted through a checkpointer |
| Checkpointer | `BaseCheckpointSaver` with get/put/list/put_writes/delete/prune/copy_thread |
| InMemorySaver | Debug/test implementation |
| Long-term memory | `BaseStore`, cross-thread namespace key-value store with semantic search and TTL |
| Cache | `BaseCache` plus memory, redis, and sqlite implementations |

Core paths:

- `libs/checkpoint/langgraph/checkpoint/base/__init__.py`
- `libs/checkpoint/langgraph/checkpoint/memory/__init__.py`
- `libs/checkpoint/langgraph/store/base/__init__.py`

Checkpoint storage includes `InMemorySaver`, `PostgresSaver` / `AsyncPostgresSaver`, and `SqliteSaver` / `AsyncSqliteSaver`. Store storage includes `InMemoryStore`, Postgres store, and SQLite store. Postgres checkpointer tables include `checkpoints`, `checkpoint_blobs`, `checkpoint_writes`, and `checkpoint_migrations`. SQLite store supports TTL and vector tables through `sqlite-vec`.

## 7. Workflow orchestration

LangGraph offers two APIs:

1. Graph API: `StateGraph`, nodes, edges, conditional edges, and subgraphs.
2. Functional API: `@entrypoint` / `@task`, wrapping normal Python workflows as durable workflows.

Runtime controls include `RetryPolicy`, `TimeoutPolicy`, `CachePolicy`, and durability modes `sync`, `async`, and `exit`.

Agent Server deployments usually include an API server, queue worker, Postgres, Redis, stream/cancel signaling, and SDK clients.

## 8. Human-in-the-loop

The core mechanism is `interrupt(value)` plus `Command(resume=...)`.

Characteristics:

- `interrupt` pauses the graph.
- A checkpointer and `thread_id` are required.
- On resume, the node re-executes from the beginning until it reaches the interrupt point.
- Multiple interrupt IDs can be resumed through a mapping.
- Stream events expose interrupts.

Core path: `libs/langgraph/langgraph/types.py`, plus the LangGraph interrupt documentation.

## 9. Events, logging, and observability

Stream modes include:

- `values`
- `updates`
- `messages`
- `custom`
- `checkpoints`
- `tasks`
- `debug`

v3 event streaming provides typed projections for messages, values, subgraphs, interrupts, and extensions.

Core paths:

- `libs/langgraph/langgraph/stream/_mux.py`
- `libs/langgraph/langgraph/stream/transformers.py`
- `libs/langgraph/langgraph/callbacks.py`

LangSmith is the official recommended solution for tracing, debugging, evaluation, deployment, and observability.

## 10. Tests and validation

Test paths:

- `libs/langgraph/tests`
- `libs/prebuilt/tests`
- `libs/checkpoint/tests`
- `libs/checkpoint-postgres/tests`
- `libs/checkpoint-sqlite/tests`

CI/tooling covers Python 3.10-3.14, ruff, ty, codespell, syrupy snapshots, pytest-xdist, CLI integration tests, schema drift checks, and strict msgpack tests.

## 11. Core source paths

| Concern | Paths |
|---|---|
| Graph builder | `libs/langgraph/langgraph/graph/__init__.py`, `graph/state.py` |
| Pregel runtime | `pregel/main.py`, `pregel/_runner.py`, `pregel/_loop.py` |
| Tool execution | `libs/prebuilt/langgraph/prebuilt/tool_node.py` |
| Checkpoint base | `libs/checkpoint/langgraph/checkpoint/base/__init__.py` |
| Memory checkpointer | `libs/checkpoint/langgraph/checkpoint/memory/__init__.py` |
| Store base | `libs/checkpoint/langgraph/store/base/__init__.py` |
| Streaming | `libs/langgraph/langgraph/stream/_mux.py`, `stream/transformers.py`, `callbacks.py` |
| SDK/CLI | `libs/sdk-py/*`, `libs/cli/*` |

## 12. Reusable lessons for {{proj}}

1. Separate builder and runtime so declaration and execution evolve independently.
2. Use channel/reducer state to support parallelism and recovery.
3. Separate checkpoint and store: short-term state and long-term memory should not be conflated.
4. Build event-stream projection layers so UI, SDK, and observability reuse the same runtime feed.
5. Inject tool runtime/state/store without exposing internal parameters to model-visible schemas.
6. Offer explicit durability modes to trade performance and reliability consciously.
7. Use remote graph composition for distributed agent systems.

## 13. Risks and anti-patterns

1. The low-level framework has a high complexity floor.
2. `create_react_agent` prebuilt APIs have migration risk.
3. Full production experience can depend on LangSmith/Agent Server.
4. v3 event streaming may continue to evolve.
5. Checkpoint pruning/copy requires care with DeltaChannel scenarios.
6. Private state streaming is not automatically redacted.
7. Model/tool permissions and sandboxing are not built into the core library.
