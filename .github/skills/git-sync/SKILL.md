---
name: git-sync
description: Synchronize the current branch with the remote. Use when asked to sync, push changes, or run git-sync.
---

## Workflow

1. **Prepare GitHub CLI** — Ensure `gh` is installed before any remote GitHub operation. If missing on Windows, install it with `winget install --id GitHub.cli -e --source winget --accept-package-agreements --accept-source-agreements`. If push credentials are missing, run `gh auth login` or otherwise configure GitHub credentials before pushing.
2. **Pull & Rebase** — Run `git pull --rebase origin <current-branch>` to sync with remote.
3. **Resolve Conflicts** — If rebase conflicts occur, resolve them based on code context, stage resolved files, and `git rebase --continue`. If conflicts cannot be resolved confidently, run `git rebase --abort` and inform the user.
4. **Stage & Commit** — If there are uncommitted changes, run `git add -A` and commit with a concise English message (20 words or fewer). Always include the trailer: `Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>`.
5. **Push** — Run `git push origin <current-branch>`.
6. **Report** — Summarize what was done (e.g., committed N files, pushed to branch X). If there are no changes, report "Already up to date."

## Rules

- Commit messages must be in **English**, concise, no more than **20 words**.
- Never force-push unless the user explicitly requests it.
- If push fails due to credentials, inform the user to push manually.
- Do not invoke `overview-engineer` as part of `git-sync`. That agent is only for explicit overview implementation changes or bug fixes, not for source layout, design metadata, or sync bookkeeping.
- Diff summaries must treat every dot-prefixed file or folder under `src/` as companion metadata maintained by `development-designer`, not as visible source content.
