# Goose Coding Agent 研究

## 1. 来源、活跃度、技术栈与性质

| 项目 | 内容 |
|---|---|
| 仓库 | https://github.com/aaif-goose/goose |
| 主要语言 | Rust、TypeScript |
| 技术栈 | Rust workspace、Tokio、Axum、SQLite/sqlx、rmcp、OpenTelemetry、React/Electron UI、Agent Client Protocol |
| 近期活跃证据 | 最近 push：2026-06-22；最新 release：`v1.38.0`，2026-06-17 |
| 许可 | Apache-2.0 |
| 组织归属 | 当前权威仓库为 `aaif-goose/goose` |

主要来源：

- https://github.com/aaif-goose/goose
- https://raw.githubusercontent.com/aaif-goose/goose/main/Cargo.toml
- https://raw.githubusercontent.com/aaif-goose/goose/main/crates/goose/src/agents/agent.rs
- https://raw.githubusercontent.com/aaif-goose/goose/main/crates/goose/src/agents/tool_execution.rs
- https://raw.githubusercontent.com/aaif-goose/goose/main/crates/goose-providers/src/base.rs
- https://raw.githubusercontent.com/aaif-goose/goose/main/crates/goose/src/context_mgmt/mod.rs
- https://raw.githubusercontent.com/aaif-goose/goose/main/crates/goose/src/session/session_manager.rs
- https://raw.githubusercontent.com/aaif-goose/goose/main/crates/goose/src/config/permission.rs

## 2. 入口与包结构

Rust workspace：

```text
crates/
  goose/              # 核心 agent、conversation、extensions、context、security
  goose-cli/          # CLI
  goose-server/       # Axum server / API / event bus
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

关键路径：

| 路径 | 作用 |
|---|---|
| `crates/goose-cli/src/main.rs` | CLI 入口 |
| `crates/goose-cli/src/cli.rs` | CLI 命令定义 |
| `crates/goose/src/agents/agent.rs` | 核心 Agent |
| `crates/goose/src/agents/tool_execution.rs` | 工具确认与执行 |
| `crates/goose/src/agents/extension_manager.rs` | extension/MCP 管理 |
| `crates/goose-providers/src/base.rs` | provider trait |
| `crates/goose/src/session/session_manager.rs` | SQLite session 存储 |
| `crates/goose-server/src/main.rs` | server 入口 |
| `crates/goose-server/src/session_event_bus.rs` | session event bus |

## 3. Agent loop

核心在 `crates/goose/src/agents/agent.rs` 的 `Agent::reply()`。

简化流程：

```text
Agent.reply(user_message, session_config, cancel_token)
  -> load session / conversation
  -> maybe auto compact
  -> prepare tools/system prompt/extensions
  -> loop until max turns / cancel / final:
       - drain pending steers
       - run UserPromptSubmit hook
       - append user/steer messages
       - stream_response_from_provider(provider, system, messages, tools)
       - yield AgentEvent::Message / Usage / McpNotification
       - collect tool requests
       - tool inspection / permission check
       - approved tools -> dispatch_tool_call()
       - ask-before tools -> ActionRequired message -> wait confirmation
       - frontend tools -> FrontendToolRequest
       - tool results -> append tool response
       - maybe summarize tool pairs
       - stop hooks can block turn end
