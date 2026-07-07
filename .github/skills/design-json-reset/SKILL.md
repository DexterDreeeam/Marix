---
name: design-json-reset
description: Reset Marix src_meta/**/design.json changeStatus fields. Use only from git-tag, before the tag is created.
---

## Purpose

Reset committed source design metadata status as part of `git-tag`, before the `marix_tag_*` tag is created.

This skill is intentionally narrow: it only resets `changeStatus` values in `design.json` files under `src_meta/`. It does not refresh structure, discover new elements, edit source code, or parse legacy Markdown design metadata.

## Trigger

- Use this skill only from `git-tag`, after `git-sync` completes and before the tag is created.
- After this skill changes `design.json`, `git-tag` commits the reset, then creates and pushes the tag on that commit.

## Workflow

1. Walk `src_meta/` recursively.
2. Ignore any dot-prefixed folders.
3. For every `src_meta/**/design.json`:
   - parse JSON,
   - set `module.changeStatus` to `unchanged` when present,
   - remove `changeStatus` from every `childModules[]` entry when present,
   - set every `elements[].changeStatus` to `unchanged` when present,
   - remove top-level `added`, `modified`, `deleted`, and `renamed` arrays,
   - preserve all other fields exactly in meaning, including `name`, `type`, `purpose`, and `codeSegments`.
4. Write formatted JSON back to files whose reset output differs from their original content.
5. Count how many `design.json` files were reset and how many already had no resettable status.
6. Report the counts to `git-tag`.

## Rules

- Do not add, remove, rename, or reorder source files.
- Do not create legacy Markdown design metadata.
- Do not change `codeSegments` unless required only to keep JSON valid.
- Do not change non-design files.
- This skill only strips status; `design.json` content and structure updates belong to `design-json-update` during source edits.

## Reporting

Report:

- number of `design.json` files reset,
- number of `design.json` files that already had no resettable status.
