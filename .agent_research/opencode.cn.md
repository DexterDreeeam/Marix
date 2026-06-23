# OpenCode Coding Agent 研究

## 1. 来源、活跃度、技术栈与性质

| 项目 | 内容 |
|---|---|
| 用户给定优先仓库 | https://github.com/sst/opencode |
| 当前权威仓库 | https://github.com/anomalyco/opencode |
| 核实结果 | GitHub API 对 `sst/opencode` 返回/重定向到 `anomalyco/opencode` |
| 默认分支 | `dev` |
| 主要语言 | TypeScript |
| 技术栈 | Bun、TypeScript、Effect、AI SDK、Hono、Drizzle SQLite、SolidJS、OpenTUI、Electron、MCP |
| 近期活跃证据 | 最近 push：2026-06-22；最新 release：`v1.17.9`，2026-06-21 |
| 许可 | MIT |

主要来源：

- https://github.com/anomalyco/opencode
- https://raw.githubusercontent.com/anomalyco/opencode/dev/package.json
- https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/opencode/src/index.ts
- https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/core/src/agent.ts
- https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/opencode/src/session/processor.ts
- https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/llm/src/tool.ts
- https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/llm/src/llm.ts
- https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/core/src/tool/bash.ts
- https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/opencode/src/tool/write.ts
- https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/opencode/src/mcp/index.ts
- https://raw.githubusercontent.com/anomalyco/opencode/dev/packages/plugin/src/index.ts

## 2. 入口与包结构

顶层结构：

```text
packages/
  opencode/       # CLI / session / tools / config / server runtime glue
  core/           # agent config、permission、session event、database、filesystem
  llm/            # LLM request/schema/tool/provider/protocol
  plugin/         # plugin SDK and hooks
  sdk/js/         # generated JS SDK
  server/         # HTTP API
  tui/            # TUI
  console/        # OpenTUI/Solid console app
  app/ web/ ui/   # web UI
  desktop/        # Electron desktop
  containers/     # container-related package
```

CLI 入口：

- `packages/opencode/src/index.ts`
- 使用 `yargs`
- 命令包括：`run`、`serve`、`web`、`attach`、`mcp`、`agent`、`models`、`session`、`plugin`、`db`、`acp` 等。

`package.json` 显示：

- `bun@1.3.14`
- `effect 4.0.0-beta.83`
- `ai 6.0.168`
- `hono`
- `drizzle-orm`
- `diff`
- `@modelcontextprotocol/sdk`
- `solid-js`
- `@opentui/*`
- `typescript 5.8.2`

## 3. Agent loop

核心 loop 在：

- `packages/opencode/src/session/processor.ts`
- session / LLM / tool / event 协同还分布在 `packages/opencode/src/session/*`

`SessionProcessor.create()` 负责将 LLM stream 转为 session message/event/tool 状态。

简化流程：

```text
创建 assistant message
  -> snapshot.track()
  -> 构造 ProcessorContext
  -> llm.stream(...)
  -> handleEvent(LLMEvent):
       - reasoning-start/delta/end
       - text-start/delta/end
       - tool-input-start/delta/end
       - tool-call
       - tool-result
       - usage/finish/error
  -> session.updatePart()
  -> EventV2Bridge.publish(SessionEvent.*)
  -> tool call 状态 pending/running/completed/error
  -> compaction / retry / stop / continue
```

文件中有 `DOOM_LOOP_THRESHOLD = 3`，用于防止某些重复异常循环。

## 4. Planner / executor

OpenCode 的 planner/executor 是 agent mode + permission + tool/runtime 的组合：

| Agent | 说明 |
|---|---|
| `build` | 默认 agent，适合执行/修改 |
| `plan` | 只读/规划倾向 |
| `general` | 子 agent / 通用任务 |

核心定义在：

- `packages/core/src/agent.ts`

`AgentV2.Info` 包含：

- `id`
- `model`
- `request`
- `system`
- `description`
- `mode`
- `hidden`
- `color`
- `steps`
- `permissions`

执行器不是单一类，而是：

- session processor；
- LLM stream client；
- tool runtime；
- permission service；
- plugin hooks；
- EventV2。

## 5. Tool abstraction

核心路径：

- `packages/llm/src/tool.ts`
- `packages/core/src/tool/*`
- `packages/opencode/src/tool/*`

`Tool.make()` 支持两种输入：

| 模式 | 说明 |
|---|---|
| Typed | Effect `Schema` 作为 parameters/success，自动 decode/encode |
| Dynamic | JSON Schema，适合 MCP/plugin 动态工具 |

