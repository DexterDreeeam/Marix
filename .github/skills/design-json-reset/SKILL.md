---
name: design-json-reset
description: Reset Marix src/.design.json changeStatus fields after a successful marix tag. Use only from git-tag after the tag is created.
---

## Purpose

Reset committed source design metadata status after a successful `marix_tag_*` tag is created.

This skill is intentionally narrow: it only resets `changeStatus` values in `.design.json` files under `src/`. It does not refresh structure, discover new elements, edit source code, or parse legacy `.design.md`.

## Trigger

- Use this skill only after `git-tag` successfully creates the annotated `marix_tag_*` tag on the source-change commit.
- After this skill changes `.design.json`, `git-tag` should create a second commit for the reset, then push the branch and the newly created tag together.

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
- Do not run this before the tag exists; pre-tag metadata updates belong to `design-json-update`.

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
