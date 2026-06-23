# OpenHands Coding Agent 研究

## 1. 来源、活跃度、技术栈与性质

| 项目 | 内容 |
|---|---|
| 仓库 | https://github.com/OpenHands/OpenHands |
| 相关核心 SDK | https://github.com/OpenHands/software-agent-sdk |
| 主要语言 | Python、TypeScript |
| 技术栈 | FastAPI、Socket.IO/SSE、Docker/Kubernetes、LiteLLM、FastMCP、SQLAlchemy、Redis、React/Vite |
| 近期活跃证据 | GitHub API 显示 `OpenHands/OpenHands` 最近 push：2026-06-22；最新 release：`1.8.0`，2026-06-10 |
| 许可 | `pyproject.toml` 声明 MIT；GitHub API license 字段为 `NOASSERTION` |

OpenHands 当前主仓库更像产品/服务端/前端壳，核心 agent 抽象已经大量迁入 `OpenHands/software-agent-sdk`，主仓库通过 `openhands-sdk==1.29.0`、`openhands-agent-server==1.29.0`、`openhands-tools==1.29.0` 依赖使用。

主要来源：

- https://github.com/OpenHands/OpenHands
- https://raw.githubusercontent.com/OpenHands/OpenHands/main/pyproject.toml
- https://github.com/OpenHands/software-agent-sdk
- https://raw.githubusercontent.com/OpenHands/software-agent-sdk/main/openhands-sdk/openhands/sdk/conversation/impl/local_conversation.py
- https://raw.githubusercontent.com/OpenHands/software-agent-sdk/main/openhands-sdk/openhands/sdk/agent/agent.py
- https://raw.githubusercontent.com/OpenHands/software-agent-sdk/main/openhands-tools/openhands/tools/apply_patch/core.py

## 2. 入口与包结构

### 主仓库结构

```text
openhands/
  app_server/          # FastAPI 应用层，conversation、sandbox、event、settings、mcp 等
  server/              # 兼容入口；server/app.py 已提示迁移到 app_server.app
  db/                  # 数据库模型/迁移相关
frontend/              # React/Vite 前端
openhands-ui/          # UI 相关包
skills/                # 内置 skill
tests/unit/            # 单元测试
```

关键文件：

| 路径 | 作用 |
|---|---|
| `openhands/app_server/app.py` | FastAPI 主应用入口 |
| `openhands/server/app.py` | 兼容 re-export，提示使用 `openhands.app_server.app` |
| `openhands/app_server/app_conversation/live_status_app_conversation_service.py` | 应用层 conversation 生命周期和状态服务 |
| `openhands/app_server/event/event_service.py` | event service 抽象 |
| `openhands/app_server/event/event_service_base.py` | 文件/后端事件存储基类 |
| `openhands/app_server/sandbox/sandbox_service.py` | sandbox 服务抽象 |
| `openhands/app_server/sandbox/process_sandbox_service.py` | 通过进程启动 agent server 的 sandbox |
| `openhands/app_server/sandbox/sandbox_spec_service.py` | sandbox spec 模板服务，含 agent-server 镜像 |
| `openhands/app_server/mcp/mcp_router.py` | FastMCP 集成入口 |

### SDK 结构

```text
openhands-sdk/
  openhands/sdk/agent/              # AgentBase、Agent、ACPAgent
  openhands/sdk/conversation/       # Conversation、LocalConversation、RemoteConversation、EventLog
  openhands/sdk/context/            # prompt/context/condenser/skills
  openhands/sdk/llm/                # LLM 封装、模型注册、LiteLLM 调用
  openhands/sdk/tool/               # Tool spec/schema/client tool
openhands-tools/
  openhands/tools/                  # terminal、apply_patch、task_tracker、browser 等工具
openhands-agent-server/
  openhands/agent_server/           # remote agent server / event service
```

## 3. Agent loop

核心 loop 在 SDK：

- `openhands-sdk/openhands/sdk/conversation/impl/local_conversation.py`
- `openhands-sdk/openhands/sdk/agent/agent.py`

简化流程：

```text
Conversation.send_message()
  -> 追加 MessageEvent
Conversation.run()/arun()
  -> lazy load plugins / skills / MCP / hooks
  -> agent.init_state()
  -> while not FINISHED/PAUSED/STUCK/ERROR:
       - stuck_detector.is_stuck()
       - 若有 pending actions：执行 pending tools
       - agent.step()/astep()
           - prepare_llm_messages(state.view, condenser, llm)
           - make_llm_completion(..., tools=tools_map)
           - classify_response()
           - TOOL_CALLS -> ActionEvent -> tool executor -> Observation/Event
           - CONTENT/EMPTY/REASONING_ONLY -> 对应处理
       - 检查 budget、max_iteration、confirmation、stop hook
```

显著设计点：

- `LocalConversation.run()` 支持同步 loop；`arun()` 支持异步 loop。
- `ConversationState` 维护 `execution_status`：`IDLE/RUNNING/PAUSED/FINISHED/STUCK/ERROR/WAITING_FOR_CONFIRMATION`。
- `StuckDetector` 检测重复错误、工具崩溃、monologue、action/observation pattern 等。
- `max_iteration_per_run` 默认 500；还支持 `max_budget_per_run` 成本上限。
- tool call 可并发，`Agent` 内部使用 `ParallelToolExecutor`。

