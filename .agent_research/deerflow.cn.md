# DeerFlow 2.x 外部 Agent 系统研究

> 目标仓库：bytedance/deer-flow — https://github.com/bytedance/deer-flow  
> 研究日期：2026-06-22  
> 覆盖范围：公开 `main` 分支源码、README、配置样例、contracts、backend/app、backend/packages/harness、frontend/src、skills、tests、docker/provisioner/nginx。  
> 限制：本研究为静态源码阅读，未 clone、未运行测试或部署；部分 GitHub metadata 采用用户提供事实。DeerFlow 2.0 与早期 v1 Deep Research 架构存在显著差异，本文以 2.x 公开源码为准。

## 1. 来源 / 活跃度

| 项 | 观察 |
|---|---|
| Repository | `bytedance/deer-flow` |
| License | MIT（用户提供 metadata） |
| Created | 2025-05-07（用户提供 metadata） |
| Pushed / Updated | 2026-06-22（用户提供 metadata） |
| Stars | 约 73k（用户提供 metadata） |
| 默认分支 | `main` |
| 版本 | `backend/pyproject.toml` 与 `backend/packages/harness/pyproject.toml` 均显示 2.1.0 |
| 主要定位 | README 称 DeerFlow 2.0 是 long-horizon SuperAgent harness；backend 描述为 LangGraph-based AI agent backend with sandbox execution capabilities |

主要源码来源：

- README：`README.md`
- LangGraph 入口：`backend/langgraph.json`
- 后端包：`backend/pyproject.toml`
- Harness 包：`backend/packages/harness/pyproject.toml`
- 前端包：`frontend/package.json`
- 主配置：`config.example.yaml`
- 扩展配置：`extensions_config.example.json`
- Subagent 契约：`contracts/subagent_status_contract.json`
- Docker / nginx / provisioner：`docker/*`

## 2. 技术栈 / 性质

DeerFlow 2.x 不是单一 Deep Research workflow，而是一个可部署的 **SuperAgent harness**：

| 层 | 技术 / 模块 |
|---|---|
| Agent runtime | LangGraph + LangChain `create_agent` |
| Backend / Gateway | Python 3.12+、FastAPI、LangGraph-compatible API |
| Harness package | `deerflow-harness`，包含 agent、middleware、tools、sandbox、models、MCP、skills、runtime |
| Frontend | Next.js 16、React 19、pnpm、LangGraph SDK、React Query、Radix、CodeMirror、streamdown |
| Sandbox | LocalSandboxProvider、AIO sandbox、Docker / Apple Container、Kubernetes provisioner |
| Persistence | LangGraph checkpointer + DeerFlow application DB；SQLite/Postgres/memory |
| Integrations | MCP、ACP agents、IM channels、OAuth/OIDC auth、Langfuse/LangSmith tracing |
| Skills | Markdown skill packages、frontmatter metadata、slash activation、security scan、optional self-evolution |

关键判断：DeerFlow 2.x 的核心不是固定 `planner -> researcher -> coder -> reporter` 图，而是：

```text
Gateway / Frontend / Channel
  -> run manager + stream bridge
  -> LangGraph lead_agent
  -> middleware chain
  -> model call
  -> tools / MCP / ACP / sandbox / skills / subagents
  -> state checkpoint + stream + run events
```

## 3. 入口与模块

