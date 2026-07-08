---
name: git-sync
description: Synchronize the current branch with the remote. Use when asked to sync, push changes, or run git-sync.
---

## Workflow

1. **Update Design Metadata** — Before staging anything, invoke the `design-json-update` skill with parameter `changed` to process `.temp/changed` and sync `src_meta/**/design.json`. If `.temp/changed` is empty, this is a no-op. Any metadata it writes is included in the commit created next.
2. **Stage & Commit Local Changes** — If there are uncommitted changes, run `git add -A` and commit with a concise English message (20 words or fewer). Record the commit hash.
3. **Pull & Rebase** — Run `git pull --rebase origin <current-branch>` to replay the local commit onto synchronized remote history.
4. **Resolve Conflicts** — Resolve based on context, stage resolved files, and `git rebase --continue`. If conflicts cannot be resolved confidently, run `git rebase --abort` and inform the user.
5. **Push** — Run `git push origin <current-branch>`.
6. **Deploy Pages for Overview Changes** — If the pushed change set includes any path under `overview/`, trigger `.github/workflows/pages.yml` with `gh workflow run pages.yml --ref <current-branch>`, find the run with `gh run list --workflow pages.yml --branch <current-branch> --limit 1 --json databaseId,status,conclusion,headSha`, and wait with `gh run watch <run-id> --exit-status`. If the push already created a Pages run for the same SHA, watch that one instead of dispatching a duplicate.
7. **Report** — Summarize the commit hash (when created), the branch pushed, and any Pages deploy. If there is nothing to apply, report "Already up to date."

## Rules

- Ensure `gh` is authenticated before remote operations; if push fails due to credentials, tell the user to push manually.
- Never force-push unless the user explicitly requests it.
- If Pages deployment cannot be triggered due to credentials or permissions, report that the push succeeded but Pages needs manual follow-up.
- Apart from the initial `design-json-update` (changed) step that syncs `src_meta/` design metadata, git-sync authors no source or overview edits and invokes no editing agent.
