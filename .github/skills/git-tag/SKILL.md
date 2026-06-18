---
name: git-tag
description: Commit current changes, create a marix tag, and synchronize the current branch with remote. Use when the user asks to tag, create a patch point, or run git-tag.
---

## Workflow

1. **Sync First** — Run `git pull --rebase origin <current-branch>` before committing, so the new tag points to the final synchronized history. Resolve conflicts if possible; if conflicts cannot be resolved confidently, run `git rebase --abort` and inform the user.
2. **Stage & Commit** — Run `git add -A` and commit all current changes with a concise English message (≤20 words). Include the trailer: `Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>`.
3. **Create Tag** — Create an annotated tag in the format:

   ```
   marix_tag_<timestamp>_<purpose>
   ```

   - `<timestamp>`: current time in `YYYYMMDD_HHmmss` format (local time).
   - `<purpose>`: a short snake_case summary of the change (e.g., `add_model_adapter`, `fix_memory_leak`).
   - Example: `marix_tag_20260618_150900_add_git_skills`

4. **Sync Remote** — Push the current branch and the created tag to remote:
   - `git push origin <current-branch>`
   - `git push origin <tag-name>`
5. **Report** — Show the commit hash, tag name, and remote synchronization result.

## Rules

- This skill only creates tags with the `marix_tag_` prefix. It does NOT modify or interact with any other user-defined tags.
- If there are no changes to commit, skip the commit step and only create the tag on the current HEAD.
- Pushing the current branch and the created `marix_tag_*` tag is part of this skill after the user explicitly invokes `git-tag`.
- Do NOT use `git push --tags`; only push the newly created `marix_tag_*` tag.