| 模块 | 核心路径 | 作用 |
|---|---|---|
| LangGraph graph | `backend/langgraph.json` | `lead_agent: deerflow.agents:make_lead_agent`；auth 与 checkpointer 入口 |
| Gateway | `backend/app/gateway/app.py` | FastAPI app、middleware、routers、lifespan、runtime 初始化 |
| Run lifecycle | `backend/app/gateway/services.py`、`runtime/runs/worker.py`、`runtime/runs/manager.py` | 创建 run、配置 context、后台执行、SSE streaming、取消/rollback |
| Lead agent | `backend/packages/harness/deerflow/agents/lead_agent/agent.py` | 模型解析、工具组装、skill/subagent/middleware 接入 |
| Prompt | `agents/lead_agent/prompt.py` | 静态 system prompt、skill metadata、subagent instructions、保密边界 |
| State | `agents/thread_state.py` | ThreadState、sandbox/artifacts/todos/promoted reducers |
| Tools | `tools/tools.py`、`sandbox/tools.py`、`tools/builtins/*` | built-ins、config tools、MCP、ACP、subagent task |
| Models | `models/factory.py`、`models/*provider*.py` | provider resolution、thinking/reasoning、patched provider |
| Sandbox | `sandbox/*`、`community/aio_sandbox` | local / container / provisioner sandbox abstraction |
| Skills | `skills/*`、`tools/skill_manage_tool.py` | skill parse/load/install/security/self-evolution |
| MCP | `mcp/*` | MCP server loading、OAuth、session pool、path rewrite |
| Frontend | `frontend/src/app/workspace/*`、`frontend/src/core/*`、`frontend/src/components/workspace/*` | Chat UI、thread/run API、uploads、artifacts、subtask rendering |
| Contracts | `contracts/subagent_status_contract.json` | 后端 task tool 与前端 subtask UI 的状态契约 |
| Docker | `docker/docker-compose*.yaml`、`docker/nginx/nginx.conf`、`docker/provisioner/*` | prod/dev deployment、routing、K8s sandbox provisioner |

## 4. Agent loop / LangGraph 执行

### 4.1 执行流

1. 前端或 IM channel 调用 LangGraph-compatible `/api/langgraph/*` 或 Gateway custom API。
2. Gateway 标准化 input/config/context，并注入认证用户 context。
3. `RunManager` 创建 run，`StreamBridge` 准备 SSE event log。
4. 后台 `runtime/runs/worker.py::run_agent`：
   - 设置 run 为 running；
   - 捕获 pre-run checkpoint，用于 rollback；
   - 构建 runtime context：`thread_id`、`run_id`、`app_config`、journal；
   - 调用 `make_lead_agent` 创建 LangGraph graph；
   - 绑定 checkpointer/store；
   - 执行 `agent.astream(...)`。
5. LangChain agent loop：
   - middleware 预处理消息、context、uploads、memory、sandbox；
   - model 产生 AI message / tool calls；
   - tool runtime 执行工具；
   - ToolMessage / Command 写回 LangGraph state；
   - 持续循环直到无 tool call 或达到限制。
6. worker 将 LangGraph stream chunks 映射为 SSE event。
7. run 完成后：
   - 刷新 journal；
   - 写 token usage / message summary；
   - 同步 thread title/status；
   - publish end sentinel；
   - 延迟清理 stream buffer。

### 4.2 Stream modes

`runtime/runs/worker.py` 支持：

- `values`
- `updates`
- `checkpoints`
- `tasks`
- `debug`
- `messages`
- `custom`

`messages-tuple` 会映射到 LangGraph `messages`。  
`events` mode 在 Gateway 中明确跳过，因为 Python public API 不能同时 `astream_events()` 和 values snapshots。

### 4.3 失败处理

| 场景 | 处理 |
|---|---|
| LLM/provider 错误 | `LLMErrorHandlingMiddleware` 转换为 fallback/error message；worker 可将 run 标记为 error |
| tool exception | `ToolErrorHandlingMiddleware` 变成 error ToolMessage，避免整轮直接崩溃 |
| sandbox acquisition | lazy init；错误返回 tool error |
| cancellation | RunRecord abort event；支持 interrupt 或 rollback |
| rollback | worker 使用 pre-run checkpoint snapshot 尝试恢复 |
| stream reconnect | `MemoryStreamBridge` 支持 bounded event log 与 `Last-Event-ID` replay |
| multi-worker | README/compose 注释指出 run state / stream bridge 是 in-process，默认单 worker |

## 5. planner / researcher / coder / reporter / subagent / SuperAgent harness 分工

DeerFlow 2.x 不再是旧式固定角色图。可以这样映射：

