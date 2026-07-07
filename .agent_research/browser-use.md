# browser-use External Source Research

## 1. Source and activity

- Official repository: <https://github.com/browser-use/browser-use>
- Website: <https://browser-use.com>
- Documentation: <https://docs.browser-use.com>
- License: MIT.
- Default branch: `main`.
- Repository metadata from the research material:
  - `created_at`: 2024-10-31
  - `pushed_at`: 2026-06-20
  - `updated_at`: 2026-06-22
  - topics include `ai-agents`, `browser-automation`, `llm`, `playwright`, and `python`
- Recent commit examples:
  - 2026-06-20 `add qa skill (#5074)`
  - 2026-06-20 `qa skill: close localhost tunnel in teardown for v2 path too`
  - 2026-06-20 `qa skill: agent self-installs browser-harness`

## 2. Technical stack and project nature

browser-use is a browser automation framework for AI agents. It offers a Python Agent API, CLI, MCP server, cloud/sandbox capabilities, and a Rust-backed beta agent wrapper introduced in the 0.13 line.

Key `pyproject.toml` facts:

| Item | Value |
|---|---|
| Python | `>=3.11,<4.0` |
| Version | `0.13.2` |
| Core dependencies | Pydantic, aiohttp, anyio, httpx, rich, click |
| Browser/CDP | `cdp-use` |
| LLM providers | OpenAI, Anthropic, Gemini, Groq, Ollama, BrowserUse, and others |
| MCP | `mcp` |
| Event bus | `bubus` |
| Optional native core | `browser-use-core==0.13.2` |
| CLI scripts | `browser-use`, `browseruse`, `bu`, `browser`, `browser-use-tui` |

## 3. Entrypoints and modules

Primary entrypoints:

- `browser_use/__init__.py`
  - Configures logging.
  - Lazily imports heavy modules such as `Agent`, `BrowserSession`, `BrowserProfile`, `Tools`, and chat models.
- `browser_use/agent/service.py`
  - Legacy Python `Agent` implementation.
- `browser_use/beta/service.py`
  - Rust-backed beta `Agent` wrapper.
- `browser_use/browser/session.py`
  - CDP browser session and watchdog orchestration.
- `browser_use/tools/service.py`
  - Built-in browser actions.
- `browser_use/mcp/server.py`
  - MCP server.
- `browser_use/sandbox/sandbox.py`
  - Cloud sandbox decorator.

Core module map:

| Module | Path | Role |
|---|---|---|
| Legacy Agent | `browser_use/agent/service.py` | Python agent loop |
| Agent schema/history | `browser_use/agent/views.py` | ActionResult, AgentOutput, AgentHistoryList |
| Message manager | `browser_use/agent/message_manager/service.py` | Prompt, browser state, history, compaction |
| Browser session | `browser_use/browser/session.py` | CDP connection, event bus, watchdogs |
| Browser profile | `browser_use/browser/profile.py` | Domain, security, download, profile configuration |
| Watchdogs | `browser_use/browser/watchdogs/*` | Security, permissions, downloads, DOM, screenshot, captcha |
| Tools | `browser_use/tools/service.py` | navigate/click/type/extract/upload/tab actions |
| Tool registry | `browser_use/tools/registry/service.py` | action decorator and dynamic Pydantic action model |
| MCP server | `browser_use/mcp/server.py` | Browser control over MCP |
| Sandbox | `browser_use/sandbox/sandbox.py` | Cloud execution decorator |
| Beta Rust wrapper | `browser_use/beta/service.py` | stdio JSON-RPC bridge to Rust terminal core |

## 4. Agent loop

### Legacy Python Agent

`Agent.step()` in `browser_use/agent/service.py` follows this flow:

1. Wait for captcha solving when needed.
2. `_prepare_context`
   - Calls `browser_session.get_browser_state_summary(include_screenshot=True)`.
   - Updates the page-specific action model.
   - Lets MessageManager build state messages.
   - Triggers message compaction.
   - Injects budget warnings, replanning, exploration, and loop-detection nudges.
3. `_get_next_action`
   - Calls `llm.ainvoke(... output_format=self.AgentOutput, session_id=...)`.
   - Supports timeout, fallback LLM, and conversation save callback.
4. `_execute_actions`
   - Calls `multi_act(...)` to run a sequence of actions.
5. `_post_process`
   - Handles downloads, plan state, loop detector, failure count, and final result.
6. `_finalize`
   - Builds `AgentHistory`, saves file-system state, emits cloud events, and increments the step counter.

`run(max_steps=500)` starts the browser, registers skills, executes initial actions, then loops through `step()` until done, max failures, pause/stop, max steps, or exception.

