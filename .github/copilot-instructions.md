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

- The Rust crate root is `src/`, not the repository root. Run Cargo commands from `src/` or pass `--manifest-path src/Cargo.toml`.
- Cargo build output is configured by `src/.cargo/config.toml` to use `src/.target/`. Rust-specific project files should remain under `src/`; repository-root files are for engineering workflow only.

## Source Architecture

- The first-level Rust source modules are `cli`, `core`, and `config`.
- `cli` owns command-line user interaction, including user command input and output rendering.
- `core` owns preprocessing, agent computation/runtime orchestration, pass-through transport boundaries, and model backend interfaces. Remote models are the current focus, but local model compatibility must remain represented.
- Shared deployment and compile-mode configuration belongs in `config`.
- Initial compile modes are `up_xcy_m`, `u_xpcy_m`, and `upxcy_m`; parse mode strings through `config::CompileMode`.
- Do not reintroduce the old Rust `agent` or `overview` source modules. Overview site implementation remains under `overview/`, and source-design data is maintained by `development-designer`.

## Source Design Maintenance

- When a task modifies any non-dot source file under `src/`, invoke the `development-designer` agent before finishing that task.
- Pass the changed non-dot source paths and the changed portions/intent to `development-designer` so it can update the affected dot-prefixed companion metadata from the actual source change.
- Do not wait until `git-sync` to refresh design documents. Design metadata should be updated as part of the same task that changes source.
- Dot-prefixed files and folders under `src/` are companion metadata maintained by `development-designer`; do not treat them as normal source files in overview file trees or marix tag diffs.