```

关键常量：

- `DEFAULT_MAX_TURNS = 1000`
- `DEFAULT_STOP_HOOK_BLOCK_CAP = 8`
- `DEFAULT_COMPACTION_THRESHOLD = 0.8`

`AgentEvent` 包括：

- `Message`
- `Usage`
- `McpNotification`
- `HistoryReplaced`

## 4. Planner / executor

Goose 的 planner/executor 也不是单一类，而是：

| 组件 | 作用 |
|---|---|
| Provider stream | 生成文本/tool requests |
| ExtensionManager | 执行 MCP/extension tools |
| ToolInspectionManager | 权限/安全分析 |
| ToolConfirmationRouter | 用户确认工具 |
| Context management | 压缩和 tool pair summary |
| Hooks | session/tool/shell/read/write 前后 |
| Recipes | workflow/task 模板 |
| Scheduler | scheduled job |

Goose mode 影响工具执行：

- chat mode 下工具可能被跳过；
- auto/agent mode 下工具按权限策略执行。

## 5. Tool abstraction

Goose 深度采用 MCP 工具模型：

- `rmcp::model::Tool`
- `CallToolRequestParams`
- `CallToolResult`
- ServerNotification

工具调用路径：

1. provider 返回 tool request；
2. `ToolRequest` 进入 conversation；
3. permission/security inspection；
4. approved：`dispatch_tool_call()`；
5. extension manager 将调用分发到 MCP client / platform extension；
6. tool result 或 notification stream 回写；
7. message 追加 tool response。

相关路径：

| 路径 | 说明 |
|---|---|
| `crates/goose/src/agents/tool_execution.rs` | confirmation 和 tool future 管理 |
| `crates/goose/src/agents/platform_tools.rs` | 平台工具 |
| `crates/goose/src/agents/platform_extensions/*` | 内置平台扩展 |
| `crates/goose/src/mcp_utils.rs` | MCP tool result 辅助 |
| `crates/goose/src/tool_monitor.rs` | tool 重复/异常监控 |

## 6. 模型/provider 适配

路径：

- `crates/goose-providers/src/base.rs`

`Provider` trait 核心方法：

| 方法 | 说明 |
|---|---|
| `stream()` | 主要 streaming 接口 |
| `complete()` | collect stream 到完整 message |
| `complete_fast()` | fast model fallback 到常规模型 |
| `get_model_config()` | 模型配置 |
| `fetch_supported_models()` | provider inventory |
| `fetch_recommended_models()` | canonical registry 过滤 |
| `manages_own_context()` | provider 是否自己管理上下文 |
| `supports_cache_control()` | cache control |
| `update_mode()` | GooseMode 切换 |
| `permission_routing()` | provider 权限路由 |

provider 层还包含：

- canonical model registry；
- retry config；
- OAuth；
- model info/context limit/cost；
- reasoning capability。

## 7. 上下文构建

主要路径：

- `crates/goose/src/context_mgmt/mod.rs`
- `crates/goose/src/prompt_template.rs`
- `crates/goose/src/prompts/*.md`
- `crates/goose/src/conversation/*`

上下文管理能力：

| 能力 | 说明 |
|---|---|
| auto compaction | token ratio 超过阈值时总结 |
| manual compact | 用户请求压缩 |
| tool-pair summarization | 对旧 tool call/result 对进行摘要 |
| agent/user visibility | message metadata 区分 user visible / agent visible |
| continuation message | 压缩后注入 agent-only continuation |
| provider-managed context | 对 Claude Code/Gemini CLI 等 provider 可跳过 goose-side 压缩 |

`compact_messages()` 策略：

1. 找到可保留的最近 user message；
2. 用 provider fast model 总结；
3. 原消息标记 agent-invisible；
4. summary 作为 agent-only；
5. 加入 continuation instruction；
6. 必要时恢复最近用户消息。

## 8. 文件编辑 / diff

Goose 自身通过工具生态执行文件操作，核心并不像 Aider/OpenCode 那样内置单一 patch engine。文件编辑可能来自：

- MCP filesystem extension；
- platform tools；
- ACP filesystem operations；
- frontend tools；
- shell 命令。

相关路径：

| 路径 | 说明 |
|---|---|
| `crates/goose/src/acp/fs.rs` | ACP file operations |
| `crates/goose/src/agents/platform_tools.rs` | 平台工具定义 |
| `crates/goose/src/agents/tool_execution.rs` | 工具执行与结果回写 |
| `crates/goose/src/tool_inspection/*` | 编辑/命令安全检查 |

因此，Goose 的重点不是 patch 格式，而是 MCP 工具调度、权限、确认、会话状态。

## 9. 命令执行 / 沙箱 / 权限

Goose 权限路径：

- `crates/goose/src/config/permission.rs`
- `crates/goose/src/permission/*`
- `crates/goose/src/agents/tool_execution.rs`

权限配置文件：

- `permission.yaml`

权限级别：

| Level | 说明 |
|---|---|
| `AlwaysAllow` | 总是允许 |
| `AskBefore` | 执行前询问 |
| `NeverAllow` | 永不允许 |

权限来源：

- user permission；
- smart approve；
- tool annotations：若 `read_only_hint=false`，会缓存为 AskBefore；
- security inspector；
- prompt injection/adversary/egress inspection；
- 用户确认：`AllowOnce`、`AlwaysAllow`、`DenyOnce`、`AlwaysDeny`。

沙箱：

- Goose 本身是本地 agent，工具通常以宿主权限运行；
- `Agent` 结构中存在 `container: Mutex<Option<Container>>`，但默认安全主线仍是权限/inspection/confirmation；
- 对命令和网络外连风险依赖 inspector 和用户确认。

## 10. 记忆 / 状态持久化

路径：

- `crates/goose/src/session/session_manager.rs`

核心常量：

- `SESSIONS_FOLDER = "sessions"`
- `DB_NAME = "sessions.db"`
- `CURRENT_SCHEMA_VERSION = 14`

`Session` 包含：

- id；
- working_dir；
- name；
- session_type；
- created_at/updated_at；
- extension_data；
- usage/accumulated_usage/cost；
- schedule_id；
- recipe；
- conversation；
- provider_name；
- model_config；
- goose_mode；
- project_id；
- last_message_snippet。

`SessionManager` 支持：

- create/get/list/delete；
- add_message；
- replace_conversation；
- export/import/copy；
- truncate_conversation；
- search_chat_history；
- update tool request metadata；
- 自动生成 session name。

## 11. 事件流 / 日志 / 审计

事件/日志能力：

| 组件 | 说明 |
|---|---|
| `AgentEvent` | agent streaming event |
| `session_event_bus.rs` | server session event bus |
| tracing | Rust tracing |
| OpenTelemetry | workspace 依赖 `opentelemetry-*` |
| usage | ProviderUsage、token/cost |
| security tracing | 用户对 security finding 的 allow/block 决策会记录 |
| session DB | message/history 持久化 |

Goose 对安全事件有显式字段，例如：

- `security.event_type`
- `security.action`
- `security.finding_id`
- `tool.request_id`
- `user.decision`

## 12. 测试策略

测试分布：

| 路径 | 说明 |
|---|---|
| Rust unit tests | 多数模块内 `#[cfg(test)]` |
| `crates/goose-test` | 测试 crate |
| `crates/goose-test-support` | 测试辅助 |
| `crates/goose-cli/src/scenario_tests` | CLI 场景测试 |
| `ui/desktop/src/App.test.tsx` | UI 测试 |
| `test_acp_client.py` | ACP client 测试 |
| `evals/` | 评估 |

测试重点：

- permission manager；
- context compaction；
- provider stream collect；
- MCP/extension；
- session import/export；
- CLI scenario；
- UI。

## 13. 插件 / MCP / 扩展机制

Goose 是 MCP-first：

| 扩展点 | 说明 |
|---|---|
| MCP extensions | 外部工具服务器 |
| platform extensions | 内置管理、scheduler、summon 等 |
| recipes | workflow recipe |
| hooks | session/tool/shell/read/write 前后 |
| frontend tools | 前端执行工具请求 |
| ACP | Agent Client Protocol 支持 |
| scheduler | 计划任务 |
| extension data | session 持久化 extension 状态 |

相关路径：

- `crates/goose/src/agents/extension_manager.rs`
- `crates/goose/src/agents/platform_extensions/`
- `crates/goose/src/recipe.rs`
- `crates/goose/src/hooks/`
- `crates/goose/src/acp/`

## 14. 对 `{{proj}}` 的借鉴

以下经验可作为 `{{proj}}` 设计 agent runtime、工具边界和工程治理时的参考：

1. **MCP-native 工具生态**：工具是外部能力，而不是全塞进核心。
2. **权限配置持久化**：`permission.yaml` 简洁可审计。
3. **tool annotations 自动影响权限**：read-only / write 风险可被机器读取。
4. **安全 inspector 分层**：permission、adversary、egress、repetition 分开。
5. **agent-visible/user-visible metadata**：压缩和 UI 展示不互相污染。
6. **session SQLite schema**：适合 CLI + Desktop + Server 多端共享。
7. **provider manages own context 标志**：对 Claude Code/Gemini CLI 类 provider 很重要。

## 15. 核心源码文件路径

建议优先阅读以下源码路径来复核架构判断：

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

## 16. 风险 / 反模式

| 风险 | 说明 |
|---|---|
| 工具执行依赖外部 MCP | 能力强但故障面大 |
| 默认非硬沙箱 | 宿主权限风险仍需权限/inspection 防护 |
| Rust + TS 双栈 | 贡献门槛高 |
| 权限 UX 复杂 | AlwaysAllow/AskBefore/NeverAllow + security finding 需要良好 UI |
| 上下文压缩复杂 | agent-visible/user-visible 语义需严格维护 |
| 多协议 | MCP + ACP + server API + desktop，集成测试成本高 |

---
