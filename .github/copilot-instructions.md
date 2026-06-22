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

## Validation Policy

- For overview/site changes, do not verify or report local preview resources unless the user explicitly asks for local access.

## Rust Workspace Policy

- The Rust workspace root is `src/`, not the repository root. Run Cargo commands from `src/` or pass `--manifest-path src/Cargo.toml`.
- Cargo build output is configured by `src/.cargo/config.toml` to use `src/.target/`. Rust-specific project files should remain under `src/`; repository-root files are for engineering workflow only.

## Source Architecture

- Keep source packages flat under `src/`; package manifests should define crate and binary targets without adding unnecessary nested source roots.
- Separate responsibilities by boundary: user interaction, shared protocols/helpers, configuration loading, runtime orchestration, transport boundaries, and model backends should remain decoupled.
- Shared protocol definitions should have a single owner and be reused across package boundaries instead of being duplicated.
- Protocol namespaces should use folder modules when they contain multiple definitions; keep each protocol definition in a focused source file and re-export it from the module entry.
- Keep deployment/topology concerns outside runtime implementation details; expose them through the configuration boundary rather than hard-coding them into core logic.
- Do not reintroduce deprecated source modules. Source-design data is maintained by `development-designer`, while overview site implementation remains outside `src/`.

## Source Design Maintenance

- A non-dot source file is any path under `src/` where neither the file name nor any parent directory segment starts with `.`.
- Do not invoke `development-designer` proactively during normal tasks. It is triggered only when the `ensure-deveopment-design` `agentStop` hook blocks and asks for design metadata updates.
- When the hook triggers, pass the changed non-dot source paths and changed portions/intent to `development-designer` so it can update `.design.json` in the changed file's folder and every ancestor folder up to `src/`.
- Do not wait until `git-sync` to refresh design documents after a hook block. Design metadata should be updated before the blocked task is completed.
- Dot-prefixed files and folders under `src/` are companion metadata maintained by `development-designer`; do not treat them as normal source files in overview file trees or {{proj_lower}} tag diffs.
- Repository hooks include an `agentStop` guard named `ensure-deveopment-design`. It checks only non-dot `src/` files written by the current agent turn and blocks task completion when those files do not have corresponding ancestor `.design.json` updates.
