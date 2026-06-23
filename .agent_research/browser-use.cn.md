# browser-use 外部源码研究

## 1. 来源与活跃度

- 官方仓库：<https://github.com/browser-use/browser-use>
- 官网：<https://browser-use.com>
- 文档：<https://docs.browser-use.com>
- License：MIT。
- 默认分支：`main`。
- 研究素材中的 repo metadata：
  - `created_at`: 2024-10-31
  - `pushed_at`: 2026-06-20
  - `updated_at`: 2026-06-22
  - topics 包含 `ai-agents`, `browser-automation`, `llm`, `playwright`, `python`
- 近期提交示例：
  - 2026-06-20 `add qa skill (#5074)`
  - 2026-06-20 `qa skill: close localhost tunnel in teardown for v2 path too`
  - 2026-06-20 `qa skill: agent self-installs browser-harness`

## 2. 技术栈与项目性质

browser-use 是面向 AI agent 的浏览器自动化框架。它既提供 Python Agent API，也提供 CLI、MCP server、cloud/sandbox 能力，以及 0.13 系列引入的 Rust-backed beta agent wrapper。

`pyproject.toml` 关键信息：

| 项 | 值 |
|---|---|
| Python | `>=3.11,<4.0` |
| Version | `0.13.2` |
| 核心依赖 | Pydantic, aiohttp, anyio, httpx, rich, click |
| 浏览器/CDP | `cdp-use` |
| LLM providers | OpenAI, Anthropic, Gemini, Groq, Ollama, BrowserUse 等 |
| MCP | `mcp` |
| Event bus | `bubus` |
| 可选 native core | `browser-use-core==0.13.2` |
| CLI scripts | `browser-use`, `browseruse`, `bu`, `browser`, `browser-use-tui` |

## 3. 入口与模块

主要入口：

- `browser_use/__init__.py`
  - 设置 logging。
  - 对重型模块做 lazy import，例如 `Agent`, `BrowserSession`, `BrowserProfile`, `Tools`, 各 Chat model。
- `browser_use/agent/service.py`
  - legacy Python `Agent` 主实现。
- `browser_use/beta/service.py`
  - Rust-backed beta `Agent` wrapper。
- `browser_use/browser/session.py`
  - CDP browser session 与 watchdog orchestration。
- `browser_use/tools/service.py`
  - 内置 browser actions。
- `browser_use/mcp/server.py`
  - MCP server。
- `browser_use/sandbox/sandbox.py`
  - cloud sandbox decorator。

核心模块表：

| 模块 | 路径 | 作用 |
|---|---|---|
| Legacy Agent | `browser_use/agent/service.py` | Python agent loop |
| Agent schema/history | `browser_use/agent/views.py` | ActionResult, AgentOutput, AgentHistoryList |
| Message manager | `browser_use/agent/message_manager/service.py` | prompt/state/history/compaction |
| Browser session | `browser_use/browser/session.py` | CDP 连接、事件总线、watchdogs |
| Browser profile | `browser_use/browser/profile.py` | domain/security/download/profile 配置 |
| Watchdogs | `browser_use/browser/watchdogs/*` | security, permissions, downloads, dom, screenshot, captcha |
| Tools | `browser_use/tools/service.py` | navigate/click/type/extract/upload/tab 等 |
| Tool registry | `browser_use/tools/registry/service.py` | action decorator、dynamic Pydantic action model |
| MCP server | `browser_use/mcp/server.py` | MCP browser control tools |
| Sandbox | `browser_use/sandbox/sandbox.py` | 云端执行装饰器 |
| Beta Rust wrapper | `browser_use/beta/service.py` | stdio JSON-RPC 到 Rust terminal core |

## 4. Agent loop

### Legacy Python Agent

`browser_use/agent/service.py` 中 `Agent.step()` 的核心流程：

1. 等待 captcha solving。
2. `_prepare_context`
   - 获取 `browser_session.get_browser_state_summary(include_screenshot=True)`。
   - 更新 page-specific action model。
   - MessageManager 构造 state messages。
   - 触发 message compaction。
   - 注入 budget warning、replan、exploration、loop-detection nudges。
3. `_get_next_action`
   - `llm.ainvoke(... output_format=self.AgentOutput, session_id=...)`。
   - 支持 timeout、fallback LLM、conversation save callback。
