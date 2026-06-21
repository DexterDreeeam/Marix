# hyperv-operator Experience

## Purpose

Durable operational notes for Hyper-V VM access and Marix CLI deployment validation.

## Current VM Context

- VM name: `vm-ee-test`.
- VM Guest Service Interface is enabled, so host-to-guest file copy with `Copy-VMFile` works.
- PowerShell Direct requires a guest credential; without it, `Invoke-Command -VMName vm-ee-test` fails with a missing `Credential` parameter.
- VM network remote execution ports were not available from the host during prior checks:
  - SSH `22`: unavailable.
  - WinRM `5985`: unavailable.
  - WinRM TLS `5986`: unavailable.
- Use `.credential/HYPERV_OPERATOR_USERNAME` and `.credential/HYPERV_OPERATOR_PASSWORD` for PowerShell Direct credentials. Never print their contents.

## Marix CLI Deployment

- Host-side prepared deploy folder: `%LOCALAPPDATA%\Temp\marix-cli-vm`.
- Guest deployment folder: `C:\MarixRemoteCli`.
- Launcher: `C:\MarixRemoteCli\run-marix-cli.cmd`.
- Launcher behavior: sets `MARIX_SRC_ROOT` to `C:\MarixRemoteCli\src` and runs `marix-cli.exe`.
- CLI remote config points to Ubuntu core at `43.142.167.218:22345`.

## Remote Core Context

- Ubuntu SSH host: `ubuntu@43.142.167.218`.
- SSH key path on host: `C:\Users\dexterzou\.ssh\marix_ubuntu`.
- `marix-core` was previously built under `~/marix-deploy/src/.target/release/marix-core`.
- Remote core listens on `0.0.0.0:22345`.
- DeepSeek API was validated from Ubuntu with HTTP 200 and a minimal chat completion.

## Validation Pattern

1. Confirm Ubuntu core is listening:
   `ssh -i C:\Users\dexterzou\.ssh\marix_ubuntu -o IdentitiesOnly=yes ubuntu@43.142.167.218 'ss -ltnp | grep 22345'`
2. Confirm host can reach port 22345:
   `Test-NetConnection -ComputerName 43.142.167.218 -Port 22345`
3. Copy CLI files to VM with `Copy-VMFile`.
4. Execute in guest with PowerShell Direct using stored credentials:
   `C:\MarixRemoteCli\run-marix-cli.cmd "Reply exactly VM_OK."`

## Safety Notes

- Do not commit credential files.
- Do not print credential contents.
- Do not restart or modify VM lifecycle state unless explicitly requested.

## Recent Validation Notes

- 2026-06-21: PowerShell Direct with `.credential` credentials succeeded for `vm-ee-test`.
- 2026-06-21: From inside `vm-ee-test`, TCP to `43.142.167.218:22345` succeeded while ICMP ping did not; rely on TCP checks for core reachability.
- 2026-06-21: `C:\MarixRemoteCli\run-marix-cli.cmd "Reply exactly VM_OK."` completed with exit code 0 and output `VM_OK`.