`multi_act()` contains important safety guards:

- `done` must be the only action.
- Multiple actions execute sequentially.
- After each action, the agent checks whether the page changed.
- If URL or focus target changed, remaining actions are stopped to avoid wrong clicks on a new page.
- An action marked as `terminates_sequence` truncates the sequence.

### Beta Rust-backed Agent

`browser_use/beta/service.py` defines a beta Agent compatible with the Python Agent API, but delegates execution to `browser-use-terminal sdk-server`:

- `RustSdkClient` starts a child process.
- Communication uses line-delimited JSON-RPC over stdio.
- `runtime.ping` negotiates the SDK protocol version.
- `agent.run_task` / `agent.run` starts work.
- `agent.event` and `agent.projected_event` notifications are collected.
- Rust terminal events are reconstructed as Python `AgentHistoryList`.

Important `_sdk_run_params()` fields:

- `task`
- `cwd`
- `llm`
- `max_steps`
- `browser_mode`
- `browser`
- `calculate_cost`
- `use_vision`
- `max_actions_per_step`
- `output_schema`

`_run_env()` converts browser/profile/permission/download/storage/domain configuration into environment variables such as:

- `BU_CDP_URL`
- `BU_CDP_HEADERS`
- `BU_BROWSER_ALLOWED_DOMAINS`
- `BU_BROWSER_PROHIBITED_DOMAINS`
- `BU_BROWSER_PERMISSIONS`
- `BU_BROWSER_DOWNLOADS_PATH`
- `BU_BROWSER_STORAGE_STATE`
- `BU_MANAGED_BROWSER_ARGS`

## 5. Tool protocol and model adaptation

`browser_use/tools/registry/service.py` uses decorators to register actions:

- `@registry.action(...)`
- Each action binds a Pydantic parameter model.
- Available actions can be filtered by current page URL.
- The registry dynamically builds an ActionModel union for structured LLM output.
- `execute_action()` injects special dependencies automatically:
  - `browser_session`
  - `file_system`
  - `page_extraction_llm`
  - `sensitive_data`
  - `available_file_paths`
  - `extraction_schema`

Built-in tools include:

- search, navigate, go back, wait
- click element and click coordinate
- input text
- upload file
- switch or close tab
- extract content
- save, download, and file-related actions

Model adaptation:

- The legacy agent calls `llm.ainvoke` and expects structured `AgentOutput`.
- Chat models cover OpenAI, Anthropic, Google, Groq, Ollama, BrowserUse Cloud, and others.
- `fallback_llm` is supported.
- Structured extraction schemas and output model schemas are supported.

## 6. Context, state, and memory

Context construction is owned by MessageManager:

| Context | Description |
|---|---|
| SystemPrompt | Task, browser-use instructions, and tool-use constraints |
| Browser state | DOM summary, URL, tabs, screenshot |
| Recent events | Recent browser/action events |
| Agent history | Previous actions, results, errors |
| File system | Available files, downloads, extracted content |
| Plan | Current plan and replan state |
| Sensitive data | Domain-scoped secret substitution |
| Compaction | LLM summary of older messages |

`ActionResult` supports fields such as `is_done`, `success`, `error`, `attachments`, `long_term_memory`, `extracted_content`, `include_extracted_content_only_once`, and `include_in_memory`.

`AgentHistoryList` supports `final_result`, `is_done`, `is_successful`, `has_errors`, `extracted_content`, `urls`, `screenshot_paths`, and `usage`.

## 7. Permissions, sandbox, and security

browser-use focuses on browser boundaries and file boundaries.

### BrowserProfile

`browser_use/browser/profile.py` supports:

- `allowed_domains`
- `prohibited_domains`
- `block_ip_addresses`
- `headless`
- `user_data_dir`
- `storage_state`
- `proxy`
- `permissions`
- `downloads_path`
- `captcha_solver`
- cloud browser
- extensions

### Security watchdog

`browser_use/browser/watchdogs/security_watchdog.py`:

- Checks whether navigation URLs are allowed before navigation.
- Redirects disallowed navigations to `about:blank`.
- Closes newly opened tabs if they are disallowed.
- Supports allowed/prohibited domain patterns.
- Can block IP address access.

### Permissions watchdog

`browser_use/browser/watchdogs/permissions_watchdog.py`:

- Uses CDP `Browser.grantPermissions` after browser connection.
- Treats permission grant failures as non-fatal.

### File upload safety

Upload logic in `tools/service.py`:

- Only allows files from `available_file_paths`, downloaded files, or files managed by FileSystem.
- Uses `realpath` checks to prevent escaping the FileSystem directory.

