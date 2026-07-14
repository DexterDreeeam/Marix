# How Mainstream Agents Orchestrate Model Messages

> Research date: 2026-07-14
> Scope: read-only research; no third-party source was added to Marix and no Git command was run.
> Evidence rule: open-source agents are pinned to commits. For Claude Code, only official documentation, the official Agent SDK, and observable public protocols are treated as evidence; third-party reconstructions and guesses are excluded.

This is the cross-cutting comparison. Per-system detail, source permalinks, and two-step sequences live in the dedicated notes: [Pi](pi.md), [Codex CLI](codex-cli.md), [Gemini CLI](gemini-cli.md), [Claude Code](claude-code.md), [Cline](cline.md), [Aider](aider.md).

## 1. Executive summary

An agent completing a tool-using task usually maintains three distinct representations:

1. **Persistent/internal records**: user input, complete assistant output, tool calls, tool results, UI events, and audit metadata.
2. **Normalized model context**: provider-neutral `user / assistant / tool-call / tool-result / reasoning` semantics.
3. **Provider wire payloads**: OpenAI `ResponseItem` or chat messages, Anthropic content blocks, or Gemini `Content/Part`.

These three layers must not be collapsed into a single "Message" type. Internal normalized events/messages and provider wire messages/blocks/items are different vocabularies with different grouping rules; mixing them produces correlation and audit bugs.

## 2. Systems and pinned versions

