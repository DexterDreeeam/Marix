# hyperv-operator Experience

## Purpose

Durable operational notes for Hyper-V VM access and {{proj}} CLI deployment validation.

## Current VM Context

- VM name: `{{proj}}_TestVm`. If it is absent, provision it fully unattended (an `Autounattend.xml` creates the `.credential` admin account; see the agent's Zero-Touch VM Provisioning section) so PowerShell Direct works immediately — no manual steps inside the guest.
- `{{proj}}_TestVm` is the only valid target VM for {{proj}} operations. Do not substitute any other existing VM when this target is missing or host permissions prevent verification; report the target as blocked instead.
- VM Guest Service Interface is enabled, so host-to-guest file copy with `Copy-VMFile` works.
- PowerShell Direct runs over VMBus and needs no guest network, NIC, or WinRM — only a running Windows 10/Server 2016+ guest and a valid local-account credential. Creating that account from `.credential` via the unattended answer file is what makes a freshly built VM controllable with zero in-guest steps.
- PowerShell Direct requires a guest credential; without it, `Invoke-Command -VMName {{proj}}_TestVm` fails with a missing `Credential` parameter.
- VM network remote execution ports were not available from the host during prior checks:
  - SSH `22`: unavailable.
  - WinRM `5985`: unavailable.
  - WinRM TLS `5986`: unavailable.
- Use `.credential/HYPERV_OPERATOR_USERNAME.txt` and `.credential/HYPERV_OPERATOR_PASSWORD.txt` for PowerShell Direct credentials. Never print their contents.

## {{proj}} CLI Deployment

- Host-side prepared deploy folder: `%LOCALAPPDATA%\Temp\{{proj_lower}}-cli-vm`.
- Guest deployment folder: `{{vm_cli_root}}`.
- Launcher: `{{vm_cli_root}}\run-{{proj_lower}}-cli.cmd`.
- Launcher behavior: sets `{{proj_upper}}_SRC_ROOT` to `{{vm_cli_root}}\src` and runs `{{proj_lower}}-cli.exe`.
- CLI remote config points to Ubuntu core at `{{ubuntu_ip}}:{{ubuntu_core_port}}`.

## Remote Core Context

- Ubuntu SSH host: `ubuntu@{{ubuntu_ip}}`.
- SSH key path on host: `{{ssh_key}}`.
- `{{proj_lower}}-core` was previously built under `~/{{proj_lower}}-deploy/src/.target/release/{{proj_lower}}-core`.
- Remote core listens on `0.0.0.0:{{ubuntu_core_port}}`.
- DeepSeek API was validated from Ubuntu with HTTP 200 and a minimal chat completion.

## Validation Pattern

1. Confirm Ubuntu core is listening:
   `ssh -i {{ssh_key}} -o IdentitiesOnly=yes ubuntu@{{ubuntu_ip}} 'ss -ltnp | grep {{ubuntu_core_port}}'`
2. Confirm host can reach core port:
   `Test-NetConnection -ComputerName {{ubuntu_ip}} -Port {{ubuntu_core_port}}`
3. Copy CLI files to VM with `Copy-VMFile`.
4. Execute in guest with PowerShell Direct using stored credentials:
   `{{vm_cli_root}}\run-{{proj_lower}}-cli.cmd "Reply exactly VM_OK."`

## Safety Notes

- Do not commit credential files.
- Do not print credential contents.
- Do not restart or modify VM lifecycle state unless explicitly requested.

## Recent Validation Notes

- 2026-06-21: PowerShell Direct with `.credential` credentials succeeded for `{{proj}}_TestVm`.
- 2026-06-21: From inside `{{proj}}_TestVm`, TCP to `{{ubuntu_ip}}:{{ubuntu_core_port}}` succeeded while ICMP ping did not; rely on TCP checks for core reachability.
- 2026-06-21: `{{vm_cli_root}}\run-{{proj_lower}}-cli.cmd "Reply exactly VM_OK."` completed with exit code 0 and output `VM_OK`.
- 2026-06-22: A non-elevated host token can see Hyper-V services running but `Get-VM` and `Copy-VMFile` fail with "required permission"; rerun from an elevated host session or an effective Hyper-V Administrators token.
- 2026-06-22: Host-side staging at `%LOCALAPPDATA%\Temp\{{proj_lower}}-cli-vm` should contain only the CLI binary, launcher, `deployment.json`, `.alias/*.txt`, and `src/**/config.json`; do not copy `.credential`.
- 2026-06-22: Host TCP and SSH checks showed the Ubuntu core host reachable and `{{proj_lower}}-core` listening on `{{ubuntu_ip}}:{{ubuntu_core_port}}`.
- 2026-06-22: Current `{{proj_lower}}-cli` source builds and no-arg execution exits 0, but chat input fails with `PipeClient implementation is not linked`; `VM_OK` validation requires a linked client transport or a previously working binary.
- 2026-06-23: During zero-touch creation of `{{proj}}_TestVm`, a Gen2 Windows ISO can fail initial DVD boot if the UEFI "press any key" prompt times out; Hyper-V `Msvm_Keyboard.TypeKey` can send Enter/Space immediately after restarting the newly created VM so unattended setup proceeds without guest interaction.
- 2026-06-23: A freshly installed `{{proj}}_TestVm` may not have the MSVC runtime needed by the default Windows Rust binary; building `{{proj_lower}}-cli.exe` with static CRT (`RUSTFLAGS=-C target-feature=+crt-static`) made the launcher exit 0 in the guest.

- 2026-06-23: Ubuntu core deploy must not reuse the CLI-facing `ubuntu_ip` alias as the bind address when that value is not assigned to the Ubuntu interface; after syncing `.alias` from the host, override the remote deploy `.alias/ubuntu_ip.txt` to `0.0.0.0` so `marix-core` binds successfully while the VM CLI keeps the host-side reachable address.
- 2026-06-23: Stop old Ubuntu core instances by PID from `pgrep -f '[m]{{proj_lower}}-core'`; older launches may show only `.target/release/{{proj_lower}}-core` in argv, so a full deploy-path pattern can miss them and falsely report a stale listener as the new core.
- 2026-06-23: Remote non-login SSH scripts may see the distro Cargo instead of rustup Cargo; source `$HOME/.cargo/env` before building, and strip CRLF when piping PowerShell here-strings into remote bash (`tr -d '\r' | bash -s`).

- 2026-06-23: If Ubuntu core exits with `ReceiveFailed("model backend request failed: builder error")` while direct DeepSeek curl succeeds, check for non-printable characters in the API key passed to the Rust process; exporting a printable-cleaned key (`[!-~]`) let `reqwest` build the Bearer header and restored VM `VM_OK` validation.
- 2026-06-23: The current Windows CLI is interactive and reads stdin; passing `"Reply exactly VM_OK."` as a launcher argument only prints the prompt and exits. Use a stdin pipe for real end-to-end validation (for example, echo the prompt into the launcher).
- 2026-06-28: In a non-elevated host session, `Get-VM -Name {{proj}}_TestVm` failed with Hyper-V permission denial before deployment; do not attempt `Copy-VMFile` until the session is elevated or has effective Hyper-V Administrators permissions.
- 2026-06-28: Current `{{proj_lower}}-cli` no-arg path returns `usage: {{proj_lower}}-cli <prompt>` with exit code 1 before loading config, so a no-arg smoke check does not require credentials or core reachability. Prompted CLI runs still need deployed config plus a non-secret placeholder DeepSeek credential while config loading eagerly reads `credential.directory`.
- 2026-06-28: If `whoami /groups` shows `BUILTIN\Administrators` as "Group used for deny only", the current process is still not elevated even if the account is an administrator. Creating a highest-privilege scheduled task also failed with access denied, so start a new elevated session before retrying `Get-VM`, `Copy-VMFile`, or PowerShell Direct.
- 2026-06-28: `{{proj_lower}}-cli.exe` built with the default MSVC runtime failed inside `{{proj}}_TestVm` with exit code `-1073741515` before printing output. Rebuilding with `RUSTFLAGS="-C target-feature=+crt-static"` produced a self-contained binary; after copying it to `{{vm_cli_root}}`, no-arg usage returned exit code `1` and `"Reply exactly VM_DEPLOY_OK."` returned `VM_DEPLOY_OK` with exit code `0`.
- 2026-06-28: `{{proj_lower}}-cli` no-argument mode is now interactive stdin mode. VM validation used a finite pipe into `{{vm_cli_root}}\run-{{proj_lower}}-cli.cmd`; `"Reply exactly VM_LOOP_ONE."` and `"Reply exactly VM_LOOP_TWO."` returned `VM_LOOP_ONE` and `VM_LOOP_TWO` with exit code `0`.
- 2026-07-04: `{{proj}}_TestVm` guest identity is host `DESKTOP-4LQ2VC3`, local admin account `marixagent`, Windows 11 `10.0.26200`, 64-bit AMD64. Starting from Off: `Start-VM` then `Wait-VM -For Heartbeat` (~<2 min) and PowerShell Direct via `.credential` connected on the first `New-PSSession` attempt. All six integration services (incl. Guest Service Interface) are enabled.
- 2026-07-04: `{{proj}}_TestVm` NIC is on the Hyper-V Default Switch NAT: IPv4 `172.22.82.30`, GW/DNS `172.22.80.1`. From inside the guest, DNS resolves and outbound internet TCP:443 works. ICMP to remote agent host `43.142.167.218` fails (blocked) — keep using TCP checks, not ping.
- 2026-07-04: Outbound reachability from the guest to the remote agent host `43.142.167.218`: port `22345` is OPEN (raw TCP + Test-NetConnection succeed), which proves guest egress + host routing are fine. Target port `22346` returns TIMEOUT/FILTERED (dropped SYN, not RST/refused) — the remote side is not listening/allowing `22346` yet. Before the host role can connect outbound to `43.142.167.218:22346`, the remote agent must listen there and its cloud security group/firewall must permit it.
- 2026-07-04: To classify a remote TCP port quickly, use `TcpClient.BeginConnect` + `AsyncWaitHandle.WaitOne(<ms>)` from inside the guest; it distinguishes CONNECTED vs REFUSED vs TIMEOUT/FILTERED in a bounded time, whereas `Test-NetConnection` on a filtered port blocks for a long timeout.
- 2026-07-04: Guest deploy dirs: `C:\MarixRemoteCli` already holds the prior CLI deploy (`.alias`, `.credential`, `src`, `config.toml`, `deployment.json`, `marix-cli.exe`, `run-marix-cli.cmd`) — a template for the layout the host role will use. Created `C:\MarixHost` for the host + tools role; in-guest write test passed and `C:` has ~48 GB free. `Copy-VMFile -FileSource Host ... -CreateFullPath -Force` host->guest round-trip verified via a unique token that matched exactly on read-back.
- 2026-07-04: Deployed the Marix "host + tools" role into `Marix_TestVm` at `C:\MarixHost` from host payload `C:\r\Marix\.deploy\host-payload` (25 files: `marix-host.exe`, `config.toml`, 5 `tool\*.exe`, 12 `.alias\*.txt`, 6 `.credential\*.txt`). Per-file `Copy-VMFile -FileSource Host -CreateFullPath -Force` (enumerate source, map each to `C:\MarixHost\<relative>`) preserves the `tool/.alias/.credential` subdirs; a guest recursive listing confirmed all 25 files with byte sizes matching the source.
- 2026-07-04: Prebuilt `marix-host.exe` is dynamically MSVC-linked (import table shows `vcruntime140.dll`). A fresh Win11 guest has no VC++ runtime, so the exe exits instantly with code `-1073741515` (`0xC0000135` STATUS_DLL_NOT_FOUND), empty stdout/stderr, and no `log` dir created — a loader failure before any app code runs (not a config/panic error). Fix without rebuilding: app-local deploy by `Copy-VMFile` of `vcruntime140.dll` (+ `vcruntime140_1.dll`, `msvcp140.dll`) from host `System32` into `C:\MarixHost`. The UCRT `api-ms-win-crt-*` imports are already satisfied by the guest OS `ucrtbase.dll`.
- 2026-07-04: Launch pattern that works over PowerShell Direct for a long-running host service: inside the `Invoke-Command` scriptblock set `$env:MARIX_CONFIG='C:\MarixHost\config.toml'` then `Start-Process -FilePath C:\MarixHost\marix-host.exe -WorkingDirectory C:\MarixHost -RedirectStandardOutput/Error C:\MarixHost\host.*.log -WindowStyle Hidden -PassThru` (no `-Wait`). The child inherits `MARIX_CONFIG` and survives the PSSession closing. `config.toml` uses `marix_path = "."`, so `.credential`, `tool`, `log` resolve against the working dir. A healthy start leaves `host.out.log`/`host.err.log` empty.
- 2026-07-04: Marix host-role config endpoints: `[client] core_address = 43.142.167.218:22345`, `[agent] host_bind_address = 43.142.167.218:22346`. After launch, `marix-host` held an Established TCP connection `172.22.82.30:<ephemeral> -> 43.142.167.218:22346`, proving outbound connection to the remote agent. Port `22346` (previously filtered/timeout in earlier notes) is now open; guest `Test-NetConnection 43.142.167.218 -Port 22346` returns `TcpTestSucceeded=True`. Verify the real connection with `Get-NetTCPConnection -OwningProcess <marix-host PID>`, not just `Test-NetConnection` (which opens its own socket).
- 2026-07-05: Fresh host payload `C:\r\Marix\.deploy\host-payload` redeployed to `Marix_TestVm:C:\MarixHost`; old `marix-host.exe` PID 6036 stopped by explicit PID, 25 payload files copied plus app-local VC++ DLLs from host System32 (`vcruntime140.dll`, `vcruntime140_1.dll`, `msvcp140.dll`). New `marix-host.exe` started with `MARIX_CONFIG=C:\MarixHost\config.toml` as PID 4284; guest TCP to `43.142.167.218:22346` connected and the process owned an Established connection to that endpoint.
