# Cline Coding Agent 研究

## 1. 来源、活跃度、技术栈与性质

| 项目 | 内容 |
|---|---|
| 仓库 | https://github.com/cline/cline |
| 主要语言 | TypeScript |
| 技术栈 | Bun、Node.js、TypeScript、VS Code WebView、OpenTUI、WebSocket、SQLite/文件存储、AI SDK、多 provider |
| 近期活跃证据 | GitHub API 显示最近 push：2026-06-22；最新 release：`cli-v3.0.29`，2026-06-20 |
| 许可 | Apache-2.0 |

主要来源：

- https://github.com/cline/cline
- https://raw.githubusercontent.com/cline/cline/main/package.json
- https://raw.githubusercontent.com/cline/cline/main/sdk/ARCHITECTURE.md
- https://raw.githubusercontent.com/cline/cline/main/sdk/packages/agents/src/agent-runtime.ts
- https://raw.githubusercontent.com/cline/cline/main/sdk/packages/shared/src/tools/create.ts
- https://raw.githubusercontent.com/cline/cline/main/sdk/packages/core/src/extensions/tools/executors/bash.ts

## 2. 入口与包结构

当前 Cline 已明显 SDK/monorepo 化：

```text
sdk/
  ARCHITECTURE.md
  packages/
    shared/       # 类型、工具、hook、storage、prompt、logging、remote-config
    llms/         # provider gateway、模型目录、AI SDK provider
    agents/       # stateless agent loop
    core/         # session lifecycle、runtime host、storage、plugins、cron、hub
    sdk/          # SDK packaging
apps/
  cli/            # Bun CLI / OpenTUI / headless
  vscode/         # VS Code extension
  cline-hub/      # hub / webview
  examples/
```

`package.json` 显示：

- package manager：`bun@1.3.13`
- Node：`>=22`
- scripts：`build:sdk`、`test:e2e`、`test:unit`
- workspaces：`sdk/packages/*`、`apps/*` 等。

关键路径：

| 路径 | 作用 |
|---|---|
| `sdk/ARCHITECTURE.md` | 官方架构说明 |
| `sdk/packages/agents/src/agent-runtime.ts` | 无状态 agent loop |
| `sdk/packages/core/src/ClineCore.ts` | 有状态编排 facade |
| `sdk/packages/core/src/runtime/host.ts` | Local/Hub/Remote runtime boundary |
| `sdk/packages/shared/src/agent.ts` | Agent message/tool/event 类型 |
| `sdk/packages/shared/src/tools/create.ts` | tool creation API |
| `sdk/packages/llms/src/providers.ts` | provider gateway |
| `apps/cli/src` | CLI/TUI 入口 |
| `apps/vscode/src/extension.ts` | VS Code 扩展入口 |

## 3. Agent loop

核心在 `sdk/packages/agents/src/agent-runtime.ts` 的 `AgentRuntime.execute()`。

简化流程：

```text
AgentRuntime.run()/continue()
  -> ensureInitialized()
     - 注册 tools
     - setup plugins/hooks
  -> status=running, runId=createUID
  -> beforeRun hooks
  -> 追加用户消息
  -> while iteration < maxIterations:
       - emit turn-started
       - generateAssistantMessage()
           - beforeModel hooks
           - model.stream/chat
           - afterModel hooks
       - 追加 assistant message
       - 提取 tool-call parts
       - 无 tool call:
           - completion guard / reminder
           - finish run
       - executeToolCalls()
           - beforeTool hooks
           - tool policy
           - execute tool sequential/parallel
           - afterTool hooks
       - tool results 追加到 messages
       - 若 completion tool 成功 -> finish run
  -> afterRun hooks
```

事件类型包括：

- `run-started`
- `turn-started`
- `message-added`
- `assistant-message`
- `tool-start`
- `tool-finish`
- `turn-finished`
- `run-finished`
- usage / content update 等。

## 4. Planner / executor

Cline 没有硬编码 planner/executor 分层，而是通过以下机制实现：

| 机制 | 说明 |
|---|---|
| plan/build 模式 | 通过工具可用性、prompt、approval policy 区分 |
| completion tool | `submit_and_exit` 类工具标记 run 完成 |
| maxIterations | 防止无限循环 |
| hooks | beforeModel/beforeTool/afterTool 可改变运行 |
| tool policies | 工具级审批/拒绝/自动允许 |
| subagent/team | SDK 提供 agent/team runtime primitives |

