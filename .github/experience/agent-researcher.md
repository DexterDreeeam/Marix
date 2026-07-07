# agent-researcher Experience

## Purpose

Persistent research notes for external AI agent implementations. Keep notes source-backed and reusable for Marix architecture work.

## Baseline Module Taxonomy

Most production AI agent systems can be compared with these modules:

- **Entry/UI layer**: CLI, TUI, web app, IDE extension, chat channel, or API.
- **Session manager**: conversation ID, current task, message history, cancellation, turn state.
- **Agent loop**: observe -> plan/model call -> tool call -> observation -> continue/stop.
- **Model provider layer**: provider selection, streaming, fallback, rate-limit handling, model capabilities.
- **Prompt/context builder**: system prompt, repository context, tool schema, memory, selected files.
- **Context budget and compaction**: token counting, summarization, truncation, tool-result clearing.
- **Tool registry**: tool schema, permission metadata, concurrency declarations, result rendering.
- **Tool runtime**: shell, file operations, browser, APIs, MCP servers, sandbox calls.
- **Permission and safety**: plan mode, confirmation, auto-approval, policy checks, sandboxing.
- **Workspace/sandbox**: local workspace, Docker/runtime server, remote workspace, file snapshots.
- **Memory**: project instructions, user/team memory, retrieval, long-term notes.
- **Task/sub-agent system**: background jobs, recursive agents, mailbox/coordinator patterns.
- **Event log/audit**: action/observation streams, replay, trace, telemetry, cost tracking.
- **Git/diff workflow**: changed-file detection, patch application, undo/redo, commit/tag integration.
- **Plugin/skill system**: custom commands, lifecycle hooks, workflow DSLs, marketplace/registry.

## Initial Research Snapshot: Coding and Automation Agents

### Claude Code / Claude Code from Source

Sources:

- https://claude-code-from-source.com/
- https://claude-code-from-source.com/ch01-architecture/
- https://claude-code-from-source.com/ch05-agent-loop/

Reusable findings:

- The central abstraction is a single async-generator query loop that streams model messages, executes tools, appends observations, and returns typed terminal reasons.
- Tools are self-describing objects with schema, permission, concurrency, progress, and rendering metadata.
- Task/sub-agent execution is recursive: sub-agents are separate query loops with isolated history and permission bubbling.
- Context management has multiple layers: tool-result budget, snip compact, microcompact, context collapse, and auto-compact.
- Production reliability comes from infrastructure around the model: permission modes, hooks, state layering, fallback, stop hooks, recovery guards, and budget tracking.

Marix takeaway:

- Model the Marix core around a typed agent loop and self-describing tools, not around one-off command handlers.
- Treat context compaction and permission modes as first-class architecture, not later optimizations.

### OpenClaw

Sources:

- https://openclaw.im/

Reusable findings:

- OpenClaw focuses on workflow automation and message routing across many channels.
- Core ideas: programmable workflow engine, universal message router, stateful context manager, plugin architecture, BYOM model layer, and self-hosted auditability.
- Its strength is cross-channel orchestration rather than code editing alone.

Marix takeaway:

- A workflow DSL and plugin registry can complement a coding-agent loop when Marix expands beyond terminal/code interactions.

### OpenHands

Sources:

- https://docs.openhands.dev/
- https://github.com/OpenHands/OpenHands

Reusable findings:

- OpenHands emphasizes runtime/sandbox architecture: backend sends actions to a Docker or remote sandbox and receives observations.
- Event streams of actions and observations make execution auditable and replayable.
- The sandbox boundary is the key safety primitive for arbitrary code execution.

Marix takeaway:

- Use an action/observation event model if Marix needs replay, audit, pause/resume, or remote runtime support.
- Keep sandbox management separate from the agent loop.

### Aider

Sources:

- https://aider.chat/docs/
- https://aider.chat/docs/usage.html
- https://aider.chat/docs/usage/modes.html

Reusable findings:

- Aider is git-first: users add files to chat, Aider edits them, shows diffs, commits changes, and supports undo.
- It uses chat modes (`code`, `ask`, `architect`, `help`) to separate planning, answering, and editing.
- Architect mode can split reasoning and edit generation between separate models.

Marix takeaway:

- Git/diff/undo should be a first-class workflow module.
- Separate plan/ask/build modes reduce accidental edits and improve UX.

### OpenCode

Sources:

- https://opencode.ai/docs/

Reusable findings:

- OpenCode provides TUI, desktop, and IDE surfaces.
- It initializes projects with `AGENTS.md`, supports plan/build modes, undo/redo, shared conversations, themes, commands, keybinds, and provider configuration.

Marix takeaway:

- Project-local instructions and plan/build mode are important UX primitives.
- Undo/redo should be considered part of the editing contract, not just git recovery.

### Continue

Sources:

- https://docs.continue.dev/

Reusable findings:

- Continue represents the IDE-native context-provider architecture: editor selection, files, terminal, docs, and codebase index provide context.
- It supports VS Code, JetBrains, and CLI modes, with configurable model providers.

Marix takeaway:

- A context-provider abstraction would let Marix reuse the agent loop across CLI, IDE, and web surfaces.

## Research Note Template

```markdown
## YYYY-MM-DD — Topic

Systems studied:

- Name — URL

Core modules:

- ...

Execution flow:

1. ...

Reusable patterns:

- ...

Risks / anti-patterns:

- ...

Marix implications:

- ...
```

## 2026-06-30 — Built-in tool boundary patterns across AI/coding agents

Systems studied:

- OpenAI Agents SDK / Responses tools — hosted WebSearchTool, FileSearchTool, ToolSearchTool, hosted/local ShellTool, ComputerTool, ApplyPatchTool, function tools, agents-as-tools.
- Claude Code — tools reference, permissions, security, MCP, hooks, settings.
- GitHub Copilot cloud agent — cloud-agent overview, MCP configuration, custom agents, memory.
- Cursor Agent / Cloud Agent — agent overview, terminal/search/browser tools, run modes, permissions, MCP, cloud capabilities.
- Devin — session tools, environment configuration, Knowledge, security.
- SWE-agent and mini-SWE-agent — configurable tool bundles vs bash-only mini baseline.
- OpenHands SDK — action-observation tool system, built-in Bash/FileEditor/Browser/ApplyPatch tools, security confirmation, sandbox docs.
- Aider — chat modes, slash commands, edit formats, git-first workflow.
- LangChain and AutoGPT Platform — tool/toolkit integrations, runtime context, blocks/components/commands.

Primary-source-backed findings:

- Mature agents classify tools by multiple axes, not a single category: capability/domain, execution resource, state side effect, permission/risk, runtime locality, model-facing schema, and extension source.
- Even when shell can perform many operations, production agents expose read/search/edit/web/memory tools because they improve model grounding, permission granularity, auditability, output compaction, cross-platform behavior, and UI rendering.
- Coding agents repeatedly converge on a small first-party surface: read/list/search, edit/apply-patch, shell/terminal, web fetch/search/browser, task/subagent, memory/instructions, and git/diff/PR workflow. Broader APIs move to MCP/plugins/skills.
- Permission systems are usually tool-aware and parameter-aware: Claude Code and Cursor both distinguish read-only file/search from write/edit and shell; both use allow/deny/ask-style rules and special treatment for network/destructive commands.
- Sandboxing is not a replacement for fine-grained tools. Cursor and Claude add sandbox/VM/container controls around shell, but still maintain structured tools for file, search, browser, MCP, and workflow operations.

Reusable architecture patterns:

- Represent category as only one facet. Add independent metadata for source, resource scope, side effects, risk tier, permission policy, platform/runtime, output type, and concurrency/streaming.
- Keep native tools small and boring; move service-specific integrations to MCP or plugin layers.
- Separate "execution primitive" tools from "context/navigation" tools. Shell is an execution primitive; read/search/glob/browser/memory are context builders and should remain first-class.
- Prefer patch/diff-oriented edit tools over whole-file write for coding tasks where review, rollback, and minimal diffs matter.
- Add annotations similar to MCP readOnly/destructive/idempotent/openWorld to support policy and model guidance.

Risks / anti-patterns:

- Treating shell as the only universal primitive hides side effects, breaks permission granularity, bloats context with unstructured output, and becomes platform-fragile.
- Over-expanding built-ins into product integrations creates maintenance and security burden; use MCP/plugins for GitHub/Jira/Slack/databases/cloud APIs.
- Conflating category with safety prevents precise policy decisions; e.g. file read and file write share domain but have different risk.
- Exposing broad HTTP/browser/write tools without allowlists or approval creates prompt-injection and data-exfiltration risk.

Marix implications:

- marix's current tool/category and native folder split is directionally right, but category should stay descriptive rather than policy-driving.
- First batch should emphasize file read/list/search, patch/edit, shell execution with strict permissions, system/env inspection, web fetch/search if needed, and memory/instructions hooks.
- Defer image transform, generic package query, broad browser automation, service APIs, and database/cloud integrations unless a concrete workflow needs them.

## 2026-07-01 — Session/task context naming and memory boundaries

Systems studied:

- AutoGPT / Forge / Agent Protocol — https://github.com/Significant-Gravitas/AutoGPT
- LangChain short-term memory — https://docs.langchain.com/oss/python/langchain/short-term-memory
- LangGraph persistence and checkpointers — https://docs.langchain.com/oss/python/langgraph/persistence and https://docs.langchain.com/oss/python/langgraph/checkpointers
- CrewAI memory and tasks — https://docs.crewai.com/v1.15.1/en/concepts/memory.md and https://docs.crewai.com/v1.15.1/en/concepts/tasks.md
- OpenAI Agents SDK sessions, results, running agents, and tracing — https://openai.github.io/openai-agents-python/sessions/, /results/, /running_agents/, /tracing/
- Claude Code sessions, memory, context window, and agentic loop — https://code.claude.com/docs/en/sessions, /memory, /context-window, /how-claude-code-works
- OpenAI Codex CLI/app-server — https://github.com/openai/codex, especially thread/turn/item APIs and Codex core session/turn/task comments.

Core modules observed:

- Conversation-scoped containers are usually named Session or Thread and hold durable conversation state, status, settings, memory eligibility, model/provider state, and persisted transcript/history.
- User-request execution is usually named Turn, Run, or Task depending on product shape: chat agents prefer Turn/Run; job/benchmark protocols prefer Task; graph systems use Run + node/task writes.
- Inner loop records are consistently modeled as Step, Item, Action/Observation, RunItem, or span. They carry model calls, tool calls, tool outputs, approvals, edits, reasoning, and messages.
- Summaries/compaction are separate artifacts from raw history: systems keep both the compacted model-visible view and audit/event records when possible.

Reusable patterns:

- Keep long-lived session/thread memory separate from one-request task/turn state.
- Store raw structured data first; format model prompts on demand through a context builder.
- Give Task/Turn context an ordered event/step log and a final summary/result field; give Session/Thread context a rolling summary over completed tasks.
- Use explicit compaction metadata: what was summarized, source range, token budget, model/prompt used, and invalidation/rebuild information.
- Avoid making tool-result text the only memory; keep typed tool call/result records for replay, audit, UI, retry, and summarization.