## 4. Planner / executor

OpenHands 没有一个显式独立的 “Planner 类”，而是通过：

- system prompt；
- `ThinkTool` / `FinishTool`；
- tool set；
- confirmation policy；
- hook；
- agent/subagent/skills；

来塑造计划与执行行为。

相关模块：

| 模块 | 作用 |
|---|---|
| `Agent.step()` | 采样 LLM、解析工具调用、执行工具、处理结果 |
| `ParallelToolExecutor` | 批量并行执行工具 |
| `FinishTool` | 结束任务 |
| `ThinkTool` | 显式思考/规划步骤 |
| `delegate` tool / subagent | 支持委托子任务 |
| hook stop | agent 想结束时，hook 可以阻止并注入反馈 |

## 5. Tool abstraction

核心路径：

- `openhands-sdk/openhands/sdk/tool/schema.py`
- `openhands-sdk/openhands/sdk/tool/spec.py`
- `openhands-tools/openhands/tools/*`
- `openhands-tools/openhands/tools/apply_patch/definition.py`

工具具备：

- action type / observation type；
- JSON schema / Pydantic 校验；
- MCP tool definition 适配；
- annotations，例如 `readOnlyHint`、`destructiveHint`、`idempotentHint`；
- confirmation/security risk；
- event 化执行结果。

内置工具方向：

| 工具 | 路径/说明 |
|---|---|
| terminal/bash | `openhands-tools/openhands/tools/terminal` |
| file editor | `openhands-tools/openhands/tools/file_editor` |
| apply_patch | `openhands-tools/openhands/tools/apply_patch` |
| task tracker | `openhands-tools/openhands/tools/task_tracker` |
| browser | preset 中可启用 browser tool |
| MCP tools | `openhands-sdk/openhands/sdk/mcp` 与主仓库 `app_server/mcp` |

## 6. 模型/provider 适配

主仓库 `pyproject.toml` 显示：

- `litellm==1.84.1`
- `openai==2.33.0`
- `anthropic[vertex]`
- `google-genai`
- `google-cloud-aiplatform`
- `boto3`

SDK 中 `LLM` 封装负责：

- 模型配置；
- prompt cache key；
- headers，例如 `x-litellm-session-id`；
- token/cost 统计；
- context window error 映射；
- condenser fallback。

路径：

| 路径 | 作用 |
|---|---|
| `openhands-sdk/openhands/sdk/llm/llm.py` | LLM 配置和 completion 封装 |
| `openhands-sdk/openhands/sdk/llm/llm_registry.py` | LLM usage registry |
| `openhands-sdk/openhands/sdk/agent/utils.py` | completion 调用、消息准备、工具调用解析 |

## 7. 上下文构建

核心路径：

- `openhands-sdk/openhands/sdk/context/`
- `openhands-sdk/openhands/sdk/context/condenser.py`
- `openhands-sdk/openhands/sdk/context/prompts/`
- `openhands-sdk/openhands/sdk/skills.py`

上下文来源：

| 来源 | 说明 |
|---|---|
| system prompt | static + dynamic context |
| conversation event view | `ConversationState.view` 增量维护 |
| workspace | local/remote workspace |
| skills | public/user/project/plugin skills |
| secrets | secret registry 只暴露 secret name/description 到动态上下文 |
| MCP config | 插件或 agent context 合并 MCP 配置 |
| condenser | context 超限或 malformed history 时触发压缩 |

重要机制：

- `prepare_llm_messages(state.view, condenser, llm)`；
- `LLMSummarizingCondenser`；
- `CondensationRequest` / `Condensation` event；
- project skills lazy loading；
- plugin skills 后加载并合并到 agent context。

## 8. 文件编辑 / diff

重点是 `apply_patch`：

- 路径：`openhands-tools/openhands/tools/apply_patch/core.py`
- 支持 `Add/Update/Delete/Move`；
- 文本 patch 格式接近 OpenAI `apply_patch` 风格；
- 有 parser、chunk、fuzz 逻辑；
- tool annotations 标明破坏性和非幂等。

主仓库 app 层还包含 Git provider / selected repository / branch 相关逻辑，负责与 GitHub/GitLab/Forgejo 等集成。

## 9. 命令执行 / 沙箱 / 权限

### 沙箱

主仓库 app 层有 sandbox service 抽象：

| 路径 | 说明 |
|---|---|
| `openhands/app_server/sandbox/sandbox_service.py` | sandbox CRUD 抽象 |
| `openhands/app_server/sandbox/process_sandbox_service.py` | 以本地进程启动 agent-server |
| `openhands/app_server/sandbox/sandbox_spec_service.py` | sandbox spec，包含 `ghcr.io/openhands/agent-server:1.29.0-python` |
| `openhands/app_server/sandbox/session_auth.py` | sandbox 会话认证 |

OpenHands 的安全边界主要不是工具本身，而是：