`@cline/agents` 保持无状态，`@cline/core` 负责状态、session、存储、默认工具、compaction、hub。

## 5. Tool abstraction

工具创建 API 位于 `sdk/packages/shared/src/tools/create.ts`。

核心字段：

| 字段 | 作用 |
|---|---|
| `name` | 工具名 |
| `description` | 给模型看的说明 |
| `inputSchema` | JSON Schema 或 Zod schema |
| `execute(input, context)` | 执行函数 |
| `lifecycle.completesRun` | 是否为终止工具 |
| `timeoutMs` | 默认 30 秒 |
| `retryable/maxRetries` | 默认可重试，最多 3 次 |

特点：

- 注册时规范化 JSON schema；
- 支持 Zod 转 JSON schema；
- input schema 必须是对象；
- 工具执行上下文可包含 signal、session 等信息；
- tool policy 通过 `toolPolicies["*"]` 与 per-tool 合并。

内置工具方向：

- `read_file`
- `write_to_file`
- `edit_file`
- `apply_patch`
- `bash`
- `search_files`
- `fetch_web`
- `list_code_definition_names`
- completion tool

## 6. 模型/provider 适配

Cline 分层：

```text
@cline/llms
  -> createGateway(providerConfigs)
  -> gateway.createAgentModel({ providerId, modelId })
@cline/agents
  -> 只依赖 AgentModel 接口
@cline/core
  -> 负责 provider settings / telemetry / runtime wiring
```

支持 provider 类型包括：

- Anthropic
- OpenAI
- Google/Gemini
- AWS Bedrock
- Azure / Vertex
- OpenRouter
- Ollama / LM Studio
- OpenAI-compatible endpoints

`agent-runtime.ts` 支持：

- streaming；
- reasoning/text/tool-call parts；
- token usage；
- cache read/write tokens；
- provider finish reason；
- abort/cancel。

## 7. 上下文构建

上下文由 `@cline/core` 负责策略，`@cline/agents` 只提供 turn preparation seam。

来源：

| 来源 | 说明 |
|---|---|
| initialMessages | runtime 初始化消息 |
| systemPrompt | host/core 注入 |
| project rules | `.cline` / `.clinerules` / managed rules |
| skills | 文件系统 materialized skills |
| hooks/extensions | 可修改 message history 或 system prompt |
| tool results | message content part |
| context compaction | core-owned compaction pipeline |

`sdk/ARCHITECTURE.md` 明确：context compaction 属于 `core`，不是 `agents`。

## 8. 文件编辑 / diff

Cline 的编辑能力集中在 core 的 tools/executors：

| 方向 | 说明 |
|---|---|
| write/edit/apply_patch | host-side tool executor |
| diff preview | VS Code/webview 中展示 |
| undo | 用户命令恢复 |
| output limits | 限制工具输出注入上下文 |
| approval | 写文件通常需要审批或 policy 允许 |

具体文件路径随 executor 分布在：

- `sdk/packages/core/src/extensions/tools/`
- `sdk/packages/shared/src/diff/`
- `apps/vscode/src/...` 的 diff UI 部分

## 9. 命令执行 / 沙箱 / 权限

命令执行路径：

- `sdk/packages/core/src/extensions/tools/executors/bash.ts`

实现特征：

| 项 | 说明 |
|---|---|
| 执行方式 | Node.js `spawn` |
| shell | Unix 默认 `$SHELL`/bash，Windows PowerShell/cmd 相关逻辑 |
| timeout | 默认 30 秒 |
| output | rolling collector，保留头尾，中间截断 |
| cancel | `AbortSignal` |
| process tree | Unix kill process group；Windows `taskkill /T /F` |
| cwd/env | 由 runtime session 提供 |

安全模型：

- **无默认强沙箱**：命令在用户工作区/宿主权限下执行；
- 依赖 approval flow、tool policy、plan mode、auto-approve 设置；
- Hub 模式有本地 discovery token，防止同机未授权进程接入 hub；
- Remote runtime 通过 RuntimeHost 边界隔离。

## 10. 记忆 / 状态持久化

Cline 架构文档明确 `@cline/core` 负责：

