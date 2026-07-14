# Google Gemini CLI 研究

> 研究日期：2026-07-14
> 关注点：Gemini CLI 如何编排 `Content`/`Part` history、function response 与 thought signature。
> 只读研究：未把 Gemini CLI 源码写入 Marix，也未运行 Git。

## 1. 来源与固定版本

| 项目 | 详情 |
|---|---|
| 仓库 | [`google-gemini/gemini-cli`](https://github.com/google-gemini/gemini-cli) |
| 固定 commit | [`fa975395bcc6b609e44735e47320e54f51535d47`](https://github.com/google-gemini/gemini-cli/commit/fa975395bcc6b609e44735e47320e54f51535d47) |
| 主要语言 | TypeScript |
| 定位 | Gemini `GenerateContent` agent loop 的第一方开源实现 |
| 关注理由 | `Content`/`Part` 编排、function response、thought signature、curated 与 comprehensive history、context 投影、并行调度、隔离的本地 subagent |

以下源码链接均使用 `/blob/fa975395bcc6b609e44735e47320e54f51535d47/...#Lx-Ly` 永久链接。

## 2. 核心模块与执行流

Gemini CLI 的真实 wire 为：

```text
model
contents: Content[]
config:
  systemInstruction
  tools: [{functionDeclarations}]
```

见 [`geminiChat.ts#L757-L863`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/core/geminiChat.ts#L757-L863)。

`LegacyAgentSession.send()` 把用户 `ContentPart[]` 转成 Gemini `Part[]` 并启动循环。CLI 使用自己的 `GeminiChat`，不依赖 provider 端 session ID；每次手工调用 `generateContentStream()` 并发送 curated `Content[]`，同时维护 comprehensive 与 curated 两套 history。见 [`legacy-agent-session.ts#L108-L153`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/agent/legacy-agent-session.ts#L108-L153)、[`geminiChat.ts#L921-L959`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/core/geminiChat.ts#L921-L959)。

## 3. System prompt 与重发行为

system instruction 是 `config.systemInstruction`，独立于 history；history role 只有 `user/model`。system instruction 随每次 `generateContentStream()` 调用提供，而不是嵌入 `contents`。

## 4. 初始用户 task

CLI 先注入一个 user `<session_context>`，包含日期、OS、目录状态和 session memory，再把真实 task 作为 user `Content` 追加。见 [`environmentContext.ts#L50-L110`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/utils/environmentContext.ts#L50-L110)。

## 5. Assistant 文本、reasoning 与工具调用

一次 model `Content` 可含多个 `functionCall` part 以及文本。`thought:true` 的文本可流向 UI 但通常不留下一轮 history；不透明 `thoughtSignature` 随 function call part 保留并重放。见 [`geminiChat.ts#L1111-L1201`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/core/geminiChat.ts#L1111-L1201)。官方要求原样回传模型给出的 signature；CLI 的 synthetic signature 修复是防御性兼容，不应复制为通用做法。

## 6. Native 工具声明

工具是 Gemini `functionDeclarations`，通过 `config.tools` 提供。把 host 执行转换为模型结果的标准构造见 [`generateContentResponseUtilities.ts#L20-L89`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/utils/generateContentResponseUtilities.ts#L20-L89)。

## 7. 并行/串行工具策略

Scheduler 默认并行执行连续可并行 calls；编辑、topic update 或 `wait_for_previous` 可强制顺序。见 [`scheduler.ts#L463-L568`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/scheduler/scheduler.ts#L463-L568)。

## 8. 工具结果与关联

所有 result 组成下一轮一个 user `Content` 中的多个 `functionResponse` part，以 `id + name` 关联。UI/session 的 `tool_response` 与模型的 `functionResponse` 是两套数据，UI 呈现不会泄入模型 history。

## 9. 下一次请求携带的 history

下一次请求的 `contents` 为先前 user contents，然后是 model `Content`（function call 加 signature），再加一个聚合全部 `functionResponse` 的 user `Content`。CLI 发送自己 curated 的 `Content[]`，而不是服务端 thread 引用。

## 10. Context 压缩、截断、continuation 与缓存

新 `ContextManager` 可对 durable history 做图式 GC、distillation、normalization 并输出 API 投影；旧 `ChatCompressionService` 在超过约 50% token limit 时摘要旧区、保留最近约 30%，并验证摘要确实变短。见 [`contextManager.ts#L90-L265`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/context/contextManager.ts#L90-L265)、[`chatCompressionService.ts#L263-L480`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/context/chatCompressionService.ts#L263-L480)。

## 11. Subagent

subagent 建立独立 Tool/Prompt/Resource registry 和独立 `GeminiChat`，禁止递归 Agent 工具，强制调用 `complete_task`，有独立 timeout/max turns；其活动映射为隔离 thread，最终结果才进入 parent。见 [`local-executor.ts#L159-L277`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/agents/local-executor.ts#L159-L277)、[`#L1054-L1087`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/agents/local-executor.ts#L1054-L1087)。

## 12. Provider adapter

Gemini CLI 直接面向 Gemini `GenerateContent` 协议；该路径没有到 OpenAI/Anthropic message 形状的跨 provider 转换。相关抽象差异是 curated 与 comprehensive history、以及 `ContextManager` 与 `ChatCompressionService` 两种 context 策略，而不是多厂商 wire 映射。

## 13. 两轮模型序列

```text
Request 1:
  systemInstruction SI
  contents [
    user(session_context),
    user(U1)
  ]
  tools functionDeclarations

Response model Content:
  [thought text? UI-only,
   functionCall A + thoughtSignature,
   functionCall B]

Host:
  execute A/B
  user Content [
    functionResponse(id=A.id,name=A.name,response=RA),
    functionResponse(id=B.id,name=B.name,response=RB)
  ]

Request 2 contents:
  previous user contents
  + model Content(function calls/signature)
  + user Content(function responses)

Response:
  model Content[final text]
```

官方协议：[GenerateContent](https://ai.google.dev/api/generate-content)、[function calling](https://ai.google.dev/gemini-api/docs/generate-content/function-calling)、[thought signatures](https://ai.google.dev/gemini-api/docs/generate-content/thought-signatures)。

## 14. 证据限制与 Marix 借鉴

证据限制：本文只反映一个 commit。旧的压缩服务与新的 `ContextManager` 并存，默认路径可能变动。synthetic signature 修复是防御性代码，不应作为通用策略复制。

Marix 借鉴：

1. 让模型 `functionResponse` 与 UI/session tool response 分离，避免呈现污染模型 history。
2. 在准确的 call part 上携带 opaque thought signature 并原样重放，不要重新生成。
3. 把环境/session context 作为显式前置 user turn 注入，而非藏在 system prompt，保持 history 可审计。
4. 用独立 registry、完成协议和 turn/时间预算隔离 subagent，只把最终结果向上返回。
5. 同时建模 “curated”（模型可见）与 “comprehensive”（完整）两套 history，而非单一扁平列表。
