---
name: design-json-reset
description: Reset {{proj}} src/.design.json changeStatus fields. Use only from git-tag, before the tag is created.
---

## Purpose

Reset committed source design metadata status as part of `git-tag`, before the `{{proj_lower}}_tag_*` tag is created.

This skill is intentionally narrow: it only resets `changeStatus` values in `.design.json` files under `src/`. It does not refresh structure, discover new elements, edit source code, or parse legacy `.design.md`.

## Trigger

- Use this skill only from `git-tag`, after `git-sync` completes and before the tag is created.
- After this skill changes `.design.json`, `git-tag` commits the reset, then creates and pushes the tag on that commit.

## Workflow

1. Walk `src/` recursively.
2. Ignore dot-prefixed folders except `.design.json` files themselves.
3. For every `src/**/.design.json`:
   - parse JSON,
   - set `module.changeStatus` to `unchanged` when present,
   - remove `changeStatus` from every `childModules[]` entry when present,
   - set every `elements[].changeStatus` to `unchanged` when present,
   - remove top-level `added`, `modified`, `deleted`, and `renamed` arrays,
   - preserve all other fields exactly in meaning, including `name`, `type`, `purpose`, and `codeSegments`.
4. Write valid formatted JSON back to the same files.

## Rules

- Do not add, remove, rename, or reorder source files.
- Do not create `.design.md`.
- Do not change `codeSegments` unless required only to keep JSON valid.
- Do not change non-design files.
- This skill only strips status; `.design.json` content and structure updates belong to `development-designer` during source edits.

## Validation

After resetting:

1. Parse every changed `.design.json` as JSON.
2. Confirm no `.design.json` under `src/` retains a non-`unchanged` `changeStatus`.
3. Confirm no `childModules[]` entry retains a `changeStatus`.
4. Confirm no `.design.json` under `src/` retains top-level `added`, `modified`, `deleted`, or `renamed` arrays.
5. Confirm no legacy `.design.md` files were created.

## Reporting

Report:

- number of `.design.json` files reset,
- whether any file already had all statuses `unchanged`,
- validation results.
