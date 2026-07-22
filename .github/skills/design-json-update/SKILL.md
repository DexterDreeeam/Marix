---
name: design-json-update
description: Update Marix src_meta/**/design.json source-design metadata. Requires mode parameter "changed" or "full". This skill can be triggered ONLY by git-sync/git-tag skills and explicit user command.
---

## Purpose

Update `design.json` source companion metadata under `src_meta/`, mirroring the design-tracked folder structure under `src/`.

This skill owns `.temp/changed` processing, design file selection, schema, source extraction rules, status rules, line-range rules, and accumulated source-design experience.

## Inputs

- Required mode parameter: `changed` or `full`.
- If the mode parameter is missing, ask the user whether to run `changed` or `full`.

## Mode Workflow

### `changed`

1. Inspect every file directly under `.temp/changed/`.
2. Treat marker file contents as changed-file lists. Merge every non-empty line from all marker files into one deduplicated changed-file list.
3. Normalize paths to repo-relative forward-slash paths and preserve first-seen order.
4. Keep only design-tracked source paths.
5. If no design-tracked source path remains, clear `.temp/changed/` and report that there was no source-design work.
6. Build the list of `design.json` files to update. A `design.json` surfaces only two layers: its own module's elements and its direct child modules' summaries. So a changed file requires updating only the `design.json` for the file's direct source folder and that folder's immediate parent `design.json`; do not propagate to further ancestors up toward `src_meta/design.json`. For a folder-level source change, update the changed folder's `design.json` and its immediate parent `design.json`. The `src/` root maps to `src_meta/design.json` and has no parent.
7. Deduplicate the collected `design.json` list.
8. Update the deduplicated `design.json` files from deepest to shallowest.
9. After every required `design.json` update succeeds, clear `.temp/changed/` by deleting every marker file in that folder.
10. If any update fails, do not clear `.temp/changed/`; report the failure and leave marker files for retry.

### `full`

1. Walk `src/`.
2. Ignore any folder named `tests` and everything under it, at any depth (any path containing a `tests` segment, e.g. `src/tests/`, `src/common/tests/`, `src/server/session/tests/`).
3. Treat every remaining folder under `src/` as a design-tracked source module.
4. Sort source module folders from deepest to shallowest, with `src/` last.
5. Update or create `design.json` at the mirrored `src_meta/` path for every source module folder in that bottom-up order.
6. After every required `design.json` update succeeds, clear `.temp/changed/` by deleting every marker file in that folder.
7. If any update fails, do not clear `.temp/changed/`; report the failure and leave marker files for retry.

## Path Rules

- Source code stays under `src/`; companion metadata lives under `src_meta/`.
- A source module `src/<rel>` maps to `src_meta/<rel>/design.json`; the `src/` root maps to `src_meta/design.json`.
- Inside `design.json`, `module.path` and every `codeSegments[].sourcePath` must still point to `src/...`, never to `src_meta/...`.
- A design-tracked source path is under `src/` and has no path segment named `tests` (i.e. its path does not contain `/tests/` and it is not directly under a `src/tests` root).
- Ignore any `tests` folder entirely, at any depth: a folder named `tests` and everything under it gets no companion, no child module, and no source status entry. This lets a module keep its own `tests/` folder without it appearing in design metadata.
- `design.json` is machine-readable JSON only. Do not generate or preserve legacy Markdown design metadata.

## Schema

Use this top-level order:

1. `schemaVersion`
2. `module`
3. `added`
4. `modified`
5. `renamed`
6. `deleted`
7. `childModules`
8. `elements`

Required shape:

```json
{
  "schemaVersion": 1,
  "module": {
    "path": "src/example",
    "name": "example",
    "purpose": "Owns example orchestration and public helpers.",
    "changeStatus": "modified"
  },
  "added": ["new_message.rs"],
  "modified": ["runtime.rs"],
  "renamed": ["config.rs"],
  "deleted": ["old_runtime.rs"],
  "childModules": [
    {
      "path": "src/example/runtime",
      "name": "runtime",
      "purpose": "Runs example workflows."
    }
  ],
  "elements": [
    {
      "name": "ExampleRunner",
      "type": "struct",
      "source_depth": 2,
      "changeStatus": "modified",
      "codeSegments": [
        {
          "sourcePath": "src/example/runtime.rs",
          "lineStart": 6,
          "lineEnd": 42,
          "language": "rust",
          "addedLines": [
            {
              "lineStart": 30,
              "lineEnd": 34
            }
          ],
          "modifiedLines": [
            {
              "lineStart": 12,
              "lineEnd": 12
            }
          ]
        }
      ]
    }
  ]
}
```

