# mx-external-researcher Experience

## Purpose

Persistent research notes for external AI agent implementations. Keep notes source-backed and reusable for {{proj}} architecture work.

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

{{proj}} takeaway:

- Model the {{proj}} core around a typed agent loop and self-describing tools, not around one-off command handlers.
- Treat context compaction and permission modes as first-class architecture, not later optimizations.

### OpenClaw

Sources:

- https://openclaw.im/

Reusable findings:

- OpenClaw focuses on workflow automation and message routing across many channels.
- Core ideas: programmable workflow engine, universal message router, stateful context manager, plugin architecture, BYOM model layer, and self-hosted auditability.
- Its strength is cross-channel orchestration rather than code editing alone.

{{proj}} takeaway:

- A workflow DSL and plugin registry can complement a coding-agent loop when {{proj}} expands beyond terminal/code interactions.

### OpenHands

Sources:

- https://docs.openhands.dev/
- https://github.com/OpenHands/OpenHands

Reusable findings:

- OpenHands emphasizes runtime/sandbox architecture: backend sends actions to a Docker or remote sandbox and receives observations.
- Event streams of actions and observations make execution auditable and replayable.
- The sandbox boundary is the key safety primitive for arbitrary code execution.

{{proj}} takeaway:

- Use an action/observation event model if {{proj}} needs replay, audit, pause/resume, or remote runtime support.
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

{{proj}} takeaway:

- Git/diff/undo should be a first-class workflow module.
- Separate plan/ask/build modes reduce accidental edits and improve UX.

### OpenCode

Sources:

- https://opencode.ai/docs/

Reusable findings:

- OpenCode provides TUI, desktop, and IDE surfaces.
- It initializes projects with `AGENTS.md`, supports plan/build modes, undo/redo, shared conversations, themes, commands, keybinds, and provider configuration.

{{proj}} takeaway:

- Project-local instructions and plan/build mode are important UX primitives.
- Undo/redo should be considered part of the editing contract, not just git recovery.

### Continue

Sources:

- https://docs.continue.dev/

Reusable findings:

- Continue represents the IDE-native context-provider architecture: editor selection, files, terminal, docs, and codebase index provide context.
- It supports VS Code, JetBrains, and CLI modes, with configurable model providers.

{{proj}} takeaway:

- A context-provider abstraction would let {{proj}} reuse the agent loop across CLI, IDE, and web surfaces.

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

{{proj}} implications:

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

{{proj}} implications:

- {{proj_lower}}'s current tool/category and native folder split is directionally right, but category should stay descriptive rather than policy-driving.
- First batch should emphasize file read/list/search, patch/edit, shell execution with strict permissions, system/env inspection, web fetch/search if needed, and memory/instructions hooks.
- Defer image transform, generic package query, broad browser automation, service APIs, and database/cloud integrations unless a concrete workflow needs them.
