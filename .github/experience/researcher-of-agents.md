# researcher-of-agents experience — Marix

## Evidence discipline

- Pin open-source claims to commit-SHA line permalinks and re-pin fast-moving repositories before reuse. Label official, third-party, and reverse-engineered evidence separately.
- Treat unpublished provider schemas and internals as unknown. UI labels, telemetry, process launch commands, code-fence labels, and internal handler names do not establish model-facing tool identities.

## Architecture findings

- Keep persistent transcript records, provider-neutral projected context, and provider wire payloads separate. Preserve originating calls, stable call IDs, result order, and opaque provider reasoning signatures during replay.
- Treat native tool calls and assistant-text JSON as different protocols. Parse only complete successful terminal output; reject refusal, truncation, empty content, unexpected calls, and schema errors instead of extracting, repairing, or executing partial JSON.
- Approval policy, workspace restrictions, process isolation, network controls, browser-profile isolation, and VM/container sandboxing are independent guarantees; never infer one from another.

## 2026-07-20 — DeepSeek V4 thinking-mode tool choice

- **System studied:** DeepSeek V4 OpenAI-compatible Chat Completions API.
- **Primary sources:** DeepSeek [Thinking Mode](https://api-docs.deepseek.com/guides/thinking_mode/), [Create Chat Completion](https://api-docs.deepseek.com/api/create-chat-completion/), [Tool Calls](https://api-docs.deepseek.com/guides/tool_calls/), [Oh My Pi integration](https://api-docs.deepseek.com/quick_start/agent_integrations/oh_my_pi/), [model list](https://api-docs.deepseek.com/api/list-models/), and [2026-04-24 changelog](https://api-docs.deepseek.com/updates/).
- **Core modules and flow:** Direct V4 model IDs default to thinking mode; `thinking.type` toggles thinking, while `reasoning_effort` only selects effort. Tool schemas enter the model-facing request, tool calls return with `reasoning_content`, and both must be replayed across tool-result turns.
- **Reusable pattern:** Keep provider capability constraints conditional on execution mode. For DeepSeek V4 thinking mode, omit `tool_choice`; the official integration guide says the mode rejects the parameter even though the generic endpoint schema lists `none`, `auto`, and `required`. If forced tool use is a hard host contract, disable thinking before using `required`.
- **Risks / anti-patterns:** Do not infer that explicit `auto` is safe merely because omission defaults to auto when tools are present. Do not drop `reasoning_content` from assistant tool-call history. Switching from V4 Flash to V4 Pro does not remove the thinking-mode restriction.
- **Marix implication:** Represent “tool_choice parameter unsupported in thinking mode” separately from “tools unsupported.” Thinking mode supports tools, but tool selection remains model-controlled; Marix must either tolerate no tool call or use non-thinking mode when a tool call is mandatory.
