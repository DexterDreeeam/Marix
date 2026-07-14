# Anthropic Claude Code Research (official evidence boundary)

> Research date: 2026-07-14
> Focus: what is publicly verifiable about Claude Code's message orchestration, and what is not.
> Read-only study: no source was copied into Marix and no Git command was run.

> **Evidence boundary.** Claude Code's complete native agent loop, full default system prompt, and internal provider adapter are **not** open source. This note uses only the official Claude Code documentation, the official Python Agent SDK, and the public Messages API as evidence. It deliberately does **not** rely on the third-party `claude-code-from-source` notes in this directory as evidence for Anthropic's private implementation; that pair is a separate reverse-engineering reference and is clearly distinguished here.

## 1. Source and pinned version

| Item | Details |
|---|---|
| Product docs | [Claude Code docs](https://code.claude.com/docs/en/overview) |
| Agent SDK docs | [Agent SDK overview](https://platform.claude.com/docs/en/agent-sdk/overview) |
| API docs | [Messages API](https://platform.claude.com/docs/en/api/messages/create) |
| Public SDK repo | [`anthropics/claude-agent-sdk-python`](https://github.com/anthropics/claude-agent-sdk-python) |
| Pinned SDK commit | [`059d3449bfc2e0dd64230bde65282df93dd21b8d`](https://github.com/anthropics/claude-agent-sdk-python/tree/059d3449bfc2e0dd64230bde65282df93dd21b8d) |
| Evidence role | The pinned SDK is evidence only for the SDK-to-CLI bridge and public message types, not for the native loop |

## 2. Core modules and execution flow (what is verifiable)

The official Python Agent SDK uses `SubprocessCLITransport` to launch the bundled CLI in `stream-json` mode; the wrapper is not the native loop. See [`subprocess_cli.py#L111-L128`](https://github.com/anthropics/claude-agent-sdk-python/blob/059d3449bfc2e0dd64230bde65282df93dd21b8d/src/claude_agent_sdk/_internal/transport/subprocess_cli.py#L111-L128) and [`#L467-L543`](https://github.com/anthropics/claude-agent-sdk-python/blob/059d3449bfc2e0dd64230bde65282df93dd21b8d/src/claude_agent_sdk/_internal/transport/subprocess_cli.py#L467-L543).

Verifiable outer flow: the SDK spawns the CLI, exchanges `stream-json` messages, and surfaces a transcript/message stream. The internal per-request payload construction inside the CLI binary is not public.

## 3. System prompt and resend behavior

- The CLI uses a Claude Code coding-agent prompt, but the complete text is private.
- The Agent SDK defaults to a smaller prompt. Selecting the `claude_code` preset uses the CLI-style prompt; a custom string replaces the default, while preset `append` retains and extends it. See [modifying system prompts](https://code.claude.com/docs/en/agent-sdk/modifying-system-prompts).
- The Messages API carries the system prompt as the top-level `system` field. The full default CLI prompt text remains unverifiable.

## 4. Initial user task

Public evidence: a task enters as a `user` message. The direct Messages API is stateless and requires the full history on each call. Agent SDK/CLI sessions support continue/resume/fork through transcript reconstruction, not a provider-side thread. See [Agent SDK sessions](https://code.claude.com/docs/en/agent-sdk/sessions).

## 5. Assistant text, reasoning, and tool calls

Assistant content can contain `text`, `thinking`, and `tool_use` blocks. See [`types.py#L920-L1037`](https://github.com/anthropics/claude-agent-sdk-python/blob/059d3449bfc2e0dd64230bde65282df93dd21b8d/src/claude_agent_sdk/types.py#L920-L1037). The thinking-block protocol is public; the CLI's native handling of thinking is not.

## 6. Native tool declaration

Tool use is a public API/SDK capability: tools are declared and the model returns `tool_use` blocks. See [handle tool calls](https://platform.claude.com/docs/en/agents-and-tools/tool-use/handle-tool-calls). The exact internal tool registry used by the closed CLI is not public.

## 7. Parallel vs sequential tool policy

One assistant turn can contain several `tool_use` blocks. The Messages API itself does not determine host concurrency. SDK documentation says read-only tools may run concurrently while state-changing tools are generally sequenced. Any specific internal scheduler thresholds are unverifiable.

## 8. Tool results and correlation

A client-tool result goes in the next `user` message as a `tool_result` block whose `tool_use_id` matches the assistant `tool_use.id`. Multiple results are normally grouped in that next user message. This correlation rule is public API behavior.

## 9. History sent in the next request

Because the direct Messages API is stateless, the next request repeats the full prior turn: user task, the complete assistant blocks (text/thinking/`tool_use`), and a user message grouping the `tool_result` blocks. Session continue/resume/fork is transcript reconstruction, not a hidden server thread.

## 10. Context compaction, truncation, continuation, caching

Claude Code/SDK automatically compacts near the limit and supports `/compact`. Public evidence establishes that a compact boundary/summary exists, but not the private summarization prompt or algorithm. See [agent loop](https://code.claude.com/docs/en/agent-sdk/agent-loop). Whether the CLI internally calls a particular public beta compaction API is unverifiable.

## 11. Subagents

A subagent is a fresh conversation with a separate prompt, tools, model, and context. Intermediate history stays out of the parent; the final result returns to the parent, and multiple children may run in parallel. See [subagents](https://code.claude.com/docs/en/agent-sdk/subagents). Internal routing thresholds are not public.

## 12. Provider adapters

The Agent SDK can pass model names to the Anthropic API, Bedrock, Vertex AI, Microsoft Foundry, or a custom gateway, but the native provider transformation code is private. This does **not** establish arbitrary non-Claude model compatibility; Bedrock/Vertex/Foundry/gateway support is still adaptation for Claude deployments.

## 13. Two-step model sequence (protocol level, not a reconstruction)

This is the public Messages API/SDK block protocol, not a claimed byte-for-byte reconstruction of Claude Code:

```text
Request 1:
  system S
  tools [read, search]
  messages [user U1]

Assistant:
  [thinking/signature?, text?,
   tool_use(id=tu1),
   tool_use(id=tu2)]

Request 2:
  system S
  messages [
    user U1,
    complete assistant blocks,
    user [
      tool_result(tool_use_id=tu1),
      tool_result(tool_use_id=tu2)
    ]
  ]

Assistant:
  [final text]
```

## 14. Evidence limitations and Marix takeaways

Claims that must **not** be presented as facts:

- The complete default system prompt.
- Exact native-binary HTTP payload construction, pruning, or caching.
- The compactor's private prompt or algorithm.
- Hidden planner or subagent-routing thresholds.
- That Claude Code internally uses a particular public beta compaction API.
- That model-name passthrough means arbitrary non-Claude models are supported.

Marix takeaways:

1. Treat the public block protocol (system / user / assistant text-thinking-tool_use / user tool_result) as a stable baseline, and keep any behavior beyond it explicitly labeled as inferred.
2. Design for stateless-API semantics first (full replay), then add session continue/resume/fork as transcript reconstruction rather than assuming a server thread.
3. Keep a firm separation between official-evidence notes and reverse-engineered notes; do not merge `claude-code-from-source` claims into official baselines.
4. Model subagents as fresh conversations that only return a final result, matching the documented isolation contract.
5. Remember that provider passthrough to Bedrock/Vertex/Foundry/gateway is still Claude adaptation, not proof of arbitrary-model portability.
