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

## Public API Documentation

- Public library-like items should have rustdoc comments.
- Document public errors, panics, edge cases, and examples when they matter to callers.
- Any public `unsafe` item must document its safety contract.
- Keep comments and rustdoc in English.

## Error Handling

- Prefer `Result<T, E>` for recoverable failures.
- Avoid `panic!` for normal control flow or recoverable errors.
- Prefer meaningful error types and actionable error messages.
- Do not swallow errors silently. Surface or propagate them through existing patterns.

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
