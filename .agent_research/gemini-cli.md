# Google Gemini CLI Research

> Research date: 2026-07-14
> Focus: how Gemini CLI orchestrates `Content`/`Part` history, function responses, and thought signatures.
> Read-only study: no Gemini CLI source was copied into Marix and no Git command was run.

## 1. Source and pinned version

| Item | Details |
|---|---|
| Repository | [`google-gemini/gemini-cli`](https://github.com/google-gemini/gemini-cli) |
| Pinned commit | [`fa975395bcc6b609e44735e47320e54f51535d47`](https://github.com/google-gemini/gemini-cli/commit/fa975395bcc6b609e44735e47320e54f51535d47) |
| Main language | TypeScript |
| Role | First-party open implementation of the Gemini `GenerateContent` agent loop |
| Why watch | `Content`/`Part` orchestration, function responses, thought signatures, curated vs comprehensive history, context projection, parallel scheduling, isolated local subagents |

All source links below use `/blob/fa975395bcc6b609e44735e47320e54f51535d47/...#Lx-Ly` permalinks.

## 2. Core modules and execution flow

Gemini CLI's actual wire representation is:

```text
model
contents: Content[]
config:
  systemInstruction
  tools: [{functionDeclarations}]
```

See [`geminiChat.ts#L757-L863`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/core/geminiChat.ts#L757-L863).

`LegacyAgentSession.send()` converts user `ContentPart[]` into Gemini `Part[]` and starts the loop. The CLI owns `GeminiChat`; it does not rely on a provider-side session ID. It calls `generateContentStream()` with curated `Content[]` and maintains both comprehensive and curated histories. See [`legacy-agent-session.ts#L108-L153`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/agent/legacy-agent-session.ts#L108-L153) and [`geminiChat.ts#L921-L959`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/core/geminiChat.ts#L921-L959).

## 3. System prompt and resend behavior

The system instruction is `config.systemInstruction`, kept outside history; history roles are only `user/model`. The system instruction is supplied with each `generateContentStream()` call rather than embedded in `contents`.

## 4. Initial user task

The CLI first injects a user `<session_context>` containing date, OS, directory state, and session memory, then appends the real task as a user `Content`. See [`environmentContext.ts#L50-L110`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/utils/environmentContext.ts#L50-L110).

## 5. Assistant text, reasoning, and tool calls

A model `Content` may contain several `functionCall` parts alongside text. `thought:true` text may be streamed to the UI but is normally omitted from next-turn history. Opaque `thoughtSignature` state stays attached to function-call parts and is replayed. See [`geminiChat.ts#L1111-L1201`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/core/geminiChat.ts#L1111-L1201). The official rule is to replay signatures exactly; the CLI's synthetic-signature repair is defensive compatibility code, not a general recommendation.

## 6. Native tool declaration

Tools are Gemini `functionDeclarations` supplied under `config.tools`. The model-facing conversion that turns host execution into model results is in [`generateContentResponseUtilities.ts#L20-L89`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/utils/generateContentResponseUtilities.ts#L20-L89).

## 7. Parallel vs sequential tool policy

The Scheduler runs consecutive safe calls in parallel; edit, topic-update, or `wait_for_previous` calls can force sequencing. See [`scheduler.ts#L463-L568`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/scheduler/scheduler.ts#L463-L568).

## 8. Tool results and correlation

All results form several `functionResponse` parts inside one next-turn user `Content` and correlate by `id + name`. UI/session `tool_response` and model-facing `functionResponse` are separate values, so UI presentation never leaks into model history.

## 9. History sent in the next request

The next request `contents` are the prior user contents, then the model `Content` (function calls plus signatures), then a user `Content` grouping every `functionResponse`. The CLI sends its curated `Content[]`, not a server-side thread reference.

## 10. Context compaction, truncation, continuation, caching

The newer `ContextManager` performs graph-based GC, distillation, normalization, and API projection over durable history. The older `ChatCompressionService` summarizes older content after roughly 50% of the token limit, retains roughly 30% of recent context, and rejects a summary that does not reduce tokens. See [`contextManager.ts#L90-L265`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/context/contextManager.ts#L90-L265) and [`chatCompressionService.ts#L263-L480`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/context/chatCompressionService.ts#L263-L480).

## 11. Subagents

A subagent gets independent Tool/Prompt/Resource registries and a separate `GeminiChat`, cannot recursively call Agent tools, must invoke `complete_task`, and has its own timeout/turn limit. Its activity maps to an isolated thread and only its final result enters the parent. See [`local-executor.ts#L159-L277`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/agents/local-executor.ts#L159-L277) and [`#L1054-L1087`](https://github.com/google-gemini/gemini-cli/blob/fa975395bcc6b609e44735e47320e54f51535d47/packages/core/src/agents/local-executor.ts#L1054-L1087).

## 12. Provider adapters

Gemini CLI targets the Gemini `GenerateContent` protocol directly; there is no cross-provider translation to OpenAI or Anthropic message shapes in this path. The relevant abstraction differences are curated vs comprehensive history and the `ContextManager` vs `ChatCompressionService` context strategies rather than multi-vendor wire mapping.

## 13. Two-step model sequence

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

Official protocol sources: [GenerateContent](https://ai.google.dev/api/generate-content), [function calling](https://ai.google.dev/gemini-api/docs/generate-content/function-calling), and [thought signatures](https://ai.google.dev/gemini-api/docs/generate-content/thought-signatures).

## 14. Evidence limitations and Marix takeaways

Evidence limitations: this note reflects one commit. Both a legacy compression service and a newer `ContextManager` coexist; the default path may change. The synthetic-signature repair is defensive code and should not be copied as a general policy.

Marix takeaways:

1. Keep model-facing `functionResponse` separate from UI/session tool responses so presentation never pollutes model history.
2. Carry opaque thought signatures on the exact call parts and replay them verbatim; do not regenerate them.
3. Inject environment/session context as an explicit leading user turn rather than hiding it in the system prompt, which keeps history auditable.
4. Isolate subagents with their own registries, completion contract, and turn/time budgets; return only the final result upward.
5. Model both "curated" (model-visible) and "comprehensive" (full) histories rather than one flat list.