| 传统角色 | DeerFlow 2.x 对应机制 |
|---|---|
| Planner | Plan mode 下 `TodoMiddleware` 提供 `write_todos`；lead agent prompt 强制 clarify -> plan -> act |
| Researcher | web tools、MCP tools、deep-research / github-deep-research skills、general-purpose subagent |
| Coder | sandbox file tools、bash tool、ACP agent bridge、custom subagents |
| Reporter | `present_files`、outputs artifact、Markdown/HTML/PPT/图表类 skills |
| Subagent | `task` tool 后台启动 isolated subagent loop；内置 `general-purpose` 与 `bash`，也支持 config custom_agents |
| Orchestrator / SuperAgent | lead agent 负责拆分、并发 subagent、工具选择、综合结果 |

Subagent 特点：

- `task` tool 会启动后台 `SubagentExecutor` 并轮询结果。
- 支持 custom stream events：`task_started`、`task_running`、`task_completed`、`task_failed`、`task_cancelled`、`task_timed_out`。
- Subagent 继承 parent sandbox/thread/model/tool groups/user guardrail attribution，但 message context 隔离。
- 禁止递归 `task`，也禁用 `ask_clarification`、`present_files`。
- 内置：
  - `general-purpose`：复杂多步任务，默认 max_turns 150。
  - `bash`：命令执行与文件工具，默认 max_turns 60；LocalSandbox host bash 禁用时隐藏。

## 6. Tool abstraction

工具来源：

1. `config.example.yaml` 中声明的 tools：
   - `web_search`
   - `web_fetch`
   - `image_search`
   - `ls`
   - `read_file`
   - `glob`
   - `grep`
   - `write_file`
   - `str_replace`
   - `bash`
2. built-ins：
   - `present_files`
   - `ask_clarification`
   - `view_image`
   - `task`
   - `skill_manage`
3. MCP cached tools。
4. ACP agent invocation tool。

关键模式：

- `tools/tools.py::get_available_tools` 根据 tool group、model capability、subagent_enabled、sandbox policy 装配工具。
- 按 tool name 去重；config tools 优先。
- LocalSandboxProvider 默认不暴露 host bash。
- `tool_search` 可延迟 MCP schema：
  - prompt 只列 deferred tool names；
  - model 调用 `tool_search` 后才获得完整 schema；
  - promotion 写入 ThreadState `promoted`；
  - catalog hash 防止 persisted bare name 映射到漂移后的不同工具。
- `tool_output` 中间件可将超大工具结果外置到磁盘，仅给模型 preview + file reference，避免上下文爆炸。

## 7. 模型 / Provider 适配

`models/factory.py::create_chat_model` 通过配置字段 `use` 动态加载 LangChain chat model class，并处理：

- `thinking_enabled`
- `reasoning_effort`
- `supports_thinking`
- `supports_reasoning_effort`
- `supports_vision`
- `when_thinking_enabled`
- `when_thinking_disabled`
- OpenAI-compatible `stream_usage`
- OpenAI-compatible `stream_chunk_timeout` 默认 240s

Provider 适配亮点：

| Provider | 适配点 |
|---|---|
| OpenAI-compatible | 支持 Responses API、stream usage、thinking extra_body |
| Claude | `ClaudeChatModel` 支持 Claude Code OAuth token、Bearer auth、billing header、prompt caching、thinking budget |
| Codex | `CodexChatModel` 使用 Codex CLI credential 和 ChatGPT Codex Responses API |
| vLLM | `VllmChatModel` 保留 reasoning 字段，避免多轮 tool-call 丢失 reasoning |
| Gemini via OpenAI gateway | `PatchedChatOpenAI` 保留 `thought_signature` |
| MiMo / StepFun / MiniMax / DeepSeek 等 | patched adapters 处理 reasoning_content、message name、provider quirks |
| MindIE | 对超长 mock streaming / timeout 做保守配置 |

架构要点：model factory 是能力开关与 provider patch 的集中点，避免 agent loop 里散落 provider-specific 分支。

## 8. 上下文构建

上下文来自多层：

