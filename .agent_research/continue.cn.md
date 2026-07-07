# Continue Coding Agent 研究

## 1. 来源、活跃度、技术栈与性质

| 项目 | 内容 |
|---|---|
| 仓库 | https://github.com/continuedev/continue |
| 主要语言 | TypeScript，另有 Kotlin/Python/Rust 少量 |
| 技术栈 | VS Code extension、JetBrains plugin、CLI、TypeScript core、MCP SDK、OpenAI adapters、SQLite、tree-sitter、Vitest/Jest |
| 维护状态 | README 明确声明：`continuedev/continue` repository is no longer actively maintained and is read-only for all users |
| 近期活动证据 | GitHub API 显示最近 push：2026-06-22；最新 release：`v2.1.0-vscode`，2026-06-19。但应视为 final/maintenance 活动，不应视为长期活跃维护 |
| 许可 | Apache-2.0 |

主要来源：

- https://github.com/continuedev/continue
- https://raw.githubusercontent.com/continuedev/continue/main/README.md
- https://raw.githubusercontent.com/continuedev/continue/main/package.json
- https://raw.githubusercontent.com/continuedev/continue/main/core/package.json
- https://raw.githubusercontent.com/continuedev/continue/main/core/core.ts
- https://raw.githubusercontent.com/continuedev/continue/main/core/llm/streamChat.ts
- https://raw.githubusercontent.com/continuedev/continue/main/core/llm/index.ts
- https://raw.githubusercontent.com/continuedev/continue/main/core/tools/callTool.ts
- https://raw.githubusercontent.com/continuedev/continue/main/core/context/index.ts
- https://raw.githubusercontent.com/continuedev/continue/main/core/context/mcp/MCPConnection.ts
- https://raw.githubusercontent.com/continuedev/continue/main/core/edit/streamDiffLines.ts
- https://raw.githubusercontent.com/continuedev/continue/main/core/tools/implementations/runTerminalCommand.ts

## 2. 入口与包结构

顶层结构：

```text
core/                  # shared core engine
  core.ts
  protocol/
  llm/
  context/
  tools/
  edit/
  diff/
  indexing/
  autocomplete/
  nextEdit/
  config/
  util/
extensions/
  vscode/              # VS Code extension
  intellij/            # JetBrains plugin
  cli/                 # CLI
gui/                   # Webview GUI
binary/                # Node binary / messenger
packages/
  config-yaml/
  openai-adapters/
  terminal-security/
  llm-info/
skills/
docs/
```

关键路径：

| 路径 | 说明 |
|---|---|
| `core/core.ts` | Core 类，IDE/webview/binary 协调中心 |
| `core/protocol/core.ts` | IDE ↔ Core RPC 协议 |
| `core/llm/streamChat.ts` | streaming chat 入口 |
| `core/llm/index.ts` | BaseLLM 抽象 |
| `core/context/index.ts` | BaseContextProvider |
| `core/tools/callTool.ts` | tool execution router |
| `core/edit/streamDiffLines.ts` | 编辑 diff streaming |
| `extensions/vscode/src/extension.ts` | VS Code 入口 |
| `extensions/vscode/src/VsCodeIde.ts` | VS Code IDE adapter |
| `binary/src/index.ts` | CLI/binary 入口 |

## 3. Agent loop

Continue 的 loop 更像 IDE-core streaming + tools protocol，不是 OpenHands/OpenCode 那种显式 autonomous loop。

核心路径：

- `core/llm/streamChat.ts`
- `core/tools/callTool.ts`
- `core/protocol/core.ts`

简化流程：

```text
GUI/IDE 发起 llm/streamChat
  -> ConfigHandler.loadConfig()
  -> 选择 chat model
  -> 若 legacy slash command:
       slashCommand.run(...) async generator
     否则:
       model.streamChat(messages, signal, completionOptions, messageOptions)
  -> yield ChatMessage chunks
  -> return PromptLog
```

工具调用流程：

```text
model/chat UI 产生 toolCall
  -> protocol: tools/call
  -> callTool(tool, toolCall, extras)
       - safeParseToolCallArgs()
       - tool.uri?
           HTTP -> POST
           MCP -> MCPManagerSingleton connection callTool
         else
           built-in -> callBuiltInTool()
       - 返回 ContextItem[] 或 errorMessage
```

## 4. Planner / executor

Continue 有 chat/agent/edit/plan 相关能力，但 planner/executor 边界主要由：

- system message；
- selected model role；
- tool policies；
- IDE UI；
- slash commands；
- MCP tools；
- edit/apply 流程；

共同实现，而非单独 planner 类。

协议中包含：

