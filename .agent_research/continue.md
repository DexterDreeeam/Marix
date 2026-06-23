# Continue Coding Agent Research

## 1. Source, activity, technical stack, and nature

Primary material: the completed external `coding-agent-research` output and upstream sources.

| Item | Details |
|---|---|
| Repository | https://github.com/continuedev/continue |
| Main languages | TypeScript, with smaller Kotlin/Python/Rust parts |
| Stack | VS Code extension, JetBrains plugin, CLI, TypeScript core, MCP SDK, OpenAI adapters, SQLite, tree-sitter, Vitest/Jest |
| Maintenance status | README says the repository is no longer actively maintained and is read-only for all users |
| Activity evidence | GitHub API showed a recent push on 2026-06-22 and release `v2.1.0-vscode` on 2026-06-19, but this should be read as final/maintenance activity |
| License | Apache-2.0 |

Continue is an IDE-first assistant architecture. Its core is a shared TypeScript engine and protocol layer used by VS Code, JetBrains, GUI, binary, and CLI surfaces. It is valuable for context-provider patterns, IDE protocol design, model abstraction, MCP routing, and streaming diff UX, but its official maintenance status is a major adoption constraint.

## 2. Entry points and modules

Top-level shape:

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
gui/                   # WebView GUI
binary/                # Node binary and messenger
packages/
  config-yaml/
  openai-adapters/
  terminal-security/
  llm-info/
skills/
docs/
```

Key paths include `core/core.ts`, `core/protocol/core.ts`, `core/llm/streamChat.ts`, `core/llm/index.ts`, `core/context/index.ts`, `core/tools/callTool.ts`, `core/edit/streamDiffLines.ts`, `extensions/vscode/src/extension.ts`, `extensions/vscode/src/VsCodeIde.ts`, and `binary/src/index.ts`.

## 3. Agent loop

Continue's loop is closer to IDE-core streaming plus a tools protocol than to an autonomous loop like OpenHands or OpenCode.

Chat flow:

```text
GUI/IDE starts llm/streamChat
  -> ConfigHandler.loadConfig()
  -> choose chat model
  -> if legacy slash command:
       slashCommand.run(...) async generator
     else:
       model.streamChat(messages, signal, completionOptions, messageOptions)
  -> yield ChatMessage chunks
  -> return PromptLog
```

Tool-call flow:

```text
model/chat UI produces toolCall
  -> protocol: tools/call
  -> callTool(tool, toolCall, extras)
       - safeParseToolCallArgs()
       - if tool.uri is HTTP -> POST
       - if tool.uri is MCP -> MCPManagerSingleton connection callTool
       - otherwise built-in -> callBuiltInTool()
       - return ContextItem[] or errorMessage
```

## 4. Planner / executor

Continue has chat, agent, edit, and plan-related capabilities, but the planner/executor boundary is implemented through system messages, selected model role, tool policies, IDE UI, slash commands, MCP tools, and edit/apply flows rather than a single planner class.

The core protocol includes `llm/streamChat`, `streamDiffLines`, `tools/call`, `tools/evaluatePolicy`, `tools/preprocessArgs`, `conversation/compact`, `process/killTerminalProcess`, and `process/markAsBackgrounded`.

## 5. Tool abstraction

Main paths:

- `core/tools/callTool.ts`
- `core/tools/builtIn.ts`
- `core/tools/definitions/`
- `core/tools/implementations/`
- `core/tools/parseArgs.ts`
- `core/tools/policies/`

Tool sources:

| Source | Behavior |
|---|---|
| Built-in | `readFile`, `grepSearch`, `runTerminalCommand`, `searchWeb`, and similar tools |
| MCP | `mcp://server/tool` URI |
| HTTP | `http(s)://` tool URI |
| Client-side edit tool | Some edit actions execute in the IDE/client side |

`callToolFromUri()` supports HTTP POST, MCP `callTool`, MCP UI resources, and mapping resource/text content back into `ContextItem` objects.

## 6. Model / provider adaptation

Core paths are `core/llm/index.ts`, `core/llm/llms/*`, and `packages/openai-adapters`.

`BaseLLM` includes:

| Method / field | Role |
|---|---|
| `streamChat()` | streaming chat |
| `chat()` | non-stream chat |
| `streamComplete()` | completion |
| `streamFim()` | fill-in-middle |
| `embed()` | embeddings |
| `supportsImages()` | image capability |
| `supportsFim()` | FIM capability |
| `supportsCompletions()` | legacy completion endpoint support |
| `supportsPrefill()` | assistant prefill support |
| `toolOverrides` | model-specific tool override |
| `cacheBehavior` | caching behavior |
| `logger` | prompt interaction logging |

Supported providers include Anthropic, OpenAI, Azure, Ollama, Bedrock, Vertex, Groq, Mistral, DeepSeek, and OpenAI-compatible endpoints.

## 7. Context construction

Main paths:

- `core/context/index.ts`
- `core/context/providers/`
- `core/context/retrieval/`
- `core/indexing/`

`BaseContextProvider` exposes:

```text
getContextItems(query, extras) -> Promise<ContextItem[]>
loadSubmenuItems(args) -> ContextSubmenuItem[]
```

