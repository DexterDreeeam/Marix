---
name: git-sync
description: Synchronize the current branch with the remote. Use when asked to sync, push changes, or run git-sync.
---

## Workflow

1. **Prepare GitHub CLI** — Ensure `gh` is installed before any remote GitHub operation. If missing on Windows, install it with `winget install --id GitHub.cli -e --source winget --accept-package-agreements --accept-source-agreements`. If push credentials are missing, run `gh auth login` or otherwise configure GitHub credentials before pushing.
2. **Stage & Commit Local Changes** — Before pulling, if there are uncommitted changes, run `git add -A` and commit with a concise English message (20 words or fewer). Always include the trailer: `Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>`. Record the commit hash if a commit is created.
3. **Pull & Rebase** — Run `git pull --rebase origin <current-branch>` to replay the local commit, if any, onto the synchronized remote history.
4. **Resolve Conflicts** — If rebase conflicts occur, resolve them based on code context, stage resolved files, and `git rebase --continue`. If conflicts cannot be resolved confidently, run `git rebase --abort` and inform the user.
5. **Track Overview Changes** — Before pushing, record whether the pushed change set includes any path under `overview/`.
6. **Push** — Run `git push origin <current-branch>`.
7. **Deploy Pages for Overview Changes** — If `overview/` changed and push succeeds, deploy the Pages site by triggering `.github/workflows/pages.yml` with `gh workflow run pages.yml --ref <current-branch>`. Then find the new run with `gh run list --workflow pages.yml --branch <current-branch> --limit 1 --json databaseId,status,conclusion,headSha` and wait for it with `gh run watch <run-id> --exit-status`. If the push to the default branch already created a Pages workflow run for the same pushed SHA, watch that run instead of creating a duplicate dispatch.
8. **Report** — Summarize what was done (e.g., committed N files, pushed to branch X, deployed Pages). Include the created commit hash when a local commit was created. If there are no local or remote changes to apply, report "Already up to date."

## Rules

- Commit messages must be in **English**, concise, no more than **20 words**.
- Never force-push unless the user explicitly requests it.
- If push fails due to credentials, inform the user to push manually.
- If Pages deployment fails or cannot be triggered due to credentials or permissions, report that the push succeeded but Pages deployment needs manual follow-up.
- Do not invoke `overview-engineer` as part of `git-sync`. That agent is only for explicit overview implementation changes or bug fixes, not for source layout, design metadata, or sync bookkeeping.
- Diff summaries must treat every dot-prefixed file or folder under `src/` as companion metadata maintained by `development-designer`, not as visible source content.
