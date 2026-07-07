# Claude Code from Source 资料研究

## 1. 来源与活跃度

- 网站：<https://claude-code-from-source.com/>
- GitHub repo：<https://github.com/alejandrobalderas/claude-code-from-source>
- 资料性质：独立教育性分析，不是 Anthropic 官方文档。
- 网站声明基于早期 npm source maps 中的 TypeScript `sourcesContent` 进行逆向/学习性架构总结，并避免转载 Claude Code 原始源码。
- 本次素材覆盖章节：
  - `/`
  - `/ch01-architecture/`
  - `/ch02-bootstrap/`
  - `/ch03-state/`
  - `/ch04-api-layer/`
  - `/ch05-agent-loop/`
  - `/ch06-tools/`
  - `/ch07-concurrency/`
  - `/ch08-sub-agents/`
  - `/ch09-fork-agents/`
  - `/ch10-coordination/`
  - `/ch11-memory/`
  - `/ch12-extensibility/`
  - `/ch15-mcp/`
  - `/ch17-performance/`
- 未完整覆盖章节：terminal UI、input interaction、remote/cloud、epilogue 等。
- 研究素材中的 repo metadata：
  - `created_at`: 2026-04-01
  - `pushed_at`: 2026-04-04
  - `updated_at`: 2026-06-22
  - stars 约 2.2k
- 可信度限制：这是二手结构化分析资料，不等于当前 Claude Code 官方源码或最新行为。

## 2. 技术栈与项目性质

该资料把 Claude Code 描述为 terminal-native coding agent，整体呈 TypeScript/Node 风格架构，并归纳为六个核心抽象：

| 抽象 | 说明 |
|---|---|
| Query loop | `query.ts` 异步生成器，是 REPL、SDK、subagent、headless print 的统一入口 |
| Tool system | Tool 自描述 schema、permission、concurrency、rendering、progress |
| Tasks | background work / subagent 状态机 |
| State | bootstrap singleton + UI reactive store 双层状态 |
| Memory | file-based memory，项目/用户/团队层级 |
| Hooks | 生命周期拦截器，可阻断、改写、注入上下文、强制继续 |

该资料最适合作为架构模式目录：agent loop 形态、tool protocol、permission resolution、context management、subagent/fork strategy、MCP adaptation、performance tuning。

## 3. 入口与模块

该资料不是一个维护中的官方源码树，而是映射了概念模块与章节：

- `query.ts`
  - REPL、SDK、subagent、headless print、compaction、internal query 共用的 async-generator loop。
- Tool system 章节
  - 工具定义、validation、permission check、execution pipeline、progress、rendering、result budgeting。
- Bootstrap/state 章节
  - 区分 infrastructure singleton state 与 UI reactive store state。
- API layer 章节
  - client factory、provider routing、prompt cache stability、raw SSE streaming、non-streaming fallback。
- Subagent/fork 章节
  - 普通 subagent、fork agent、prompt-cache-preserving parallel branches、cleanup。
- Coordination 章节
  - task types、lifecycle status、foreground/background transition、output file 与 notification channels。
- Memory 章节
  - Markdown memory taxonomy、always-loaded index、relevance-selected detailed memory files。
- Extensibility/MCP 章节
  - hooks、skills、MCP tool wrapping、OAuth、transport、trust boundary。
- Performance 章节
  - startup fast path、dynamic imports、prompt cache ordering、token budgeting、search indexing、streaming watchdog。

## 4. Agent loop

资料重点强调 Claude Code 的 agent loop 是 async generator，而不是 callback tree 或普通 event emitter。

核心结论：

- loop yield `Message/Event`，return typed terminal reason。
- generator 提供：
  - backpressure
  - cancellation
  - `yield*` composition
  - 明确终止原因
- `query()` 统一用于：
  - REPL
  - SDK
  - sub-agent
  - `--print`
  - compaction/internal query
- loop 内部状态每次 continue 会重建完整 state object，而不是局部 mutate，便于测试和审计。

典型单轮流程：

