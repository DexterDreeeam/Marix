# hyperv-operator Experience

## Purpose

Durable operational notes for Hyper-V VM access and {{proj}} CLI deployment validation.

## Current VM Context

- VM name: `{{proj}}_TestVm`. If it is absent, provision it fully unattended (an `Autounattend.xml` creates the `.credential` admin account; see the agent's Zero-Touch VM Provisioning section) so PowerShell Direct works immediately — no manual steps inside the guest.
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