1. **静态 system prompt**
   - `SYSTEM_PROMPT_TEMPLATE` 尽量静态，利于 provider prefix cache。
   - 用户输入用 `--- BEGIN USER INPUT ---` / `--- END USER INPUT ---` 包裹并声明为 untrusted data。
   - system-context confidentiality 明确禁止泄露内部 prompt/skills/subagent/system tags。

2. **动态 context**
   - `DynamicContextMiddleware` 注入当前日期。
   - memory 被作为 hidden HumanMessage 注入，而不是 system authority，避免用户可编辑 memory 获得 system 权限。

3. **Uploads**
   - `UploadsMiddleware` 将当前/历史上传文件以 `<uploaded_files>` 注入最后一个 HumanMessage。
   - 若有转换后的 Markdown，注入 outline 或 preview，提示用 `read_file`/`grep` 定位。

4. **Skills**
   - base prompt 只列 skill metadata。
   - `/skill-name` slash activation 会注入完整 `SKILL.md`。
   - Summarization 会保护最近加载 skill 内容，避免压缩丢失。

5. **Summarization**
   - `DeerFlowSummarizationMiddleware` 按 token/message/fraction 触发。
   - 可保留最近 N 条消息或 token fraction。

6. **Plan mode**
   - `TodoMiddleware` 为复杂任务添加 todo 工具和规则。

7. **Vision**
   - 仅当当前模型 `supports_vision` 时注入 `ViewImageMiddleware` 和 `view_image` tool。

## 9. 文件 / 报告 / 制品生成

虚拟路径契约：

- `/mnt/user-data/workspace`
- `/mnt/user-data/uploads`
- `/mnt/user-data/outputs`
- `/mnt/skills`
- `/mnt/acp-workspace`
- deployment-configured custom mounts

制品生成与展示：

- Agent 应将用户可见结果写到 `/mnt/user-data/outputs`。
- `present_files` 只允许展示 outputs 下文件。
- Artifact API：`/api/threads/{thread_id}/artifacts/{path}`。
- HTML/XHTML/SVG 等 active content 强制 attachment 下载，避免在 app origin 执行脚本。
- `.skill` archive preview 有 member size cap。
- IM channel artifact delivery 也只发送 outputs 下文件。

Uploads：

- 上传目录按 thread/user 隔离。
- filename normalization + unsafe filename skip。
- no-follow symlink open。
- max files、single file size、total size。
- 自动 Office/PDF 转 Markdown 默认关闭，因为 host-side parser 有风险。
- 非 LocalSandboxProvider 时会同步文件到 sandbox。

## 10. Sandbox / 命令执行 / 权限 / 安全边界

### 10.1 Sandbox 抽象

`Sandbox` 定义：

- `execute_command`
- `read_file`
- `download_file`
- `list_dir`
- `write_file`
- `glob`
- `grep`
- `update_file`

Provider：

| Provider | 特点 |
|---|---|
| LocalSandboxProvider | 将虚拟路径映射到 per-thread host dirs；不是安全隔离边界 |
| AIO sandbox | Docker / Apple Container 隔离执行 |
| Provisioner mode | K8s Pod + NodePort + mounted user-data/skills |
| Custom mounts | 可配置 host_path -> container_path，支持 read_only |

### 10.2 Local sandbox 风险控制

- LocalSandbox host bash 默认禁用：`sandbox.allow_host_bash: false`。
- 即使 opt-in host bash，`validate_local_bash_command_paths` 仍做 best-effort：
  - 拒绝 `file://`
  - 拒绝 `..`
  - 限制绝对路径到 `/mnt/user-data`、skills、ACP、custom mounts、少数系统路径。
- file tools 校验路径族：
  - `/mnt/user-data/*` 可读写。
  - `/mnt/skills/*` 只读。
  - `/mnt/acp-workspace/*` 只读。
  - custom mount 尊重 read_only。
- 错误输出会 mask host path 为 virtual path，避免泄露宿主路径。

### 10.3 其他安全边界

