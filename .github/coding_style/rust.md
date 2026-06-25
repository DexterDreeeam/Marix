# Rust Coding Style

This document is the Rust coding style source for {{proj}}. Use it when reviewing,
drafting, or changing Rust code.

## Naming

Follow the official Rust API Guidelines naming rules:

https://github.com/rust-lang/api-guidelines/blob/master/src/naming.md

Project-specific Rust style below must not override those naming rules.

## Scope

- Apply these rules to Rust source files only.
- Treat the file-size rule as required during code evaluation.
- Treat hygiene, readability, and API-shape findings as style findings unless a task explicitly asks for automatic cleanup.
- Ignore dot-prefixed files and folders under `src/`; they are companion metadata, not normal source.
- Do not install new tooling to satisfy this document. Use existing repository tooling only.

## File Size

- Keep every evaluated code file at or below **500 lines**.
- If a file exceeds 500 lines, split it into focused modules or extract shared logic.
- Prefer cohesive modules over arbitrary line-count splitting.
- Generated, build, vendor, cache, and manifest artifacts are outside normal style review unless explicitly requested.

## Formatting and Tooling

- The Rust workspace root is `src/`.
- Run Cargo commands from `src/` or pass `--manifest-path src/Cargo.toml`.
- Cargo build output belongs under `src/.target/`.
- Prefer `cargo fmt` when Rust formatting is needed.
- Prefer `cargo clippy --all-targets --all-features` when linting is needed and available.
- If the repository cannot run a command, report that limitation instead of adding new tools.

## Comments

- Prefer self-explanatory names, signatures, and module structure over comments.
- Add comments only for non-obvious intent, invariants, safety contracts, or caller-visible edge cases.
- Do not add comments to every public interface or restate what a signature already says.
- Keep comments brief. If a module needs longer context, put that explanation once at the top of the file.
- Any public `unsafe` item must still document its safety contract.
- Keep comments and rustdoc in English.

## Error Handling

- Prefer `Result<T, E>` for recoverable failures.
- Avoid `panic!` for normal control flow or recoverable errors.
- Prefer meaningful error types and actionable error messages.
- Do not swallow errors silently. Surface or propagate them through existing patterns.
- Put module-specific error types and their `From`, `Display`, and `Error` implementations in a dedicated `error.rs`; re-export them from the module entry when they are public API.

## Production Hygiene

Flag non-test uses of these unless locally justified:

- `unwrap()`
- `expect()`
- `panic!`
- `todo!`
- `unimplemented!`
- `dbg!`
- `println!`
- `eprintln!`

Prefer structured diagnostics or existing logging paths over raw print macros.

## Readability and API Shape

- Keep functions focused and shallow.
- Avoid deeply nested control flow.
- Extract repeated logic into cohesive modules or public/private APIs that match the task.
- Prefer enums or typed options over unclear boolean mode parameters.
- Keep visibility private by default; use `pub` only for intentional API surfaces.
- Keep deployment, topology, transport, model, and configuration boundaries decoupled.
- Separate public inherent methods and private inherent methods into different `impl`
  blocks for the same type.
- Route third-party crate types, functions, and macros used by Rust source through
  crate-local wrappers under `src/common/external/<crate>.rs` instead of importing
  those crates directly from feature code.
- Import crate-local external namespaces with `use crate::common::external::*;`,
  then call through the released crate namespace, such as `tokio::Runtime`,
  `tokio::TcpStream`, or `remoc::connect_remoc(...)`, instead of deep-importing
  third-party members in feature modules.
- Re-export derive macros needed by feature modules from `common::external` and
  use them by their released names, such as `Serialize` and `Deserialize`.
- If a namespaced macro conflicts with Rust extern prelude resolution, qualify it
  through the local scope, such as `self::serde_json::json!`.
- Place `impl fmt::Debug for ...` blocks at the end of Rust files, after behavior impls and helper functions.