Risks / anti-patterns:

- Conflating SessionContext with the current user request makes resume, branching, parallel tasks, and multi-turn memory hard.
- Naming every unit "task" hides important boundaries: task as user request vs background runtime task vs tool step.
- Persisting only prompt strings loses provenance and makes later compaction/audit unreliable.
- Letting message history grow without trimming/summarization causes latency, cost, context overflow, and stale-context distraction.

Marix implications:

- Keep `SessionContext` as the conversation/thread-level container; add model-visible task summaries there, not raw step logs.
- Keep `TaskContext` as one user request/turn/run; it should own an ordered list of execution steps/items plus result and summary.
- Consider `TaskStep` or `TaskEvent` for inner actions; use `TaskSummary` and `SessionSummary` as explicit model-context records.
- For the model, expose a compact `ModelContext` built from session summary + recent task summaries + current task input + selected step observations.

## 2026-07-04 — Advertising the tool catalog to the model (tool/function calling)

Context: informs Marix `ExecutionEvent::Preview { tools: Vec<ToolPreview> }`, `ToolPreview { name, description, schema: ToolSchema { input, output } }`, and how a chosen call maps to `ExecutionEvent::Evoke(ExecutionRequest { signature, prompt, tool_request, user_options })`. Marix backend is DeepSeek (OpenAI-compatible, SSE streaming) and today only sends `{model, messages, stream}` and reads `choices[].delta.content` — no native tool calling yet.

Systems studied (primary sources):

- OpenAI Chat Completions function calling — https://platform.openai.com/docs/guides/function-calling and https://developers.openai.com/api/docs/guides/function-calling
- Anthropic Claude tool use — https://platform.claude.com/docs/en/agents-and-tools/tool-use/overview and /define-tools, /tool-search-tool
- Google Gemini function calling — https://ai.google.dev/gemini-api/docs/function-calling
- DeepSeek function calling (OpenAI-compatible) — https://api-docs.deepseek.com/guides/function_calling (archived 20241226)
- Model Context Protocol tools — https://modelcontextprotocol.io/specification/2025-06-18/server/tools
- Frameworks: LangChain, LlamaIndex, CrewAI, smolagents, OpenHands, SWE-agent, AutoGPT (repo + docs, see below)
- Anthropic "Writing tools for agents" — https://www.anthropic.com/engineering/writing-tools-for-agents ; "Advanced tool use" — https://www.anthropic.com/engineering/advanced-tool-use
- Papers: RAG-MCP arXiv:2505.03275; "Less is More" arXiv:2411.15399; Gorilla arXiv:2305.15334; ToolLLM/ToolBench arXiv:2307.16789

Core finding — the near-universal tool-advertisement shape is `{ name, description, input-JSON-Schema }`:

- OpenAI: `tools:[{type:"function", function:{name, description, parameters:<JSON Schema>}}]`; `tool_choice` = `auto|none|required|{type:function,function:{name}}`; response `message.tool_calls[]={id,type:"function",function:{name, arguments:<JSON string>}}`; `content` is null on a tool call; arguments are a STRINGIFIED JSON object; `strict:true` + `additionalProperties:false` for schema conformance. INPUT parameters only — no output schema.
- Anthropic: `tools:[{name, description, input_schema:<JSON Schema>}]`; returns `tool_use` block `{type:"tool_use", id, name, input:<OBJECT>}` with `stop_reason:"tool_use"`; you reply with a `tool_result` block referencing `tool_use_id`; `tool_choice:{type:"auto"}` default; `strict:true` supported; `input_examples` field boosts accuracy (Anthropic internal 72%->90%). INPUT only — no output schema. (Note: Anthropic `input` is an object, OpenAI/DeepSeek `arguments` is a string.)
- Gemini: `tools:[{functionDeclarations:[{name, description, parameters:<JSON Schema/OpenAPI subset>}]}]` (newer Interactions API uses `{type:"function", name, description, parameters}`); returns `functionCall{name, args:<OBJECT>}`. INPUT only — no output schema.
- DeepSeek: fully OpenAI-compatible. Same `tools` array (`{type:"function", function:{name, description, parameters}}`), returns `message.tool_calls[0]` with `.id`, fed back as `{role:"tool", tool_call_id, content}`. INPUT only. Known limits: practical cap ~64-128 tools; streaming of tool_calls historically weaker; reliability comparable to OpenAI, degrades with vague descriptions. Model itself never executes the function.
- MCP: `tools/list` -> `{name, title?, description, inputSchema:<JSON Schema>, outputSchema?:<JSON Schema>, annotations?}`; `tools/call` -> `{content:[...], structuredContent?, isError?}`. MCP is the ONLY widely-used protocol that advertises an OPTIONAL OUTPUT schema; if present, server MUST return conforming `structuredContent` and client SHOULD validate. `annotations` (readOnlyHint/destructiveHint/idempotentHint/openWorldHint) are UNTRUSTED unless from a trusted server. Supports pagination + `notifications/tools/list_changed`.

Injection point — two mechanisms; native array is default, prompt-text is the fallback:

- Native provider `tools` array (structured): OpenAI, Anthropic, Gemini, DeepSeek, and by default LangChain, LlamaIndex, CrewAI, OpenHands, SWE-agent, AutoGPT. Preferred whenever the model supports native tool calling.
- System-prompt TEXT rendering (tool catalog serialized into the prompt): used for models WITHOUT native tool calling, or for code-based agents. Examples: smolagents CodeAgent renders tools as Python function signatures in the system prompt and the model writes Python that calls them; OpenHands non-native fallback renders `<function=name><parameter=x>value</parameter></function>` XML; AutoGPT renders a TypeScript namespace (`format_function_specs_as_typescript_ns`); SWE-agent non-function-calling parse modes render `generate_command_docs`. All still describe the same fields (name + description + params); only the serialization differs.

Framework tool-definition fields (all convert to JSON Schema input; none advertise an output schema to the model for selection, except smolagents partial):

- LangChain: `@tool`/`StructuredTool`/`Tool` + `args_schema` (Pydantic) -> `convert_to_openai_tool()` -> `{type:function,function:{name,description,parameters}}`. `bind_tools()` attaches to the call. `response_format` is internal only.
- LlamaIndex: `FunctionTool` + `ToolMetadata{name,description,fn_schema}`; `to_openai_tool()`/`get_parameters_dict()` -> JSON Schema. Native array only.
- CrewAI: `BaseTool{name,description,args_schema}`; via LangChain compat -> OpenAI format. "Typed outputs" are post-call Pydantic results, NOT advertised.
- smolagents: `Tool{name, description, inputs:{arg:{type,description}} (own DSL), output_type:str, output_schema?:dict}`. `output_type` always advertised; `output_schema` advertised only to CodeAgent via docstring. CodeAgent = prompt text (Python); ToolCallingAgent = both text + native.
- OpenHands: Pydantic `Action` -> `to_mcp_schema()`/`to_openai_tool()`. Native default; XML-tag prompt fallback. Output schema only on MCP path, not on the LLM-selection path.
- SWE-agent: YAML `Command`/`Argument` -> `get_function_calling_tool()` JSON Schema. Native when `parse_function: function_calling`; else prompt docs.
- AutoGPT: custom `JSONSchema` Pydantic -> native for OpenAI/Anthropic; TypeScript-namespace prompt fallback (`=> any` return, i.e. no output type).

Scaling to many tools (hard numbers, source-backed):

- OpenAI guide: soft suggestion to keep to **< 20 tools** per turn; functions are billed as input tokens; suggests fewer tools / shorter descriptions / tool search.
- Anthropic: tool-selection quality **degrades once you exceed ~30-50 tools**; a 5-MCP-server setup = ~55K tokens (worst observed 134K) of definitions before any user message.
- Anthropic Tool Search: mark tools `defer_loading:true`, load on demand -> ~85% token reduction (72K->8.7K); selection accuracy Opus 4 **49%->74%**, Opus 4.5 **79.5%->88.1%**. Variants: regex + BM25 tool-search tools; discovered tools returned as `tool_reference` blocks.
- OpenAI Tool Search (gpt-5.4+): `defer_loading:true` + `{type:"tool_search"}`; hosted or client-executed (`tool_search_call`/`tool_search_output`); namespace objects (`{type:"namespace", name, description, tools:[...]}`); best practice "< 10 functions per namespace".
- Papers: RAG-MCP retrieve-then-prompt **13.62%->43.13%** accuracy, tokens 2134->1084; "Less is More" top-k retrieval -> Llama3.1-8B 93.8%, context 16K->8K; Gorilla retrieval cuts hallucinated APIs (GPT-4 36-78% -> Gorilla 5-11%); ToolLLM/ToolBench neural API retriever over 16k APIs.
- Framework retrieval: LlamaIndex `ObjectIndex` + `as_retriever` (agent uses retriever not full list); LangGraph `langgraph-bigtool` (embed tools, per-turn similarity search); LangChain `create_retriever_tool`.
- Degradation mechanism: context bloat (input tokens), attention dilution, semantic confusion between overlapping tool names/descriptions.

Best practices for descriptions/schemas (consensus across OpenAI + Anthropic):

- Naming: `snake_case`, match `^[a-zA-Z0-9_-]{1,64}$` (Anthropic API-enforced), `verb_noun` (`get_weather`, `search_logs`), `service_verb_noun`/prefix namespacing for many/multi-server tools (`github_list_prs`); prefix-vs-suffix has measurable eval effects.
- Descriptions: extremely detailed, "explain like to a new hire", state what the tool does NOT do if ambiguous, put format guidance in the description; unambiguous parameter names (`user_id` not `user`).
- JSON Schema: `additionalProperties:false` always; `strict:true`; explicit `required`; `enum` to make invalid states unrepresentable (avoid `on:bool,off:bool`); per-property `description`; Anthropic `input_examples` for complex inputs; don't add params for values you already know (inject in code).
- Tool design: prefer few high-impact consolidated tools over thin API wrappers (`schedule_event` over list+create); return high-signal context, resolve UUIDs to human-meaningful ids; `response_format` enum (concise/detailed) to control verbosity; paginate/truncate (Claude Code caps tool output ~25K tokens); actionable error messages, not raw tracebacks.
- Scaling techniques ranked by prevalence: (1) keep tools few/consolidated, (2) namespacing/grouping, (3) tool search / `defer_loading`, then retrieval/RAG, dynamic per-turn subsetting, fine-tuning.

Reusable patterns:

- The de-facto interoperable tool descriptor is `{name, description, parameters:<JSON Schema>}`. An OUTPUT schema is NOT part of model-native selection; it exists only in MCP (for post-call validation/structured content) and partially smolagents.
- Advertise via the native `tools` array whenever the backend supports it; keep a prompt-text renderer as a fallback for non-native models. Same field set, different serialization.
- `tool_choice` and per-tool `strict` are the standard control knobs. Tool-call arguments arrive as a JSON string (OpenAI/DeepSeek) — parse+validate against the input schema before executing.
- For large catalogs, don't dump all tools: consolidate, namespace, then retrieve/defer.