| 安全点 | 实现 |
|---|---|
| Auth | `AuthMiddleware` fail-closed，非 public path 需 session/internal token |
| CSRF | Double Submit Cookie；state-changing methods 需 header + cookie |
| CORS | 需要显式 `GATEWAY_CORS_ORIGINS` |
| Owner isolation | authz decorators + repository user_id filtering |
| LangGraph compatibility auth | JWT + CSRF + metadata.user_id filter |
| Guardrails | 可配置 built-in allowlist / OAP / custom provider |
| Safety finish reason | provider safety stop 时抑制不可靠 tool_calls |
| Upload safety | traversal/filename/symlink/size/default no conversion |
| Skill safety | archive extraction guard + LLM security scanner fail-closed |
| Tool loop | loop detection + token budget + output budget |

README / Docker 注释也明确提示：DeerFlow 更适合 local/trusted deployment；公网或 LAN 部署需额外 IP allowlist、pre-auth gateway、network isolation。

## 11. Memory / State / Checkpoint / Storage

### 11.1 ThreadState

`ThreadState` 扩展 LangChain `AgentState`：

- `sandbox`
- `thread_data`
- `title`
- `artifacts`
- `todos`
- `uploaded_files`
- `viewed_images`
- `promoted`

Reducers：

- `merge_sandbox`：只接受同一 sandbox id；冲突 fail-closed。
- `merge_artifacts`：去重保序。
- `merge_todos`：last non-None wins。
- `merge_promoted`：catalog hash 改变时替换，避免 deferred tool 漂移。

### 11.2 Checkpointer / Database

`runtime/checkpointer/async_provider.py`：

- legacy `checkpointer` 配置优先。
- 否则使用 unified `database`。
- 支持 memory/sqlite/postgres。
- sqlite 会创建 parent dir，postgres 使用 pool + keepalive。

`persistence/engine.py`：

- application data 使用 SQLAlchemy async engine。
- sqlite 启用 WAL、foreign_keys。
- postgres 可 auto-create DB（dev convenience）。
- production 建议 Alembic，而不是只依赖 `create_all`。

### 11.3 Run events

`run_events` backend：

- `memory`：无持久化。
- `db`：SQLAlchemy-backed，支持 user_id filtering、trace truncation、per-thread seq、Postgres advisory lock。
- `jsonl`：每 run 一个 JSONL；适合 single-process；多进程 seq 可能重复或乱序。

### 11.4 Memory

Memory 默认 file-backed，按 user / agent scope：

- `user`
- `history`
- `facts`

更新方式：

- `MemoryMiddleware` 在 agent 结束后将过滤后的用户/助手消息加入 queue。
- `MemoryUpdater` 异步 debounce，用 LLM 生成 JSON update。
- 会过滤 upload event，避免 session-scoped 文件被长期记忆。
- injection 时按 token budget 格式化 memory。
- tiktoken 可预热；受限网络可配置 char estimate。

## 12. Message gateway / integrations

Gateway routers：

- models
- MCP
- memory
- skills
- artifacts
- uploads
- threads
- agents
- suggestions
- channel connections
- channels
- assistants compatibility
- auth
- feedback
- thread runs
- stateless runs

IM channels：

- Telegram
- Slack
- Discord
- Feishu/Lark
- DingTalk
- WeChat
- WeCom

Channel architecture：

```text
External IM platform
  -> Channel adapter
  -> MessageBus inbound queue
  -> ChannelManager
  -> Gateway / LangGraph-compatible run API
  -> stream / wait result
  -> OutboundMessage
  -> Channel adapter send / upload files
```

关键点：

- 支持 per-user channel binding。
- `channel_connections.require_bound_identity` 可阻止未绑定外部用户直接创建 DeerFlow threads/runs。
- inbound dedupe 只在有稳定 workspace/team/guild namespace 时启用，避免误合并。
- channel workers 使用 internal auth + CSRF header/cookie 调 Gateway。
- inbound files staged 到同一 user/thread storage bucket，避免 agent 读写路径与 channel staging 不一致。
- 仅发送 outputs artifacts。

## 13. 事件流 / 日志 / 观测

