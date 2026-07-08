---
name: feature-implement
description: Implement feature behavior using existing Rust public interfaces. Use when the user asks to implement/实现 a feature, not when they ask to design/设计 the API.
---

## Purpose

Implement feature behavior on top of an already designed Rust interface.

This skill is for filling in behavior behind existing public interfaces. It should not shape a new public API. Use `feature-design` instead when the user asks to design/设计 the feature interface.

## Trigger

Use this skill when the user invokes `feature-implement`, asks to implement a feature, or uses the Chinese keyword `实现` for a feature request.

Do not use this skill when the user asks to design/设计 an API or feature boundary. That is `feature-design`.

Examples:

- `feature-implement add local cache configuration`
- `feature-implement complete the transport retry behavior`
- `实现 task runtime status streaming`

## Boundary With feature-design

- `feature-design` designs the public interface. It may add or modify public structs, public struct data, public enums, public traits, public type aliases, public constants, public functions, and public methods. It should not focus on private helpers or behavior.
- `feature-implement` implements behavior under the existing interface. It must not add new public enums, structs, functions, methods, traits, type aliases, constants, or public data fields.

## Workflow

1. **Understand Existing Design** — Identify the requested behavior and the public interface that is supposed to support it.
2. **Inspect Current Contracts** — Read the relevant public types, methods, errors, configuration, and protocol messages before editing.
3. **Check Feasibility Before Editing** — Decide whether the current public interface can support a correct and natural implementation.
4. **Stop on Interface Gaps** — If the current public interface cannot support the behavior, would make the implementation infeasible, or would force an awkward workaround, stop immediately. Tell the user that the current outward-facing interface cannot implement the feature cleanly, propose one concrete interface improvement, and wait for the user's next instruction. Do not implement the workaround and do not update the design yourself.
5. **Implement Behind the Interface** — Add or modify private functions, private methods, private data, internal algorithms, IO, validation, persistence, runtime orchestration, and error handling only as needed to satisfy the existing contract.
6. **Preserve Public Shape** — Keep all public API names, visibility, data shapes, and semantic contracts unchanged unless the user explicitly approves a design change.
7. **Verify Compilation** — Run `cargo check` from `src/` and ensure it passes cleanly before reporting. Run tests only when the user explicitly asks.
8. **Report** — Summarize what behavior was implemented, files changed, and any remaining limitations.

## Rules

- Do not add new `pub`, `pub(crate)`, or `pub(super)` enums, structs, traits, type aliases, constants, free functions, inherent methods, trait methods, or public data fields.
- Do not widen visibility of existing items.
- Do not change an existing public signature, public enum variant set, public struct field set, or public trait contract unless the user explicitly approved that design change.
- Do not implement by relying on strange adapters, hidden global state, lossy conversions, or duplicated protocol types to avoid admitting an interface gap.
- Do not silently ignore unsupported inputs or errors. Surface errors through the existing error handling pattern.
- Do not add broad catch-all fallbacks that make failures look successful.
- Prefer existing helpers, configuration boundaries, protocol types, and module ownership.
- Keep code comments and rustdoc in English.
- If Rust source files under `src/` are changed, keep source design metadata consistent through the normal source-design workflow.

## Feasibility Stop Message

When stopping because the current public interface is insufficient, report:

- the requested behavior,
- the existing public interface that blocks a clean implementation,
- why implementation would be impossible, unsafe, or awkward under the current contract,
- one concrete design improvement suggestion,
- that you are waiting for user approval before changing the interface.

## Reporting

Report:

- behavior implemented,
- public interfaces preserved,
- files added or changed,
- any deliberate omissions,
- any interface gaps discovered and whether work was stopped for user approval.
