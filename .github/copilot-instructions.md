# Copilot Instructions

## Project Name Aliasing

Except for content under `src/`, all text must use `{{name}}` placeholders instead of project-specific literals (the project name, machine-specific local paths, and similar values). Each placeholder maps to a `.txt` file in the root `.alias/` folder: the file stem is the key and its content is the replacement value — for example `{{proj}}` is replaced by the contents of `.alias/proj.txt`. New aliases are added simply by creating new `.alias/*.txt` files, so this set will grow over time. When editing any file outside `src/`, always write placeholders (never literals) and resolve them by reading `.alias/` when interpreting content. The `overview/` pages load `.alias/*.txt` at runtime and substitute placeholders before rendering.

## Language Rules

### Conversation
- All session/conversation content MUST be in **Chinese (中文)**.

### Documentation
- Documentation uses **dual-language** versions:
    - English version: `filename.md` (e.g., `README.md`, `DESIGN.md`)
    - Chinese version: `filename.cn.md` (e.g., `README.cn.md`, `DESIGN.cn.md`)
- Both versions must be kept in sync when content changes.

## Git Policy

- **Do NOT** run any git commands (`git add`, `git commit`, `git push`, `git pull`, etc.) unless the user explicitly requests a git operation (e.g., invoking `/git-sync` or asking to commit/push).
- For explicit git operations that need GitHub access, ensure GitHub CLI (`gh`) is installed first.

## Source Editing Policy

- Files under `src/` other than the two companion types below go through the `coding-programmer` agent.
- The two companion types under `src/`, `.design.json` and `.workflow.mmd`, go through the `development-designer` agent.
- Files under `overview/` go through the `overview-engineer` agent.