- `llm/streamChat`
- `streamDiffLines`
- `tools/call`
- `tools/evaluatePolicy`
- `tools/preprocessArgs`
- `conversation/compact`
- `process/killTerminalProcess`
- `process/markAsBackgrounded`

## 5. Tool abstraction

路径：

- `core/tools/callTool.ts`
- `core/tools/builtIn.ts`
- `core/tools/definitions/`
- `core/tools/implementations/`
- `core/tools/parseArgs.ts`
- `core/tools/policies/`

Tool 来源：

| 类型 | 说明 |
|---|---|
| Built-in | readFile、grepSearch、runTerminalCommand、searchWeb 等 |
| MCP | `mcp://server/tool` URI |
| HTTP | `http(s)://` tool URI |
| Client-side edit tool | 部分编辑工具在 IDE/client 侧执行 |

`callToolFromUri()` 支持：

- HTTP POST；
- MCP callTool；
- MCP UI resource；
- resource/text content 映射为 `ContextItem`。

## 6. 模型/provider 适配

核心路径：

- `core/llm/index.ts`
- `core/llm/llms/*`
- `packages/openai-adapters`

`BaseLLM` 能力：

| 方法/字段 | 说明 |
|---|---|
| `streamChat()` | streaming chat |
| `chat()` | non-stream chat |
| `streamComplete()` | completion |
| `streamFim()` | fill-in-middle |
| `embed()` | embedding |
| `supportsImages()` | 图像能力 |
| `supportsFim()` | FIM |
| `supportsCompletions()` | legacy completion endpoint |
| `supportsPrefill()` | assistant prefill |
| `toolOverrides` | model-specific tool override |
| `cacheBehavior` | caching |
| `logger` | prompt interaction log |

支持 provider 包括：

- Anthropic；
- OpenAI；
- Azure；
- Ollama；
- Bedrock；
- Vertex；
- Groq；
- Mistral；
- DeepSeek；
- OpenAI-compatible。

## 7. 上下文构建

路径：

- `core/context/index.ts`
- `core/context/providers/`
- `core/context/retrieval/`
- `core/indexing/`

`BaseContextProvider`：

```text
getContextItems(query, extras) -> Promise<ContextItem[]>
loadSubmenuItems(args) -> ContextSubmenuItem[]
```

上下文 provider 类型：

| Provider | 说明 |
|---|---|
| Codebase | embedding/RAG 搜索代码块 |
| FileTree | 文件树 |
| CurrentFile | 当前文件 |
| RepoMap | 仓库结构图 |
| Docs | 文档索引 |
| Web | 网页 |
| Postgres | 数据库 |
| HTTP | HTTP context |
| Greptile | 外部代码检索 |

上下文流程：

```text
用户输入 / @mention / selected code
  -> context/getContextItems
  -> ConfigHandler 获取 enabled providers
  -> provider.getContextItems()
  -> token/context filtering
  -> 注入 ChatMessage
```

## 8. 文件编辑 / diff

路径：

- `core/edit/streamDiffLines.ts`
- `core/diff/streamDiff.ts`
- `core/diff/myers.ts`
- `extensions/vscode/src/apply`
- `extensions/vscode/src/diff`

`streamDiffLines()` 流程：

```text
prefix/highlighted/suffix/input
  -> constructEditPrompt() 或 constructApplyPrompt()
  -> 注入 rules/system message
  -> recursiveStream(llm, abortController, type, prompt, prediction)
  -> streamLines()
  -> 过滤 markdown/code fence/英文解释/空白
  -> streamDiff(oldLines, generatedLines)
  -> insertion-only 时恢复 indentation
  -> yield DiffLine
```

特点：

- 支持 edit/apply 两种类型；
- streaming diff；
- 过滤模型多余自然语言；
- 与 VS Code diff UI 配合；
- client 侧最终应用/接受/拒绝 diff。

## 9. 命令执行 / 沙箱 / 权限

路径：

- `core/tools/implementations/runTerminalCommand.ts`
- `core/util/processTerminalStates.ts`
- `packages/terminal-security`

执行模型：

| 场景 | 行为 |
|---|---|
| local workspace | Node `child_process.spawn()` |
| remote workspace | 委托 `ide.runCommand()`，避免本地 extension host 跑错机器 |
| shell | Windows PowerShell；Unix login shell |
| timeout | 默认 2 分钟 |
| output | partial output streaming |
| background | `waitForCompletion=false` 可后台运行 |
| kill | 支持 process tracking / kill |

安全模型：

- `ToolPolicy` 评估；
- `tools/evaluatePolicy`；
- `tools/preprocessArgs`；
- UI 侧确认；
- 无强沙箱，命令仍在用户/IDE环境执行。

