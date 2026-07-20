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
- Deployment-related work must go through the `deployment-engineer` agent. Deployment uses fixed physical placement: Ubuntu machine deploys Server and Server Telemetry, Hyper-V VM `Marix_TestVm` deploys Host under `C:\MarixHost\`, and the local physical machine deploys Client. Post-deployment startup order is Server Telemetry → Server → Host; Client is deployed locally but not started by deployment.
- Research questions about external agent implementations must go through the `agent-researcher` agent.