Risks / anti-patterns:

- Typing schemas as opaque strings that are not actually valid JSON Schema — breaks native `tools` handoff and client-side validation.
- Advertising an output schema to the model expecting it to improve selection — it does not; it only helps the host/client validate results (MCP semantics).
- Dumping the entire tool catalog every turn -> token bloat + accuracy degradation past ~20-50 tools.
- Vague/overlapping tool names and descriptions; missing per-parameter descriptions; free-text args instead of JSON Schema.
- Treating tool annotations from untrusted servers as authoritative for safety decisions.

Marix implications:

- Marix `ToolPreview { name, description, schema }` already maps 1:1 onto OpenAI/Anthropic/DeepSeek tool definitions. Native tools are self-describing and emit `ToolPreview` JSON via `--preview` (see `src/host/executor/tool.rs`, `src/tool/native/file/read_file.rs`), and the concrete `ToolSchema.input`/`output` strings ALREADY contain JSON Schema documents — they are just typed as `String`.
- Recommendation: keep `input`/`output` as JSON Schema, but treat them as JSON Schema (validate on load; optionally type as `serde_json::Value`) rather than opaque strings. Render `Vec<ToolPreview>` into DeepSeek's native `tools` array (`{type:function,function:{name,description,parameters:input}}`), NOT system-prompt text, because DeepSeek supports native tool calling. Keep a prompt-text fallback only if a non-tool-calling model is ever used.
- `output` schema: unusual among model-native APIs; keep it (MCP-style) for host-side validation, UI rendering, and step-result typing, but do NOT send it in the DeepSeek `tools` array. Document it as advisory/host-facing.
- Map model output back to `ExecutionEvent::Evoke`: DeepSeek `tool_calls[i].function.name` -> `ExecutionSignature.name` / `ExecutionRequest`; `tool_calls[i].function.arguments` (JSON string) -> `ExecutionRequest.tool_request` (already a raw JSON string — good match). Preserve the provider `tool_call.id` (currently no field for it) to correlate the async result back as the `{role:"tool", tool_call_id, content}` message; consider adding an id to `ExecutionSignature`/`ExecutionRequest`.
- Interface gaps to consider: no `tool_choice` concept; no place for the provider `tool_call_id`; no per-tool `strict`/annotations/namespace metadata; `ToolSchema` lacks input_examples; `ExecutionEvent::Preview` has no catalog-subsetting notion for scaling (fine while tool count is small; revisit past ~20 tools with namespacing/retrieval).
- The DeepSeek backend (`src/agent/model/backend_deepseek.rs`) must be extended to (a) send `tools` (+ optional `tool_choice`), and (b) parse `delta.tool_calls` from the SSE stream (accumulate streamed `arguments` fragments) in addition to `delta.content`.

## 2026-07-04 — CONCRETE tool-advertisement formats (literal templates + rendered examples)

Follow-up to the section above. Goal: copy-paste-quality, source-grounded examples of how frameworks serialize the tool set to the model. Every rendering uses the same two sample tools so formats are directly comparable:

- `read_file` — "Read a file from disk and return its text contents." — `path: string [required]`, `max_bytes: integer [optional, default 65536]` -> file text.
- `search_text` — "Search a file for lines matching a pattern and return the matching lines." — `pattern: string [required]`, `path: string [required]`, `regex: boolean [optional]` -> matching lines.

### A. Native structured `tools` payloads (on the wire)

All four reduce to `name + description + input-JSON-Schema`; MCP alone adds an optional output schema.

- **OpenAI / DeepSeek** (src: platform.openai.com/docs/guides/function-calling; api-docs.deepseek.com/guides/function_calling). Request: `{"model","messages","tools":[{"type":"function","function":{"name","description","parameters":<JSONSchema type:object>, "strict":true}}], "tool_choice":"auto"}`. `tool_choice` = `"auto"|"none"|"required"|{"type":"function","function":{"name":"read_file"}}`. Response: assistant msg with `content:null` and `tool_calls:[{"id":"call_0_9f83a1c2","type":"function","function":{"name":"read_file","arguments":"{\"path\":\"./README.md\",\"max_bytes\":4096}"}}]` — **arguments is a JSON STRING**. Reply: `{"role":"tool","tool_call_id":"call_0_9f83a1c2","content":"<file text>"}`. DeepSeek-specific: drop-in OpenAI shape (`base_url=https://api.deepseek.com`, `model=deepseek-chat`); model never executes; streamed `delta.tool_calls[].function.arguments` arrive fragmented and must be accumulated by `index` (most bug-prone area); `deepseek-reasoner` has had fn-calling restrictions vs `deepseek-chat`.
- **Anthropic** (src: platform.claude.com/docs/en/agents-and-tools/tool-use/overview). `tools:[{"name","description","input_schema":<JSONSchema>}]`, `tool_choice:{"type":"auto"}`. Response content block `{"type":"tool_use","id":"toolu_01A09...","name":"read_file","input":{"path":"./README.md","max_bytes":4096}}` with `stop_reason:"tool_use"` — **input is an OBJECT**. Reply: a `user` msg with `{"type":"tool_result","tool_use_id":"toolu_01A09...","content":"<file text>"}`.
- **Gemini** (src: ai.google.dev/gemini-api/docs/function-calling). `tools:[{"functionDeclarations":[{"name","description","parameters":<OpenAPI-3.0 subset, no default>}]}]` (newer: `parametersJsonSchema` for full JSON Schema). Response part `{"functionCall":{"name":"read_file","args":{...}}}`. Reply part `{"functionResponse":{"name":"read_file","response":{...object...}}}`.
- **MCP** (src: modelcontextprotocol.io/specification/2025-06-18/server/tools). `tools/list` result: `{"tools":[{"name","title","description","inputSchema":<JSONSchema>,"outputSchema":<JSONSchema, OPTIONAL>,"annotations":{"readOnlyHint":true,...}}],"nextCursor":"..."}`. `tools/call` -> `{"content":[{"type":"text","text":...}],"structuredContent":{...},"isError":false}`. Only widely-used protocol advertising an output schema; if present, `structuredContent` MUST conform.

