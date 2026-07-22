# Copilot Instructions

## Language

- All session/conversation content MUST be in **Chinese (中文)**.
- Documentation uses dual-language versions, English `filename.md` and Chinese `filename.cn.md`, and both versions must be kept in sync when content changes.

## Policy

- For explicit git operations that need GitHub access, ensure GitHub CLI (`gh`) is installed first.
- **Do NOT** run any git commands (`git add`, `git commit`, `git push`, `git pull`, etc.) unless the user explicitly requests a git operation (e.g., invoking `/git-sync` or asking to commit/push).
- **Do NOT** run unrequested `cargo check`, tests or deployment actions unless the user explicitly asks.
- Place all temporary files under the repository-root `.temp/` directory; never create them directly in the repository root.
- All file operations under `src/` except reading must go through the `engineer-of-source` agent.
- All file operations under `overview/` except reading must go through the `engineer-of-overview` agent.
- Deployment-related work must go through the `engineer-of-deployment` agent.
- Research questions about external agent implementations must go through the `researcher-of-agents` agent.
