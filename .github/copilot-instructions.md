# Copilot Instructions for Marix

## Language Rules

### Conversation
- All session/conversation content MUST be in **Chinese (中文)**.

### Documentation
- Documentation uses **dual-language** versions:
  - English version: `filename.md` (e.g., `README.md`, `DESIGN.md`)
  - Chinese version: `filename.cn.md` (e.g., `README.cn.md`, `DESIGN.cn.md`)
- Most documentation files are Markdown (`.md`).
- Both versions must be kept in sync when content changes.

### Code
- All code MUST be written in **English**, including:
  - Variable names, function names, class names
  - Comments
  - Commit messages
  - Log messages
- The **only exception** is specific Chinese string literals that are required by the application logic (e.g., user-facing Chinese text, i18n strings).

## Git Policy

- **Do NOT** run any git commands (`git add`, `git commit`, `git push`, `git pull`, etc.) unless the user explicitly requests a git operation (e.g., invoking `/git-sync` or asking to commit/push).
- Code changes should be made to files only. Let the user decide when to stage, commit, and push.
- For explicit git operations that need GitHub access, ensure GitHub CLI (`gh`) is installed first. If it is missing on Windows, install it with `winget install --id GitHub.cli -e --source winget --accept-package-agreements --accept-source-agreements`, then authenticate or configure credentials before pushing.
