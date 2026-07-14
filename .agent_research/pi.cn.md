# Pi Coding Agent 研究

> 研究日期：2026-07-14
> 关注点：Pi 如何跨 provider 构造、保存并重发模型消息。
> 只读研究：未把 Pi 源码写入 Marix，也未运行 Git。

## 1. 来源与固定版本

| 项目 | 详情 |
|---|---|
| 仓库 | [`earendil-works/pi`](https://github.com/earendil-works/pi)（原 [`badlogic/pi-mono`](https://github.com/badlogic/pi-mono)） |
| 固定 commit | [`0e6909f050eeb15e8f6c05185511f3788357ddb3`](https://github.com/earendil-works/pi/commit/0e6909f050eeb15e8f6c05185511f3788357ddb3) |
| 主要语言 | TypeScript（monorepo：`packages/ai`、`packages/agent`、`packages/coding-agent`） |
| 定位 | 精简可读的 coding agent 栈，具备显式规范化消息层与多个 provider adapter |
| 关注理由 | 清晰的三层消息模型、并行工具执行、显式 compaction 记录、基于 extension 的 subagent 示例 |

以下源码链接均使用 `/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/...#Lx-Ly` 永久链接。

## 2. 核心模块与执行流

Pi 明确分三层：

```text
AgentMessage / session entry
  → context hook + convertToLlm()
  → pi-ai Message/Context
  → provider adapter
  → OpenAI / Anthropic / Gemini wire
```

核心类型在 [`packages/ai/src/types.ts#L323-L450`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/ai/src/types.ts#L323-L450)；主循环及请求上下文构造在 [`agent-loop.ts#L155-L374`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/agent/src/agent-loop.ts#L155-L374)。

1. `AgentSession.prompt()` 处理 extension command、input hook、skill/template 展开和预压缩。
2. 初始 task 成为 `role:"user"` message，content 为 text/image blocks。
3. `before_agent_start` 可注入消息或临时替换 system prompt。
4. 每次模型调用把当前 context 通过 `convertToLlm()` 投影为完整模型 history。
5. 流式 assistant 先作为 partial state，结束后保存完整 text/thinking/toolCall blocks。
6. 执行所有 calls，产生带同一 `toolCallId` 的独立 `ToolResultMessage`。
7. 下一次模型调用包含原 user、完整 assistant tool-call turn 以及全部 results。

初始输入与 override 见 [`agent-session.ts#L1076-L1223`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/coding-agent/src/core/agent-session.ts#L1076-L1223)。

## 3. System prompt 与重发行为

`Context` 只有 `systemPrompt`、`messages`、`tools`，没有单独 developer prompt 字段。coding agent 的 system prompt 包含身份、工具指南、项目 context/AGENTS、skills、日期和 cwd；custom system 可替换默认正文，但附加 context 仍可加入。见 [`system-prompt.ts#L28-L172`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/coding-agent/src/core/system-prompt.ts#L28-L172)。

system prompt 是每次逻辑模型调用都携带的独立字段，而不是混入 `messages` 的 item。

## 4. 初始用户 task

首个 task 是 `role:"user"` 的 `Message`，content 为 text/image blocks 列表。extension command、input hook 和 skill/template 展开可在其进入 history 前改写输入。

## 5. Assistant 文本、reasoning 与工具调用

一个完成的 assistant turn 是一条 `Message`，含有序 blocks：可选 `thinking`（兼容时带 provider signature）、可选 `text`、以及一个或多个工具调用。流式期间这些 delta 作为 partial state 累积，流结束后才提交。

## 6. Native 工具声明

工具使用 provider-native JSON Schema，位于 `Context.tools`。内部 tool result 的 `content` 给模型，`details` 给 host/UI；`details` 不是 provider-facing output schema，也不影响工具选择。

## 7. 并行/串行工具策略

默认 `toolExecution="parallel"`；只要全局或 batch 中任一工具要求 sequential，整个 batch 串行。并行执行用 `Promise.all`，完成事件可按实际完成时间发出，但最终 results 按原 call 顺序写入 history。见 [`agent-loop.ts#L413-L550`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/agent/src/agent-loop.ts#L413-L550)。

## 8. 工具结果与关联

每个 call 产生一个独立 `ToolResultMessage`，携带与发起 call 相同的 `toolCallId`。因此 Pi 规范化层内的关联是逐 call 的；合并为 provider envelope 发生在稍后的 adapter。

## 9. 下一次请求携带的 history

每步都通过 `convertToLlm()` 重新投影完整逻辑 history：原 user item、完整 assistant tool-call turn（兼容时含 thinking/signature）以及全部 `ToolResultMessage`。只回传结果绝不是默认行为。

## 10. Context 压缩、截断、continuation 与缓存

默认在 `contextTokens > contextWindow - 16384` 时触发压缩，尽量保留最近约 20k tokens，在合法 turn 边界切分，把旧区摘要成显式 compaction summary，并保留 kept boundary。见 [`compaction.ts#L100-L212`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/coding-agent/src/core/compaction/compaction.ts#L100-L212)、[`#L282-L608`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/coding-agent/src/core/compaction/compaction.ts#L282-L608)。

缓存不等于压缩：OpenAI adapter 可发送 session 派生的 `prompt_cache_key`；Anthropic adapter 在 system、工具和最后 user 边界放置 ephemeral cache control；普通 Gemini adapter 不因此改变逻辑 history。即使缓存命中，默认模型语义仍是完整当前 context。

## 11. Subagent

Pi core 没有统一 subagent runtime。官方 extension 示例把 `subagent` 作为普通工具，启动独立 `pi --mode json -p --no-session` 进程，不继承父 history，最终摘要作为父工具结果返回。见 [`subagent/index.ts#L267-L371`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/coding-agent/examples/extensions/subagent/index.ts#L267-L371)。

## 12. Provider adapter

- OpenAI Responses：完整 `input`、`store:false`，通常无 `previous_response_id`；call/output 为独立 items。见 [`openai-responses.ts#L232-L287`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/ai/src/api/openai-responses.ts#L232-L287)。
- OpenAI Chat：一个 assistant message 含 `tool_calls[]`，每个结果为独立 tool message。见 [`openai-completions.ts#L854-L1114`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/ai/src/api/openai-completions.ts#L854-L1114)。
- Anthropic：顶层 system；多个连续内部 result 合并为一个 user message 中的多个 `tool_result` blocks。见 [`anthropic-messages.ts#L919-L1284`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/ai/src/api/anthropic-messages.ts#L919-L1284)。
- Gemini：`systemInstruction`，model `functionCall`，user `functionResponse`；连续结果合并为一个 `Content`。见 [`google-shared.ts#L69-L287`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/ai/src/api/google-shared.ts#L69-L287)。

公共转换只原样保留同 provider/API/model 可重放的 thinking signature；跨模型时可降为普通 text，并会为缺失结果补错误结果。见 [`transform-messages.ts#L63-L222`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/ai/src/api/transform-messages.ts#L63-L222)。

## 13. 两轮模型序列

```text
Request 1 logical context:
  systemPrompt S
  messages [user U1]
  tools [read, grep]

Assistant A1:
  [thinking/signature?, text, toolCall(tc1), toolCall(tc2)]

Host:
  tc1/tc2 默认并行
  R1=ToolResultMessage(tc1)
  R2=ToolResultMessage(tc2)

Request 2:
  S + [U1, A1, R1, R2] + tools

Assistant A2:
  [final text]
```

## 14. 证据限制与 Marix 借鉴

证据限制：结论固定在快速演进仓库的单个 commit。行号范围与默认阈值（例如 16384 token 的压缩余量、约 20k 的保留目标）可能上游变动；复用前请重新固定。

Marix 借鉴：

1. 保留 provider 中立的规范化 `Message` 层，把 provider 分组（Chat tool message 与 Anthropic/Gemini content block）完全下沉到 adapter。
2. 每个 call 存一个带稳定 `toolCallId` 的结果；由 adapter 决定 envelope 分组。
3. 把压缩记录为带 kept boundary 的显式 item，而非静默改写 history。
4. 把 `prompt_cache_key` 与 ephemeral cache control 当作叠加在完整可重放 history 之上的优化。
5. signature/thinking 重放按 provider/model 做兼容性门控，不假设可互换。