| 模块 | 作用 |
|---|---|
| `MemoryStreamBridge` | per-run bounded event log、SSE replay、heartbeat、end sentinel |
| `RunJournal` | LangChain callbacks -> run events；记录 run.start/end/error、LLM input/response、tool result、token usage |
| `RunManager` | in-memory registry + optional persistent RunStore；status/progress/token summary |
| tracing | LangSmith / Langfuse callbacks；root graph 绑定，避免重复 spans |
| token usage | per caller bucket：lead_agent / subagent / middleware；per-model breakdown |
| frontend | subtask cards、token usage UI、artifact preview、stream rendering |

Run statuses：

- `pending`
- `running`
- `success`
- `error`
- `timeout`
- `interrupted`

重要 caveat：docker compose 生产默认单 Gateway worker，因为 run cancellation、SSE reconnect、request dedupe、IM channels 依赖 in-process state；多 worker 需要共享 stream bridge（当前未实现）。

## 14. 测试策略

测试覆盖非常宽，说明 DeerFlow 2.x 已将很多边界变成回归测试：

| 区域 | 示例测试 |
|---|---|
| Gateway/auth | `test_auth_*`、`test_csrf_middleware.py`、`test_owner_isolation.py` |
| Runtime/runs | `test_run_manager.py`、`test_run_worker_rollback.py`、`test_runtime_lifecycle_e2e.py` |
| Checkpointer/storage | `test_checkpointer.py`、`test_persistence_engine_sqlite.py`、`test_run_event_store.py` |
| Sandbox/tools | `test_sandbox_tools_security.py`、`test_local_sandbox_*`、`test_write_file_tool_size_guard.py` |
| Subagents | `test_subagent_executor.py`、`test_task_tool_core_logic.py`、`test_subagent_status_contract.py` |
| MCP | `test_mcp_*`、`test_mcp_session_pool.py`、`test_mcp_oauth.py` |
| Skills | `test_skills_*`、`test_security_scanner.py`、`test_skill_manage_tool.py` |
| Models | `test_model_factory.py`、`test_claude_provider_*`、`test_codex_provider.py`、provider patch tests |
| Channels | `test_telegram_*`、`test_slack_*`、`test_discord_*`、`test_dingtalk_channel.py`、`test_wechat_channel.py` |
| Frontend | Playwright e2e：chat、artifact preview、subtask card、thread history、channels |
| Contracts | `contracts/subagent_status_contract.json` 后端/前端共同解析 |

可借鉴点：对跨语言 UI/backend contract 建 fixture，避免靠字符串前缀隐式约定漂移。

## 15. 插件 / Skills / MCP / 扩展机制

### 15.1 Skills

Skill 是 Markdown package：

- `SKILL.md`
- YAML frontmatter：
  - `name`
  - `description`
  - optional `license`
  - optional `allowed-tools`
- 可有 `references/`、`templates/`、`scripts/` 等 support files。

加载策略：

- Prompt base 只列 metadata。
- `/skill-name` 显式激活加载完整 skill。
- skill allowed-tools 可限制工具集合。
- public skills 与 custom skills 分离。
- skill self-evolution 默认关闭；开启后 `skill_manage` 可 create/edit/patch/delete/write support files。

安全：

- ZIP archive：
  - 拒绝 absolute path。
  - 拒绝 `..` traversal。
  - 跳过 symlink。
  - 总解压大小上限，防 zip bomb。
  - 禁止 nested `SKILL.md`。
- LLM security scanner：
  - `allow|warn|block`
  - unparseable / failure 默认 block。
  - executable content 更严格。

### 15.2 MCP

`extensions_config.example.json` 支持：

- `mcpServers`
  - `stdio`
  - `sse`
  - `http`
- `mcpInterceptors`
- per-server env/header/OAuth。

MCP 关键设计：

