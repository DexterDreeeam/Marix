---
name: git-tag
description: Run git-sync, create a {{proj_lower}} tag on the synced source commit, reset design metadata, and push. Use when the user asks to tag, create a patch point, or run git-tag.
---

## Workflow

1. **Run Git Sync First** — Execute the `git-sync` skill and follow its workflow completely. It must commit current local changes first, then pull/rebase, resolve conflicts if needed, track overview changes, push the branch, and deploy Pages for overview changes. Record the source commit hash created by `git-sync`; if `git-sync` created no commit, use the current `HEAD` as the tag target.
2. **Create Tag** — Create an annotated tag on the recorded source commit in the format:

   ```
   {{proj_lower}}_tag_<timestamp>_<purpose>
   ```

   - `<timestamp>`: current time in `YYYYMMDD_HHmmss` format (local time).
   - `<purpose>`: a short snake_case summary of the change (e.g., `add_model_adapter`, `fix_memory_leak`).
   - Example: `{{proj_lower}}_tag_20260618_150900_add_git_skills`

3. **Reset Design JSON After Tag** — After the tag is successfully created, run `design-json-reset`. This is the ONLY design-metadata action in `git-tag`: it just strips `changeStatus`/status arrays from `.design.json` files. Do not read source, recompute line numbers, or refresh element structure here — every `.design.json` is already kept in sync with source incrementally by `development-designer` after each source edit, so at tag time the elements already correspond to the current source. If `design-json-reset` changes any `.design.json` files, create a follow-up commit with a concise English message (≤20 words) and the same `Co-authored-by` trailer. The tag must remain on the recorded source commit, not this reset commit.
4. **Push Branch And Tag** — Push the current branch and the created tag to remote:
   - `git push origin <current-branch>`
   - `git push origin <tag-name>`
5. **Report** — Show the source commit hash tagged by `git-tag`, reset commit hash if any, tag name, and remote synchronization result.

## Rules

- This skill only creates tags with the `{{proj_lower}}_tag_` prefix. It does NOT modify or interact with any other user-defined tags.
- If `git-sync` creates no source commit, create the tag on the current HEAD after `git-sync` completes.
- If `design-json-reset` produces a reset commit, push the branch after that reset commit so the branch HEAD has reset `.design.json` statuses while the tag points at the source commit recorded from `git-sync`.
- Pushing the current branch and the created `{{proj_lower}}_tag_*` tag is part of this skill after the user explicitly invokes `git-tag`.
- Do NOT use `git push --tags`; only push the newly created `{{proj_lower}}_tag_*` tag.

## Scope Boundaries

`git-tag` is a git bookkeeping operation, not a source or design authoring task. Keep it to `git-sync` + create tag + `design-json-reset` + push. Specifically:

- Do NOT read or re-scan source files to (re)generate `.design.json`. Source-to-design correspondence is maintained incrementally right after each source edit (via the `ensure-deveopment-design` hook and `development-designer`), not at tag time.
- Do NOT invoke `development-designer` or `design-json-update` during `git-tag`. If `git-sync`'s commit step is blocked by `ensure-deveopment-design`, that means an earlier source edit skipped its design update — fix that as part of that edit, then re-run `git-tag`; do not turn tagging into a design-refresh pass.
- Do NOT run builds, tests, linters, or `cargo check`. `git-tag` does not compile or verify source; tagging assumes the working tree is already in the intended, validated state.
- The only file writes `git-tag` performs are the `design-json-reset` status strip and the two commits (source via `git-sync`, plus the optional reset commit).
