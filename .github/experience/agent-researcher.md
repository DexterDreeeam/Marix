# agent-researcher experience — Marix

## Durable research takeaways

- Tool selection degrades when catalogs exceed roughly 20-50 tools. Prefer consolidated tools, namespaces, retrieval/deferred loading, and precise parameter descriptions.
- OpenAI/DeepSeek tool-call arguments arrive as JSON strings and may stream in fragments; preserve provider call IDs when later tool-result correlation is needed.
- Executable plans need strict typed JSON/Pydantic-style shapes. Markdown checklists, numbered prose, and XML-only plans are fragile for validation, UI preview, retry, and resume.
- Keep session/thread memory, per-request task/run state, and inner step/action records separate. Store typed raw events first; format compact model context on demand.
- Sandbox/runtime boundaries and permission policy are different layers. Shell-only designs hide side effects and weaken auditability even when a sandbox exists.
