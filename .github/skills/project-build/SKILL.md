---
name: project-build
description: Build and verify four complete, locally deployable Marix release bundles on Windows.
---

## Purpose

Use the unified PowerShell entry point to generate complete Server Telemetry,
Server, Client, and Host release bundles locally. It never deploys, starts, or
configures remote services. Server targets Linux GNU; the others target Windows.

## Entry Point

Run from the repository root:

```powershell
& .\.github\skills\project-build\build.ps1
```

The script resolves the repository from its own location and is current-directory safe.

## Output

The script recreates only `.temp\project-build` and leaves Cargo caches intact.
Exactly four top-level bundle folders are generated:

```text
.temp\project-build\
  server-telemetry\
    marix-server-telemetry
    config.toml
  server\
    marix-server
    config.toml
    src\server\prompt\template\<all current prompt files>
  client\
    App\
      marix-client-app.exe
      config.toml
    Cli\
      marix-client-cli.exe
      config.toml
  host\
    marix-host.exe
    config.toml
    tool\<every discovered Tool executable>
```

## Configuration Bundling

Each executable directory receives an independently resolved `config.toml`.
Placeholders are discovered from the root template and matched under `.credential`.

Credentials must be non-empty UTF-8 and already decrypted. A git-crypt header
fails explicitly; values and resolved configs are never logged or passed to Git.
Resolved configs use UTF-8 without a BOM.

Sensitive bundles exist only under the ignored `.temp\project-build` directory.

Server prompt files are discovered and copied recursively from the repository
template directory. Missing, empty, or mismatched prompt sets fail the build.

Tool binary targets, names, and required features are read dynamically from
Cargo metadata. Each Tool is built and preview-verified once, then copied to
Host only; the Host Tool count must match metadata. Tools belong only to the
Host bundle because Client is a pure user-interaction endpoint.

## Prerequisites

The machine must provide Cargo, Windows and Linux GNU Rust targets,
`cargo-zigbuild`, and Zig via `PATH` or Python `ziglang`. The script checks
these prerequisites but never installs or switches tooling.

## Tool Isolation and Verification

Each Tool target declares exactly one feature, builds in its own Cargo
invocation, and passes exact executable `--preview` name verification.

Cargo unifies features within one invocation, so isolated Tool builds prevent
the conditional `SelectedTool` implementation from selecting the wrong Tool.

## Failure Handling

Every native command is checked and every expected artifact must exist before copying.
