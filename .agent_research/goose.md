# Goose Coding Agent Research

## 1. Source, activity, technical stack, and nature

Primary material: the completed external `coding-agent-research` output and upstream sources.

| Item | Details |
|---|---|
| Repository | https://github.com/aaif-goose/goose |
| Main languages | Rust, TypeScript |
| Stack | Rust workspace, Tokio, Axum, SQLite/sqlx, rmcp, OpenTelemetry, React/Electron UI, Agent Client Protocol |
| Activity evidence | Recent push on 2026-06-22; latest release `v1.38.0` on 2026-06-17 |
| License | Apache-2.0 |
| Ownership signal | Current authoritative repository is `aaif-goose/goose` |

Goose is an MCP-first local agent platform with Rust core runtime, provider abstraction, session persistence, security inspection, extension management, server APIs, desktop UI, recipes, scheduling, and Agent Client Protocol support.

## 2. Entry points and modules

Rust workspace shape:

```text
crates/
  goose/              # core agent, conversation, extensions, context, security
  goose-cli/          # CLI
  goose-server/       # Axum server, API, event bus
  goose-providers/    # LLM provider abstraction and implementations
  goose-mcp/          # MCP support
  goose-sdk/          # SDK
  goose-sdk-types/    # SDK types
  goose-test/         # tests
  goose-test-support/ # test support
ui/
  desktop/            # TypeScript/React/Electron UI
evals/
workflow_recipes/
recipe-scanner/
```

Important entry and coordination paths include `crates/goose-cli/src/main.rs`, `crates/goose-cli/src/cli.rs`, `crates/goose/src/agents/agent.rs`, `crates/goose/src/agents/tool_execution.rs`, `crates/goose/src/agents/extension_manager.rs`, `crates/goose-providers/src/base.rs`, `crates/goose/src/session/session_manager.rs`, `crates/goose-server/src/main.rs`, and `crates/goose-server/src/session_event_bus.rs`.

## 3. Agent loop

The core loop is `Agent::reply()` in `crates/goose/src/agents/agent.rs`.

```text
Agent.reply(user_message, session_config, cancel_token)
  -> load session / conversation
  -> maybe auto compact
  -> prepare tools, system prompt, and extensions
  -> loop until max turns / cancel / final:
       - drain pending steers
       - run UserPromptSubmit hook
       - append user and steer messages
       - stream_response_from_provider(provider, system, messages, tools)
       - yield AgentEvent::Message / Usage / McpNotification
       - collect tool requests
       - tool inspection / permission check
       - approved tools -> dispatch_tool_call()
       - ask-before tools -> ActionRequired message and wait confirmation
       - frontend tools -> FrontendToolRequest
       - tool results -> append tool response
       - maybe summarize tool pairs
       - stop hooks can block turn end
```

Key constants include `DEFAULT_MAX_TURNS = 1000`, `DEFAULT_STOP_HOOK_BLOCK_CAP = 8`, and `DEFAULT_COMPACTION_THRESHOLD = 0.8`. `AgentEvent` includes `Message`, `Usage`, `McpNotification`, and `HistoryReplaced`.

## 4. Planner / executor

Goose composes planning and execution from runtime components rather than one planner class.

| Component | Role |
|---|---|
| Provider stream | Produces text and tool requests |
| `ExtensionManager` | Executes MCP and platform extension tools |
| `ToolInspectionManager` | Performs permission and security analysis |
| `ToolConfirmationRouter` | Routes ask-before tools to user confirmation |
| Context management | Compaction and tool-pair summarization |
| Hooks | Session/tool/shell/read/write lifecycle customization |
| Recipes | Workflow and task templates |
| Scheduler | Scheduled jobs |

Goose mode changes execution behavior. Chat mode can skip tools, while auto/agent mode follows permission policy and may execute approved tools.

## 5. Tool abstraction

Goose is deeply MCP-native. Core tool types are `rmcp::model::Tool`, `CallToolRequestParams`, `CallToolResult`, and server notifications.