1. context management。
2. call model / stream response。
3. 收集 tool calls。
4. 执行 tools。
5. tool results 追加到 message history。
6. 如果无 tool calls，则进入 stop hooks / completion / token budget 判断。
7. 根据 terminal 或 continue reason 结束或下一轮。

资料列出的 terminal states 包括：

- `completed`
- `model_error`
- `prompt_too_long`
- `aborted_streaming`
- `aborted_tools`
- `stop_hook_prevented`
- `hook_stopped`
- `max_turns`
- `blocking_limit`
- `image_error`

continue states 包括：

- `next_turn`
- `reactive_compact_retry`
- `max_output_tokens_recovery`
- `stop_hook_blocking`
- `token_budget_continuation`

## 5. 工具协议、模型适配与并发

Tool 系统是资料中最值得关注的架构模式之一。

Tool 接口重点字段：

- `call()`
- `inputSchema`
- `isConcurrencySafe(input)`
- `checkPermissions()`
- `validateInput()`

关键模式：

1. fail-closed defaults：
   - 新工具默认非并发安全。
   - 新工具默认非 read-only。
2. input-dependent safety：
   - Bash `ls` 可能并发安全，`rm` 不安全。
3. 统一执行 pipeline：
   - lookup
   - abort check
   - schema validation
   - semantic validation
   - speculative classifier
   - input backfill
   - PreToolUse hooks
   - permission resolution
   - denial handling
   - call execution
   - result budgeting
   - PostToolUse hooks
   - new messages
   - error classification
4. 结果预算：
   - 单工具输出限制。
   - 聚合 conversation budget。
   - 大结果持久化到磁盘并给模型 preview/path。
5. tool result protocol safety：
   - 对 orphaned `tool_use` 自动补 synthetic error `tool_result`，避免下轮 API protocol error。

工具并发拆为两层：

| 层 | 说明 |
|---|---|
| Batch orchestration | 模型响应完成后，把 tool calls 按安全性切分为 parallel/serial batches |
| Streaming executor | tool_use block 一旦 stream 完整就可投机启动工具 |

并发规则：

- Read/Grep 等可并发。
- Edit/写文件/shell mutation 串行。
- Bash 是否并发安全取决于本次命令输入，而不仅是工具名。
- 结果按模型请求顺序 yield，而不是按完成顺序。
- Bash 错误会触发 sibling abort cascade，取消同批相关 shell。

模型/API 适配模式：

- `getAnthropicClient()` 作为统一 client factory。
- query loop 不关心 provider，API layer 负责 direct API、Bedrock、Vertex、Foundry 等路由。
- system prompt 以 cache stability 为架构约束。
- 静态 prompt section 放前面，动态/用户相关 section 放后面。
- 明确区分会破坏 cache 的 volatile prompt section。
- raw SSE streaming 替代 SDK 高层 parser，避免大 tool JSON input 被重复 partial parse。
- streaming idle watchdog 与 HTTP request timeout 分离。
- non-streaming fallback 可处理代理/网络异常，但 streaming tool execution 已产生副作用时不能随意 fallback，避免工具重复执行。
- 默认 output cap 约 8K，命中截断再升级到更大 cap，以节省上下文槽位。

## 6. 上下文、状态与记忆

### Context management

资料将上下文处理拆成多层：

1. tool result budget。
2. snip compact。
3. microcompact。
4. context collapse。
5. auto-compact。

原则：

- 先轻量删除/截断，再重型 summary。
- auto-compact 需要 circuit breaker，避免 compact-fail-retry 无限烧 token。
- recoverable errors 先 withheld，不立即 yield 给 SDK consumer，避免消费者看到稍后可恢复的失败。

### State 双层架构

`ch03-state` 描述：

| 层 | 作用 |
|---|---|
| Bootstrap mutable singleton | cwd、session id、model config、cost、telemetry、prompt cache latch |
| UI reactive store | messages、input mode、tool approval、progress、tasks |

理由：

- 基础设施状态不应触发 React re-render。
- UI 状态需要 reactive。
- bootstrap state 需在 React 和插件之前可用。
- 通过 getter/setter 和 side-effect bridge 同步两层状态。

### Memory

`ch11-memory` 描述 file-based memory：