### Cloud sandbox

`browser_use/sandbox/sandbox.py` provides `@sandbox(...)`:

- Requires the first function parameter to be `browser`.
- Extracts function source, required imports, arguments, and closure variables.
- Serializes arguments with `cloudpickle`.
- Encodes execution code as base64 and POSTs it to `https://sandbox.api.browser-use.com/sandbox-stream`.
- Receives SSE events such as `browser_created`, `instance_ready`, `log`, `result`, and `error`.
- Supports live browser URL, cloud profile, proxy country, timeout, and environment variables.

The main risk is that the sandbox is an external cloud service. Code and parameters are sent to a remote execution environment when this feature is used; the local legacy agent is not equivalent to an isolated sandbox.

## 8. Events, logs, and observability

`browser_use/browser/events.py` defines typed bubus events, including:

- NavigateToUrlEvent
- ClickElementEvent
- TypeTextEvent
- ScrollEvent
- ScreenshotEvent
- BrowserStateRequestEvent
- BrowserStart/Stop/Launch/Kill
- BrowserConnected/Stopped
- TabCreated/Closed
- NavigationStarted/Complete
- Download events
- Dialog events
- Captcha solver events

Observability paths:

- `observability.py` wraps Laminar `observe` and `observe_debug`.
- The legacy agent emits ProductTelemetry `AgentTelemetryEvent`.
- The beta agent reconstructs LLM/tool spans, usage, and cost from terminal events.
- The sandbox uses SSE event streams for runtime logs, result, and error.

## 9. MCP

`browser_use/mcp/server.py` exposes browser-use as an MCP server with tools such as:

- `browser_navigate`
- `browser_click`
- `browser_type`
- `browser_get_state`
- `browser_extract_content`
- `browser_get_html`
- `browser_screenshot`
- `browser_scroll`
- `browser_go_back`
- tab/session tools
- `retry_with_browser_use_agent`

In MCP mode, logging goes to stderr so JSON-RPC stdout is not polluted.

## 10. Testing and validation

The test suite under `tests/ci/` covers:

- browser, CDP, navigation, screenshot, tabs, profile, proxy
- tool registry parameter injection and validation
- security:
  - domain filtering
  - IP blocking
  - upload containment
  - sensitive data
  - MCP allowed domains
  - download filename sanitization
- models:
  - OpenAI, Anthropic, Google, Azure, BrowserUse
- agent:
  - planning
  - loop detection
  - fallback LLM
  - action timeout
  - budget warning
  - beta agent
- CLI, cloud, setup, doctor, tunnel
- extraction, markdown, file system, structured output

GitHub workflows include:

- `test.yaml`
- `lint.yml`
- `eval-on-pr.yml`
- `cloud_evals.yml`
- `docker.yml`
- `publish.yml`

## 11. Core paths

Recommended paths for deeper follow-up:

- `browser_use/__init__.py`
- `browser_use/agent/service.py`
- `browser_use/agent/views.py`
- `browser_use/agent/message_manager/service.py`
- `browser_use/browser/profile.py`
- `browser_use/browser/session.py`
- `browser_use/browser/events.py`
- `browser_use/browser/watchdogs/security_watchdog.py`
- `browser_use/browser/watchdogs/permissions_watchdog.py`
- `browser_use/tools/service.py`
- `browser_use/tools/registry/service.py`
- `browser_use/mcp/server.py`
- `browser_use/sandbox/sandbox.py`
- `browser_use/beta/service.py`

## 12. Lessons for Marix

1. A browser agent should have a strong page-change guard: stop remaining actions after URL or focus changes.
2. Dynamically generate the action model from the current page and allowed actions to reduce LLM misuse.
3. Keep browser state, screenshot, history, and compaction in an independent MessageManager.
4. Domain filtering, IP blocking, and upload containment are minimum safety baselines for browser agents.
5. A typed event bus decouples browser session, watchdogs, and agent history.
6. A beta wrapper can preserve a stable Python API while moving performance-critical execution to a native/Rust core.
7. MCP can expose both low-level browser actions and high-level autonomous retry behavior.

## 13. Risks and anti-patterns

- Browser automation is fragile: DOM changes, ads, cookies, login, and captcha can break behavior.
- Security depends heavily on domain/profile/watchdog configuration; bad configuration can enable overreach.
- The beta Rust core runs through the `browser-use-terminal` binary and is less transparent than pure Python source.
- Cloud sandbox execution sends code and parameters to an external service and needs enterprise review.
- Structured LLM action output needs retry, fallback, and loop detection to avoid spinning.
- Screenshot/vision-heavy workflows can significantly increase token and cost usage.
