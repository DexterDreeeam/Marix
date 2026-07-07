# Copilot Instructions

## Language Rules

- All session/conversation content MUST be in **Chinese (中文)**.
- Documentation uses dual-language versions, English `filename.md` and Chinese `filename.cn.md`, and both versions must be kept in sync when content changes.

## Git Policy

- **Do NOT** run any git commands (`git add`, `git commit`, `git push`, `git pull`, etc.) unless the user explicitly requests a git operation (e.g., invoking `/git-sync` or asking to commit/push).
- For explicit git operations that need GitHub access, ensure GitHub CLI (`gh`) is installed first.

## Work Dispatching Policy

- All file operations under `src/` except reading must go through the `source-programmer` agent. This includes creating, modifying, deleting, moving, or renaming files.
- All file operations under `overview/` except reading must go through the `overview-engineer` agent. This includes creating, modifying, deleting, moving, or renaming files.
- Deployment-related work must go through the `deployment-engineer` agent. Deployment follows a three-endpoint model: Ubuntu machine deploys Server, VM deploys Host, and local machine deploys Client.
- Research questions about external agent implementations must go through the `agent-researcher` agent.