- Markdown 文件而非 vector DB。
- human-readable / human-editable / version-controllable。
- memory 类型：
  - user
  - feedback
  - project
  - reference
- `MEMORY.md` 是 always-loaded index。
- individual memory files 由 LLM side-query 按 relevance 选择。
- stale memory 会附带年龄提示，提醒模型验证当前代码。

## 7. 权限、沙箱与安全

资料列出七种 permission modes：

| Mode | 行为 |
|---|---|
| `bypassPermissions` | 全允许，内部/测试 |
| `dontAsk` | 不问用户，通常后台场景自动拒绝 prompt 类操作 |
| `auto` | 轻量 LLM classifier 判定 |
| `acceptEdits` | 文件编辑自动批准，其他 mutation 询问 |
| `default` | 标准交互批准 |
| `plan` | 只读 |
| `bubble` | subagent 权限上浮给 parent |

权限 resolution chain：

1. PreToolUse hook decision。
2. allow/deny/ask rules。
3. tool-specific check。
4. permission mode default。
5. interactive prompt。
6. auto classifier。

Hooks 系统：

- Skills 是 content/capability。
- Hooks 是 control flow/lifecycle。
- 重要事件：
  - PreToolUse
  - PostToolUse
  - Stop
  - SessionStart
  - UserPromptSubmit
  - SubagentStart/SubagentStop
  - PreCompact/PostCompact
- command hook 通过 exit code 表达：
  - 0 success
  - 2 blocking
  - other warning
- hooks config 在 trust boundary 后 snapshot，避免 TOCTOU：用户信任后仓库再改 hooks 不会自动生效。

安全重点：

- workspace trust boundary。
- hooks snapshot。
- permission bubble for subagents。
- MCP skills 不执行 inline shell。
- SSRF/DNS rebinding 类风险需在连接级验证。

资料并未把简单容器沙箱作为主要边界；更可迁移的是 permission、trust、hooks、transport validation 的分层边界。

## 8. Subagent、Fork agent 与 Task

### 普通 subagent

`runAgent` lifecycle 包含：

- model resolution
- agent ID
- context preparation
- read-only agents 的项目指令剥离
- permission isolation
- tool resolution
- system prompt
- abort controller isolation
- hook registration
- skill preloading
- MCP init
- subagent context creation
- query loop
- cleanup

关键点：

- sync agent 共享 parent abort controller。
- async agent 使用独立 abort controller。
- async agent 隔离 UI app state，但共享 task state channel。
- agent-specific hooks 最终 cleanup。
- `runAgent` 是 async generator，必须在 `finally` 中可靠清理资源。

### Fork agent

Fork agent 的核心是 prompt cache：

- fork child 继承 parent 已渲染 system prompt，不重新生成。
- 继承 parent exact tool array。
- 克隆 parent conversation history。
- 使用 parent model/thinking config。
- 每个 child 只在最后 directive 不同，从而最大化 byte-identical prefix。
- 保留 Agent tool 以维持 tool array 一致，但用 querySource/message tag 防递归 fork。

### Task coordination

task types 包括：

- `local_bash`
- `local_agent`
- `remote_agent`
- `in_process_teammate`
- `local_workflow`
- `monitor_mcp`
- `dream`

statuses 包括：

- `pending`
- `running`
- `completed`
- `failed`
- `killed`

background task 通过 output file、offset、notification、pending inbox 通信。foreground agent 可通过 race 机制中途转 background，不丢失 history。

## 9. MCP 与扩展

`ch15-mcp` 把 MCP 描述为 JSON-RPC 2.0 tool discovery/invocation protocol：

- MCP client 调 `tools/list` 获取 name/description/schema。
- 调 `tools/call` 执行。
- MCP tool wrap 成内部 Tool interface。
- 工具名规范化为 `mcp__{serverName}__{toolName}`。
- description 截断，避免 OpenAPI-generated server 把超长描述塞进上下文。
- MCP annotations：
  - `readOnlyHint`
  - `destructiveHint`
- 支持 stdio、HTTP、SSE、WebSocket/IDE/internal transports 等。
- OAuth 支持 PKCE、discovery、token refresh、error normalization。
- connection states 包括 connected、failed、needs-auth、pending、disabled。
- 本地 server 分批连接，远程 server 分批连接，避免资源耗尽。

