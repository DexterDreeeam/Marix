# 主流 Agent 的模型消息编排

> 研究日期：2026-07-14
> 范围：只读研究；没有把第三方源码写入 Marix，也没有运行 Git。
> 证据原则：开源 Agent 固定到明确 commit；Claude Code 只陈述官方文档、官方 Agent SDK 和公开协议可证明的事实，不把第三方重建或推测当成官方实现。

这是横向比较文档。各系统细节、源码永久链接与两轮序列见专属笔记：[Pi](pi.cn.md)、[Codex CLI](codex-cli.cn.md)、[Gemini CLI](gemini-cli.cn.md)、[Claude Code](claude-code.cn.md)、[Cline](cline.cn.md)、[Aider](aider.cn.md)。

## 1. 结论先行

完成一个需要工具的 task，主流 Agent 通常维护三种不同的数据：

1. **持久化/内部记录**：用户输入、完整 assistant 输出、工具调用、工具结果、UI 事件、审计元数据。
2. **模型规范化上下文**：跨 provider 的 `user / assistant / tool-call / tool-result / reasoning` 语义。
3. **provider wire payload**：OpenAI `ResponseItem` 或 chat message、Anthropic content block、Gemini `Content/Part`。

这三层不可塌缩为单一 “Message” 类型。内部规范化 event/message 与 provider wire message/block/item 是不同词汇、不同分组规则；混用会产生关联与审计缺陷。

## 2. 研究对象与固定版本

