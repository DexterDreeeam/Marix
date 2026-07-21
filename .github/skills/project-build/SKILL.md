---
name: project-build
description: Build Marix release binaries for the four deployable modules — Server Telemetry, Server, Client, Host (including all native Tools). Use when the user asks to build, rebuild, or compile Marix artifacts for deployment.
---

## Purpose

Produce correct release binaries for each of Marix's four deployable
modules from `src/` (workspace root `src/Cargo.toml`, `resolver = "2"`).
This skill only builds; it does not deploy, copy, start, or stop anything.
Deployment placement, config resolution, and startup order belong to
`engineer-of-deployment`.

## Scope

Four modules, each built independently:

1. **Server Telemetry** — package `marix-server-telemetry`, native Ubuntu build.
2. **Server** — package `marix-server`, native Ubuntu build.
3. **Client** — package `marix-client`, local Windows build, two binaries.
4. **Host** — package `marix-host` plus every package `marix-tool` binary
   (all native Tools Host loads at runtime), local Windows build.

## 1. Server Telemetry

- Package: `marix-server-telemetry`. Binary: `marix-server-telemetry`
  (`src/server_telemetry/main.rs`).
- Build natively on the Ubuntu server with its installed Rust toolchain, from
  a sanitized current-source copy (excludes `.git`, `target`, `.credential`,
  generated deployment configs, browser profiles, logs, temporary caches).
- Locked build, reusing the installed toolchain and target cache:

  ```bash
  cargo fetch --locked
  cargo build --release --locked -p marix-server-telemetry
  ```
- Output: `target/release/marix-server-telemetry`. No prompt templates and no
  Cargo features to select — a single plain binary.

## 2. Server

- Package: `marix-server`. Binary: `marix-server` (`src/server/main.rs`).
- Build natively on the Ubuntu server the same way as Telemetry, from the same
  sanitized source copy:

  ```bash
  cargo fetch --locked
  cargo build --release --locked -p marix-server
  ```
- Output: `target/release/marix-server`. Server additionally needs its prompt
  templates at runtime — preserve directory hierarchy when placing
  `src/server/prompt/template/` alongside the deployed binary; this is a
  deployment-placement concern, not a build step, and belongs to
  `engineer-of-deployment`.

## 3. Client

- Package: `marix-client`. Two binaries, both built the same way, no Cargo
  features involved:
  - `marix-client-cli` (`src/client/cli/main.rs`)
  - `marix-client-app` (`src/client/app/main.rs`)
- Build locally on the Windows machine with its local Rust toolchain:

  ```powershell
  cargo build --release -p marix-client
  ```
  This single invocation is safe and correct for Client because neither
  binary is gated behind mutually exclusive features — unlike Host's Tools
  (see below), building both Client binaries together in one invocation does
  not create any feature-unification hazard.
- Output: `target/release/marix-client-cli.exe` and
  `target/release/marix-client-app.exe`.

## 4. Host (including all Tools)

Host itself and its native Tools are two different packages with different
build requirements. Build both; Host needs every Tool binary placed beside it
at runtime, but building them is still two distinct steps.

### 4a. Host binary

- Package: `marix-host`. Binary: `marix-host` (`src/host/main.rs`). No Cargo
  features involved.

  ```powershell
  cargo build --release -p marix-host
  ```
- Output: `target/release/marix-host.exe`.

### 4b. Native Tools — one isolated cargo invocation per tool, never combined

- Package: `marix-tool` (`src/tool/`). It defines 15 `[[bin]]` targets that
  all share the same source file `tool_main.rs`; each tool's real
  implementation is selected through `#[cfg(feature = "<name>")] pub use
  self::<Type> as SelectedTool;` in its own source file under
  `src/tool/native/**`, gated by that bin's `required-features` entry in
  `src/tool/Cargo.toml`. No feature is enabled by default.
