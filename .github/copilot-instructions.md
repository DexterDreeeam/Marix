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

## Validation Policy

- For overview/site changes, do not verify or report local preview resources unless the user explicitly asks for local access.

## Rust Workspace Policy

- The Rust workspace root is `src/`, not the repository root. Run Cargo commands from `src/` or pass `--manifest-path src/Cargo.toml`.
- Cargo build output is configured by `src/.cargo/config.toml` to use `src/.target/`. Rust-specific project files should remain under `src/`; repository-root files are for engineering workflow only.

## Source Architecture

- `src/Cargo.toml` defines a workspace with flat member packages: `common` provides shared protocols/helpers, `config` provides the `marix-config` lib crate, `core` provides the `marix_core` lib crate and `marix-core` bin crate, and `cli` provides the `marix-cli` bin crate. Do not add nested `src/` folders under `common`, `config`, `core`, or `cli`.
- `cli` owns command-line user interaction, including input, output, and interface files.
- `common` owns cross-package protocol types such as `UserInput`; CLI and core should reuse those protocol definitions instead of duplicating input structs.
- `core` owns preprocessing, agent computation/runtime orchestration, pass-through transport boundaries, and model backend interfaces. Remote models are the current focus, but local model compatibility must remain represented.
- Shared CLI/core configuration belongs in the `marix-config` crate as merged JSON loaded from `src/**/config.json`; callers access values by string keys such as `config["cli"]["interface"]`.
- Deployment topology is read from root `deployment.json` and stored under the `deployment` node without making deployment details part of core config.
- Do not reintroduce the old Rust `agent` or `overview` source modules. Overview site implementation remains under `overview/`, and source-design data is maintained by `development-designer`.

## Source Design Maintenance

- A non-dot source file is any path under `src/` where neither the file name nor any parent directory segment starts with `.`.
- Do not invoke `development-designer` proactively during normal tasks. It is triggered only when the `ensure-deveopment-design` `agentStop` hook blocks and asks for design metadata updates.
- When the hook triggers, pass the changed non-dot source paths and changed portions/intent to `development-designer` so it can update `.design.json` in the changed file's folder and every ancestor folder up to `src/`.
- Do not wait until `git-sync` to refresh design documents after a hook block. Design metadata should be updated before the blocked task is completed.
- Dot-prefixed files and folders under `src/` are companion metadata maintained by `development-designer`; do not treat them as normal source files in overview file trees or marix tag diffs.
- Repository hooks include an `agentStop` guard named `ensure-deveopment-design`. It checks only non-dot `src/` files written by the current agent turn and blocks task completion when those files do not have corresponding ancestor `.design.json` updates.