- agent server / sandbox 隔离；
- Docker/remote/process sandbox；
- session API key；
- per-conversation secret registry；
- confirmation policy；
- security analyzer。

### 权限

SDK 中 `Agent._requires_user_confirmation()` 会：

1. 分析 ActionEvent；
2. security analyzer 评估风险；
3. confirmation policy 判断是否进入 `WAITING_FOR_CONFIRMATION`；
4. pending actions 在下一次 run 时执行。

## 10. 记忆 / 状态持久化

核心路径：

| 路径 | 作用 |
|---|---|
| `openhands-sdk/openhands/sdk/conversation/state.py` | ConversationState |
| `openhands-sdk/openhands/sdk/conversation/event_store.py` | EventLog |
| `openhands-sdk/openhands/sdk/conversation/secret_registry.py` | secret persistence |
| `openhands/app_server/app_conversation/*` | app 层 conversation metadata |
| `openhands/app_server/event/*` | event service / storage |

状态模型：

- event-sourcing 风格；
- `EventLog` 持久化 event；
- `ConversationState.rebuild_view()` 可从 events 重建；
- `fork()` 深拷贝事件历史并创建新 conversation；
- secret 可用 cipher 加密，否则序列化时 redacted；
- session title、tags、stats、usage/cost 均在 state/event 中维护。

## 11. 事件流 / 日志 / 审计

事件类型覆盖：

- `MessageEvent`
- `SystemPromptEvent`
- `ActionEvent`
- `ObservationEvent`
- `AgentErrorEvent`
- `ConversationErrorEvent`
- `CondensationRequest`
- `PauseEvent`
- `InterruptEvent`

主仓库 `app_server/event` 提供事件 API；SDK `EventLog` 提供持久化和回放基础。OpenTelemetry / Laminar 相关依赖用于观测。

## 12. 测试策略

主仓库：

- `pytest`
- `pytest-asyncio`
- `pytest-xdist`
- `pytest-playwright`
- `ruff`
- `mypy`

SDK 仓库也有 cross tests，例如 stuck detector、conversation、tool 相关测试。

测试重点：

- event/state；
- sandbox service；
- MCP；
- app server API；
- frontend e2e；
- context/stuck detection；
- tool parser。

## 13. 插件 / MCP / 扩展机制

OpenHands 的扩展层很完整：

| 扩展点 | 说明 |
|---|---|
| PluginSource | 支持 GitHub/git/local plugin source |
| Plugin.load | 加载 skills、MCP config、hooks、agents |
| Hooks | session_start、stop hook、user prompt submit、tool hooks |
| Skills | user/project/public/plugin skills |
| MCP | FastMCP client/proxy，支持 streamable HTTP 等 |
| file-based agents | `.agents/agents/*.md`、`.openhands/agents/*.md` |
| client tools | 前端或外部 client 注册动态工具，由 event consumer 执行 |

## 14. 对 `{{proj}}` 的借鉴

以下经验可作为 `{{proj}}` 设计 agent runtime、工具边界和工程治理时的参考：

1. **事件溯源式 conversation state**：action/observation/message 都是 event，便于恢复、审计、fork。
2. **sandbox 与 agent loop 分离**：agent 不直接关心 Docker/remote/process sandbox。
3. **confirmation policy + security risk**：权限不是简单 allow/deny，而与风险分析、工具 annotation 结合。
4. **condenser first-class**：context 压缩是 loop 内的一等恢复路径。
5. **plugin 合并语义清晰**：skills 覆盖、MCP config 合并、hooks 追加。
6. **client tool 机制**：允许 UI/外部客户端实际执行工具，agent 只发 ActionEvent。
7. **stuck detector**：生产 agent 必须内置“卡死检测”。

## 15. 核心源码文件路径

建议优先阅读以下源码路径来复核架构判断：

- `openhands/app_server/app.py`
- `openhands/app_server/app_conversation/live_status_app_conversation_service.py`
- `openhands/app_server/event/event_service.py`
- `openhands/app_server/sandbox/process_sandbox_service.py`
- `openhands/app_server/mcp/mcp_router.py`
- `openhands-sdk/openhands/sdk/conversation/impl/local_conversation.py`
- `openhands-sdk/openhands/sdk/agent/agent.py`
- `openhands-sdk/openhands/sdk/agent/utils.py`
- `openhands-sdk/openhands/sdk/context/condenser.py`
- `openhands-tools/openhands/tools/apply_patch/core.py`

## 16. 风险 / 反模式

| 风险 | 说明 |
|---|---|
| 架构分散 | 主仓库 + SDK + tools + agent-server，多仓库理解成本高 |
| 依赖重 | Docker/K8s/Redis/Postgres/FastMCP/LiteLLM/Playwright 等组合复杂 |
| LiteLLM pin | 锁定版本提升稳定性，但新模型/新 API 适配慢 |
| 事件一致性 | 多后端 event service 会带来一致性、分页、恢复语义差异 |
| 权限复杂 | security analyzer、confirmation、hook、sandbox 多层叠加，调试难 |
| 插件安全 | plugin 可以引入 hooks/MCP/skills，需要供应链与 secret 展开防护 |

---