Omit empty status arrays. `childModules` entries must not contain `changeStatus`.

## Module Rules

- Each `design.json` describes one source module folder.
- `module.path` is the `src/...` module path.
- `module.name` is the source folder name; use `src` for the root module.
- `module.purpose` states current truth, not history. Convey removals through absence and status arrays, not prose such as "removed X".
- `module.changeStatus` uses the current tag-window status when known.
- `childModules` lists direct design-tracked child source folders only.
- Do not list child folders in the parent status arrays. Child folders record their own `"."` status in their own `design.json`.
- A pure-descendant immediate parent with no direct file changes uses `modified: ["."]` and communicates the descendant change through purpose and child module summaries.

## Element Rules

- `elements` contains one entry per concrete outward-facing definition owned by this module layer and by its direct child modules (one layer down). Do not include definitions from deeper descendant modules.
- Build `elements` only from Rust source files (`*.rs`).
- Treat `pub`, `pub(crate)`, `pub(super)`, and `pub(in ...)` as outward-facing.
- `function` elements are module-scope free functions only. Keep methods, including trait methods, in the owning type's `codeSegments`; omit them when the owner is not represented.
- A private struct or enum may be an element only when it genuinely coordinates module behavior.
- Do not include import/export wiring: `mod`, `pub mod`, `pub use`, private helper wiring, or package metadata.
- Do not include Cargo manifests as elements; they can affect module purpose but are not selectable source elements.
- Do not expose single-field tuple wrappers unless they have meaningful behavior beyond wrapping.
- Create one element per definition. Do not combine names with `/`, commas, or summary labels.
- Use semantic `type` values: `trait`, `struct`, `enum`, `type-alias`, `const`, `static`, `function`, `class`, `global`, or `bin`.
- Every element includes only `name`, `type`, `source_depth`, `changeStatus`, and `codeSegments`.
- `source_depth` is the number of segments in the element's own source folder path: `src` = 1, `src/common` = 2, `src/common/config` = 3. Own-layer elements match this module's `module.path` depth; direct child module elements are one level deeper. A `design.json` therefore holds only elements at the module's depth and one level below.
- Do not store signatures or copied source code in metadata.

## Code Segment Rules

- `codeSegments` points to current source locations and never embeds source code.
- Each segment includes `sourcePath`, `lineStart`, `lineEnd`, `language`, `addedLines`, and `modifiedLines`.
- Start a segment at the relevant doc/attribute/derive/visibility line and end at the closing `}` or `;`.
- Include related impl blocks as additional segments on the owning struct/enum/trait element when appropriate.
- A single element may have multiple segments, such as a struct definition plus multiple impl blocks.
- `addedLines` means inserted code. `modifiedLines` means existing changed lines.
- Import, `use`, module wiring, and unrelated nearby lines are not highlighted unless they are part of the owned element's meaningful change.
- A pure deletion may have no line to highlight: set the owning element `modified` with empty highlight arrays.
- Any inserted or removed line shifts later ranges. Recompute all segment bounds from current source for every touched `design.json`, including unchanged sibling elements, so ranges stay in bounds.
- Count source lines with APIs that preserve the real file length; do not rely on line counters that undercount files without final newlines.

## Status Rules

