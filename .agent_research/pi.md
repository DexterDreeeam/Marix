# Pi Coding Agent Research

> Research date: 2026-07-14
> Focus: how Pi builds, stores, and resends model messages across providers.
> Read-only study: no Pi source was copied into Marix and no Git command was run.

## 1. Source and pinned version

| Item | Details |
|---|---|
| Repository | [`earendil-works/pi`](https://github.com/earendil-works/pi) (formerly [`badlogic/pi-mono`](https://github.com/badlogic/pi-mono)) |
| Pinned commit | [`0e6909f050eeb15e8f6c05185511f3788357ddb3`](https://github.com/earendil-works/pi/commit/0e6909f050eeb15e8f6c05185511f3788357ddb3) |
| Main language | TypeScript (monorepo: `packages/ai`, `packages/agent`, `packages/coding-agent`) |
| Role | Compact, readable coding-agent stack with an explicit normalized message layer and multiple provider adapters |
| Why watch | Clear three-layer message model, parallel tool execution, explicit compaction records, extension-based subagent example |

All source links below use `/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/...#Lx-Ly` permalinks.

## 2. Core modules and execution flow

Pi makes its three message layers explicit:

```text
AgentMessage / session entry
  → context hook + convertToLlm()
  → pi-ai Message/Context
  → provider adapter
  → OpenAI / Anthropic / Gemini wire
```

Core types are in [`packages/ai/src/types.ts#L323-L450`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/ai/src/types.ts#L323-L450); the loop and LLM-context assembly are in [`agent-loop.ts#L155-L374`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/agent/src/agent-loop.ts#L155-L374).

1. `AgentSession.prompt()` runs extension commands, the input hook, skill/template expansion, and preemptive compaction.
2. The task becomes a `role:"user"` message with text/image blocks.
3. `before_agent_start` may inject messages or temporarily replace the system prompt.
4. Every model step projects the current context through `convertToLlm()` and supplies the complete logical history.
5. The streaming assistant temporarily occupies a partial state and is replaced with complete text/thinking/tool-call blocks.
6. Calls execute and produce independent `ToolResultMessage` values carrying the same `toolCallId`.
7. The next request includes the original user item, the complete assistant tool-call turn, and every result.

Initial input and prompt overrides are in [`agent-session.ts#L1076-L1223`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/coding-agent/src/core/agent-session.ts#L1076-L1223).

## 3. System prompt and resend behavior

`Context` has `systemPrompt`, `messages`, and `tools`, but no separate developer-prompt field. The coding-agent prompt includes identity, tool guidance, project/AGENTS context, skills, date, and cwd. A custom prompt replaces the default body while appended context can still be added. See [`system-prompt.ts#L28-L172`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/coding-agent/src/core/system-prompt.ts#L28-L172).

The system prompt is a separate field carried on every logical model call, not an item mixed into `messages`.

## 4. Initial user task

The first task is a `role:"user"` `Message` whose content is a list of text/image blocks. Extension commands, the input hook, and skill/template expansion can transform this input before it enters history.

## 5. Assistant text, reasoning, and tool calls

A completed assistant turn is one `Message` holding ordered blocks: optional `thinking` (with a provider signature when compatible), optional `text`, and one or more tool calls. During streaming these deltas accumulate as a partial state and are only committed once the stream completes.

## 6. Native tool declaration

Tools use provider-native JSON Schema and live on `Context.tools`. A host result's `content` is model-visible, while `details` is for host/UI use; `details` is not a model-facing output schema and does not drive tool selection.

## 7. Parallel vs sequential tool policy

`toolExecution="parallel"` is the default. A global sequential policy, or any sequential tool in the batch, serializes the entire batch. Allowed executions use `Promise.all`; completion events may follow actual completion order, but result messages are recorded in original call order. See [`agent-loop.ts#L413-L550`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/agent/src/agent-loop.ts#L413-L550).

## 8. Tool results and correlation

Each call produces one independent `ToolResultMessage` carrying the same `toolCallId` as the originating call. Correlation is therefore per-call inside Pi's normalized layer; grouping into provider envelopes happens later in the adapters.

## 9. History sent in the next request

Every step re-projects the full logical history through `convertToLlm()`: original user item, the complete assistant tool-call turn (including thinking/signature when compatible), and every `ToolResultMessage`. Returning only results is never the default.

## 10. Context compaction, truncation, continuation, caching

Compaction normally triggers when `contextTokens > contextWindow - 16384`, tries to retain roughly 20k recent tokens, splits at legal turn boundaries, summarizes the older region, and records an explicit compaction summary plus kept boundary. See [`compaction.ts#L100-L212`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/coding-agent/src/core/compaction/compaction.ts#L100-L212) and [`#L282-L608`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/coding-agent/src/core/compaction/compaction.ts#L282-L608).

Caching is not compaction. The OpenAI adapter can send a session-derived `prompt_cache_key`; the Anthropic adapter places ephemeral cache controls at system, tool, and final-user boundaries; ordinary Gemini adaptation does not thereby alter logical history. Even on a cache hit, the default model semantics remain the complete current context.

## 11. Subagents

Pi core has no unified subagent runtime. The official extension example exposes a normal `subagent` tool, starts a separate `pi --mode json -p --no-session` process without inheriting parent history, and returns its final summary as a parent tool result. See [`subagent/index.ts#L267-L371`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/coding-agent/examples/extensions/subagent/index.ts#L267-L371).

## 12. Provider adapters

- OpenAI Responses: complete `input`, `store:false`, normally no `previous_response_id`; calls and outputs are independent items. [`openai-responses.ts#L232-L287`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/ai/src/api/openai-responses.ts#L232-L287)
- OpenAI Chat: one assistant message can contain `tool_calls[]`; each result is a separate tool message. [`openai-completions.ts#L854-L1114`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/ai/src/api/openai-completions.ts#L854-L1114)
- Anthropic: top-level system; consecutive internal results become several `tool_result` blocks in one user message. [`anthropic-messages.ts#L919-L1284`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/ai/src/api/anthropic-messages.ts#L919-L1284)
- Gemini: `systemInstruction`, model `functionCall`, user `functionResponse`; consecutive results are grouped into one `Content`. [`google-shared.ts#L69-L287`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/ai/src/api/google-shared.ts#L69-L287)

The shared transformation retains provider-specific thinking/signatures only for compatible provider/API/model combinations. Cross-model thinking can become text, and missing tool results are repaired with error outputs. See [`transform-messages.ts#L63-L222`](https://github.com/earendil-works/pi/blob/0e6909f050eeb15e8f6c05185511f3788357ddb3/packages/ai/src/api/transform-messages.ts#L63-L222).

## 13. Two-step model sequence

```text
Request 1 logical context:
  systemPrompt S
  messages [user U1]
  tools [read, grep]

Assistant A1:
  [thinking/signature?, text, toolCall(tc1), toolCall(tc2)]

Host:
  tc1/tc2 run in parallel by default
  R1=ToolResultMessage(tc1)
  R2=ToolResultMessage(tc2)

Request 2:
  S + [U1, A1, R1, R2] + tools

Assistant A2:
  [final text]
```

## 14. Evidence limitations and Marix takeaways

Evidence limitations: findings are pinned to one commit of a fast-moving repository. Line ranges and default thresholds (for example the 16384-token compaction margin and the ~20k retention target) can change upstream; re-pin before reuse.

Marix takeaways:

1. Keep a provider-neutral normalized `Message` layer and push provider grouping (Chat tool message vs Anthropic/Gemini content blocks) entirely into adapters.
2. Store one result per call with a stable `toolCallId`; let the adapter decide envelope grouping.
3. Record compaction as an explicit item with a kept boundary, not a silent history rewrite.
4. Treat `prompt_cache_key` and ephemeral cache controls as optimizations layered on top of complete replayable history.
5. Keep signatures/thinking replay compatibility-gated per provider/model rather than assumed interchangeable.
