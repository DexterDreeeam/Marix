---
name: design-json-update
description: Update {{proj}} src/.design.json metadata. Use from development-designer after ensure-deveopment-design blocks, before git-tag, or directly when the user asks to refresh source design JSON.
---

## Purpose

Update `.design.json` source companion metadata for Rust source modules under `src/`.

This skill owns the mechanics of discovering which `.design.json` files need updates and how complete each metadata file must be. `development-designer` should delegate `.design.json` writing to this skill after the `ensure-deveopment-design` hook blocks.

## Inputs

- **Focused update** — The caller provides a list of changed design-tracked source paths under `src/`, plus the changed portions or intent. This is the normal hook flow:
  `ensure-deveopment-design` → `development-designer` → `design-json-update`.
- **Full update** — If the user invokes this skill directly without a file list, refresh every `.design.json` under `src/` and create missing `.design.json` files for every design-tracked source folder.
- **Pre-tag update** — When invoked from `git-tag`, refresh `.design.json` before the tag commit so `changeStatus` records the actual source changes being tagged.

## Path Rules

- A non-dot source path is any path under `src/` where neither the file name nor any parent directory segment starts with `.`.
- A design-tracked source path is a non-dot source path that is not under `src/tests/`.
- `src/tests/` contains integration tests. Ignore that folder completely for design metadata: do not create `src/tests/.design.json`, do not list it as a child module, and do not treat its files as source changes.
- Every dot-prefixed file or folder under `src/` is companion metadata and must not be listed as a normal source file, child module, or source change entry.
- `.design.json` is the only design metadata format. Do not generate, parse, or preserve legacy Markdown design metadata.

## Design File Selection

For a focused update:

1. Normalize the provided changed paths.
2. Keep only design-tracked source paths under `src/`.
3. For each changed file, update `.design.json` in:
   - the file's direct folder,
   - every ancestor folder,
   - up to and including `src/.design.json`.
4. If a changed path is a folder-level source change, update that folder and every ancestor up to `src/`.

For a full update:

1. Walk `src/`.
2. Ignore every dot-prefixed file/folder and the `src/tests/` folder.
3. Treat every remaining non-dot folder under `src/` as a source module.
4. Update or create `.design.json` in every source module folder.

## Required Metadata Completeness

Each `.design.json` must describe the current source truth for its module:

- `module`: path, name, purpose, and `changeStatus` when known.
- `childModules`: direct design-tracked child source folders only. Do not put `changeStatus` on child module entries; each child folder records its own folder status in its own `.design.json`.
- Direct module/file status arrays for changed items in the current folder only:
  - `added`,
  - `modified`,
  - `deleted`,
  - `renamed`.
- `elements`: one entry per meaningful public definition in the module's direct source files:
  - traits,
  - structs,
  - enums,
  - type aliases,
  - constants/statics,
  - public functions,
  - meaningful impl blocks,
- Every element should include only:
  - `name`,
  - `type`,
  - `changeStatus`,
  - `codeSegments`.
- Do not store signatures or copied source code in `.design.json`.
- `codeSegments` is an array of implementation locations. Each segment must include `sourcePath`, `lineStart`, `lineEnd`, and optionally `language`.
- Each code segment should also include `addedLines` and `modifiedLines` arrays. Each entry is a source-line range object such as `{ "lineStart": 12, "lineEnd": 14 }` using the same absolute source-file line numbers as the segment. Use empty arrays when no lines of that kind exist.
- For `added` elements, either mark the element `changeStatus` as `added` or set segment `addedLines` to cover the whole segment; overview renders added elements as fully green.
- For `modified` elements, set `addedLines` for newly inserted source lines and `modifiedLines` for existing source lines whose content or behavior changed. Do not mark unchanged lines inside the same segment.
- A single element may have multiple `codeSegments`, for example a struct definition plus one or more impl blocks.

## Elements

- `elements` must include one entry per concrete public definition that downstream tools may select.
- Include public traits, structs, enums, type aliases, constants, statics, and public functions.
- Do not include import/export wiring such as `mod ...`, `pub mod ...`, `pub use ...` in `elements`.
- Do not include Cargo manifests or other package metadata as `elements`; they can affect module purpose but are not selectable source elements.
- Do not expose single-field tuple wrappers unless they have meaningful behavior beyond wrapping.
- Use semantic `type` values such as `trait`, `struct`, `enum`, `type-alias`, `const`, `static`, or `function`; downstream renderers decide shape/color.

## Status Rules

- Use actual source changes to set `changeStatus`.
- Valid statuses: `unchanged`, `added`, `modified`, `deleted`, `renamed`.
- Do not mark an item `added` merely because metadata was regenerated.
- If previous-tag comparison data is unavailable, preserve unaffected current source definitions as `unchanged`.
- File-level changes do not automatically make every element in that file changed. Determine each element status from changes that overlap that element's own `codeSegments`.
- Mark an element `modified` only when the element's own definition, related impl block, public API, behavior, or owned source location changed. If unrelated imports, sibling elements, wiring, or nearby code changed in the same file, keep the unaffected element `unchanged`.
- Updating `codeSegments` line numbers because surrounding code moved does not by itself make an element `modified`; the element can remain `unchanged` when its own source text and meaning are unchanged.
- When precise element-level evidence is unavailable for a modified file, prefer preserving existing element statuses and report the ambiguity instead of blanket-marking all file elements as `modified`.
- Also write top-level status arrays for every changed direct item in that `.design.json` module:
  - Use `"."` to represent the module folder itself.
  - Use direct file names such as `"lib.rs"` or `"user_input.rs"` for files directly inside the module folder.
  - Do not list child folders in the parent `.design.json`, and do not put `changeStatus` on `childModules`; each child folder records its own `"."` status in its own `.design.json`.
  - If the whole module folder is newly added, `added: ["."]` is sufficient; direct files do not need to be repeated unless they require a different status.
  - Omit empty status arrays; unchanged files should not be listed.
- Keep element `changeStatus` aligned with actual source changes, but file tree status is represented by the top-level status arrays and dynamic tag diff, not by element `codeSegments`.

## Validation

After updating:

1. Parse every changed `.design.json` as JSON.
2. Confirm no legacy Markdown design metadata files exist.
3. Confirm no dot-prefixed source path and no `src/tests/` path is listed as a normal child module or file.
4. Confirm `childModules` entries do not contain `changeStatus`.
5. Confirm top-level status arrays list only `"."` or direct current-folder file names, never child folder paths or unchanged files.
6. For each changed design-tracked source file, compare public definitions in source with top-level `elements`; verify each element has all relevant `codeSegments`, including impl blocks where applicable. Report any deliberate omissions.
7. Confirm element `changeStatus` is not inherited from file status wholesale: unchanged elements in modified files must remain `unchanged` when their own `codeSegments` did not change.
8. Ensure `ensure-deveopment-design` would return `allow` for the current agent's changed files.

## Reporting

Report:

- input mode: focused or full,
- changed source paths considered,
- `.design.json` files updated,
- public definitions intentionally omitted, if any,
- validation results.