### B. System-prompt TEXT renderings (tools serialized into the prompt string)

**B1. ReAct / classic LangChain.** Prompt src: `libs/langchain/langchain_classic/agents/react/agent.py` (create_react_agent docstring = hub `hwchase17/react`); `.../agents/mrkl/prompt.py`. Tool renderer: `libs/core/langchain_core/tools/render.py` `render_text_description` = `f"{name}{sig} - {description}"` (uses Python `inspect.signature` when tool has `.func`, else `f"{name} - {description}"`); `{tool_names}` = `", ".join(t.name)`. Literal template:
```
Answer the following questions as best you can. You have access to the following tools:

{tools}

Use the following format:

Question: the input question you must answer
Thought: you should always think about what to do
Action: the action to take, should be one of [{tool_names}]
Action Input: the input to the action
Observation: the result of the action
... (this Thought/Action/Action Input/Observation can repeat N times)
Thought: I now know the final answer
Final Answer: the final answer to the original input question

Begin!

Question: {input}
Thought:{agent_scratchpad}
```
Rendered `{tools}` (with-signature case): `read_file(path: str, max_bytes: int = 65536) - Read a file from disk and return its text contents.` / `search_text(pattern: str, path: str, regex: bool = False) - Search a file...`. `{tool_names}` = `read_file, search_text`. Action Input is FREE TEXT (no per-arg schema) — weak for multi-arg tools. Richer variant `render_text_description_and_args` appends `, args: {'path': {'title':'Path','type':'string'}, ...}`. Chat variant (`.../agents/chat/prompt.py`) uses a `$JSON_BLOB` with `action`/`action_input`.

**B2. smolagents CodeAgent — `to_code_prompt()`.** Renderer src: `src/smolagents/tools.py`; scaffold `src/smolagents/prompts/code_agent.yaml`. Builds `def {name}({arg}: {inputs[arg].type}, ...) -> {output_type}:` + docstring (description, `Args:` arg:description lines, optional `Returns:` with output_schema JSON only if tool has `output_schema`). Types are smolagents' OWN DSL strings (`string`/`integer`/`boolean`), NOT Python `str`; no defaults/optional markers in the signature. Rendered:
```python
def read_file(path: string, max_bytes: integer) -> string:
    """Read a file from disk and return its text contents.

    Args:
        path: Absolute or relative path to the file to read.
        max_bytes: Maximum number of bytes to read.
    """

def search_text(pattern: string, path: string, regex: boolean) -> string:
    """Search a file for lines matching a pattern and return the matching lines.

    Args:
        pattern: Text or regular expression to search for.
        path: Path of the file to search.
        regex: Treat pattern as a regular expression.
    """
```
Scaffold framing: "you only have access to these tools, behaving like regular python functions:" then a ```py fenced block of the defs; model responds Thought:/```py block that CALLS the functions and `print()`s intermediates (-> Observation:).

**B3. smolagents ToolCallingAgent — `to_tool_calling_prompt()`.** Src: same tools.py; scaffold `toolcalling_agent.yaml`. `f"{name}: {description}\n    Takes inputs: {inputs}\n    Returns an output of type: {output_type}"` rendered as `- ` list items; `inputs` is Python repr (single quotes, `True`, optional args carry `'nullable': True`):
```
- read_file: Read a file from disk and return its text contents.
    Takes inputs: {'path': {'type': 'string', 'description': 'Absolute or relative path to the file to read.'}, 'max_bytes': {'type': 'integer', 'description': 'Maximum number of bytes to read.', 'nullable': True}}
    Returns an output of type: string
- search_text: Search a file for lines matching a pattern and return the matching lines.
    Takes inputs: {'pattern': {'type': 'string', ...}, 'path': {'type': 'string', ...}, 'regex': {'type': 'boolean', ..., 'nullable': True}}
    Returns an output of type: string
```
Model replies with `Action:\n{ "name": "read_file", "arguments": {"path":"./README.md","max_bytes":4096} }`. NOTE: this text is the readable half; ToolCallingAgent ALSO passes JSON schema to the model's native tool API (`get_tool_json_schema`).

**B4. AutoGPT — `format_function_specs_as_typescript_ns()`.** Src: `classic/forge/forge/llm/providers/openai.py` (+ `format_openai_function_for_prompt`); type map in `classic/forge/forge/models/json_schema.py` `typescript_type` (string->string, integer/number->number, boolean->boolean, array->Array<...>, object->interface/Record<string,any>, enum->'a' | 'b'). Optional params get `?`; each param line ends with comma; literal space in `(_ :{`; return always `=> any`. Rendered:
```typescript
namespace functions {

// Read a file from disk and return its text contents.
type read_file = (_ :{
// Absolute or relative path to the file to read.
path: string,
// Maximum number of bytes to read.
max_bytes?: number,
}) => any;

// Search a file for lines matching a pattern and return the matching lines.
type search_text = (_ :{
// Text or regular expression to search for.
pattern: string,
// Path of the file to search.
path: string,
// Treat pattern as a regular expression.
regex?: boolean,
}) => any;

} // namespace functions
```
Placed under `# Tools\n\n## functions\n\n` (see `count_openai_functions_tokens`) — mirrors OpenAI's internal token-accounting layout.