- stdio MCP 使用 persistent session pool。
- session scope 是 `(server_name, user_id:thread_id)`，防跨用户/thread 状态泄漏。
- session pool LRU 256。
- stdio server cwd 和 temp dir pin 到 thread workspace，使 Playwright 等工具产物可被 artifact API 解析。
- MCP output local file path 会保守重写为 `/mnt/user-data/...`，仅当路径存在且位于当前 thread user-data。
- OAuth token manager 支持 client_credentials / refresh_token，并可通过 interceptor 注入 Authorization。

### 15.3 ACP agents

`acp_agents` 可声明 Claude Code / Codex 等 ACP-compatible agents。  
DeerFlow 暴露 `invoke_acp_agent` tool。  
ACP workspace 位于 `/mnt/acp-workspace`；要交付给用户需复制到 `/mnt/user-data/outputs`。

## 16. 核心源码路径清单

| 路径 | 说明 |
|---|---|
| `README.md` | 2.0 定位、部署、安全提示 |
| `backend/langgraph.json` | graph/auth/checkpointer entry |
| `config.example.yaml` | models/tools/sandbox/subagents/skills/memory/database/channels/auth 配置 |
| `extensions_config.example.json` | MCP servers/interceptors/skills 扩展配置 |
| `backend/app/gateway/app.py` | FastAPI app 与 router/middleware/lifespan |
| `backend/app/gateway/services.py` | run lifecycle service |
| `backend/app/gateway/auth_middleware.py` | fail-closed auth |
| `backend/app/gateway/csrf_middleware.py` | CSRF double-submit |
| `backend/app/gateway/langgraph_auth.py` | LangGraph compatibility auth |
| `backend/app/gateway/routers/uploads.py` | uploads safety / conversion |
| `backend/app/gateway/routers/artifacts.py` | artifact serving / active content download |
| `backend/packages/harness/deerflow/agents/lead_agent/agent.py` | lead agent factory |
| `backend/packages/harness/deerflow/agents/lead_agent/prompt.py` | prompt / skills / subagents |
| `backend/packages/harness/deerflow/agents/thread_state.py` | state schema / reducers |
| `backend/packages/harness/deerflow/agents/middlewares/*` | context、memory、summarization、guard、loop、token、tool error |
| `backend/packages/harness/deerflow/tools/tools.py` | tool registry |
| `backend/packages/harness/deerflow/tools/builtins/tool_search.py` | deferred MCP schema |
| `backend/packages/harness/deerflow/tools/builtins/task_tool.py` | subagent task tool |
| `backend/packages/harness/deerflow/subagents/*` | subagent config/registry/executor |
| `backend/packages/harness/deerflow/models/*` | provider factory / patched adapters |
| `backend/packages/harness/deerflow/sandbox/*` | sandbox abstraction / local provider / tools |
| `backend/packages/harness/deerflow/mcp/*` | MCP client/session pool/OAuth/tools |
| `backend/packages/harness/deerflow/skills/*` | skills parser/installer/security/tool policy |
| `backend/packages/harness/deerflow/runtime/*` | runs/checkpointer/store/events/stream bridge/journal |
| `contracts/subagent_status_contract.json` | backend/frontend subagent status contract |
| `frontend/src/core/api/api-client.ts` | LangGraph SDK client + CSRF + stream sanitize |
| `frontend/src/core/tasks/subtask-result.ts` | subagent status parser |
| `frontend/src/components/workspace/messages/subtask-card.tsx` | subtask UI |
| `frontend/src/components/workspace/input-box.tsx` | model/mode/skill suggestions/uploads UI |
| `docker/docker-compose.yaml` | production topology |
| `docker/nginx/nginx.conf` | route rewrite / SSE support |
| `docker/provisioner/*` | K8s sandbox provisioner |

## 17. 对 `Marix` 的借鉴

1. **以 harness 而非单个 workflow 组织 agent**
   - `Marix` 可将 agent loop、runtime、tool registry、sandbox、memory、observability 拆成独立模块。
   - 避免把 planner/researcher/coder/reporter 写死在一张固定图里。

2. **静态 prompt + 动态 context 分离**
   - 静态 prompt 利于 prefix cache。
   - 日期、memory、uploads、skills 通过 middleware 注入。
   - 用户可编辑 memory 不应提升为 system 权限。

