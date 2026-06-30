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

## File Structure

- Organize each Rust module file into two sections: the top section is the
  module's outward-facing interface, usually items with `pub`, `pub(crate)`, or
  `pub(super)` visibility; the bottom section is module-internal information and
  implementation details.
- Separate the two sections with exactly this marker line:

  ```rust
  // -- Private -- //
  ```

- Put private helper types, private helper functions, private inherent impl
  blocks, and other module-internal implementation details below that marker.
  If a file has no private section, omit the marker.
- Place file-level `const` and `static` items immediately after imports and
  module declarations, before other top-level items. Keep them above the private
  section marker even when their visibility is private.
- Omit the marker in binary `main.rs` entry files that have no primary module
  struct/API type, even when the file contains private constants or helper
  functions.
- Do not apply the public/private section marker rule to integration tests under
  `src/tests/`; tests should not contain a private section marker.
- Keep `mod.rs` files limited to module declarations and re-exports. Do not put
  concrete types, constants, functions, trait definitions, impl blocks, behavior,
  or helper code in `mod.rs`; move those items into focused sibling module files
  and wire them from `mod.rs`.
- After imports and module declarations, place top-level declarations in this
  general order:
  1. enums,
  2. traits,
  3. free functions that are truly module-level and cannot belong to a type,
  4. structs.
- For each struct, place that struct's inherent `impl` blocks immediately after
  the struct definition before declaring the next struct.
- When a struct has both public and private inherent methods, split them into
  separate `impl` blocks and place the public `impl` before the private `impl`.
- Avoid top-level free functions in Rust module files unless there is a clear
  module-level reason they cannot belong to a type. This is especially important
  in a primary module file whose behavior is centered on one main struct: put
  helper behavior in that struct's second, private `impl` block instead of as
  free functions.
- Place public enums near the top of the file, before traits, functions, or
  structs that use or return them, unless local readability strongly favors
  another order.
- Place `impl fmt::Debug for ...` blocks at the end of Rust files, after behavior
  impls and helper functions.
- Keep existing files coherent when touching them: do not perform broad
  structure-only rewrites unless the task asks for cleanup.

## Readability and API Shape

- Keep functions focused and shallow.
- Avoid deeply nested control flow.
- Avoid `type` aliases unless the underlying type is a long generic with at least
  two levels of generic nesting (for example
  `Arc<Mutex<HashMap<K, Sender<V>>>>`). Do not alias plain types such as `String`
  or shallow generics; spell them out so call sites stay self-describing.
- Extract repeated logic into cohesive modules or public/private APIs that match the task.
- Prefer enums or typed options over unclear boolean mode parameters.
- Keep visibility private by default; use `pub` only for intentional API surfaces.
- Do not add or widen `pub`, `pub(crate)`, or `pub(super)` functions beyond the
  agreed interface. If a change appears to require a new public method or
  function, stop and ask for approval before adding it.
- Keep deployment, topology, transport, model, and configuration boundaries decoupled.
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
