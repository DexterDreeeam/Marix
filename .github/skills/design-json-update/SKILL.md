---
name: design-json-update
description: Update Marix src/.design.json metadata. Use from development-designer after ensure-deveopment-design blocks, before git-tag, or directly when the user asks to refresh source design JSON.
---

## Purpose

Update `.design.json` source companion metadata for Rust source modules under `src/`.

This skill owns the mechanics of discovering which `.design.json` files need updates and how complete each metadata file must be. `development-designer` should delegate `.design.json` writing to this skill after the `ensure-deveopment-design` hook blocks.

## Inputs

- **Focused update** — The caller provides a list of changed non-dot source paths under `src/`, plus the changed portions or intent. This is the normal hook flow:
  `ensure-deveopment-design` → `development-designer` → `design-json-update`.
- **Full update** — If the user invokes this skill directly without a file list, refresh every `.design.json` under `src/` and create missing `.design.json` files for every non-dot source folder.
- **Pre-tag update** — When invoked from `git-tag`, refresh `.design.json` before the tag commit so `changeStatus` records the actual source changes being tagged.

## Path Rules

- A non-dot source path is any path under `src/` where neither the file name nor any parent directory segment starts with `.`.
- Every dot-prefixed file or folder under `src/` is companion metadata and must not be listed as a normal source file, child module, or source change entry.
- `.design.json` is the only design metadata format. Do not generate, parse, or preserve legacy Markdown design metadata.

## Design File Selection

For a focused update:

1. Normalize the provided changed paths.
2. Keep only non-dot source paths under `src/`.
3. For each changed file, update `.design.json` in:
   - the file's direct folder,
   - every ancestor folder,
   - up to and including `src/.design.json`.
4. If a changed path is a folder-level source change, update that folder and every ancestor up to `src/`.

For a full update:

1. Walk `src/`.
2. Ignore every dot-prefixed file/folder.
3. Treat every non-dot folder under `src/` as a source module.
4. Update or create `.design.json` in every source module folder.

## Required Metadata Completeness

Each `.design.json` must describe the current source truth for its module:

- `module`: path, name, purpose, and `changeStatus` when known.
- `childModules`: direct non-dot child source folders only.
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

## Validation

After updating:

1. Parse every changed `.design.json` as JSON.
2. Confirm no legacy Markdown design metadata files exist.
3. Confirm no dot-prefixed source path is listed as a normal child module or file.
4. For each changed source file, compare public definitions in source with top-level `elements`; verify each element has all relevant `codeSegments`, including impl blocks where applicable. Report any deliberate omissions.
5. Ensure `ensure-deveopment-design` would return `allow` for the current agent's changed files.

## Reporting

Report:

- input mode: focused or full,
- changed source paths considered,
- `.design.json` files updated,
- public definitions intentionally omitted, if any,
- validation results.
