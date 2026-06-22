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
