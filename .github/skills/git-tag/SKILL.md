---
name: git-tag
description: Run git-sync, reset design metadata, create a {{proj_lower}} tag, and push. Use when the user asks to tag, create a patch point, or run git-tag.
---

## Workflow

1. **Run Git Sync First** — Execute the `git-sync` skill and follow its workflow completely.
2. **Reset Design Metadata** — Run the `design-json-reset` skill.
3. **Create Tag** — Create an annotated tag on the current `HEAD` in the format:

   ```
   {{proj_lower}}_tag_<timestamp>_<purpose>
   ```

   - `<timestamp>`: current local time in `YYYYMMDD_HHmmss` format.
   - `<purpose>`: a short snake_case summary of the change (e.g., `add_model_adapter`, `fix_memory_leak`).
   - Example: `{{proj_lower}}_tag_20260618_150900_add_git_skills`

4. **Push Branch And Tag** — `git push origin <current-branch>` then `git push origin <tag-name>`.
5. **Report** — Show the tagged commit hash, tag name, and remote sync result.

## Rules

- This skill only creates tags with the `{{proj_lower}}_tag_` prefix; it does not modify or interact with any other tags.
- Except `git-sync` resolving rebase conflicts and `design-json-reset` operating on `.design.json`, `git-tag` reads no source files and needs none. Derive the tag `<purpose>` by summarizing the `git-sync` commit message(s), never by reading source files.
- If `design-json-reset` changes any `.design.json` files, commit them with a concise English message (≤20 words) before creating the tag.
- Create the tag on the current `HEAD` after `git-sync` and `design-json-reset` complete.
- Do NOT use `git push --tags`; push only the newly created `{{proj_lower}}_tag_*` tag.

## Scope Boundaries

`git-tag` is a git bookkeeping operation: `git-sync` + `design-json-reset` + create tag + push. It authors no source or design content.

- Do NOT invoke `development-designer` or `design-json-update`. If `git-sync`'s commit is blocked because an earlier source edit skipped its design update, fix that edit and re-run `git-tag`; do not turn tagging into a design refresh.
- Do NOT run builds, tests, linters, or `cargo check`; `git-tag` assumes the working tree is already validated.