4. `_execute_actions`
   - 调用 `multi_act(...)` 执行动作序列。
5. `_post_process`
   - 处理 downloads、plan、loop detector、failure count、final result。
6. `_finalize`
   - 生成 `AgentHistory`，保存 file system state，发 cloud events，step counter++。

`run(max_steps=500)` 会启动 browser、注册 skills、执行 initial actions，然后循环 `step()`，直到 done、max failures、stop/pause、max steps 或异常。

`multi_act()` 的关键安全逻辑：

- `done` 必须是唯一 action。
- 多 action 顺序执行。
- 每个 action 后检查页面是否变化。
- 如果 URL/focus target 变化，停止后续动作，避免在新页面继续错误点击。
- `terminates_sequence` action 会主动截断序列。

### Beta Rust-backed Agent

`browser_use/beta/service.py` 定义了兼容 Python Agent API 的 beta Agent，但实际把任务交给 `browser-use-terminal sdk-server`：

- `RustSdkClient` 启动子进程。
- 通过 stdio line-delimited JSON-RPC 通信。
- 调用 `runtime.ping` 协商 SDK protocol version。
- 调用 `agent.run_task` / `agent.run`。
- 收集 `agent.event` / `agent.projected_event` notifications。
- 将 Rust terminal events 重构为 Python `AgentHistoryList`。

`_sdk_run_params()` 发送给 Rust core 的关键字段：

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

`_run_env()` 将 browser/profile/permission/download/storage/domain 配置转换为环境变量，例如：

- `BU_CDP_URL`
- `BU_CDP_HEADERS`
- `BU_BROWSER_ALLOWED_DOMAINS`
- `BU_BROWSER_PROHIBITED_DOMAINS`
- `BU_BROWSER_PERMISSIONS`
- `BU_BROWSER_DOWNLOADS_PATH`
- `BU_BROWSER_STORAGE_STATE`
- `BU_MANAGED_BROWSER_ARGS`

## 5. 工具协议与模型适配

`browser_use/tools/registry/service.py` 的 registry 以 decorator 注册 action：

- `@registry.action(...)`
- 每个 action 绑定 Pydantic param model。
- 根据当前 page URL 过滤可用 action。
- 动态生成 ActionModel union 给 LLM 作为结构化输出目标。
- `execute_action()` 自动注入特殊依赖：
  - `browser_session`
  - `file_system`
  - `page_extraction_llm`
  - `sensitive_data`
  - `available_file_paths`
  - `extraction_schema`

内置 Tools 包括：

- search / navigate / go_back / wait
- click element / click coordinate
- input text
- upload file
- switch/close tab
- extract content
- save/download/文件相关动作

模型适配：

- legacy agent 走 `llm.ainvoke`，要求输出 `AgentOutput` 结构。
- 支持 OpenAI、Anthropic、Google、Groq、Ollama、BrowserUse Cloud 等 chat model。
- 支持 `fallback_llm`。
- 支持 structured extraction schema / output_model_schema。

## 6. 上下文、状态与记忆

上下文构建由 `MessageManager` 负责：

| 上下文 | 说明 |
|---|---|
| SystemPrompt | 任务、工具使用约束、browser-use 指令 |
| Browser state | DOM summary、URL、tabs、截图 |
| Recent events | 最近 browser/action 事件 |
| Agent history | 历史动作、结果、错误 |
| File system | 可用文件、下载文件、提取内容 |
| Plan | 当前 plan / replan 状态 |
| Sensitive data | 支持 domain-scoped secret 替换 |
| Compaction | 对旧消息做 LLM summary |

`ActionResult` 支持 `is_done`, `success`, `error`, `attachments`, `long_term_memory`, `extracted_content`, `include_extracted_content_only_once`, `include_in_memory` 等字段。

`AgentHistoryList` 支持 `final_result`, `is_done`, `is_successful`, `has_errors`, `extracted_content`, `urls`, `screenshot_paths`, `usage` 等字段。

## 7. 权限、沙箱与安全

browser-use 的安全重点是浏览器边界和文件边界。

### BrowserProfile

`browser_use/browser/profile.py` 支持：

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

`browser_use/browser/watchdogs/security_watchdog.py`：