风险：

- MCP server 可错误/恶意标注 destructive tool 为 read-only。
- 远程 MCP 涉及 OAuth、SSRF、timeout、session expiry。
- description/schema 是 prompt attack 面，需要截断和清洗。

## 10. 事件、日志与观测

资料更强调事件形态的控制流，而不是单一 logging subsystem：

- query loop yield messages/events，并 return typed terminal reasons。
- tool execution 通过共享 pipeline 暴露 progress、permission、denial、result、error 信息。
- background task 通过 output file、offset、notification、pending inbox 通信。
- streaming 有 idle watchdog，可以把 stream body 卡住与 request setup failure 分开观测。
- startup / performance 优化使用 profiling checkpoints。
- API usage 与 token count 尽量锚定 provider usage 数据。
- telemetry/profiling 用于验证 startup 与 context-performance 优化。

对 Marix 的可迁移点是：事件、终止原因、task notification 应该类型化、可审计，而不只是普通日志行。

## 11. 测试与验证

资料中提到的验证方式：

- Query loop 通过 narrow `QueryDeps` 注入 fake model、fake compactor、UUID generator。
- context/memory prompt 设计经过 eval 调整。
- startup profiling 有多个 checkpoints。
- performance 优化基于 telemetry/profiling。
- 网站自身称书稿由多组 AI agents 分阶段分析/写作/审阅，并做了避免原始源码残留的 audit。

可信度限制：

- 非官方。
- 基于早期 source maps，不代表当前 Claude Code 版本。
- 网站内容有叙事化与归纳，不是可执行源码。
- 本研究只总结架构模式，不复制或复现原始 Claude Code 源码。

## 12. 核心章节

建议后续引用章节：

- `https://claude-code-from-source.com/ch01-architecture/`
- `https://claude-code-from-source.com/ch02-bootstrap/`
- `https://claude-code-from-source.com/ch03-state/`
- `https://claude-code-from-source.com/ch04-api-layer/`
- `https://claude-code-from-source.com/ch05-agent-loop/`
- `https://claude-code-from-source.com/ch06-tools/`
- `https://claude-code-from-source.com/ch07-concurrency/`
- `https://claude-code-from-source.com/ch08-sub-agents/`
- `https://claude-code-from-source.com/ch09-fork-agents/`
- `https://claude-code-from-source.com/ch10-coordination/`
- `https://claude-code-from-source.com/ch11-memory/`
- `https://claude-code-from-source.com/ch12-extensibility/`
- `https://claude-code-from-source.com/ch15-mcp/`
- `https://claude-code-from-source.com/ch17-performance/`

## 13. 对 Marix 的借鉴

1. Agent loop 用 async generator，终止原因类型化。
2. 工具系统自描述：schema、权限、并发、安全、渲染、预算归工具定义。
3. 权限模式集中化，而不是散落在工具内部。
4. Recoverable error 先内部恢复，失败后再暴露给 consumer。
5. 上下文压缩分层，先轻后重，并加 circuit breaker。
6. Subagent 是同一个 query loop 的递归实例，而非特殊分支。
7. Fork agent 可把多 agent 并行与 prompt cache 优化结合。
8. Hooks 与 Skills 分离：内容扩展和控制流扩展不要混在一起。
9. MCP tool description/schema 必须截断、规范化、审计。
10. Memory 先用文件化、人类可编辑、LLM relevance selector，避免过早引入向量库。

## 14. 风险与反模式

- 不要把所有状态放入一个 reactive store；会造成 UI 和基础设施耦合。
- 不要把权限检查散落在各工具；会导致不一致。
- 不要无上限 retry/compact；会烧 API budget。
- 不要让动态 tool description 进入稳定 prompt prefix；会破坏 cache。
- 不要让 subagent 自批危险操作；权限应 bubble 或由 parent/user 决策。
- 不要将 memory 当事实数据库；memory 是可能过时的观察，需要 staleness cue。
- 不要信任 MCP annotations；它们是 server 声明，不是安全证明。