| System | 官方来源 | 固定版本/证据边界 |
|---|---|---|
| **Pi** | [`earendil-works/pi`](https://github.com/earendil-works/pi)（原 `badlogic/pi-mono`） | [`0e6909f050eeb15e8f6c05185511f3788357ddb3`](https://github.com/earendil-works/pi/commit/0e6909f050eeb15e8f6c05185511f3788357ddb3) |
| **OpenAI Codex CLI** | [`openai/codex`](https://github.com/openai/codex) | [`393f64565ab46f09d99ca4d9bd973537e72a114b`](https://github.com/openai/codex/commit/393f64565ab46f09d99ca4d9bd973537e72a114b) |
| **Google Gemini CLI** | [`google-gemini/gemini-cli`](https://github.com/google-gemini/gemini-cli) | [`fa975395bcc6b609e44735e47320e54f51535d47`](https://github.com/google-gemini/gemini-cli/commit/fa975395bcc6b609e44735e47320e54f51535d47) |
| **Anthropic Claude Code** | [Claude Code docs](https://code.claude.com/docs/en/overview)、[Agent SDK](https://platform.claude.com/docs/en/agent-sdk/overview)、[Messages API](https://platform.claude.com/docs/en/api/messages/create) | 完整 native agent loop 与默认 system prompt 未开源。公开 Python SDK 固定到 [`059d3449bfc2e0dd64230bde65282df93dd21b8d`](https://github.com/anthropics/claude-agent-sdk-python/tree/059d3449bfc2e0dd64230bde65282df93dd21b8d)，只作为 SDK→CLI 和公开 message 类型的证据 |
| **Cline** | [`cline/cline`](https://github.com/cline/cline) | [`ab68fd7f34e563c82d223592fbf61c74cfd8804e`](https://github.com/cline/cline/commit/ab68fd7f34e563c82d223592fbf61c74cfd8804e) |
| **Aider** | [`Aider-AI/aider`](https://github.com/Aider-AI/aider) | [`5dc9490bb35f9729ef2c95d00a19ccd30c26339c`](https://github.com/Aider-AI/aider/commit/5dc9490bb35f9729ef2c95d00a19ccd30c26339c) |

## 3. 横向比较

| 维度 | Pi | Codex CLI | Gemini CLI | Claude Code | Cline | Aider |
|---|---|---|---|---|---|---|
| Canonical model history | `Message[]`：user/assistant/toolResult | `ResponseItem[]` | curated `Content[]` | 私有；SDK 暴露 transcript/message stream | runtime `AgentMessage[]`，持久化另有 `MessageWithMetadata[]` | `done_messages + cur_messages` |
| System prompt | 独立 `systemPrompt`，每次逻辑调用携带 | `instructions`，developer context 另作 input item | `config.systemInstruction` | API 顶层 `system`；CLI 完整内容不公开 | 独立 `systemPrompt` | 普通 system message；部分模型降级为 user+ack |
| 状态模型 | 默认全量重发；Codex WS adapter 有增量特例 | HTTP 全量；WS 可 `previous_response_id` 增量 | 本地 chat 管历史，每次发 curated 全量 | Messages API 无状态；SDK session resume 由 CLI transcript 重建 | 每次从完整 runtime/store 历史发出 | 每次重建并全量发送 |
| Native tools | 是 | 是，Responses tools | 是，Gemini function declarations | 是，公开 API/SDK 协议可证 | 是，AI SDK adapters | 标准编辑路径否；主要为文本 edit format |
| 多工具 | 默认并行，可声明串行 | 模型可多 call；宿主按工具并行能力读写锁执行 | Scheduler 默认并行，变更类工具可强制顺序 | API 支持多 block；SDK 文档称只读工具可并行 | 默认串行，可配置并行 | 不适用标准路径 |
| 内部 result | 每个 call 独立 result message | 每个 call 独立 output item | UI result 与 `functionResponse` 分层 | SDK stream 有 user/tool-result 语义；私有内部未知 | runtime `role:"tool"`；持久化为 user block | 本地编辑结果/反思文本，不是 tool result |
| Thinking | provider signature 按兼容性保留/降级 | reasoning item 和 encrypted content 可重放 | thought UI 文本通常去除，signature 保留 | thinking block 协议可证；CLI 私有处理未知 | 统一 reasoning part 再适配 | 可读取 reasoning 字段，编辑主要看 content |
| Context 处理 | summary + kept boundary；显式 compaction entry | local/remote v1/v2/token-budget | ContextManager 或摘要服务 | 自动 `/compact` 可证，私有算法未知 | canonical transcript + API 投影/compaction | 摘要旧 `done_messages`，repo map 单独预算 |
| Subagent | core 无统一实现；官方 extension 示例起独立进程 | 独立 child thread，可选 fork 父历史 | 独立 `GeminiChat`、工具集与完成协议 | fresh conversation；最终结果返回 parent | `spawn_agent` 创建独立 session，结果回父工具 | architect→editor 顺序组合，不是通用 child runtime |

## 4. 一个 task 内的角色划分

在单个 task 内，Agent 这样划分角色：

- **System/developer**：身份、工具指南、环境和策略。Pi、Cline 携带独立 `systemPrompt`；Codex 发送顶层 `instructions`，并把 developer/权限/skills 作为独立 input item；Gemini 用 `config.systemInstruction`；Claude Code 用 API 顶层 `system`；Aider 发 system message（对禁用 system role 的模型降级为 user+ack）。
- **User**：task，加上注入的环境/session context（Gemini 的 `<session_context>`、Codex developer item、Cline 带文件/图片的 task）。
- **Assistant**：文本、可选 reasoning/thinking 和一个或多个工具调用的有序 block/item。
- **Tool**：结果关联回发起它的 call。

## 5. 一个工具调用是不是一条 message？

没有统一的跨 provider 答案：

- **OpenAI Responses**：每个 function call 和每个 output 通常是独立 **item**，不是 chat message。
- **OpenAI Chat Completions**：多个调用可同处一条 assistant message 的 `tool_calls[]`；每个结果通常各是一条 `role:"tool"` message。
- **Anthropic Messages**：多个 `tool_use` block 可同处一条 assistant message；对应的 `tool_result` block 通常合并到下一条 user message。
- **Gemini**：多个 `functionCall` part 可同处一个 model `Content`；对应的 `functionResponse` part 通常合并到下一条 user `Content`。
- **内部格式与以上都不同。** Pi 每个 call 存一个 `ToolResultMessage`；Cline 存 runtime `role:"tool"` 结果并持久化为 user block。内部逐 call 记录与 provider envelope 是不同层。

## 6. Provider wire 速查

假设 assistant 一次请求两个工具 A/B：

```text
OpenAI Responses
  input:
    message(user)
    reasoning?
    function_call(call_id=A)
    function_call(call_id=B)
    function_call_output(call_id=A)
    function_call_output(call_id=B)

OpenAI Chat Completions
  messages:
    user
    assistant {tool_calls:[A,B]}
    tool {tool_call_id:A}
    tool {tool_call_id:B}

Anthropic Messages
  system: ...
  messages:
    user
    assistant [thinking?, tool_use{id:A}, tool_use{id:B}]
    user [tool_result{tool_use_id:A}, tool_result{tool_use_id:B}]

Gemini GenerateContent
  systemInstruction: ...
  contents:
    user [...]
    model [functionCall{id:A}, functionCall{id:B}]
    user [functionResponse{id:A}, functionResponse{id:B}]
```

关键聚合差异：Responses 把 call 和 output 保持为扁平并列的 **item**；Chat Completions 把 call 聚在一条 assistant message，却把每个 output 拆成独立 `tool` message；Anthropic 与 Gemini 把 call 聚为一个 assistant/model turn，并把全部结果聚到下一条 user turn 的 **content block/part**。

## 7. 每轮重发与 continuation

下一次模型请求必须同时保留**原 assistant tool call**及其结果。只回传结果会破坏 call/result 关联，也可能丢失 provider reasoning signature。

- Pi、Gemini CLI、Cline、Aider、Codex HTTP 与无状态 Claude Messages API 默认全量重放。
- continuation 是优化而非事实源：Codex 的 WebSocket `previous_response_id` 仅在严格前缀匹配时发增量，否则回退全量。prompt cache（`prompt_cache_key`、ephemeral cache control）是第三种独立机制，不改变逻辑 history。

## 8. Reasoning 与 thought signature

- Pi 只对兼容 provider/API/model 保留 thinking/signature，跨模型可降为 text。
- Codex 用 `reasoning.encrypted_content` 重放 reasoning item。
- Gemini 通常从 history 去除 UI `thought` 文本，但在 call part 上原样重放 opaque `thoughtSignature`。
- Claude Code thinking block 协议公开，native 处理私有。

signature 是 opaque 的 provider 范围状态，跨 provider 盲目重放不安全。

## 9. Context 压缩

压缩会重建或替换逻辑 history，与缓存、continuation 不同：

- Pi：摘要旧区并记录显式 compaction summary 加 kept boundary（默认触发 `contextTokens > contextWindow - 16384`，保留约 20k）。
- Codex：token-budget、local summary、remote v1、remote v2 四种路径。
- Gemini：`ContextManager` 图式 GC/distillation，或旧 `ChatCompressionService`（约 50% 触发，保留约 30%，拒绝不变短的摘要）。
- Cline：API 投影截断加可选 compaction，保存 “摘要投影 + canonical tail”，不删除原始 transcript。
- Claude Code：文档化自动 `/compact`；算法私有。
- Aider：摘要较旧 `done_messages`；repo map 有独立预算。

## 10. Subagent 返回

subagent 只向上返回最终结果：

- Pi 的 extension subagent 起独立进程、不带父历史，返回摘要作为 tool result。
- Codex 的 `spawn_agent` 给 child 独立 thread，可选 fork 过滤后的父 rollout。
- Gemini 本地 subagent 有独立 registry，必须调用 `complete_task`，返回最终值。
- Claude Code subagent 是 fresh conversation，中间历史不进入 parent。
- Cline 的 `spawn_agent` 创建 child `SessionRuntime`，parent 以 tool result 接收最终值。
- Aider 的 architect→editor 是顺序组合，不是通用 child runtime。

默认把全部父历史复制进 child 会造成泄露、token 膨胀和权限漂移。

## 11. 可供 Marix 借鉴的统一抽象

这部分是研究建议，不是实现变更。

```text
ConversationItem
  id
  run_id / turn_id
  origin = user | assistant | host | system
  visibility = model | host | ui | audit
  payload:
    UserContent | AssistantText | ReasoningItem
    | ToolCall | ToolResult | ContextCompaction | SubagentBoundary

ToolCall
  call_id
  name
  arguments
  provider_metadata        # item id, thought/reasoning signature 等
  parallel_safety

ToolResult
  call_id
  model_content            # 文本/图像/结构化内容
  host_details             # UI、diff、exit code、审计；默认不发模型
  is_error

ContextCompaction
  source_range / kept_boundary
  summary
  token_estimate
  compactor_model
  original_items_retained_for_audit
```

建议的模块边界：

1. **Immutable transcript/event log**：保存原始输入输出、call id、provider metadata 和失败。
2. **Context projector**：从 transcript 生成当前模型可见 history；负责过滤 UI 事件、tool output 截断、summary 和 subagent 结果投影。
3. **Provider adapter**：只做 wire 映射、schema 兼容、signature/call-id 保真、stream delta 组装。
4. **Tool scheduler**：并行能力、权限、取消、超时、稳定结果排序。
5. **Continuation state**：`previous_response_id`/connection cache 只作为可失效优化；本地 history 仍可完整重放。
6. **Compaction ledger**：把压缩记录成显式 item；不要静默破坏审计 history。
7. **Reasoning policy**：可见 thinking 文本、不可见/加密 reasoning、opaque signature 分开保存；跨 provider 不可假设可互换。
8. **Subagent run**：独立 run/history/tool policy/budget；parent 只接收明确状态与最终 result，除非显式 fork。

## 12. 风险与反模式

- 用一种 “Message” 类型同时承载 wire、UI、持久化和审计。
- 只保存 tool result，不保存产生它的 assistant tool call。
- 并行工具按完成顺序回填，却没有稳定 call id。
- 把 host `details` 或 output schema 误认为能改善模型的工具选择；模型选择主要依赖名称、描述和输入 schema。
- 把 prompt cache 命中、provider response continuation 和真正 history 压缩当成同一件事。
- 跨 provider 盲目重放 thinking/signature。
- 把 Claude Code 闭源细节或第三方重建写成官方事实。
- 为 subagent 默认复制全部父历史，造成泄露、token 膨胀和权限漂移。
