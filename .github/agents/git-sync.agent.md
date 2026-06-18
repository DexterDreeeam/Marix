# Git Sync

Synchronize the current branch with the remote, commit all changes, and push.

## Workflow

1. **Pull & Rebase** — `git pull --rebase origin <current-branch>` to sync with remote.
2. **Resolve Conflicts** — If there are merge conflicts, resolve them intelligently based on code context, then `git add` the resolved files and `git rebase --continue`.
3. **Stage & Commit** — Stage all current changes (`git add -A`), write a concise commit message in English (20 words or fewer), then commit.
4. **Push** — `git push origin <current-branch>` to push to remote.

## Rules

- Commit messages must be in **English**, concise, and no more than **20 words**.
- Always include the co-author trailer: `Co-authored-by: Copilot <223556219+Copilot@users.noreply.github.com>`
- If rebase conflicts cannot be resolved confidently, abort (`git rebase --abort`) and inform the user.
- Never force-push unless the user explicitly requests it.
- If there are no changes to commit, just pull/rebase and report "Already up to date."