## 10. 记忆 / 状态持久化

路径：

- `core/util/history.ts`
- `core/data/devdataSqlite.js`
- `core/data/log.js`

`HistoryManager`：

| 方法 | 说明 |
|---|---|
| `list()` | 读 sessions.json |
| `load(sessionId)` | 读单个 session JSON |
| `save(session)` | 写 session JSON 和 sessions list |
| `delete()` | 删除 session |
| `clearAll()` | 清空 sessions folder |

Session 保存：

- `sessionId`
- `title`
- `workspaceDirectory`
- `history`
- `mode`
- `chatModelTitle`
- `usage`

DevData SQLite：

- token per day；
- token per model；
- devdata logs；
- LLM interaction logs。

## 11. 事件流 / 日志 / 审计

协议事件：

- `devdata/log`
- `stats/getTokensPerDay`
- `stats/getTokensPerModel`
- `history/*`
- `llm/streamChat`
- `tools/call`
- `process/*`

`BaseLLM` 中 `_logEnd()` 会记录：

- prompt tokens；
- generated tokens；
- thinking tokens；
- usage；
- success/error/cancel；
- DataLogger devdata。

PromptLog 返回：

- modelTitle；
- modelProvider；
- completion；
- prompt；
- completionOptions。

## 12. 测试策略

`core/package.json`：

- Jest；
- Vitest；
- TypeScript check；
- ESLint；
- coverage。

测试文件类型：

| 路径 | 说明 |
|---|---|
| `core/llm/*.vitest.ts` | provider/stream |
| `core/tools/*.vitest.ts` | tool args / MCP names |
| `core/edit/*.test.ts` | diff/edit |
| `core/context/*vitest.ts` | context provider loading |
| `extensions/vscode/*` | VS Code integration |
| `gui/*` | UI tests |

## 13. 插件 / MCP / 扩展机制

### MCP

路径：

- `core/context/mcp/MCPConnection.ts`
- `core/context/mcp/MCPManagerSingleton.ts`
- `core/tools/callTool.ts`

支持 transport：

- stdio；
- SSE；
- streamable HTTP；
- WebSocket。

能力映射：

| MCP capability | Continue 映射 |
|---|---|
| tools | Tool |
| resources | Context Provider |
| prompts | Slash command |
| OAuth | SSE protected resource auth |
| UI resource | MCP UI state |

### 其他扩展

Continue 更偏 config-driven，而不是 OpenCode 那类 plugin hooks：

- `.continuerc.json`
- `continue.yaml`
- config blocks；
- rules；
- prompt files；
- custom context providers；
- slash commands；
- MCP servers；
- IDE extension commands。

## 14. 对 `Marix` 的借鉴

以下经验可作为 `Marix` 设计 agent runtime、工具边界和工程治理时的参考：

1. **IDE protocol-first**：`core/protocol/core.ts` 把 IDE ↔ Core 能力定义得很清楚。
2. **ContextProvider pattern**：非常适合把 IDE selection、文件树、RAG、docs、数据库统一成上下文。
3. **MCP URI tool routing**：HTTP/MCP/built-in 工具统一在 `callTool()`。
4. **streaming diff**：边生成边转 diff line，适合 IDE 交互。
5. **remote workspace command delegation**：避免 VS Code extension host 在错误机器执行命令。
6. **历史 JSON 简单可读**：session 存储格式适合导出/迁移。
7. **BaseLLM 抽象完整**：chat、completion、fim、embedding、capability detection。

## 15. 核心源码文件路径

建议优先阅读以下源码路径来复核架构判断：

- `core/core.ts`
- `core/protocol/core.ts`
- `core/llm/streamChat.ts`
- `core/llm/index.ts`
- `core/context/index.ts`
- `core/context/mcp/MCPConnection.ts`
- `core/tools/callTool.ts`
- `core/tools/implementations/runTerminalCommand.ts`
- `core/edit/streamDiffLines.ts`
- `extensions/vscode/src/extension.ts`
- `binary/src/index.ts`

## 16. 风险 / 反模式

| 风险 | 说明 |
|---|---|
| 已不积极维护 | README 明确 read-only，不宜作为长期依赖 |
| agent loop 不集中 | autonomous agent 逻辑分散在 UI/protocol/tool/model |
| 权限依赖 UI | 缺少强进程沙箱 |
| 编辑应用在 client | core 与 IDE 侧状态一致性复杂 |
| context indexing 重 | embedding/indexer/tree-sitter 依赖多 |
| session 非 event-sourcing | JSON history 简单但不适合精细 replay/audit |
| 维护状态冲突 | 虽有近期 release/push，但官方说明是 final/read-only，应谨慎解读 |

---