- navigation 前检查 URL 是否允许。
- redirect 到 disallowed URL 后导航到 `about:blank`。
- 新 tab 如果 disallowed 则关闭。
- 支持 allowed/prohibited domain pattern。
- 可阻止 IP 地址访问。

### Permissions watchdog

`browser_use/browser/watchdogs/permissions_watchdog.py`：

- browser connected 后用 CDP `Browser.grantPermissions` 授权配置的 permissions。
- 授权失败为 non-fatal。

### 文件上传安全

`tools/service.py` 的 upload file 逻辑：

- 只允许上传 `available_file_paths`、downloaded files 或 FileSystem 管理的文件。
- 用 `realpath` 检查文件路径不逃逸 FileSystem dir。

### Cloud sandbox

`browser_use/sandbox/sandbox.py` 提供 `@sandbox(...)` decorator：

- 要求函数第一个参数是 `browser`。
- 提取函数源码、必要 import、参数/闭包变量。
- 用 `cloudpickle` 序列化参数。
- 将 execution code base64 后 POST 到 `https://sandbox.api.browser-use.com/sandbox-stream`。
- 通过 SSE 接收 `browser_created`, `instance_ready`, `log`, `result`, `error` 等事件。
- 支持 live browser URL、cloud profile、proxy country、timeout、env vars。

主要风险是 sandbox 属于外部云服务能力；使用该能力时，代码和参数会发送到远端执行环境。本地 legacy agent 不等价于隔离沙箱。

## 8. 事件、日志与观测

`browser_use/browser/events.py` 定义了大量 typed bubus events：

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

观测链路：

- `observability.py` 包装 Laminar `observe` / `observe_debug`。
- legacy agent 发送 ProductTelemetry `AgentTelemetryEvent`。
- beta agent 从 terminal events 重构 LLM/tool spans、usage/cost。
- sandbox 使用 SSE event stream 输出 runtime logs/result/error。

## 9. MCP

`browser_use/mcp/server.py` 暴露 browser-use 为 MCP server：

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

MCP 模式下 logging 写 stderr，避免污染 JSON-RPC stdout。

## 10. 测试与验证

仓库测试覆盖面较强，`tests/ci/` 包含：

- browser/CDP/navigation/screenshot/tabs/profile/proxy
- tools registry 参数注入、validation
- security:
  - domain filtering
  - IP blocking
  - upload containment
  - sensitive data
  - MCP allowed domains
  - download filename sanitization
- models:
  - OpenAI / Anthropic / Google / Azure / BrowserUse
- agent:
  - planning
  - loop detection
  - fallback LLM
  - action timeout
  - budget warning
  - beta agent
- CLI / cloud / setup / doctor / tunnel
- extraction / markdown / file system / structured output

GitHub workflows 包含：

- `test.yaml`
- `lint.yml`
- `eval-on-pr.yml`
- `cloud_evals.yml`
- `docker.yml`
- `publish.yml`

## 11. 核心路径

建议后续重点阅读：

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

## 12. 对 {{proj}} 的借鉴

1. 浏览器 agent 应有强 page-change guard：URL/focus 变化后停止后续动作。
2. 动作模型应根据当前页面和可用 action 动态生成，减少 LLM 误用。
3. MessageManager 独立负责 browser state、screenshot、history、compaction。
4. domain/IP/upload containment 是浏览器 agent 的最低安全底线。
5. typed event bus 让 browser session、watchdogs、agent history 解耦。
6. Beta wrapper 可保持 Python API 兼容，同时把高性能核心迁移到 native/Rust。
7. MCP server 可同时暴露低级 browser actions 和高级 autonomous agent retry。

## 13. 风险与反模式

- 浏览器自动化天然脆弱：DOM 变化、广告、cookie、登录、captcha 都会影响稳定性。
- 安全大量依赖 domain/profile/watchdog 配置，错误配置可能导致越权访问。
- beta Rust core 通过 `browser-use-terminal` 二进制运行，透明度低于纯 Python 源码。
- cloud sandbox 会把代码与参数发送到外部服务，企业场景需额外审计。
- LLM 结构化动作输出失败时，需要 retry/fallback/loop detection，否则容易空转。
- 对 screenshot/vision 的依赖会显著增加 token/cost。