字段：

- `description`
- `parameters`
- `success`
- `execute`
- `toModelOutput`
- `toStructuredOutput`
- `_decode`
- `_encode`
- `_definition`

工具输出可映射为：

- structured output；
- model-facing text content；
- legacy result。

内置工具示例：

| 工具 | 路径 | 说明 |
|---|---|---|
| bash | `packages/core/src/tool/bash.ts` | shell 命令 |
| write | `packages/opencode/src/tool/write.ts` | 写文件、diff、格式化、LSP diagnostics |
| read/edit | `packages/opencode/src/tool/*` | 文件读取/编辑 |
| MCP tools | `packages/opencode/src/mcp/index.ts` | MCP tools 动态映射 |
| web/search 等 | 分布在 tool 目录 |

## 6. 模型/provider 适配

路径：

- `packages/llm/src/provider.ts`
- `packages/llm/src/llm.ts`
- `packages/llm/src/route/*`
- `packages/llm/src/providers/*`

`Provider.Definition`：

- `id`
- `model(factory)`
- optional `apis`

`LLM.request()` 统一构造：

- system；
- prompt；
- messages；
- tools；
- toolChoice；
- generation；
- providerOptions；
- HTTP options。

`generateObject()` 通过强制 synthetic tool call 实现跨 provider 结构化输出，而不是依赖 provider-native JSON mode。

支持 provider 依赖 AI SDK 与内部 route/protocol，常见包括 OpenAI、Anthropic、Google、Bedrock、Groq、xAI 等。

## 7. 上下文构建

上下文来源：

| 来源 | 说明 |
|---|---|
| AGENTS.md / config | 项目本地规则 |
| Agent system | `AgentV2.Info.system` |
| session messages | `SessionV1` / `SessionMessage` |
| snapshot | 工具执行前文件系统快照 |
| LSP diagnostics | write 后加入反馈 |
| plugin hooks | `experimental.chat.messages.transform`、`experimental.chat.system.transform` |
| compaction | session compaction event |
| location/worktree | active location / worktree |

路径：

- `packages/core/src/session/message.ts`
- `packages/core/src/session/event.ts`
- `packages/opencode/src/session/summary.ts`
- `packages/opencode/src/session/overflow.ts`

## 8. 文件编辑 / diff

`write` 工具路径：

- `packages/opencode/src/tool/write.ts`

流程：

```text
resolve file path
  -> assertExternalDirectoryEffect()
  -> read existing content + BOM
  -> createTwoFilesPatch()
  -> ctx.ask(permission="edit", metadata.diff)
  -> fs.writeWithDirs()
  -> format.file()
  -> publish FileSystem.Event.Edited
  -> publish Watcher.Event.Updated
  -> LSP touchFile + diagnostics
  -> 返回 diagnostics/output
```

特点：

- 保留 BOM；
- diff preview 进入 permission metadata；
- 写后自动格式化；
- 写后 LSP diagnostics；
- 外部目录访问需 approval。

## 9. 命令执行 / 沙箱 / 权限

命令工具路径：

- `packages/core/src/tool/bash.ts`

参数：

- `command`
- `workdir`
- `timeout`
- `description`

默认：

- timeout 2 分钟；
- 最大 timeout 10 分钟；
- stdout/stderr capture 各 1MB；
- POSIX 默认 `/bin/sh`，Windows 默认 `COMSPEC`/`cmd.exe`；
- detached process group；
- `AppProcess.run()` 控制 timeout/output。

权限：

- `PermissionV2.Rule`：`action/resource/effect`
- effect：`allow/deny/ask`
- bash 运行前 `permission.assert({ action: "bash", resources: [command] })`
- 外部目录通过 `external_directory` 权限断言。

安全注意：

- bash 描述明确：使用 host user's filesystem/process/network authority；
- 默认不是强沙箱；
- 有 `packages/containers`，但核心 bash 工具是宿主权限边界。

## 10. 记忆 / 状态持久化

OpenCode 强事件化/数据库化：

| 路径 | 说明 |
|---|---|
| `packages/core/src/session/event.ts` | durable/ephemeral session events |
| `packages/core/src/database/` | 数据库层 |
| `drizzle-orm` | SQLite schema/ORM |
| `SessionEvent.*` | step/text/tool/reasoning/compaction/retry |
| snapshot | session step 快照 |
| message parts | text/reasoning/tool 等 part |

`SessionEvent` 区分：

- Durable：可回放完整状态；
- Ephemeral：stream delta，例如 text delta、reasoning delta、tool input delta。