Tool flow:

1. Provider returns a tool request.
2. `ToolRequest` is added to the conversation.
3. Permission and security inspection run.
4. Approved requests call `dispatch_tool_call()`.
5. `ExtensionManager` dispatches to an MCP client or platform extension.
6. Tool results or notification streams are written back.
7. Conversation history receives the tool response.

Related paths include `crates/goose/src/agents/tool_execution.rs`, `crates/goose/src/agents/platform_tools.rs`, `crates/goose/src/agents/platform_extensions/*`, `crates/goose/src/mcp_utils.rs`, and `crates/goose/src/tool_monitor.rs`.

## 6. Model / provider adaptation

The provider trait is in `crates/goose-providers/src/base.rs`.

| Method | Role |
|---|---|
| `stream()` | Main streaming interface |
| `complete()` | Collects a stream into a full message |
| `complete_fast()` | Uses a fast model with fallback to normal model |
| `get_model_config()` | Returns model configuration |
| `fetch_supported_models()` | Provider inventory |
| `fetch_recommended_models()` | Canonical registry filtering |
| `manages_own_context()` | Declares provider-managed context |
| `supports_cache_control()` | Cache-control capability |
| `update_mode()` | GooseMode switching |
| `permission_routing()` | Provider-aware permission routing |

The provider layer also tracks a canonical model registry, retry config, OAuth, model information, context limits, cost, and reasoning capabilities.

## 7. Context construction

Main paths:

- `crates/goose/src/context_mgmt/mod.rs`
- `crates/goose/src/prompt_template.rs`
- `crates/goose/src/prompts/*.md`
- `crates/goose/src/conversation/*`

Context capabilities:

| Capability | Behavior |
|---|---|
| Auto compaction | Summarizes when token ratio crosses threshold |
| Manual compact | User-triggered compaction |
| Tool-pair summarization | Summarizes old tool call/result pairs |
| Agent/user visibility | Metadata separates agent-visible and user-visible messages |
| Continuation message | Adds agent-only continuation after compaction |
| Provider-managed context | Skips Goose-side compaction for providers that own context |

`compact_messages()` keeps a recent user message when possible, summarizes with the provider fast model, marks original messages agent-invisible, inserts an agent-only summary, adds continuation instructions, and restores recent user messages if needed.

## 8. File editing and diff

Goose itself does not center on one patch engine. File operations can come from MCP filesystem extensions, platform tools, ACP filesystem operations, frontend tools, or shell commands.

Relevant paths:

| Path | Role |
|---|---|
| `crates/goose/src/acp/fs.rs` | ACP file operations |
| `crates/goose/src/agents/platform_tools.rs` | platform tool definitions |
| `crates/goose/src/agents/tool_execution.rs` | tool execution and result recording |
| `crates/goose/src/tool_inspection/*` | edit and command security inspection |

Goose's emphasis is MCP tool dispatch, permission, confirmation, and session state rather than a single model-specific diff format.

## 9. Command execution, sandbox, and permissions

Permission paths include `crates/goose/src/config/permission.rs`, `crates/goose/src/permission/*`, and `crates/goose/src/agents/tool_execution.rs`.

The permission file is `permission.yaml`. Permission levels are:

| Level | Meaning |
|---|---|
| `AlwaysAllow` | Always allow |
| `AskBefore` | Ask before executing |
| `NeverAllow` | Never allow |

Permission inputs include user permission, smart approve, tool annotations, security inspector findings, prompt-injection/adversary/egress inspection, and user decisions such as `AllowOnce`, `AlwaysAllow`, `DenyOnce`, and `AlwaysDeny`.

Goose usually runs tools with host authority. `Agent` has a `container: Mutex<Option<Container>>`, but the default safety story is still permission, inspection, and confirmation rather than a guaranteed hard sandbox.

## 10. Memory and state persistence

Session persistence lives in `crates/goose/src/session/session_manager.rs`.

Important constants:

- `SESSIONS_FOLDER = "sessions"`
- `DB_NAME = "sessions.db"`
- `CURRENT_SCHEMA_VERSION = 14`

A session stores id, working directory, name, session type, timestamps, extension data, usage, accumulated usage, cost, schedule id, recipe, conversation, provider name, model config, goose mode, project id, and last message snippet.

`SessionManager` supports create/get/list/delete, add message, replace conversation, export/import/copy, truncate conversation, search chat history, update tool request metadata, and automatic session name generation.

## 11. Event stream, logging, and audit

Event and logging capabilities include:

| Component | Behavior |
|---|---|
| `AgentEvent` | agent streaming event |
| `session_event_bus.rs` | server-side session event bus |
| `tracing` | Rust tracing |
| OpenTelemetry | workspace dependencies for observability |
| Usage | provider usage, tokens, and cost |
| Security tracing | records allow/block decisions for security findings |
| Session DB | durable message/history storage |

Security events include fields such as `security.event_type`, `security.action`, `security.finding_id`, `tool.request_id`, and `user.decision`.

## 12. Testing strategy

Tests are distributed across Rust unit tests, `crates/goose-test`, `crates/goose-test-support`, `crates/goose-cli/src/scenario_tests`, `ui/desktop/src/App.test.tsx`, `test_acp_client.py`, and `evals/`.

Key test areas are permission manager, context compaction, provider stream collection, MCP/extension behavior, session import/export, CLI scenarios, and UI behavior.

## 13. Plugins, MCP, and extension model

Goose is MCP-first.

| Extension point | Role |
|---|---|
| MCP extensions | external tool servers |
| Platform extensions | built-in management, scheduler, summon, and similar capabilities |
| Recipes | workflow recipes |
| Hooks | before/after session, tool, shell, read, and write events |
| Frontend tools | frontend-executed tool requests |
| ACP | Agent Client Protocol support |
| Scheduler | scheduled jobs |
| Extension data | session-persisted extension state |

Relevant paths include `crates/goose/src/agents/extension_manager.rs`, `crates/goose/src/agents/platform_extensions/`, `crates/goose/src/recipe.rs`, `crates/goose/src/hooks/`, and `crates/goose/src/acp/`.

## 14. Core source file paths

Recommended paths for architecture review:

- `crates/goose-cli/src/main.rs`
- `crates/goose-cli/src/cli.rs`
- `crates/goose/src/agents/agent.rs`
- `crates/goose/src/agents/tool_execution.rs`
- `crates/goose/src/agents/extension_manager.rs`
- `crates/goose-providers/src/base.rs`
- `crates/goose/src/context_mgmt/mod.rs`
- `crates/goose/src/session/session_manager.rs`
- `crates/goose/src/config/permission.rs`
- `crates/goose-server/src/session_event_bus.rs`

## 15. Lessons for `Marix`

1. Use MCP as a native tool boundary when external tool ecosystems matter.
2. Persist permission configuration in a simple auditable format like `permission.yaml`.
3. Let tool annotations influence permission defaults automatically.
4. Separate permission, adversary, egress, and repetition/security inspections.
5. Preserve agent-visible versus user-visible metadata so compaction does not pollute UI history.
6. Use a SQLite session schema when CLI, desktop, and server surfaces share the same runtime history.
7. Add a provider-managed-context flag for providers that already own compaction or conversation state.

## 16. Risks and anti-patterns

| Risk / anti-pattern | Why it matters |
|---|---|
| External MCP dependency | Tool capability is powerful but failure surface is wide |
| Default non-hard sandbox | Host authority still needs permission and inspection controls |
| Rust + TypeScript stack | Contributor and build complexity are higher |
| Permission UX complexity | AlwaysAllow/AskBefore/NeverAllow plus findings need clear UI |
| Context visibility semantics | Agent-visible/user-visible rules must be maintained precisely |
| Multiple protocols | MCP, ACP, server API, and desktop integration raise test cost |
| Tool sprawl | Adding many extensions without capability boundaries can make audits difficult |