- **Hard rule: build every Tool binary in its own separate `cargo build`
  invocation, enabling only that one bin's own feature. Never build two or
  more Tool binaries — let alone all 15 — in a single invocation with
  multiple/all features enabled together, and never use `--all-features` on
  this package.** Cargo unifies features across a single invocation: if more
  than one Tool feature is active at once, every `#[cfg(feature = "...")]
  pub use ... as SelectedTool;` for every active feature becomes active
  simultaneously in the same compiled `marix_tool` library, and the resulting
  name collision does not error — it silently resolves `SelectedTool` to
  whichever tool is declared first in the module tree
  (`native/mod.rs`'s `mod coding;` before `file`/`process`/`shell`/`sys`/`web`,
  and `coding/mod.rs`'s `mod get_code_outline;` before `replace_in_file`),
  so every bin built in that shared invocation silently embeds and executes
  `get_code_outline`'s logic regardless of its own name or intended feature.
  This produces 15 distinct, differently-hashed `.exe` files that all behave
  identically and wrongly — verified by reproducing it with a real build and
  by then building `marix_tool_web_search` in total isolation, which then
  correctly reported itself as `web_search`. There is no compiler warning or
  error for this; the only way to catch it is to never do it and to verify
  each binary's own `--preview` output names the tool that binary is
  supposed to be (see Verification below).
- Run each of these as its own separate command (order does not matter, but
  each is a fully separate `cargo build` process):

  ```powershell
  cargo build --release -p marix-tool --bin marix_tool_read_file --features read_file
  cargo build --release -p marix-tool --bin marix_tool_write_file --features write_file
  cargo build --release -p marix-tool --bin marix_tool_list_directory --features list_directory
  cargo build --release -p marix-tool --bin marix_tool_search_text --features search_text
  cargo build --release -p marix-tool --bin marix_powershell --features powershell
  cargo build --release -p marix-tool --bin marix_command_prompt --features command_prompt
  cargo build --release -p marix-tool --bin marix_bash --features bash
  cargo build --release -p marix-tool --bin marix_tool_os_env --features os_env
  cargo build --release -p marix-tool --bin marix_tool_web_fetch --features web_fetch
  cargo build --release -p marix-tool --bin marix_tool_web_search --features web_search
  cargo build --release -p marix-tool --bin marix_tool_get_code_outline --features get_code_outline
  cargo build --release -p marix-tool --bin marix_tool_replace_in_file --features replace_in_file
  cargo build --release -p marix-tool --bin marix_tool_start_process --features start_process
  cargo build --release -p marix-tool --bin marix_tool_read_process_output --features read_process_output
  cargo build --release -p marix-tool --bin marix_tool_stop_process --features stop_process
  ```
- If `src/tool/Cargo.toml` gains or removes a `[[bin]]`/feature pair, update
  this list to match — the bin-name/feature-name pairing above must always be
  read from that file's current `[[bin]]`/`[features]` entries, not assumed
  stale from this skill file.
- Output: 15 files under `target/release/` — 4 of them use a shortened bin
  name instead of the `marix_tool_<feature>` pattern: `marix_powershell`,
  `marix_command_prompt`, and `marix_bash` (shell tools), plus
  `marix_tool_get_code_outline` which does follow the pattern but is called
  out here because its feature name equals its own bin-name suffix; every
  other bin is `marix_tool_<feature>.exe`.
- Host loads every file present in its configured `tool.directory` at Host
  process startup only, with no rescan afterward (`ToolRegistry::new()`,
  `src/host/executor/registry.rs`) — deploy the complete, correctly-built set
  of 15 Tool binaries into place before Host is started or restarted; a Host
  process that already started with an incomplete or wrong Tools directory
  will keep serving that stale/wrong snapshot for its entire remaining
  lifetime regardless of what is copied in afterward. This placement and
  startup-ordering concern belongs to `engineer-of-deployment`, not this
  build skill.

## Verification (do this for every Tool binary after building)

`Tool::load` (`src/host/executor/tool.rs`) silently skips (`Option`, no log
line) any tool file whose `--preview` invocation fails to spawn, exits
non-zero, or produces invalid JSON — a wrongly-built or misnamed Tool binary
produces no build error and no runtime warning, only a missing or wrong tool
at Host startup. After building Host's Tools, verify every one individually:

```powershell
foreach ($exe in Get-ChildItem "target\release\marix_*.exe", "target\release\marix_tool_*.exe") {
    $preview = & $exe.FullName --preview 2>&1 | Select-Object -First 1
    Write-Host "$($exe.Name): $preview"
}
```

Confirm each binary's own `"name"` field in its printed preview JSON matches
the tool that binary name and feature were supposed to build — for example
`marix_tool_web_search.exe` must report `"name":"web_search"`, never
`"name":"get_code_outline"` or any other tool's name. Do not consider Host's
Tools build complete until every single one passes this check; a shared
build invocation that silently produced 15 working-but-wrong binaries is
exactly the failure mode this check exists to catch.

## Rules

- This skill only builds. It does not copy artifacts into place, resolve or
  write configs, start or stop services, or touch `Marix_TestVm`,
  `C:\MarixHost\`, `C:\MarixClient\`, or the Ubuntu systemd units — that is
  `engineer-of-deployment`'s and `win-hyperv-operation`'s responsibility.
- Never combine more than one `marix-tool` feature in a single `cargo build`
  invocation, and never pass `--all-features` to `marix-tool`. This is the
  one hard rule in this skill; every other build step in this file may be
  batched or reordered freely.
- Prefer `--locked` (with a prior `cargo fetch --locked`) for the Ubuntu-built
  Server and Server Telemetry modules, reusing the installed toolchain and
  target cache, per `engineer-of-deployment`'s existing build process. Client
  and Host builds run locally with the local Rust toolchain and do not need
  the Ubuntu sanitized-source-copy step.
- Do not install new tooling or change Cargo/toolchain versions to work
  around a build failure; report the failure instead.
