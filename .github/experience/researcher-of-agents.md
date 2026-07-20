# researcher-of-agents experience — Marix

## Evidence discipline

- Pin open-source claims to commit-SHA line permalinks and re-pin fast-moving repositories before reuse. Label official, third-party, and reverse-engineered evidence separately.
- Treat unpublished provider schemas and internals as unknown. UI labels, telemetry, process launch commands, code-fence labels, and internal handler names do not establish model-facing tool identities.

## Architecture findings

- Keep persistent transcript records, provider-neutral projected context, and provider wire payloads separate. Preserve originating calls, stable call IDs, result order, and opaque provider reasoning signatures during replay.
- Treat native tool calls and assistant-text JSON as different protocols. Parse only complete successful terminal output; reject refusal, truncation, empty content, unexpected calls, and schema errors instead of extracting, repairing, or executing partial JSON.
- Approval policy, workspace restrictions, process isolation, network controls, browser-profile isolation, and VM/container sandboxing are independent guarantees; never infer one from another.