3. **Tool schema deferred loading**
   - 当 `Marix` 接入大量 MCP/tools 时，可以只在 prompt 暴露名字，用 tool_search 按需加载 schema。
   - promotion 需绑定 catalog hash，防止工具漂移。

4. **Subagent 作为 runtime capability，而不是 prompt trick**
   - 背景任务、隔离 context、状态 contract、timeout/cancel/token usage 都要工程化。
   - 前后端共享 contract fixture，减少 UI/backend drift。

5. **Sandbox 是单独层**
   - Local mode 不能宣称安全隔离。
   - 对 shell、file、artifact、upload 分别做路径与权限策略。
   - 生成物统一 outputs 目录，便于 UI/IM/API 交付。

6. **Run lifecycle 要可观测**
   - RunManager + StreamBridge + RunJournal 是非常实用的组合。
   - `Marix` 可采用 run_id/thread_id/event_id 三元结构支持 replay、resume、audit。

7. **配置驱动 provider patch**
   - 模型能力和 provider quirks 应在 model factory 集中处理。
   - thinking/reasoning/vision/tool-call replay 不能散落在 agent 逻辑中。

8. **Skills 安全扫描和 allowed-tools**
   - 若 `Marix` 支持用户/agent 写入技能，必须有 archive extraction guard、LLM scanner fail-closed、tool allowlist。

9. **单 worker caveat 要显式**
   - 如果 `Marix` 使用 in-process stream/run registry，就不要默认多 worker。
   - 多 worker 需要 Redis/NATS/DB-backed stream bridge 和 distributed run manager。

## 18. 风险 / 反模式

| 风险 | DeerFlow 观察 | 对 `Marix` 的提醒 |
|---|---|---|
| Local sandbox 被误认为安全隔离 | DeerFlow 明确禁用 host bash 默认值 | `Marix` 文档必须区分 convenience sandbox 与 security sandbox |
| in-process state 限制扩展 | RunManager/StreamBridge/IM services 多为进程内 | 上生产多副本前必须设计共享状态 |
| provider patch 复杂 | Gemini/vLLM/MiMo/Codex/Claude 都需要特殊处理 | provider abstraction 需要测试矩阵 |
| skills self-evolution 有供应链风险 | 默认关闭，写入前扫描 | `Marix` 不应默认允许 agent 自改 skills |
| MCP state leakage | DeerFlow 用 user_id:thread_id scope | `Marix` 需要明确 MCP session isolation key |
| Artifact active content | HTML/SVG 强制下载 | `Marix` 不应在 app origin inline 运行生成 HTML |
| Upload conversion parser risk | 自动转换默认关闭 | host-side parser 要 opt-in + sandbox |
| Prompt/context 泄露 | System-context confidentiality 明确禁止 | 仍需 UI 与日志侧防泄露 |
| Tool output 上下文爆炸 | ToolOutputBudgetMiddleware 外置大输出 | `Marix` 应内建 tool result budget |
| 旧架构资料误导 | v1 planner/researcher/coder/reporter 不等于 2.x | 研究/实现时必须标注版本 |

## 19. 总结模块图

```text
[Frontend / IM Channel / SDK]
        |
        v
[FastAPI Gateway]
  - Auth / CSRF / owner isolation
  - runs / threads / uploads / artifacts / models / skills / MCP routers
        |
        v
[RunManager + StreamBridge + RunJournal]
        |
        v
[LangGraph lead_agent]
  - static prompt
  - dynamic context middleware
  - uploads / memory / summarization / title / token / loop / safety / guardrails
        |
        v
[Model Provider Factory]
        |
        v
[Tool Runtime]
  - file/bash/web/image
  - MCP deferred tools
  - ACP agents
  - skill_manage
  - task subagents
        |
        v
[Sandbox + Storage]
  - /mnt/user-data/workspace
  - /mnt/user-data/uploads
  - /mnt/user-data/outputs
  - checkpointer / database / run_events / memory
```
