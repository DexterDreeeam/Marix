---
name: draft-feature
description: Draft Rust public interfaces for a requested feature without implementing behavior. Use when the user asks to draft a feature API.
---

## Purpose

Draft the public Rust interface for a requested feature without implementing the feature behavior.

This skill is for API shaping and review only. It may add or edit Rust source files, modules, traits, structs, enums, type aliases, constants, and public function or method signatures, but it must not add working behavior. Drafted code does not need to compile.

## Trigger

Use this skill when the user invokes `draft-feature` and appends the feature they want to design.

Examples:

- `draft-feature add local cache configuration`
- `draft-feature define the transport retry API`

## Workflow

1. **Understand Feature Boundary** — Identify the user-facing capability, public types, ownership boundaries, input/output data, errors, and module placement.
2. **Inspect Existing Public APIs** — Reuse existing naming, module layout, error conventions, and protocol types where appropriate.
3. **Draft Public Interface Only** — Add or modify only public API surfaces needed to describe the feature:
   - public traits,
   - public structs,
   - public enums,
   - public type aliases,
   - public constants when they are part of the API,
   - public free functions,
   - public inherent methods.
4. **Use Non-Implementation Stubs** — Where a method or function body is useful to show the intended public call shape, the body must be only:

   ```rust
   panic!("not implemented")
   ```

   Trait methods should use signature-only declarations with semicolons unless a body is part of the public API shape. Do not add bodies, fields, helper types, or implementation scaffolding only to satisfy the compiler.
5. **Review the Interface** — After editing, review the drafted public interface for cohesion, naming, ownership, error shape, extensibility, and whether it exposes too much or too little.
6. **Report** — Summarize the public API that was drafted, any files added or edited, and the interface review findings.

## Rules

- Do not implement feature behavior.
- Do not add private helper functions, private helper methods, internal algorithms, parsing logic, validation logic, IO, networking, threading, persistence, or background work.
- Drafted Rust code does not need to compile.
- Do not add fields, marker types, `PhantomData`, constructors, impl blocks, type bounds, or visibility only to make the draft compile.
- Do not add tests for behavior or compilation shape.
- Do not use `todo!()` or `unimplemented!()`; use only `panic!("not implemented")` when a body is required.
- Do not add default trait method implementations unless the default body is strictly part of the public API contract and contains only `panic!("not implemented")`.
- Do not implement a trait for a concrete type unless the user explicitly asks for that relationship to be part of the public interface. If such an impl is necessary, every required method body must contain only `panic!("not implemented")`.
- Keep visibility private by default. Use `pub` only for intentional feature-facing API.
- Prefer typed errors and explicit result types when callers need to handle failures.
- Prefer small cohesive modules over broad catch-all files.
- Keep code comments and rustdoc in English.
- Do not update non-Rust behavior, deployment files, generated files, or overview UI as part of this skill unless the user explicitly asks.
- If the requested feature cannot be represented cleanly as public interface only, stop and ask one focused clarification question.

## Rust Interface Checklist

Use this checklist before reporting:

- Public names match existing Rust naming conventions.
- New modules are wired only as needed for public access.
- Public structs expose only fields that must be directly constructed or inspected by callers.
- Public enums cover caller-visible states or errors without encoding internal implementation details.
- Trait methods describe capability boundaries rather than algorithm steps.
- Function and method signatures express the intended public API shape; compilation is not required.
- Structs may be fieldless drafts even when generic parameters would require implementation scaffolding to compile.
- No private helper APIs or real behavior were added.
- No source text contains `todo!()` or `unimplemented!()`.

## Source Design Metadata

If Rust source files under `src/` are changed, keep source design metadata consistent through the normal source-design workflow. If the source-design hook blocks, pass the changed design-tracked source paths and interface intent to the design metadata updater before completing the task.

## Reporting

Report:

- feature boundary drafted,
- public traits, structs, enums, type aliases, constants, functions, and methods added or changed,
- files added or changed,
- any deliberate omissions,
- interface review findings,
- whether any function bodies were included as `panic!("not implemented")` stubs,
- whether the draft intentionally does not compile and why.