Context provider types include codebase embedding/RAG search, file tree, current file, repo map, docs, web, Postgres, HTTP context, and Greptile. The flow is user input, @mentions, or selected code -> `context/getContextItems` -> `ConfigHandler` gets enabled providers -> providers return `ContextItem[]` -> token/context filtering -> injection into `ChatMessage`.

## 8. File editing and diff

Relevant paths:

- `core/edit/streamDiffLines.ts`
- `core/diff/streamDiff.ts`
- `core/diff/myers.ts`
- `extensions/vscode/src/apply`
- `extensions/vscode/src/diff`

`streamDiffLines()` flow:

```text
prefix/highlighted/suffix/input
  -> constructEditPrompt() or constructApplyPrompt()
  -> inject rules/system message
  -> recursiveStream(llm, abortController, type, prompt, prediction)
  -> streamLines()
  -> filter markdown/code fences/explanations/blank lines
  -> streamDiff(oldLines, generatedLines)
  -> restore indentation for insertion-only edits
  -> yield DiffLine
```

Traits: edit and apply modes, streaming diff, filtering of extra natural language, VS Code diff UI integration, and client-side accept/reject/application.

## 9. Command execution, sandbox, and permissions

Paths:

- `core/tools/implementations/runTerminalCommand.ts`
- `core/util/processTerminalStates.ts`
- `packages/terminal-security`

Execution model:

| Scenario | Behavior |
|---|---|
| Local workspace | Node `child_process.spawn()` |
| Remote workspace | Delegates to `ide.runCommand()` so command runs on the correct remote machine |
| Shell | Windows PowerShell; Unix login shell |
| Timeout | Default 2 minutes |
| Output | partial output streaming |
| Background | `waitForCompletion=false` allows background execution |
| Kill | process tracking and kill support |

Security is policy-based: `ToolPolicy`, `tools/evaluatePolicy`, `tools/preprocessArgs`, and UI confirmation. There is no strong sandbox by default; commands still execute in the user's IDE/workspace environment.

## 10. Memory and state persistence

Paths:

- `core/util/history.ts`
- `core/data/devdataSqlite.js`
- `core/data/log.js`

`HistoryManager` supports listing sessions from `sessions.json`, loading individual session JSON, saving sessions and the session list, deleting, and clearing all sessions.

Session JSON includes `sessionId`, `title`, `workspaceDirectory`, `history`, `mode`, `chatModelTitle`, and `usage`. DevData SQLite stores tokens per day, tokens per model, devdata logs, and LLM interaction logs.

## 11. Event stream, logging, and audit

Protocol events include `devdata/log`, `stats/getTokensPerDay`, `stats/getTokensPerModel`, `history/*`, `llm/streamChat`, `tools/call`, and `process/*`.

`BaseLLM._logEnd()` records prompt tokens, generated tokens, thinking tokens, usage, success/error/cancel state, and DataLogger devdata. `PromptLog` returns model title, model provider, completion, prompt, and completion options.

## 12. Testing strategy

`core/package.json` includes Jest, Vitest, TypeScript checking, ESLint, and coverage. Test files cover provider/stream behavior under `core/llm/*.vitest.ts`, tool argument and MCP naming behavior under `core/tools/*.vitest.ts`, diff/edit tests under `core/edit/*.test.ts`, context provider loading under `core/context/*vitest.ts`, VS Code integration tests, and GUI tests.

## 13. Plugins, MCP, and extension model

MCP paths:

- `core/context/mcp/MCPConnection.ts`
- `core/context/mcp/MCPManagerSingleton.ts`
- `core/tools/callTool.ts`

Supported transports include stdio, SSE, streamable HTTP, and WebSocket. MCP tools map to tools, resources map to context providers, prompts map to slash commands, OAuth supports SSE protected-resource auth, and MCP UI resources map to UI state.

Other extension mechanisms are configuration-driven rather than hook-heavy: `.continuerc.json`, `continue.yaml`, config blocks, rules, prompt files, custom context providers, slash commands, MCP servers, and IDE extension commands.

## 14. Core source file paths

Recommended paths for architecture review:

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

## 15. Lessons for `{{proj}}`

1. Define IDE-to-core protocol messages explicitly so editor surfaces stay replaceable.
2. Use a `ContextProvider` pattern to unify selection, file tree, RAG, docs, web, and database context.
3. Route HTTP, MCP, and built-in tools through one tool-call function.
4. Use streaming diff lines for responsive IDE editing UX.
5. Delegate remote workspace commands to the IDE/runtime that actually owns the remote environment.
6. Keep session JSON readable for simple export and migration when event replay is not required.
7. Build model capabilities into the LLM abstraction: chat, completion, FIM, embeddings, images, and prefill.

## 16. Risks and anti-patterns

| Risk / anti-pattern | Why it matters |
|---|---|
| Not actively maintained | README marks the repository read-only, so long-term dependency risk is high |
| Agent loop is not centralized | Autonomous behavior is spread across UI, protocol, tools, and model paths |
| UI-dependent permission | No strong process sandbox by default |
| Client-side edit application | Core and IDE state consistency can become complex |
| Heavy indexing stack | Embeddings, indexers, and tree-sitter increase operational cost |
| Non-event-sourced sessions | JSON history is simple but weak for replay and fine-grained audit |
| Conflicting activity signals | Recent release/push should not override official final/read-only status |
