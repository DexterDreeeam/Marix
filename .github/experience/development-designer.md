# development-designer Experience

## Purpose

Persistent implementation notes for generating and maintaining Marix source design documents.

## Current Design Contract

- Maintain `.design.json` files under every `src/` folder.
- The `design-json-update` skill owns the mechanics of selecting and updating `.design.json` files. `development-designer` should use that skill when the hook provides changed source paths.
- JSON is the only source-design companion metadata format going forward.
- Every dot-prefixed file or folder under `src/` is companion metadata owned by development-designer. Maintain these paths beside source files, but never list them as normal source files, child modules, or source change entries.
- `.design.json` content should be machine-readable JSON.
- All paths should be repository-rooted and under `src/`.
- Do not list dot-prefixed files or folders as child modules/files.
- Keep design documents concise and source-focused.

## Extraction Rules

- Treat every folder under `src/` as a module.
- Describe direct child modules and direct child source files.
- List meaningful interfaces, traits, structs, enums, impl blocks, functions, type aliases, constants, statics, and data structures as top-level `elements`.
- Every element should include `name`, `type`, `changeStatus`, and `codeSegments`.
- Do not store signatures or copied source code in metadata.
- `codeSegments` should point to implementation file and line ranges; one item may have multiple segments, such as a struct plus its impl blocks.

## Elements

- `elements` lists public source definitions that downstream consumers may display.
- Include only concrete public definitions owned by the module layer.
- Do not include wiring declarations such as `mod ...`, `pub mod ...`, or `pub use ...` in `elements`.
- Do not include Cargo manifests or package metadata as `elements`; they can inform module purpose but are not source elements.
- Do not expose single-field tuple wrappers such as `pub struct ModuleId(pub String);` unless they have meaningful behavior beyond wrapping.
- Create one exposed element per public definition; never combine names with `/`, commas, or summary labels.
- Use concise `type` values such as `trait`, `struct`, `enum`, `type-alias`, `const`, `static`, or `function`.
- For struct elements, keep the struct as the primary metadata unit and list related impl/method implementation ranges in `codeSegments`; avoid scattering every impl method unless it is a standalone public function/API.

## Status Rules

- Include `changeStatus` when known on modules and elements.
- Use top-level status arrays (`added`, `modified`, `deleted`, `renamed`) for the current folder and its direct files only. Use `"."` for the current folder itself; do not put `changeStatus` on `childModules`.
- Valid statuses are `unchanged`, `added`, `modified`, `deleted`, and `renamed`.
- Prefer explicit item-level `changeStatus`.
- Status values should reflect source changes, not metadata generation side effects.
- `development-designer` should be triggered only by the `ensure-deveopment-design` hook, not proactively during normal source-editing tasks.
- For every changed non-dot source path under `src/`, update `.design.json` in that file's folder and every ancestor folder up to `src/`.
- The `ensure-deveopment-design` `agentStop` hook checks only non-dot `src/` files written by the current agent turn and blocks completion if their ancestor `.design.json` files were not also updated.
- When explicit tag comparison data is available, use it as evidence. Otherwise update statuses from the current task's source changes and preserve unaffected definitions as `unchanged`.

## Update Workflow

- When source structure or public interfaces change, update the design document in the affected `src/` folder.
- Keep module-level purpose and element definitions aligned.
- Keep the JSON payload schema-compatible and machine-readable without custom prose parsing.

## Lessons

- 2026-06-19: Do not mark current source definitions `added` merely because design metadata is regenerated; when previous-tag comparison data is unavailable, default checked-in source modules and elements to `unchanged` until a real add/modify/delete/rename is known.
- 2026-06-20: Design refresh is enforced by the `ensure-deveopment-design` hook, not by proactive calls during normal tasks or by `git-sync`. When the hook blocks, invoke `development-designer` with the current-agent changed paths/sections so `.design.json` stays in sync before task completion.
- 2026-06-19: When the Rust crate root lives under `src`, Cargo metadata may inform root module purpose, but `src/Cargo.toml` itself is not an element. Keep Cargo dot-prefixed configuration hidden as companion metadata.
- 2026-06-21: For focused implementation-signature changes such as `src/core/preprocess/preprocessor.rs` switching to `InputChatMessage`, mark the owning element and direct source file `modified`; ancestor design files should not list descendant paths.