**B5. OpenHands — XML tags (non-native path).** Src: `openhands/llm/fn_call_converter.py` at pinned commit `fc4c62a73db32b1f0a2dbbc7c107f7a0bebf145c` (file REMOVED on main after the Agent-SDK refactor; parent of deletion commit `aea6116...`). Catalog renderer `convert_tools_to_description`; wrapper `SYSTEM_PROMPT_SUFFIX_TEMPLATE`; call encoder `convert_tool_call_to_string`; regex `<function=([^>]+)>\n(.*?)</function>` + `<parameter=([^>]+)>(.*?)</parameter>`. Catalog rendered:
```
---- BEGIN FUNCTION #1: read_file ----
Description: Read a file from disk and return its text contents.
Parameters:
  (1) path (string, required): Absolute or relative path to the file to read.
  (2) max_bytes (integer, optional): Maximum number of bytes to read.
---- END FUNCTION #1 ----
---- BEGIN FUNCTION #2: search_text ----
Description: Search a file for lines matching a pattern and return the matching lines.
Parameters:
  (1) pattern (string, required): Text or regular expression to search for.
  (2) path (string, required): Path of the file to search.
  (3) regex (boolean, optional): Treat pattern as a regular expression.
---- END FUNCTION #2 ----
```
Wrapped by SYSTEM_PROMPT_SUFFIX_TEMPLATE: "You have access to the following functions:\n\n{description}\n\nIf you choose to call a function ONLY reply in the following format with NO suffix:\n\n<function=example_function_name>\n<parameter=example_parameter_1>value_1</parameter>\n...\n</function>\n\n<IMPORTANT>\nReminder:\n- Function calls MUST ... start with <function= and end with </function>\n- Required parameters MUST be specified\n- Only call one function at a time\n...</IMPORTANT>". Model invocation (list/dict values -> inline JSON):
```
<function=read_file>
<parameter=path>./README.md</parameter>
<parameter=max_bytes>4096</parameter>
</function>
```
Result returned as `EXECUTION RESULT of [read_file]:\n<contents>`.

**B6. Other plain-text.** (a) AutoGPT compact `CompletionModelFunction.fmt_line` (`classic/forge/forge/llm/providers/schema.py`): `f"{name}: {description}. Params: ({name}{'?' if optional}: {ts_type}, ...)"` -> `read_file: Read a file from disk and return its text contents.. Params: (path: string, max_bytes?: number)`. (b) LangChain `render_text_description_and_args` (shown in B1). (c) Gorilla/Toolformer (arXiv:2305.15334): natural-language API cards (name/desc/args-with-types/example call) for retrieval-augmented calling — representative but non-standard.

### Comparison (format | structured/text | arg style | invoke | token cost)

- OpenAI/DeepSeek `tools` | structured JSON | JSON Schema | native tool_calls, args=JSON string | med (billed input)
- Anthropic `tools` | structured JSON | JSON Schema | native tool_use, input=object | med
- Gemini functionDeclarations | structured JSON | OpenAPI subset / parametersJsonSchema | native functionCall{args} | med
- MCP tools/list | structured JSON | JSON Schema input + OPTIONAL output | host -> tools/call -> model native | med-high
- ReAct/LangChain | text | `name(pysig) - desc` (args untyped) | free-text Action/Action Input | low
- smolagents CodeAgent | text | Python sig + docstring Args/Returns | model writes Python calling fn | med
- smolagents ToolCallingAgent | text (+native) | `name: desc / Takes inputs:{dict} / Returns type` | Action JSON blob {name,arguments} (+native) | low-med
- AutoGPT TS namespace | text | TS types + // JSDoc, `=> any` | model emits fn+args | low-med
- OpenHands XML | text | `(n) name (type, required/optional): desc` | `<function=..><parameter=..>` | med
- AutoGPT fmt_line / LC _and_args | text (compact) | `name: desc. Params: (a: type, b?: type)` | listing/token-count | very low

### Marix implications (concrete)

- DeepSeek native: render each `ToolPreview` -> `{"type":"function","function":{"name","description","parameters":<schema.input parsed as JSON>}}`; add `additionalProperties:false` (+ `strict:true` if honored). `schema.input` is ALREADY a JSON Schema doc — parse, don't double-encode. Do NOT send `schema.output` (no native API consumes it); keep it host-side (MCP-style) for result validation/UI/step typing. Preserve returned `tool_calls[i].id` (Marix has no field for it — add to `ExecutionSignature`/`ExecutionRequest`) to correlate the `{role:"tool",tool_call_id,content}` reply. `arguments` (JSON string) maps cleanly to `ExecutionRequest.tool_request`.
- Prompt-text fallback (non-tool-calling model): BEST fit for `ToolPreview{name,description,schema{input,output}}` (input already JSON Schema) is the **OpenHands `convert_tools_to_description`** style — deterministic walk of `properties`/`required` into `(n) name (type, required|optional): desc` blocks + `<function=..><parameter=..>`/JSON-blob invocation. Most token-efficient faithful option is **AutoGPT's TypeScript namespace**. Avoid ReAct `name - description` (drops arg schema) and smolagents Python-signature (assumes a Python exec runtime Marix lacks). Fold `schema.output` into text only for code-writing models (smolagents `Returns:` style).

