---
name: git-tag-diff
description: Show the diff between the two most recent marix tags. Use when the user asks to diff tags, review tag changes, or run git-tag-diff.
---

## Workflow

1. **Find Marix Tags** — List all tags matching the `marix_tag_*` pattern, sorted by creation time.
2. **Identify Range** — Pick the two most recent `marix_tag_*` tags:
   - `<previous-tag>`: the second most recent.
   - `<latest-tag>`: the most recent.
3. **Diff** — Run `git diff <previous-tag>..<latest-tag>`, excluding every path under `src/` that has any dot-prefixed file or folder segment.
4. **Include Untracked for Worktree Diffs** — When the comparison target is the current working tree instead of another tag, also run `git ls-files --others --exclude-standard` and treat matching visible source files as added (`A`). Plain `git diff <tag>` does not include untracked files, but Marix tag diffs must.
5. **Report** — Summarize the filtered diff: files changed, insertions, deletions.

## Rules

- This skill ONLY operates on tags with the `marix_tag_` prefix. All other user-defined tags are ignored.
- If there is only one `marix_tag_*` tag, diff from that tag to the current HEAD and inform the user.
- If there are no `marix_tag_*` tags, inform the user that no marix tags exist yet.
- Dot-prefixed files and folders under `src/` are companion metadata maintained by `development-designer`; exclude all such paths from displayed diffs and summary counts.
- Treat untracked visible source files as added when diffing a tag against the working tree. This is required for newly split files such as `src/common/protocol/user_input.rs` before they are committed.