- Valid statuses: `unchanged`, `added`, `modified`, `deleted`, `renamed`.
- Base status on actual source changes, not metadata regeneration.
- Do not mark an item `added` merely because metadata was regenerated or created.
- File-level changes do not automatically make every element in that file changed.
- Mark an element `modified` only when its own definition, related impl block, public API, behavior, or owned source location changed.
- If unrelated imports, sibling elements, wiring, or nearby code changed in the same file, keep unaffected elements `unchanged`.
- Updating line numbers because surrounding code moved does not by itself make an element `modified`.
- When precise element-level evidence is unavailable, preserve existing element statuses and report the ambiguity instead of blanket-marking elements as `modified`.
- Top-level status arrays cover the current folder and direct files only:
  - Use `"."` for the module folder itself.
  - Use direct file names such as `"lib.rs"` or `"config.rs"` for files directly inside that folder.
  - Omit unchanged files.
  - If the whole module folder is newly added, `added: ["."]` is sufficient unless direct files need different statuses.
- Accumulate statuses across a tag window until the next reset. Do not reset an already changed element you did not touch.

## Structural Change Rules

- File rename: put the new direct filename in `renamed`, repoint affected `sourcePath` values, and mark elements `modified` only if their body also changed.
- Type or identifier rename: rename the element in place; never keep the old element or add a duplicate. Use `renamed` or `modified`, and highlight the declaration line.
- Method rename: do not use file-level `renamed` or create a function element; mark the owning struct, enum, trait, or class element `modified` and highlight the relevant definition/call sites when they are in owned segments.
- Flat-file to folder split: put the old direct file in the parent's `deleted` only if it existed before, add the child to `childModules`, and create the child `design.json` with `module.changeStatus: "added"`, `added: ["."]`, and moved elements as `added`.
- Moved type: represent it as `added` in the new owner and absent from the old owner. Do not fabricate deleted folder metadata.
- Added-then-modified within one tag window remains `added` until the next reset.
- After rename/removal, scan companions for stale banned tokens and keep only intentional historical prose or deleted-array filenames.

## Reset and Tag Interaction

- `design-json-reset` flips module/element `changeStatus` to `unchanged` and drops top-level status arrays.
- Reset may leave old `addedLines` or `modifiedLines` fossils. When flipping an element to `modified` or `added`, clear fossils and recompute highlights from current evidence.
- Leave genuinely unchanged elements' old highlight arrays untouched unless their segment bounds must be recomputed for a touched file.
- Do not run a full source re-scan during `git-tag` unless explicitly requested. Design ranges should be maintained incrementally after source edits.

## Grounding Rules

- Prefer the caller's changed-path list plus directly viewed current source.
- Trust source and provided diffs over prose. Callers can mislabel add/modify/rename cases.
- If a caller supplies a cumulative diff, use it as evidence but touch only the current wave's required modules.
- Do not run git unless the user explicitly asks.
- Keep design content concise, source-focused, and machine-readable.
- Write design files in English.
- Do not add generated manifest JSON files.

## Editing Rules

- Parse JSON and mutate structured data; avoid raw whole-file string replacement for metadata changes.
- Preserve each touched file's existing line ending style and write UTF-8 without BOM.
- Rebuild key order as `schemaVersion`, `module`, status arrays, `childModules`, `elements`.
- Prefer inline arrays for pure-string status arrays, such as `["a.rs", "b.rs"]`.
- Preserve unrelated entries and statuses whenever possible.

## Validation

After updating:

1. Parse every changed `design.json` as JSON.
2. Confirm no legacy Markdown design metadata was created.
3. Confirm no path with a `tests` segment (any `/tests/` path, at any depth) is listed as a normal child module or source file.
4. Confirm `childModules` entries do not contain `changeStatus`.
5. Confirm top-level status arrays list only `"."` or direct current-folder filenames.
6. Confirm every element has `name`, `type`, `source_depth`, `changeStatus`, and `codeSegments`.
7. Confirm every `codeSegments[].sourcePath` is a Rust source file and all ranges are in bounds.
8. Confirm unchanged elements in modified files remain `unchanged` when their own segments did not change.
9. Confirm every `function` element is module-scope; methods appear only in the owning type's `codeSegments`.
10. Report any deliberate omissions, ambiguous statuses, or validation limitations.

## Reporting

Report:

- mode parameter: `changed` or `full`,
- changed source paths considered, for `changed` mode,
- `design.json` files updated,
- public definitions intentionally omitted, if any,
- validation results.