- session lifecycle；
- storage and persistence；
- config watching/loading；
- cron durable queue；
- hub sessions/events/approvals/schedules；
- usage telemetry。

持久化形态：

| 类型 | 说明 |
|---|---|
| session messages | core storage adapter 管理 |
| cron | `packages/core/src/cron/`，SQLite `cron.db` |
| settings | core settings facade / watcher |
| rules/skills/hooks/plugins | 文件系统 watcher |
| hub discovery | owner-only discovery record + auth token |
| usage | root usage 与 aggregate usage 分桶 |

## 11. 事件流 / 日志 / 审计

事件流从 `AgentRuntimeEvent` 到 host UI / hub transport：

- 文本、reasoning delta；
- tool start/finish；
- message-added；
- assistant-message；
- run-finished；
- task.completed telemetry；
- usage/cost buckets。

日志抽象：

- `BasicLogger`：`debug/log/error`；
- CLI 用 Pino adapter；
- VS Code 用 OutputChannel；
- telemetry sink 可把 telemetry mirror 到 logger。

## 12. 测试策略

`package.json` 显示：

- `bun --parallel ... test`
- `test:unit`
- `test:e2e`
- Vitest

重点测试：

| 路径 | 说明 |
|---|---|
| `sdk/packages/agents/src/agent-runtime.test.ts` | agent loop |
| `sdk/packages/core/src/ClineCore.test.ts` | core session orchestration |
| `sdk/packages/shared/src/tools/create.test.ts` | tool API/schema |
| `sdk/packages/shared/src/vcr.test.ts` | HTTP VCR/replay |
| `apps/cli/*e2e*` | CLI e2e |

## 13. 插件 / MCP / 扩展机制

Cline 扩展层分为：

| 扩展点 | 说明 |
|---|---|
| plugins | setup 时注册 tools/hooks |
| hooks | beforeRun/afterRun/beforeModel/afterModel/beforeTool/afterTool/onEvent |
| MCP | core extension 可接 MCP tools/resources |
| file watchers | rules/workflows/skills/agents/hooks/plugins |
| cron automation | Markdown spec + YAML frontmatter + durable queue |
| remote config | materialize managed rules/workflows/skills 到 workspace-local `.cline/...` |

## 14. 对 `{{proj}}` 的借鉴

以下经验可作为 `{{proj}}` 设计 agent runtime、工具边界和工程治理时的参考：

1. **清晰 layered architecture**：shared → llms → agents → core → apps。
2. **agent loop 无状态**：便于复用、测试和嵌入。
3. **RuntimeHost 边界**：Local/Hub/Remote 的统一抽象非常适合多运行端。
4. **工具 API 简洁**：`createTool()` 将 schema、timeout、retry、lifecycle 合并。
5. **文件化扩展**：rules/skills/hooks/plugins 通过 watcher 统一加载。
6. **completion tool 终止契约**：比“模型说完成了”更可靠。
7. **Hub 共享会话**：多个客户端 attach/detach，不必停止 authority runtime。

## 15. 核心源码文件路径

建议优先阅读以下源码路径来复核架构判断：

- `sdk/ARCHITECTURE.md`
- `sdk/packages/agents/src/agent-runtime.ts`
- `sdk/packages/core/src/ClineCore.ts`
- `sdk/packages/core/src/runtime/host.ts`
- `sdk/packages/shared/src/agent.ts`
- `sdk/packages/shared/src/tools/create.ts`
- `sdk/packages/llms/src/providers.ts`
- `sdk/packages/core/src/extensions/tools/executors/bash.ts`
- `sdk/packages/shared/src/diff/`
- `apps/vscode/src/extension.ts`

## 16. 风险 / 反模式

| 风险 | 说明 |
|---|---|
| 默认无硬沙箱 | shell 直接在宿主权限执行 |
| Bun/Node 版本要求高 | Bun 1.3.13、Node >=22 |
| 架构很新 | SDK monorepo、Hub、Remote config 等快速演进，学习成本高 |
| 并发编辑冲突 | 多 session/agent 同时编辑文件需额外锁/merge 策略 |
| 复杂 watcher | 文件化扩展好用，但 watcher、reconcile、cache 会带来一致性问题 |
| approval UX | 过多审批影响自动化；过少审批风险高 |

---