## 11. 事件流 / 日志 / 审计

`packages/core/src/session/event.ts` 定义大量 event：

- `Step.Started/Ended/Failed`
- `Text.Started/Delta/Ended`
- `Reasoning.Started/Delta/Ended`
- `Tool.Input.Started/Delta/Ended`
- `Tool.Called/Progress/Success/Failed`
- `Shell.Started/Ended`
- `Compaction.Started/Delta/Ended`
- `Retried`

这套模型适合：

- UI streaming；
- replay；
- audit；
- partial/durable event 分离；
- 工具运行状态恢复。

## 12. 测试策略

从仓库结构和 package scripts 看：

- root 不直接跑所有 test，提示 “do not run tests from root”；
- `packages/opencode/test/*` 包含 CLI/server/MCP lifecycle/httpapi 测试；
- Playwright 作为 e2e 依赖；
- TypeScript typecheck、oxlint；
- provider/tool/session 相关测试分散在包内。

测试重点：

- CLI MCP add；
- HTTP API MCP；
- MCP lifecycle；
- config；
- server API；
- session event；
- tool/permission。

## 13. 插件 / MCP / 扩展机制

### Plugin

路径：

- `packages/plugin/src/index.ts`

Plugin 形式：

```text
Plugin(input, options) => Promise<Hooks>
```

`PluginInput` 提供：

- SDK client；
- project；
- directory；
- worktree；
- workspace adapter registry；
- serverUrl；
- Bun shell `$`。

Hooks 很丰富：

| Hook | 作用 |
|---|---|
| `event` | 订阅事件 |
| `config` | 修改配置 |
| `tool` | 注册工具 |
| `auth` | provider auth |
| `provider` | provider/model hook |
| `chat.message` | 新消息 |
| `chat.params` | 修改 LLM 参数 |
| `chat.headers` | 修改请求 header |
| `permission.ask` | 修改权限决策 |
| `command.execute.before` | 命令前 |
| `tool.execute.before/after` | 工具前后 |
| `shell.env` | shell 环境 |
| `experimental.chat.*` | 修改消息/system |
| `experimental.session.compacting` | compaction prompt |

### MCP

路径：

- `packages/opencode/src/mcp/index.ts`

支持：

- stdio local MCP；
- streamable HTTP；
- SSE fallback；
- OAuth；
- tools/prompts/resources；
- `mcp.tools.changed` event；
- roots capability；
- status：connected/disabled/failed/needs_auth/needs_client_registration。

## 14. 对 `{{proj}}` 的借鉴

以下经验可作为 `{{proj}}` 设计 agent runtime、工具边界和工程治理时的参考：

1. **Effect + Schema 工具抽象**：类型安全、错误模型统一。
2. **durable/ephemeral event 分离**：流式 UI 与可回放状态两者兼顾。
3. **权限 metadata 带 diff**：审批时展示真实变更。
4. **写文件后 LSP diagnostics**：让 agent 立即看到编辑副作用。
5. **plugin hooks 非常细**：chat、tool、permission、shell env、compaction 都能扩展。
6. **MCP 生命周期完整**：local/remote/OAuth/status/resources/prompts/tools 全覆盖。
7. **snapshot before LLM stream**：避免 provider 内部工具执行导致状态丢失。

## 15. 核心源码文件路径

建议优先阅读以下源码路径来复核架构判断：

- `packages/opencode/src/index.ts`
- `packages/core/src/agent.ts`
- `packages/opencode/src/session/processor.ts`
- `packages/core/src/session/event.ts`
- `packages/llm/src/tool.ts`
- `packages/llm/src/llm.ts`
- `packages/llm/src/provider.ts`
- `packages/core/src/tool/bash.ts`
- `packages/opencode/src/tool/write.ts`
- `packages/opencode/src/mcp/index.ts`
- `packages/plugin/src/index.ts`

## 16. 风险 / 反模式

| 风险 | 说明 |
|---|---|
| Effect 学习曲线 | 对普通 TS 开发者不友好 |
| Bun 绑定 | 部署/企业环境兼容性需评估 |
| shell 非沙箱 | 默认宿主权限 |
| SQLite 并发 | 高并发/多 agent 可能有锁竞争 |
| 插件权限大 | plugin hooks 可深度改写行为，需要供应链审计 |
| 事件模型复杂 | durable/ephemeral 双轨需要严格设计，否则 replay bug 难查 |
| dev 分支为默认 | 稳定版/发布版与 dev 关系需持续跟踪 |

---
