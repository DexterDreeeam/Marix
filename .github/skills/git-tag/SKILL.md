---
name: git-tag
description: Commit current changes, create a {{proj_lower}} tag, and synchronize the current branch with remote. Use when the user asks to tag, create a patch point, or run git-tag.
---

## Workflow

1. **Prepare GitHub CLI** — Ensure `gh` is installed before any remote GitHub operation. If missing on Windows, install it with `winget install --id GitHub.cli -e --source winget --accept-package-agreements --accept-source-agreements`. If push credentials are missing, run `gh auth login` or otherwise configure GitHub credentials before pushing.
2. **Sync First** — Run `git pull --rebase origin <current-branch>` before committing, so the new tag points to the final synchronized history. Resolve conflicts if possible; if conflicts cannot be resolved confidently, run `git rebase --abort` and inform the user.
3. **Update Design JSON** — Run `design-json-update` before the tag commit so `.design.json` reflects the source changes being tagged.
4. **Stage & Commit Source Changes** — Run `git add -A` and commit all current changes with a concise English message (≤20 words). Include the trailer: `Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>`.
5. **Create Tag** — Create an annotated tag on this source-change commit in the format:

   ```
   {{proj_lower}}_tag_<timestamp>_<purpose>
   ```

   - `<timestamp>`: current time in `YYYYMMDD_HHmmss` format (local time).
   - `<purpose>`: a short snake_case summary of the change (e.g., `add_model_adapter`, `fix_memory_leak`).
   - Example: `{{proj_lower}}_tag_20260618_150900_add_git_skills`

6. **Reset Design JSON After Tag** — After the tag is successfully created, run `design-json-reset`. If it changes any `.design.json` files, create a second commit with a concise English message (≤20 words) and the same `Co-authored-by` trailer. The tag must remain on the previous source-change commit, not this reset commit.
7. **Sync Remote** — Push the current branch and the created tag to remote:
   - `git push origin <current-branch>`
   - `git push origin <tag-name>`
8. **Report** — Show the source commit hash, reset commit hash if any, tag name, and remote synchronization result.

## Rules

- This skill only creates tags with the `{{proj_lower}}_tag_` prefix. It does NOT modify or interact with any other user-defined tags.
- If there are no changes to commit, skip the commit step and only create the tag on the current HEAD.
- If `design-json-reset` produces a reset commit, push the branch after that reset commit so the branch HEAD has reset `.design.json` statuses while the tag points at the source-change commit.
- Pushing the current branch and the created `{{proj_lower}}_tag_*` tag is part of this skill after the user explicitly invokes `git-tag`.
- Do NOT use `git push --tags`; only push the newly created `{{proj_lower}}_tag_*` tag.
