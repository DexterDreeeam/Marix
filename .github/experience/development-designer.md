# development-designer Experience

## Purpose

Persistent implementation notes for generating and maintaining {{proj}} source design documents.

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
- For every changed design-tracked source path under `src/`, update `.design.json` in that file's folder and every ancestor folder up to `src/`.
- The `ensure-deveopment-design` `agentStop` hook checks only design-tracked `src/` files written by the current agent turn and blocks completion if their ancestor `.design.json` files were not also updated.
- When explicit tag comparison data is available, use it as evidence. Otherwise update statuses from the current task's source changes and preserve unaffected definitions as `unchanged`.
- Element status is more precise than file status: a modified file can contain unchanged elements. Do not mark every element in a changed file as `modified`; only mark elements whose own definition, related impl blocks, public API, behavior, or owned source location changed.

## Update Workflow

- When source structure or public interfaces change, update the design document in the affected `src/` folder.
- Keep module-level purpose and element definitions aligned.
- Keep the JSON payload schema-compatible and machine-readable without custom prose parsing.

## Lessons

- 2026-06-19: Do not mark current source definitions `added` merely because design metadata is regenerated; when previous-tag comparison data is unavailable, default checked-in source modules and elements to `unchanged` until a real add/modify/delete/rename is known.
- 2026-06-20: Design refresh is enforced by the `ensure-deveopment-design` hook, not by proactive calls during normal tasks or by `git-sync`. When the hook blocks, invoke `development-designer` with the current-agent changed paths/sections so `.design.json` stays in sync before task completion.
- 2026-06-19: When the Rust crate root lives under `src`, Cargo metadata may inform root module purpose, but `src/Cargo.toml` itself is not an element. Keep Cargo dot-prefixed configuration hidden as companion metadata.
- 2026-06-21: For focused implementation-signature changes such as `src/core/preprocess/preprocessor.rs` switching to `InputChatMessage`, mark the owning element and direct source file `modified`; ancestor design files should not list descendant paths.
- 2026-06-21: A direct source file listed in `.design.json` `modified` does not imply every element in that file is modified. Preserve elements such as unchanged request/response structs as `unchanged` when their `codeSegments` text did not change.
- 2026-06-25: Treat `src/tests/` as integration-test code outside source-design metadata. Do not create `src/tests/.design.json`, list it as a child module, or require design updates for files under it.
- 2026-06-27: For engine/frontdoor API redrafts, exclude module wiring elements (`pub mod`, `pub use`) and root Cargo/config metadata; mark ancestor folders with `modified: ["."]` rather than listing changed child folders. Source: ensure-deveopment-design focused update for `src/agent/engine` and `src/agent/frontdoor`.
- 2026-06-27: When a Rust module changes from sibling file-plus-folder (`message.rs` plus `message/`) to folder-only (`message/mod.rs`), remove parent module-file elements, mark the deleted parent file in the parent `.design.json`, and move current public elements to the folder module metadata without listing module wiring. Source: focused message module layout update.
- 2026-06-27: When a correction splits a folder module entry from concrete definitions, keep `mod.rs` wiring-only in metadata, point elements at the concrete leaf file, and remove deleted aliases such as `TaskRuntimeReceiver` from current elements. Source: focused pre-tag correction for `src/common/message` and `src/agent/engine`.
- 2026-06-27: For style-only Rust refactors that move top-level helper functions into a main struct's private impl without changing crate-visible APIs, mark the owning struct/impl element and direct source file `modified`, update current line ranges, and keep sibling elements unchanged. Source: focused `src/agent/engine/loop_engine.rs` LoopEngine helper-method refactor.
- 2026-06-27: For hosted task-context frontdoor changes, mark only the owning task element lines that changed (for example `AgentTask::run` creating/returning `TaskContext`) and clear stale highlights on unchanged transport methods such as `send`, `receive`, and `complete`. Source: focused `src/agent/frontdoor/task.rs` update.
- 2026-06-27: For refactors that make an engine/session context cloneable to keep private helpers as `&self` methods, record the derive line as an added segment on the owning struct, mark the method/impl segment modified, and list only the direct changed files in the module status array. Source: focused `src/agent/engine/{loop_engine.rs,session_context.rs}` update.
- 2026-06-30: For new public child modules exposed by a parent `mod.rs`, keep the parent childModules entry status-free, mark the direct parent `mod.rs` as modified, and put `added: ["."]` in the new child module's own `.design.json`; for config-gated fields, highlight only the owning public struct's added field lines. Source: focused common logging module update.
- 2026-06-30: For logging shortcut API additions, record each new public shortcut function as its own `added` element in the owning logger file, keep re-export-only `mod.rs` changes out of `elements`, and leave moved private statics/constants out of selectable metadata. Source: focused common logging warning/error API update.
- 2026-06-30: For private runtime-path or log-directory behavior changes, keep private helpers out of `elements` and attach their line ranges to the affected public loading/logging element; for a new native catalog child module, mark the child module `added: ["."]` and each public enum/struct/const catalog item as `added` while leaving parent re-export wiring out of elements. Source: focused update for `src/common/config`, `src/common/logging`, and `src/client/tool/native`.
- 2026-06-30: When native tool metadata moves from manager/catalog files to folder-per-category implementation modules, keep parent `childModules` status-free, mark deleted direct files in the parent `deleted` array, and model each concrete native tool struct with segments for the struct, associated `DEFINITION`, `ToolClassification`, `Tool`, and `NativeTool` impls. Source: focused tool/native API layout redesign update.
- 2026-06-30: When removing native-only tool abstractions, delete stale NativeTool/NativeToolDefinition/ToolClassification elements from current metadata, model concrete tools as renamed Tool-implementing structs with ToolDefinition impl segments, and show PRIMARY_NATIVE_TOOL_LIST as its own native-list const. Source: focused src/client/tool native/category interface redesign.
- 2026-06-30: When descriptor APIs are removed in favor of ToolDefinition advertising, remove deleted descriptor elements from current metadata, record deleted source files at their direct parent, model moved classification enums under the new owner file, and update workflows from descriptor edges to definition edges. Source: focused src/client/tool category/module cleanup.
- 2026-06-30: When ToolDefinition/ToolCategoryDefinition are removed in favor of ToolPreview/CategoryPreview, delete those stale elements from current metadata, model preview structs as added public data records, attach concrete tool PREVIEW constants plus expanded Tool impl stubs to each tool struct, and update workflows from definition edges to preview edges. Source: focused `src/client/tool` preview interface redesign.
- 2026-06-30: When a Rust child module entry moves from `child/mod.rs` to sibling `child.rs` while keeping `child/` leaf files, model public entry definitions in the parent folder metadata, mark the parent direct `child.rs` as added, mark the child folder's deleted `mod.rs`, and update workflows to show the sibling entry feeding leaf preview/list files. Source: focused `src/client/tool/category` module-entry cleanup.
- 2026-06-30: When removing a public Rust tool submodule such as `mcp.rs`, delete the stale public element from current `elements`, record the direct file in the owning folder's `deleted` array, and update workflows to route registry preview APIs through current structs such as `DefaultPreview` and `ToolPreview`. Source: focused `src/client/tool` MCP removal and registry preview API draft.
- 2026-06-30: When the Tool trait changes from plural platform/category slices to singular `platform()` and `category()`, mark the Tool trait and every concrete Tool impl element modified, highlight only those method lines for stubs, and attach newly implemented private helper ranges to the owning public tool element for invoke behavior. Source: focused `src/client/tool` registry/native tool metadata update.
- 2026-06-30: For native tool behavior completion, keep concrete tool structs as the public elements and attach newly implemented private helper ranges to those structs; update workflows from stub/preview-only nodes to actual filesystem, reqwest/DNS, shell, system/env, PowerShell package, and image runtime edges. Re-export-only common::external wrappers such as ImageReader remain omitted from elements while direct files are tracked in status arrays. Source: focused client tool/native implementation metadata update.
- 2026-07-01: For a public runtime/cancel API draft, model `ToolRuntime` as an added struct with its impl segment, model `ToolInvocationStatus` as an added enum, remove deleted `ToolOutput`/`ToolOutcome` from current elements, and reduce native tool elements to struct/PREVIEW/Tool-impl stub segments instead of stale behavior helper ranges. Source: focused `src/client/tool` runtime/cancel interface redesign update.
- 2026-07-01: For native Tool::invoke runtime implementations, keep the concrete public tool struct as the selectable element, attach private parser/result/child-process/image-helper ranges to that struct's codeSegments, and update workflows from panic-stub edges to ToolRuntime status, cancellation, and data-source edges. Source: focused system/package/image native runtime implementation update.
- 2026-07-01: When moving platform selection from client tool abstractions into common config, remove stale ToolPlatform elements, model common::config::Platform as an added enum, mark Config/ToolRegistry/Tool and concrete Tool impl platform segments modified, and update workflows so registry default previews are fed from platform-filtered registered tools. Source: focused platform/config/tool registry migration.
- 2026-07-01: When native helper cancellation variants are unified behind a new public `ToolExecutionResult<T>`, model that enum as an added public element in `src/client/tool/tool.rs`, keep re-export-only `mod.rs` changes out of elements, and update each concrete native tool struct segment for helper `Result<ToolExecutionResult<_>, ToolError>` mappings while omitting removed private per-tool cancellation/error enums. Source: focused native tool cancellation helper result update.
- 2026-07-01: For interface-only agent context drafts, model new session/task/model context structs and enums as added elements, mark only draft accessor impl ranges modified, keep unchanged status enums unhighlighted, list the new direct file under `added`, and show workflow edges as model-facing views rather than persistent state ownership. Source: focused `src/agent/engine` SessionBrief/TaskTrace/ModelContext update.
- 2026-07-01: For agent frontdoor interface renames, rename current selectable elements (`Session`, `Task`, `TaskBrief`) instead of keeping deleted old names, remove deleted stale elements such as `TaskOutcome`, record moved source files as deleted/added in their owning folders, and move ModelContext workflow ownership from engine to model. Source: focused draft-feature interface rename/module move update.
- 2026-07-01: When a Tool method changes return type to a shared protocol struct (Tool::schema() -> common::protocol ToolSchema) and each native tool swaps a single `pub const PREVIEW: ToolPreview` for `pub const NAME/DESCRIPTION/SCHEMA` in an inherent impl, only the const block above the `impl Tool` block changes line count; shift every downstream codeSegment by that const-block delta, set modifiedLines to just the changed NAME..SCHEMA and the name()/description()/schema()/invoke-param lines, and clear (position-shift only) fossil highlights on untouched trailing helper segments. Source: focused client native tool schema/preview const migration.
- 2026-07-01: `ToolInvocation.arguments: String` -> `parameter: ToolParameter` and `ToolPreview.schema: &'static str` -> `ToolSchema` are same-line-count edits, so mark only those field lines modified and keep the rest of the struct/impl unchanged; ToolPreview losing Copy is a derive-line change, not a new element. Source: client/tool/tool.rs shared-protocol adoption.
- 2026-07-01: For a brand-new single-owner shared module used by two consumers (src/common/protocol owning ToolSchema/ToolParameter), model the module changeStatus "added" with top-level added:["."], list each struct definition + its inherent impl as separate codeSegments, and register it as a childModule (no changeStatus) in the parent. Reference the shared owner from consumers only at purpose/workflow level unless a consumer element concretely names the type — do not fabricate element-level protocol usage in agent/engine when its listed elements (Plan/Job/TaskStep) do not import the protocol types. Source: common::protocol shared tool schema/parameter introduction.
- 2026-07-01: Ancestor rollups whose only real change is in descendants use module.changeStatus "modified" + modified:["."] (never list descendant folder paths in top-level arrays), and communicate cross-module additions purely by editing the affected childModule purpose text and the module purpose (e.g. "client::tool sources ToolSchema/ToolParameter from common::protocol"; engine "model-produced Plan/Job planning"). When the ancestor's own mod.rs actually changed, list the real filename instead of ".". Source: src/agent, src/client, src rollups for common::protocol + engine planning.
- 2026-07-01: When `pub const` list arrays are replaced by `pub fn` builders (NATIVE_TOOL_LIST/PRIMARY_NATIVE_TOOL_LIST -> native_tools()/native_tool_list()/primary_native_tool_list()), delete the old const elements and add the public functions as new elements; verify the actual source rather than the request text (the real file exposed a public native_tools() builder, not the "private preview() helper" the brief mentioned). Source: native/native_tool_list.rs const-to-function migration.