## 2026-07-04 — Planning Output Formats for Agent Task Plans

Systems studied:

- OpenAI Structured Outputs / Agents SDK — https://developers.openai.com/api/docs/guides/structured-outputs, https://openai.github.io/openai-agents-python/agents/
- Anthropic Claude / Claude Code — https://platform.claude.com/docs/en/build-with-claude/prompt-engineering/claude-prompting-best-practices, https://platform.claude.com/docs/en/agents-and-tools/tool-use/overview, https://code.claude.com/docs/en/common-workflows
- LangGraph Plan-and-Execute — `LangChain-OpenTutorial/LangChain-OpenTutorial/docs/17-LangGraph/03-Use-Cases/05-LangGraph-Plan-and-Execute.md`
- LlamaIndex SubQuestion planning — `run-llama/llama_index/llama-index-core/llama_index/core/question_gen/{types.py,prompts.py}`
- AutoGPT classic — `Significant-Gravitas/AutoGPT/classic/original_autogpt/autogpt/agents/prompt_strategies/one_shot.py`
- BabyAGI archive — `yoheinakajima/babyagi_archive/babyagi.py`
- SWE-agent 0.7 — `SWE-agent/SWE-agent/config/sweagent_0_7/07_thought_action_xml.yaml`
- smolagents — `huggingface/smolagents/src/smolagents/prompts/code_agent.yaml`
- Aider modes — https://aider.chat/docs/usage/modes.html

Core modules observed:

- Planning is usually either (a) strict typed output (`Plan.steps`, `SubQuestionList.items`, JSON Schema/Pydantic), (b) free-text plan in a delimited section (`## 2. Plan`, `<end_plan>`), or (c) short-horizon ReAct/action formats where each turn includes a discussion/thought and one command.
- LangGraph's canonical pattern keeps `Plan.steps: list[str]`, state fields `input/plan/past_steps/response`, and a replanner returning either `Response` or `Plan`.
- smolagents separates a facts survey from the high-level plan and explicitly says not to detail individual tool calls, ending with `<end_plan>`.
- AutoGPT embeds plan as part of every action proposal (`thoughts.plan: list[str]`) rather than a standalone executable DAG.
- BabyAGI uses numbered-line task lists and reparses by splitting on periods; simple but brittle.
- LlamaIndex sub-question planning assigns each sub-question to a `tool_name`, a useful pattern for Marix execution steps.
- Claude/Anthropic guidance favors XML tags for complex prompts and native `tool_use` blocks with `strict:true` for schema-conformant tool calls; Claude Code plan mode is an approval-gated UX rather than a published machine-readable plan schema.
- SWE-agent constrains each turn to one `DISCUSSION` plus one XML `<command>...</command>`, prioritizing reliable stepwise execution over global plans.

Reusable architecture patterns:

- Use strict JSON Schema / Pydantic for plans that need host-side validation, UI rendering, resumption, and conversion to protocol events.
- Keep model-facing planner fields semantically small: step number, name, kind, brief, optional tool request, expected result, dependencies, and user gate semantics.
- Prefer a separate `TaskPlan` type over overloading `TaskResult.content`; keep an opaque `content` summary for human display but store typed steps for execution.
- For user approval, model explicit steps with `kind: "user"` and a small enum (`verdict`, `warrant`) rather than burying approval language inside a free-text execution step.
- For tool execution, follow LlamaIndex's `tool_name` assignment and OpenAI/Anthropic's schema-driven argument objects; do not ask the model to write shell text if a typed `ExecutionRequest` exists.
- Add repair loop: if strict parse fails, pass validation errors back to a "repair to schema only" model call. With strict structured outputs this should be rare, but non-OpenAI/local models still need it.

Risks / anti-patterns:

- Markdown checklists and BabyAGI numbered lists are cheap and readable but fragile under nested steps, missing numbers, translated punctuation, and mixed prose.
- AutoGPT-style `thoughts/reasoning/criticism/plan/speak` is verbose and may leak unnecessary reasoning; the `plan` field is not directly executable.
- XML tags are readable and Claude-friendly but require robust escaping and do not guarantee enum/schema validity.
- ReAct `Thought/Action` and SWE-agent command formats are good for incremental execution but weak as a previewable, approvable full task plan.
- Embedding full `ExecutionRequest` inside `StepKind` can make plan diffs and UI summaries awkward; consider a separate `planned_action` payload beside a compact signature.

Marix implications:

- Recommended planning response: strict JSON object `{task, steps, final_expected_result}` where `steps[]` has `signature:{step_no,name,kind}`, `brief`, `expected_result`, optional `tool`, optional `tool_request`, optional `requires_user`, optional `depends_on`, and optional `risk`.
- Map `kind.model.initial/job_plan` to Marix `StepKind::Model(ModelStepKind::{Initial,JobPlan})`; `kind.execution.invocation` + `tool_request` to `ExecutionStepKind::Invocation(ExecutionRequest)`; `kind.user.verdict/warrant` to `UserStepKind::{Verdict,Warrant}`.
- Current protocol likely needs a first-class `TaskPlan` / `PlannedStep` / `UserGate` type. `TaskPreview { result }` and `TaskResult { content }` are too opaque for validation, partial execution, and UI approval. `StepSignature` may also need task identity or plan identity for stable cross-task references.