| System | Official source | Pinned version / evidence boundary |
|---|---|---|
| **Pi** | [`earendil-works/pi`](https://github.com/earendil-works/pi), formerly `badlogic/pi-mono` | [`0e6909f050eeb15e8f6c05185511f3788357ddb3`](https://github.com/earendil-works/pi/commit/0e6909f050eeb15e8f6c05185511f3788357ddb3) |
| **OpenAI Codex CLI** | [`openai/codex`](https://github.com/openai/codex) | [`393f64565ab46f09d99ca4d9bd973537e72a114b`](https://github.com/openai/codex/commit/393f64565ab46f09d99ca4d9bd973537e72a114b) |
| **Google Gemini CLI** | [`google-gemini/gemini-cli`](https://github.com/google-gemini/gemini-cli) | [`fa975395bcc6b609e44735e47320e54f51535d47`](https://github.com/google-gemini/gemini-cli/commit/fa975395bcc6b609e44735e47320e54f51535d47) |
| **Anthropic Claude Code** | [Claude Code docs](https://code.claude.com/docs/en/overview), [Agent SDK](https://platform.claude.com/docs/en/agent-sdk/overview), [Messages API](https://platform.claude.com/docs/en/api/messages/create) | Native loop and default system prompt are not open source. Public Python SDK pinned to [`059d3449bfc2e0dd64230bde65282df93dd21b8d`](https://github.com/anthropics/claude-agent-sdk-python/tree/059d3449bfc2e0dd64230bde65282df93dd21b8d), solely as evidence for the SDK-to-CLI bridge and public message types |
| **Cline** | [`cline/cline`](https://github.com/cline/cline) | [`ab68fd7f34e563c82d223592fbf61c74cfd8804e`](https://github.com/cline/cline/commit/ab68fd7f34e563c82d223592fbf61c74cfd8804e) |
| **Aider** | [`Aider-AI/aider`](https://github.com/Aider-AI/aider) | [`5dc9490bb35f9729ef2c95d00a19ccd30c26339c`](https://github.com/Aider-AI/aider/commit/5dc9490bb35f9729ef2c95d00a19ccd30c26339c) |

## 3. Cross-system comparison

| Dimension | Pi | Codex CLI | Gemini CLI | Claude Code | Cline | Aider |
|---|---|---|---|---|---|---|
| Canonical model history | `Message[]`: user/assistant/tool result | `ResponseItem[]` | Curated `Content[]` | Private; SDK exposes transcript/message stream | Runtime `AgentMessage[]`, separate persistent `MessageWithMetadata[]` | `done_messages + cur_messages` |
| System prompt | Separate `systemPrompt`, present in each logical call | `instructions`; developer context is input items | `config.systemInstruction` | API top-level `system`; complete CLI prompt is private | Separate `systemPrompt` | System message; downgraded for some models |
| Conversation state | Full replay by default; Codex WS adapter is an exception | HTTP full replay; WS can send a `previous_response_id` delta | Local chat owns history and sends curated history | Messages API is stateless; SDK resume rebuilds from CLI transcript | Every request uses the full runtime/store projection | Every request rebuilds and resends context |
| Native tools | Yes | Yes, Responses tools | Yes, Gemini declarations | Yes at public API/SDK protocol level | Yes through AI SDK adapters | No in the normal editing path |
| Multiple tools | Parallel by default; tools can require sequencing | Model may emit many; host enforces per-tool parallel safety | Scheduler parallelizes by default; mutating tools can serialize | Multiple blocks supported; SDK docs describe read-only parallelism | Sequential by default, configurable parallelism | Not applicable in normal path |
| Internal results | One result message per call | One output item per call | UI result separated from `functionResponse` | Public user/tool-result semantics; private internals unknown | Runtime `role:"tool"`, persisted as user blocks | Local edit/reflection text, not tool results |
| Thinking | Provider signatures retained only when compatible | Reasoning item and encrypted content are replayable | Thought UI text usually removed; signature retained | Thinking block protocol is public; native handling is private | Normalized reasoning part | Can read reasoning fields; edits use content |
| Context handling | Explicit summary plus kept boundary | Local/remote v1/v2/token-budget paths | ContextManager or compression service | Automatic `/compact` is documented; algorithm is private | Canonical transcript plus API projection/compaction | Summarizes old done history; repo map has its own budget |
| Subagents | No core runtime; official extension starts processes | Independent child thread, optional parent-history fork | Independent `GeminiChat`, tools, and completion contract | Fresh conversation; final result returns to parent | `spawn_agent` creates a child session | Sequential architect→editor composition |

## 4. How a task's roles are divided

Within a single task, agents separate roles as follows:

- **System/developer**: identity, tool guidance, environment, and policy. Pi and Cline carry a dedicated `systemPrompt`; Codex sends top-level `instructions` and adds developer/permissions/skills as separate input items; Gemini uses `config.systemInstruction`; Claude Code uses the API top-level `system`; Aider emits a system message (downgraded to user+ack for models that disable the system role).
- **User**: the task, plus injected environment/session context (Gemini's `<session_context>`, Codex developer items, Cline task with files/images).
- **Assistant**: ordered blocks/items of text, optional reasoning/thinking, and one or more tool calls.
- **Tool**: results correlated back to the originating call.

## 5. Is one tool call one message?

There is no provider-neutral answer:

- **OpenAI Responses**: each function call and each output is normally an independent **item**, not a chat message.
- **OpenAI Chat Completions**: multiple calls can share one assistant message's `tool_calls[]`; each output is normally a separate `role:"tool"` message.
- **Anthropic Messages**: multiple `tool_use` blocks can share one assistant message; their `tool_result` blocks are normally grouped in the next user message.
- **Gemini**: multiple `functionCall` parts can share one model `Content`; their `functionResponse` parts are normally grouped in the next user `Content`.
- **Internal formats differ from all of the above.** Pi stores one `ToolResultMessage` per call; Cline stores runtime `role:"tool"` results and persists them as user blocks. The internal per-call record and the provider envelope are different layers.

## 6. Provider wire quick reference

For one assistant turn that requests tools A and B:

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

The key aggregation differences: Responses keeps calls and outputs as flat sibling **items**; Chat Completions groups calls inside one assistant message but splits each output into its own `tool` message; Anthropic and Gemini group calls into one assistant/model turn and group all results into the next user turn as **content blocks/parts**.

## 7. Resend and continuation each turn

The next model request must preserve both the **original assistant tool call** and its result. Returning only the result breaks call/result correlation and may discard provider reasoning signatures.

- Full replay is the default in Pi, Gemini CLI, Cline, Aider, Codex HTTP, and the stateless Claude Messages API.
- Continuation is an optimization, not the source of truth: Codex's WebSocket `previous_response_id` sends a delta only when a strict-prefix match holds and falls back to full payload otherwise. Prompt caching (`prompt_cache_key`, ephemeral cache controls) is a third, separate mechanism that does not change logical history.

## 8. Reasoning and thought signatures

- Pi retains provider thinking/signatures only for compatible provider/API/model combinations and can downgrade cross-model thinking to text.
- Codex replays reasoning items with `reasoning.encrypted_content`.
- Gemini usually drops UI `thought` text from history but replays the opaque `thoughtSignature` verbatim on the call part.
- Claude Code's thinking-block protocol is public; native handling is private.

Signatures are opaque, provider-scoped state. Blind cross-provider replay is unsafe.

## 9. Context compaction

Compaction rebuilds or replaces logical history and is distinct from caching and continuation:

- Pi: summarizes the older region and records an explicit compaction summary plus kept boundary (default trigger `contextTokens > contextWindow - 16384`, ~20k retained).
- Codex: token-budget, local summary, remote v1, and remote v2 compaction paths.
- Gemini: `ContextManager` graph GC/distillation, or the older `ChatCompressionService` (~50% trigger, ~30% retained, rejects non-shrinking summaries).
- Cline: API projection truncation plus optional compaction storing "summary projection + canonical tail" without deleting the original transcript.
- Claude Code: automatic `/compact` documented; algorithm private.
- Aider: summarizes older `done_messages`; the repo map has an independent budget.

## 10. Subagent return

Subagents return only a final result upward:

- Pi's extension subagent runs a separate process without parent history and returns a summary as a tool result.
- Codex's `spawn_agent` gives a child an independent thread, optionally forking a filtered parent rollout.
- Gemini's local subagent has independent registries, must call `complete_task`, and returns its final value.
- Claude Code subagents are fresh conversations whose intermediate history stays out of the parent.
- Cline's `spawn_agent` creates a child `SessionRuntime`; the parent receives the final value as a tool result.
- Aider's architect→editor is sequential composition, not a general child runtime.

Copying full parent history into a child by default causes leakage, token growth, and permission drift.

## 11. A unified abstraction for Marix

This is a research recommendation, not an implementation change.

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
  provider_metadata        # item id, thought/reasoning signatures
  parallel_safety

ToolResult
  call_id
  model_content            # text/image/structured content
  host_details             # UI, diff, exit code, audit; hidden by default
  is_error

ContextCompaction
  source_range / kept_boundary
  summary
  token_estimate
  compactor_model
  original_items_retained_for_audit
```

Recommended module boundaries:

1. **Immutable transcript/event log**: preserve raw inputs, outputs, call IDs, provider metadata, and failures.
2. **Context projector**: generate model-visible history; filter UI events, truncate tool outputs, apply summaries, and project subagent results.
3. **Provider adapter**: only wire mapping, schema compatibility, signature/call-ID preservation, and stream-delta assembly.
4. **Tool scheduler**: parallel safety, permissions, cancellation, timeout, and stable result ordering.
5. **Continuation state**: treat `previous_response_id` and connection caches as invalidatable optimizations; keep replayable local history.
6. **Compaction ledger**: record compaction explicitly rather than silently destroying audit history.
7. **Reasoning policy**: store visible thinking, hidden/encrypted reasoning, and opaque signatures separately; do not assume cross-provider interchangeability.
8. **Subagent run**: independent run/history/tool policy/budget; return explicit status and final result unless a parent-history fork is requested.

## 12. Risks and anti-patterns

- One "Message" type serving wire, UI, persistence, and audit simultaneously.
- Saving tool results without the assistant calls that produced them.
- Reordering parallel results by completion time without stable call IDs.
- Treating host `details` or an output schema as model tool-selection guidance; selection mainly depends on name, description, and input schema.
- Conflating prompt-cache hits, provider response continuation, and actual history compaction.
- Blindly replaying thinking/signatures across providers.
- Presenting closed Claude Code details or third-party reconstructions as official facts.
- Copying all parent history into subagents by default, causing leakage, token growth, and permission drift.
