# OpenAI Codex CLI Research

> Research date: 2026-07-14
> Focus: how Codex CLI represents model history, resends context, and schedules tools on the OpenAI Responses wire.
> Read-only study: no Codex source was copied into Marix and no Git command was run.

## 1. Source and pinned version

| Item | Details |
|---|---|
| Repository | [`openai/codex`](https://github.com/openai/codex) |
| Pinned commit | [`393f64565ab46f09d99ca4d9bd973537e72a114b`](https://github.com/openai/codex/commit/393f64565ab46f09d99ca4d9bd973537e72a114b) |
| Main language | Rust (`codex-rs` workspace) |
| Role | First-party CLI coding agent built directly on the OpenAI Responses API |
| Why watch | Responses `ResponseItem` history, encrypted reasoning replay, HTTP full-history vs WebSocket continuation, multiple compaction paths, ordered parallel tools, independent child threads |

All source links below use `/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/...#Lx-Ly` permalinks.

## 2. Core modules and execution flow

Codex distinguishes four representations:

- `RolloutItem`: persistent JSONL envelope around events, metadata, compaction checkpoints, and `ResponseItem`; not sent wholesale.
- `ResponseItem`: model history, shaped as a Responses-wire union.
- `ResponseInputItem`: the narrower user/host-output input type.
- `TurnItem`: TUI/app-server semantic event.

See [`models.rs#L805-L1080`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/protocol/src/models.rs#L805-L1080) and [`#L1613-L1667`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/protocol/src/models.rs#L1613-L1667).

Each step runs `clone_history().for_prompt()` and constructs `Prompt { input, tools, parallel_tool_calls, base_instructions }`. Reasoning, assistant messages, function/custom calls, and outputs remain in history. See [`turn.rs#L271-L295`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/session/turn.rs#L271-L295) and [`history.rs#L121-L144`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/context_manager/history.rs#L121-L144).

## 3. System prompt and resend behavior

Base system/model instructions are stored separately as `BaseInstructions` and sent as `instructions`. Permissions, developer instructions, skills, plugins, and world state can become developer/user input items. See [`client_common.rs#L16-L36`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/client_common.rs#L16-L36) and [`session/mod.rs#L3252-L3474`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/session/mod.rs#L3252-L3474).

The base instructions are not an item in `input`; they ride the top-level `instructions` field on every request.

## 4. Initial user task

The user task becomes a `role:"user"` `ResponseInputItem::Message`. Developer context (permissions, skills, world state) is added as separate developer/user input items rather than folded into the task text.

## 5. Assistant text, reasoning, and tool calls

Responses returns independent items: a `reasoning` item (optionally carrying encrypted content), assistant `message` items, and `function_call` / custom-call items. Each item is a discrete unit of history; a tool call is an item, not a chat message with an embedded `tool_calls[]` array.

## 6. Native tool declaration

Tools are Responses-native tool declarations passed on `Prompt.tools`. Requests also include `include:["reasoning.encrypted_content"]`, reasoning configuration, `parallel_tool_calls`, `prompt_cache_key`, store, and streaming. See [`client.rs#L864-L907`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/client.rs#L864-L907).

## 7. Parallel vs sequential tool policy

Function/custom/tool-search calls normalize to `{tool_name, call_id, payload}`. The model can emit several calls. Parallel-safe tools take a shared read lock; unsafe tools take an exclusive write lock. Every result preserves `call_id`; `FuturesOrdered` keeps history in model-call order even when execution completes out of order. See [`router.rs#L113-L160`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/tools/router.rs#L113-L160) and [`parallel.rs#L94-L156`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/tools/parallel.rs#L94-L156).

## 8. Tool results and correlation

Each call produces one `function_call_output` item carrying the same `call_id`. Because results are ordered by `FuturesOrdered`, the persisted item order matches the model's original call order regardless of completion timing.

## 9. History sent in the next request

The HTTP `ResponsesApiRequest` has no `previous_response_id`; each request sends the complete `input: Vec<ResponseItem>` — reasoning, assistant messages, calls, and outputs. See [`common.rs#L215-L239`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/codex-api/src/common.rs#L215-L239).

## 10. Context compaction, truncation, continuation, caching

Codex supports token-budget, local summary, remote `/responses/compact` v1, and remote compaction-item v2 paths. These replace or rebuild logical history and are distinct from prompt caching and WebSocket continuation. See [`turn.rs#L955-L1028`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/session/turn.rs#L955-L1028).

Only the WebSocket DTO adds `previous_response_id`. Delta input is used only when model, instructions, tools, reasoning, store, cache key, and other non-input fields match and the new input has the prior request input plus prior response output as a strict prefix. Otherwise Codex falls back to the full payload. See [`common.rs#L241-L293`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/codex-api/src/common.rs#L241-L293) and [`client.rs#L1164-L1253`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/client.rs#L1164-L1253). `previous_response_id` is therefore a transport optimization, not the sole durable state.

## 11. Subagents

`spawn_agent` is a normal function tool. A child gets an independent thread/session/history/model loop with either new history or a filtered parent rollout when `fork_context=true`; control-tree and budget state are shared. See [`multi_agents/spawn.rs#L88-L140`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/tools/handlers/multi_agents/spawn.rs#L88-L140) and [`agent/control/spawn.rs#L451-L620`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/core/src/agent/control/spawn.rs#L451-L620).

## 12. Provider adapters

This commit supports the Responses wire only. Provider adapters handle URL, auth, headers, retry, HTTP/WS, and Azure differences rather than translating to Anthropic or Gemini messages. See [`model-provider-info/src/lib.rs#L55-L83`](https://github.com/openai/codex/blob/393f64565ab46f09d99ca4d9bd973537e72a114b/codex-rs/model-provider-info/src/lib.rs#L55-L83).

## 13. Two-step model sequence

```text
HTTP Request 1:
  instructions S
  input [developer D, user U1]
  tools T
  parallel_tool_calls true

Response:
  R1=reasoning(encrypted_content)
  C1=function_call(call_id=c1)
  C2=function_call(call_id=c2)

Host:
  O1=function_call_output(call_id=c1)
  O2=function_call_output(call_id=c2)

HTTP Request 2:
  instructions S
  input [D,U1,R1,C1,C2,O1,O2]

WebSocket cache-hit alternative:
  previous_response_id=resp1
  incremental input [O1,O2]
```

Official OpenAI cross-checks: [conversation state](https://developers.openai.com/api/docs/guides/conversation-state), [function calling](https://developers.openai.com/api/docs/guides/function-calling), [WebSocket mode](https://developers.openai.com/api/docs/guides/websocket-mode), and [compaction](https://developers.openai.com/api/docs/guides/compaction).

## 14. Evidence limitations and Marix takeaways

Evidence limitations: this note reflects one commit; Codex evolves rapidly, so line ranges, compaction paths, and multi-agent controls can change. This commit only targets the Responses wire, so no Anthropic/Gemini translation behavior can be inferred from it.

Marix takeaways:

1. Model history as an ordered item list (reasoning / message / call / output) maps cleanly onto Marix's normalized layer; a tool call need not be a chat message.
2. Preserve `call_id` and enforce stable, model-call-ordered result recording even under out-of-order completion.
3. Treat `previous_response_id` and connection continuation as invalidatable transport optimizations; always keep a full local replay path.
4. Distinguish prompt caching, response continuation, and true compaction as three separate mechanisms.
5. Encrypted reasoning replay shows why hidden reasoning must be carried as opaque, provider-scoped state.
