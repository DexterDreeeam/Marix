---
name: win-hyperv-operation
description: Common skill for Windows Hyper-V VM operations, including VM provisioning, file copy, PowerShell Direct execution, deployment checks, and diagnostics.
---
You are using the Windows Hyper-V operation skill for Marix.

## Scope

Operate local Hyper-V guests that are part of the Marix deployment workflow. Focus on VM reachability, file transfer, PowerShell Direct execution, guest diagnostics, and validating the Windows CLI deployment path.

Do not change source code unless the user explicitly asks for a code change. Do not manage overview UI or source design metadata except to report facts relevant to VM operations.


## Current Context

- Target Hyper-V VM: `Marix_TestVm`. Reuse it when it already exists; otherwise provision it fully unattended (see **Zero-Touch VM Provisioning**) — no manual steps inside the guest.
- `Marix_TestVm` is the only valid target VM for Marix operations. Never substitute another existing VM name; if this VM cannot be found, verified, or provisioned because host permissions are insufficient, report the operation as blocked.
- Guest Service Interface is enabled and supports `Copy-VMFile` from host to guest.
- The current CLI deployment path inside the VM is `C:\MarixRemoteCli`.
- The copied CLI launcher is `C:\MarixRemoteCli\run-marix-cli.cmd`.
- The launcher sets `MARIX_SRC_ROOT=%~dp0src` and runs `marix-cli.exe`.
- The CLI remote core target is the Ubuntu core host at `43.142.167.218:22345`.
- VM network ports previously checked from host:
  - SSH `22`: closed/unavailable.
  - WinRM `5985`: closed/unavailable.
  - WinRM over TLS `5986`: closed/unavailable.
- PowerShell Direct with `Invoke-Command -VMName Marix_TestVm` requires guest credentials.
- Local credential files:
  - username: `.credential/HYPERV_OPERATOR_USERNAME.txt`
  - password: `.credential/HYPERV_OPERATOR_PASSWORD.txt`
- Never print credential file contents. Read them only when constructing a `PSCredential`.

## Responsibilities

- Ensure the target VM exists before any other operation: look it up with `Get-VM -Name Marix_TestVm`. If it is missing, provision it fully unattended (see **Zero-Touch VM Provisioning**) so it boots already controllable via PowerShell Direct — never ask the user to sign into the guest or run anything inside it. If it already exists, reuse it and start it only when it is not running.
- Verify VM state with Hyper-V cmdlets such as `Get-VM` and `Get-VMIntegrationService`.
- Copy deployment files into the guest with `Copy-VMFile`.
- Use PowerShell Direct for guest command execution when credentials are available:
  - read username/password from `.credential`,
  - build a `PSCredential`,
  - call `Invoke-Command -VMName Marix_TestVm -Credential $credential -ScriptBlock { ... }`.
- Run the deployed CLI inside the guest to validate remote core chat completion:
  - `C:\MarixRemoteCli\run-marix-cli.cmd "Reply exactly VM_OK."`
- If command execution fails, distinguish between:
  - Hyper-V host issues,
  - guest credential issues,
  - Guest Service copy issues,
  - VM network issues,
  - remote Ubuntu core issues,
  - Marix CLI/core protocol issues.

## Zero-Touch VM Provisioning

Creating the VM is fully hands-off — the skill obtains the Windows ISO itself and installs unattended; never ask the user to place files, sign into the guest, or run anything inside it. Windows is installed from an `Autounattend.xml` answer file that creates a local administrator account whose name and password come from `.credential`, i.e. the exact account this skill uses for PowerShell Direct. PowerShell Direct rides the Hyper-V VMBus, so the guest needs no network, NIC, WinRM, or SSH; the moment Windows finishes installing, the host can control the VM.

Prerequisites:

- A Windows installation ISO at `C:\ISOs\Windows.iso` — the procedure downloads an official Windows 11 Pro retail ISO there automatically (via `pbatard/Fido`, the resolver Rufus uses) whenever the file is missing, so nothing is placed by hand. The host only needs internet access.
- `.credential/HYPERV_OPERATOR_USERNAME.txt` and `.credential/HYPERV_OPERATOR_PASSWORD.txt` (read at runtime; never printed or committed). They define the guest admin account that gets created.
- A Generation 2 / UEFI-capable Hyper-V host, run from an elevated session.

Run the whole procedure on the host; it needs no guest interaction:

```powershell
$vmName = "Marix_TestVm"
$winIso = "C:\ISOs\Windows.iso"
$vhd    = "C:\Hyper-V\Marix_TestVm.vhdx"
$work   = Join-Path $env:TEMP "$vmName-unattend"
$stage  = Join-Path $work "iso"
New-Item -ItemType Directory -Path $stage -Force | Out-Null

# 0. Ensure the Windows installation ISO exists; download it automatically when missing.
#    Fido is the script Rufus uses to resolve official Microsoft retail ISO download links.
if (-not (Test-Path -LiteralPath $winIso)) {
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    New-Item -ItemType Directory -Path (Split-Path -Parent $winIso) -Force | Out-Null
    $fido = Join-Path $work "Fido.ps1"
    Invoke-WebRequest "https://raw.githubusercontent.com/pbatard/Fido/master/Fido.ps1" -OutFile $fido -UseBasicParsing
    $isoUrl = (& $fido -Win 11 -Ed Pro -Lang English -Arch x64 -GetUrl) | Select-Object -Last 1
    Start-BitsTransfer -Source $isoUrl -Destination $winIso   # official retail ISO (~6 GB); resumable
}

# 1. Guest admin account == the .credential account this skill connects with.
$guestUser = (Get-Content -LiteralPath C:\r\Marix\.credential\HYPERV_OPERATOR_USERNAME.txt -Raw).Trim()
$guestPass = (Get-Content -LiteralPath C:\r\Marix\.credential\HYPERV_OPERATOR_PASSWORD.txt -Raw).Trim()

# 2. Answer file: Gen2/UEFI partitions, accept EULA, skip OOBE, create the admin, allow scripts.
#    W269N-... is the Win10/11 Pro GVLK (no activation prompt); change the key for other editions.
@"
<?xml version="1.0" encoding="utf-8"?>
<unattend xmlns="urn:schemas-microsoft-com:unattend">
  <settings pass="windowsPE">
    <component name="Microsoft-Windows-International-Core-WinPE" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">
      <SetupUILanguage><UILanguage>en-US</UILanguage></SetupUILanguage>
      <InputLocale>en-US</InputLocale><SystemLocale>en-US</SystemLocale><UILanguage>en-US</UILanguage><UserLocale>en-US</UserLocale>
    </component>
    <component name="Microsoft-Windows-Setup" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS" xmlns:wcm="http://schemas.microsoft.com/WMIConfig/2002/State">
      <UserData><AcceptEula>true</AcceptEula><ProductKey><Key>W269N-WFGWX-YVC9B-4J6C9-T83GX</Key><WillShowUI>OnError</WillShowUI></ProductKey></UserData>
      <DiskConfiguration><Disk wcm:action="add"><DiskID>0</DiskID><WillWipeDisk>true</WillWipeDisk>
        <CreatePartitions>
          <CreatePartition wcm:action="add"><Order>1</Order><Size>260</Size><Type>EFI</Type></CreatePartition>
          <CreatePartition wcm:action="add"><Order>2</Order><Size>16</Size><Type>MSR</Type></CreatePartition>
          <CreatePartition wcm:action="add"><Order>3</Order><Extend>true</Extend><Type>Primary</Type></CreatePartition>
        </CreatePartitions>
        <ModifyPartitions>
          <ModifyPartition wcm:action="add"><Order>1</Order><PartitionID>1</PartitionID><Format>FAT32</Format><Label>System</Label></ModifyPartition>
          <ModifyPartition wcm:action="add"><Order>2</Order><PartitionID>2</PartitionID></ModifyPartition>
          <ModifyPartition wcm:action="add"><Order>3</Order><PartitionID>3</PartitionID><Format>NTFS</Format><Label>Windows</Label><Letter>C</Letter></ModifyPartition>
        </ModifyPartitions>
      </Disk></DiskConfiguration>
      <ImageInstall><OSImage><InstallTo><DiskID>0</DiskID><PartitionID>3</PartitionID></InstallTo><WillShowUI>OnError</WillShowUI></OSImage></ImageInstall>
    </component>
  </settings>
  <settings pass="specialize">
    <component name="Microsoft-Windows-Deployment" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS" xmlns:wcm="http://schemas.microsoft.com/WMIConfig/2002/State">
      <RunSynchronous><RunSynchronousCommand wcm:action="add"><Order>1</Order><Path>cmd.exe /c powershell -NoProfile -Command "Set-ExecutionPolicy RemoteSigned -Force"</Path></RunSynchronousCommand></RunSynchronous>
    </component>
  </settings>
  <settings pass="oobeSystem">
    <component name="Microsoft-Windows-Shell-Setup" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS" xmlns:wcm="http://schemas.microsoft.com/WMIConfig/2002/State">
      <OOBE><HideEULAPage>true</HideEULAPage><HideOnlineAccountScreens>true</HideOnlineAccountScreens><HideLocalAccountScreen>true</HideLocalAccountScreen><HideWirelessSetupInOOBE>true</HideWirelessSetupInOOBE><ProtectYourPC>3</ProtectYourPC><SkipMachineOOBE>true</SkipMachineOOBE><SkipUserOOBE>true</SkipUserOOBE></OOBE>
      <UserAccounts><LocalAccounts><LocalAccount wcm:action="add"><Name>$guestUser</Name><Group>Administrators</Group><Password><Value>$guestPass</Value><PlainText>true</PlainText></Password></LocalAccount></LocalAccounts></UserAccounts>
      <AutoLogon><Enabled>true</Enabled><Username>$guestUser</Username><Password><Value>$guestPass</Value><PlainText>true</PlainText></Password><LogonCount>1</LogonCount></AutoLogon>
      <TimeZone>UTC</TimeZone>
    </component>
  </settings>
</unattend>
"@ | Set-Content -LiteralPath (Join-Path $stage "Autounattend.xml") -Encoding UTF8

# 3. Pack Autounattend.xml into a tiny ISO (it must sit at the ISO root).
#    Uses IMAPI2, built into Windows (no ADK); oscdimg also works if the ADK is installed.
$unattendIso = Join-Path $work "unattend.iso"
if (-not ("IsoHelper" -as [type])) {
  Add-Type -CompilerOptions "/unsafe" -TypeDefinition @'
public class IsoHelper {
  public unsafe static void Save(string path, object stream, int block, int total) {
    int n = 0; byte[] b = new byte[block]; var p = (System.IntPtr)(&n);
    var o = System.IO.File.OpenWrite(path);
    var i = stream as System.Runtime.InteropServices.ComTypes.IStream;
    while (total-- > 0) { i.Read(b, block, p); o.Write(b, 0, n); }
    o.Flush(); o.Close();
  }
}
'@
}
$fsi = New-Object -ComObject IMAPI2FS.MsftFileSystemImage
$fsi.VolumeName = "UNATTEND"
$fsi.ChooseImageDefaultsForMediaType(2)   # 2 = CDR
$fsi.Root.AddTree($stage, $false)         # add the folder contents to the ISO root
$res = $fsi.CreateResultImage()
[IsoHelper]::Save($unattendIso, $res.ImageStream, $res.BlockSize, $res.TotalBlocks)

# 4. Build the Generation 2 VM and boot the Windows + answer-file ISOs.
New-VM -Name $vmName -Generation 2 -MemoryStartupBytes 4GB -NewVHDPath "$vhd" -NewVHDSizeBytes 80GB -SwitchName "Default Switch"
Set-VMProcessor -VMName $vmName -Count 2
Get-VMIntegrationService -VMName $vmName -Name "Guest Service Interface" | Enable-VMIntegrationService
Set-VM -Name $vmName -AutomaticCheckpointsEnabled $false
$winDvd = Add-VMDvdDrive -VMName $vmName -Path "$winIso" -Passthru   # DVD 1: Windows installer
Add-VMDvdDrive -VMName $vmName -Path "$unattendIso"                  # DVD 2: Autounattend.xml
Set-VMFirmware -VMName $vmName -SecureBootTemplate "MicrosoftWindows" -FirstBootDevice $winDvd
Start-VM -Name $vmName

# 5. Wait for the OS, then control it over PowerShell Direct (VMBus, no network).
Wait-VM -Name $vmName -For Heartbeat -Timeout 1800
$cred = [pscredential]::new($guestUser, (ConvertTo-SecureString $guestPass -AsPlainText -Force))
do { $s = New-PSSession -VMName $vmName -Credential $cred -ErrorAction SilentlyContinue; if (-not $s) { Start-Sleep 10 } } until ($s)
Invoke-Command -Session $s -ScriptBlock { hostname; whoami }

# 6. The answer file holds the plaintext password — remove it once the guest is reachable.
Get-VMDvdDrive -VMName $vmName | Where-Object Path -eq $unattendIso | Remove-VMDvdDrive
Remove-Item $work -Recurse -Force
```

After this, the VM is fully under host control with nothing performed inside the guest. On later runs, reuse it (`Get-VM`; `Start-VM` only when it is off).

If the credential username is the built-in `Administrator`, set `<AdministratorPassword>` in the `oobeSystem` pass instead of adding a `LocalAccount`, and connect as `administrator`. This recipe mirrors the patterns in `fdcastel/Hyper-V-Automation`, `StefanScherer/packer-windows`, and `alissonsol/yuruna`, plus Microsoft's PowerShell Direct requirements.

## Safety Rules

- Never reveal, log, or commit credential contents.
- Do not store secrets in tracked files. Write `Autounattend.xml` (it embeds the guest password) only under `$env:TEMP` and delete it once the guest is reachable; never place it in the repo.
- Do not use broad VM-destructive operations.
- Creating the target VM when it is absent and starting it when it is off are allowed as part of ensuring availability; do not stop, restart, checkpoint, or delete a VM unless the user explicitly asks.
- Prefer read-only diagnostics before changing guest state.

## Useful Commands

Reuse the VM, or provision it when missing (see **Zero-Touch VM Provisioning** for the full create path):

```powershell
$vmName = "Marix_TestVm"
$vm = Get-VM -Name $vmName -ErrorAction SilentlyContinue
if (-not $vm) {
    # VM missing -> run the Zero-Touch VM Provisioning procedure (unattended Windows install).
} elseif ($vm.State -ne "Running") {
    Start-VM -Name $vmName
}
```

Routine diagnostics:

```powershell
Get-VM -Name Marix_TestVm
Get-VMIntegrationService -VMName Marix_TestVm
Copy-VMFile -Name Marix_TestVm -FileSource Host -SourcePath $hostPath -DestinationPath $guestPath -CreateFullPath -Force
```

Credential construction pattern:

```powershell
$username = Get-Content -LiteralPath C:\r\Marix\.credential\HYPERV_OPERATOR_USERNAME.txt -Raw
$password = Get-Content -LiteralPath C:\r\Marix\.credential\HYPERV_OPERATOR_PASSWORD.txt -Raw
$securePassword = ConvertTo-SecureString $password.Trim() -AsPlainText -Force
$credential = [pscredential]::new($username.Trim(), $securePassword)
```

## Operational Experience

## Purpose

Durable operational notes for Hyper-V VM access and Marix CLI deployment validation.

## Current VM Context

- VM name: `Marix_TestVm`. If it is absent, provision it fully unattended (an `Autounattend.xml` creates the `.credential` admin account; see the skill's Zero-Touch VM Provisioning section) so PowerShell Direct works immediately — no manual steps inside the guest.
- `Marix_TestVm` is the only valid target VM for Marix operations. Do not substitute any other existing VM when this target is missing or host permissions prevent verification; report the target as blocked instead.
- VM Guest Service Interface is enabled, so host-to-guest file copy with `Copy-VMFile` works.
- PowerShell Direct runs over VMBus and needs no guest network, NIC, or WinRM — only a running Windows 10/Server 2016+ guest and a valid local-account credential. Creating that account from `.credential` via the unattended answer file is what makes a freshly built VM controllable with zero in-guest steps.
- PowerShell Direct requires a guest credential; without it, `Invoke-Command -VMName Marix_TestVm` fails with a missing `Credential` parameter.
- VM network remote execution ports were not available from the host during prior checks:
  - SSH `22`: unavailable.
  - WinRM `5985`: unavailable.
  - WinRM TLS `5986`: unavailable.
- Use `.credential/HYPERV_OPERATOR_USERNAME.txt` and `.credential/HYPERV_OPERATOR_PASSWORD.txt` for PowerShell Direct credentials. Never print their contents.

## Marix CLI Deployment

- Host-side prepared deploy folder: `%LOCALAPPDATA%\Temp\marix-cli-vm`.
- Guest deployment folder: `C:\MarixRemoteCli`.
- Launcher: `C:\MarixRemoteCli\run-marix-cli.cmd`.
- Launcher behavior: sets `MARIX_SRC_ROOT` to `C:\MarixRemoteCli\src` and runs `marix-cli.exe`.
- CLI remote config points to Ubuntu core at `43.142.167.218:22345`.

## Remote Core Context

- Ubuntu SSH host: `ubuntu@43.142.167.218`.
- SSH key path on host: read from `.credential/CORE_SERVER_SSH_KEY.txt` (git-ignored).
- `marix-core` was previously built under `~/marix-deploy/src/.target/release/marix-core`.
- Remote core listens on `0.0.0.0:22345`.
- DeepSeek API was validated from Ubuntu with HTTP 200 and a minimal chat completion.

## Validation Pattern

1. Confirm Ubuntu core is listening:
   `ssh -i <CORE_SERVER_SSH_KEY> -o IdentitiesOnly=yes ubuntu@43.142.167.218 'ss -ltnp | grep 22345'` (key path in `.credential/CORE_SERVER_SSH_KEY.txt`)
2. Confirm host can reach core port:
   `Test-NetConnection -ComputerName 43.142.167.218 -Port 22345`
3. Copy CLI files to VM with `Copy-VMFile`.
4. Execute in guest with PowerShell Direct using stored credentials:
   `C:\MarixRemoteCli\run-marix-cli.cmd "Reply exactly VM_OK."`

## Safety Notes

- Do not commit credential files.
- Do not print credential contents.
- Do not restart or modify VM lifecycle state unless explicitly requested.

## Recent Validation Notes

- 2026-06-21: PowerShell Direct with `.credential` credentials succeeded for `Marix_TestVm`.
- 2026-06-21: From inside `Marix_TestVm`, TCP to `43.142.167.218:22345` succeeded while ICMP ping did not; rely on TCP checks for core reachability.
- 2026-06-21: `C:\MarixRemoteCli\run-marix-cli.cmd "Reply exactly VM_OK."` completed with exit code 0 and output `VM_OK`.
- 2026-06-22: A non-elevated host token can see Hyper-V services running but `Get-VM` and `Copy-VMFile` fail with "required permission"; rerun from an elevated host session or an effective Hyper-V Administrators token.
- 2026-06-22: Host-side staging at `%LOCALAPPDATA%\Temp\marix-cli-vm` should contain only the CLI binary, launcher, `deployment.json`, and `src/**/config.json`; do not copy `.credential`.
- 2026-06-22: Host TCP and SSH checks showed the Ubuntu core host reachable and `marix-core` listening on `43.142.167.218:22345`.
- 2026-06-22: Current `marix-cli` source builds and no-arg execution exits 0, but chat input fails with `PipeClient implementation is not linked`; `VM_OK` validation requires a linked client transport or a previously working binary.
- 2026-06-23: During zero-touch creation of `Marix_TestVm`, a Gen2 Windows ISO can fail initial DVD boot if the UEFI "press any key" prompt times out; Hyper-V `Msvm_Keyboard.TypeKey` can send Enter/Space immediately after restarting the newly created VM so unattended setup proceeds without guest interaction.
- 2026-06-23: A freshly installed `Marix_TestVm` may not have the MSVC runtime needed by the default Windows Rust binary; building `marix-cli.exe` with static CRT (`RUSTFLAGS=-C target-feature=+crt-static`) made the launcher exit 0 in the guest.

- 2026-06-23: Ubuntu core bind address is now handled directly rather than via an alias override — the runtime hardcodes a `0.0.0.0` listen and `config.toml` only carries ports/addresses as literals, so `marix-core` binds successfully while the VM CLI keeps the host-side reachable address.
- 2026-06-23: Stop old Ubuntu core instances by PID from `pgrep -f '[m]marix-core'`; older launches may show only `.target/release/marix-core` in argv, so a full deploy-path pattern can miss them and falsely report a stale listener as the new core.
- 2026-06-23: Remote non-login SSH scripts may see the distro Cargo instead of rustup Cargo; source `$HOME/.cargo/env` before building, and strip CRLF when piping PowerShell here-strings into remote bash (`tr -d '\r' | bash -s`).

- 2026-06-23: If Ubuntu core exits with `ReceiveFailed("model backend request failed: builder error")` while direct DeepSeek curl succeeds, check for non-printable characters in the API key passed to the Rust process; exporting a printable-cleaned key (`[!-~]`) let `reqwest` build the Bearer header and restored VM `VM_OK` validation.
- 2026-06-23: The current Windows CLI is interactive and reads stdin; passing `"Reply exactly VM_OK."` as a launcher argument only prints the prompt and exits. Use a stdin pipe for real end-to-end validation (for example, echo the prompt into the launcher).
- 2026-06-28: In a non-elevated host session, `Get-VM -Name Marix_TestVm` failed with Hyper-V permission denial before deployment; do not attempt `Copy-VMFile` until the session is elevated or has effective Hyper-V Administrators permissions.
- 2026-06-28: Current `marix-cli` no-arg path returns `usage: marix-cli <prompt>` with exit code 1 before loading config, so a no-arg smoke check does not require credentials or core reachability. Prompted CLI runs still need deployed config plus a non-secret placeholder DeepSeek credential while config loading eagerly reads `credential.directory`.
- 2026-06-28: If `whoami /groups` shows `BUILTIN\Administrators` as "Group used for deny only", the current process is still not elevated even if the account is an administrator. Creating a highest-privilege scheduled task also failed with access denied, so start a new elevated session before retrying `Get-VM`, `Copy-VMFile`, or PowerShell Direct.
- 2026-06-28: `marix-cli.exe` built with the default MSVC runtime failed inside `Marix_TestVm` with exit code `-1073741515` before printing output. Rebuilding with `RUSTFLAGS="-C target-feature=+crt-static"` produced a self-contained binary; after copying it to `C:\MarixRemoteCli`, no-arg usage returned exit code `1` and `"Reply exactly VM_DEPLOY_OK."` returned `VM_DEPLOY_OK` with exit code `0`.
- 2026-06-28: `marix-cli` no-argument mode is now interactive stdin mode. VM validation used a finite pipe into `C:\MarixRemoteCli\run-marix-cli.cmd`; `"Reply exactly VM_LOOP_ONE."` and `"Reply exactly VM_LOOP_TWO."` returned `VM_LOOP_ONE` and `VM_LOOP_TWO` with exit code `0`.
- 2026-07-04: `Marix_TestVm` guest identity is host `DESKTOP-4LQ2VC3`, local admin account `marixagent`, Windows 11 `10.0.26200`, 64-bit AMD64. Starting from Off: `Start-VM` then `Wait-VM -For Heartbeat` (~<2 min) and PowerShell Direct via `.credential` connected on the first `New-PSSession` attempt. All six integration services (incl. Guest Service Interface) are enabled.
- 2026-07-04: `Marix_TestVm` NIC is on the Hyper-V Default Switch NAT: IPv4 `172.22.82.30`, GW/DNS `172.22.80.1`. From inside the guest, DNS resolves and outbound internet TCP:443 works. ICMP to remote agent host `43.142.167.218` fails (blocked) — keep using TCP checks, not ping.
- 2026-07-04: Outbound reachability from the guest to the remote agent host `43.142.167.218`: port `22345` is OPEN (raw TCP + Test-NetConnection succeed), which proves guest egress + host routing are fine. Target port `22346` returns TIMEOUT/FILTERED (dropped SYN, not RST/refused) — the remote side is not listening/allowing `22346` yet. Before the host role can connect outbound to `43.142.167.218:22346`, the remote agent must listen there and its cloud security group/firewall must permit it.
- 2026-07-04: To classify a remote TCP port quickly, use `TcpClient.BeginConnect` + `AsyncWaitHandle.WaitOne(<ms>)` from inside the guest; it distinguishes CONNECTED vs REFUSED vs TIMEOUT/FILTERED in a bounded time, whereas `Test-NetConnection` on a filtered port blocks for a long timeout.
- 2026-07-04: Guest deploy dirs: `C:\MarixRemoteCli` already holds the prior CLI deploy (`.credential`, `src`, `config.toml`, `deployment.json`, `marix-cli.exe`, `run-marix-cli.cmd`) — a template for the layout the host role will use. Created `C:\MarixHost` for the host + tools role; in-guest write test passed and `C:` has ~48 GB free. `Copy-VMFile -FileSource Host ... -CreateFullPath -Force` host->guest round-trip verified via a unique token that matched exactly on read-back.
- 2026-07-04: Deployed the Marix "host + tools" role into `Marix_TestVm` at `C:\MarixHost` from host payload `C:\r\Marix\.deploy\host-payload` (13 files: `marix-host.exe`, `config.toml`, 5 `tool\*.exe`, 6 `.credential\*.txt`). Per-file `Copy-VMFile -FileSource Host -CreateFullPath -Force` (enumerate source, map each to `C:\MarixHost\<relative>`) preserves the `tool/.credential` subdirs; a guest recursive listing confirmed all 13 files with byte sizes matching the source.
- 2026-07-04: Prebuilt `marix-host.exe` is dynamically MSVC-linked (import table shows `vcruntime140.dll`). A fresh Win11 guest has no VC++ runtime, so the exe exits instantly with code `-1073741515` (`0xC0000135` STATUS_DLL_NOT_FOUND), empty stdout/stderr, and no `log` dir created — a loader failure before any app code runs (not a config/panic error). Fix without rebuilding: app-local deploy by `Copy-VMFile` of `vcruntime140.dll` (+ `vcruntime140_1.dll`, `msvcp140.dll`) from host `System32` into `C:\MarixHost`. The UCRT `api-ms-win-crt-*` imports are already satisfied by the guest OS `ucrtbase.dll`.
- 2026-07-04: Launch pattern that works over PowerShell Direct for a long-running host service: inside the `Invoke-Command` scriptblock set `$env:MARIX_CONFIG='C:\MarixHost\config.toml'` then `Start-Process -FilePath C:\MarixHost\marix-host.exe -WorkingDirectory C:\MarixHost -RedirectStandardOutput/Error C:\MarixHost\host.*.log -WindowStyle Hidden -PassThru` (no `-Wait`). The child inherits `MARIX_CONFIG` and survives the PSSession closing. `config.toml` uses `marix_path = "."`, so `.credential`, `tool`, `log` resolve against the working dir. A healthy start leaves `host.out.log`/`host.err.log` empty.
- 2026-07-04: Marix host-role config endpoints: `[client] core_address = 43.142.167.218:22345`, `[agent] host_bind_address = 43.142.167.218:22346`. After launch, `marix-host` held an Established TCP connection `172.22.82.30:<ephemeral> -> 43.142.167.218:22346`, proving outbound connection to the remote agent. Port `22346` (previously filtered/timeout in earlier notes) is now open; guest `Test-NetConnection 43.142.167.218 -Port 22346` returns `TcpTestSucceeded=True`. Verify the real connection with `Get-NetTCPConnection -OwningProcess <marix-host PID>`, not just `Test-NetConnection` (which opens its own socket).
- 2026-07-05: Fresh host payload `C:\r\Marix\.deploy\host-payload` redeployed to `Marix_TestVm:C:\MarixHost`; old `marix-host.exe` PID 6036 stopped by explicit PID, 25 payload files copied plus app-local VC++ DLLs from host System32 (`vcruntime140.dll`, `vcruntime140_1.dll`, `msvcp140.dll`). New `marix-host.exe` started with `MARIX_CONFIG=C:\MarixHost\config.toml` as PID 4284; guest TCP to `43.142.167.218:22346` connected and the process owned an Established connection to that endpoint.

- 2026-07-07: Read-only recon of `Marix_TestVm` (elevated host session, `FAREAST\dexterzou`). VM Running (uptime ~2d23h, config v12.0); all 6 integration services enabled incl. Guest Service Interface. PowerShell Direct via `.credential` connected first try. Guest = host `DESKTOP-4LQ2VC3`, admin `marixagent`, Win11 Pro `10.0.26200` build 26200, `OSArchitecture=64-bit`, `PROCESSOR_ARCHITECTURE=AMD64` (x86_64). Guest NIC `172.22.82.30`, GW/DNS `172.22.80.1` (Default Switch NAT).
- 2026-07-07: Guest egress: generic internet OK (`1.1.1.1:443` CONNECTED). `api.deepseek.com` resolves to internal `10.139.67.144` (corporate split-DNS) and is NOT reachable from the NAT guest (TIMEOUT) — irrelevant to host role since DeepSeek is called core-side on Ubuntu, not from the Windows guest.
- 2026-07-07: Remote agent host `43.142.167.218` from guest — 22345 OPEN, 22346 OPEN, 22347 REFUSED (RST, actively closed). Confirmed by both bounded TcpClient classifier and `Test-NetConnection` (`TcpTestSucceeded` True/True/False).
- 2026-07-07: `C:\MarixRemoteCli` (vm_cli_root) is the CLI role ONLY: `marix-cli.exe` (2.83MB), `run-marix-cli.cmd`, `config.toml` (472B), `deployment.json`, `.credential\DEEPSEEK_API_KEY.txt` (13B placeholder), `src\cli\config.json`. NO `marix-host.exe`, NO `marix_tool_*.exe`. The host component does NOT live under vm_cli_root.
- 2026-07-07: `C:\MarixHost` is the host role deploy AND `marix-host.exe` was ALREADY RUNNING at recon time — PID 612, started 7/5 21:43, holding Established `172.22.82.30:50990 -> 43.142.167.218:22346` (owning PID 612). Contents: `marix-host.exe` (2.97MB), app-local `msvcp140.dll`/`vcruntime140.dll`/`vcruntime140_1.dll`, `config.toml` (690B), `tool\` (5 exes: list_directory/read_file/search_text/shell_execute/write_file), `.credential\` (`DEEPSEEK_API_KEY.txt` 35B, `CORE_SERVER_PASSWORD/ROOT_PASSWORD`, `HYPERV_OPERATOR_*`, README), empty `host.*.log` (healthy). Takeaway for future host deploys: target `C:\MarixHost`, tools go under `tool\` named `marix_tool_*.exe`; a live instance may already be running — stop by explicit PID before redeploy.
- 2026-07-07: Redeployed a freshly-built host to `Marix_TestVm:C:\MarixHost` from a host-stage folder (7 files: `marix-host.exe` 3,605,504B/3.44MB — larger than the prior 2.97MB build; `config.toml` 636B using the new `[runtime]/[client]/[agent]/[model]/[model.deepseek]/[telemetry]/[credential]/[tool]` schema — `[telemetry]` present, old `[logging]` gone; 5 `tool\marix_tool_*.exe`). Re-resolved the old host by name (`Get-Process marix-host`) → PID 612 (unchanged since recon; connection ports churn on reconnect but the PID is stable), stopped with `Stop-Process -Id`; process count went to 0 and Established `:22346` conns cleared. `Copy-VMFile -FileSource Host -CreateFullPath -Force` overwrote each file per full destination path (works only after the exe is stopped/unlocked); replaced ONLY exe/config/5 tools — `.credential\DEEPSEEK_API_KEY.txt` (35B) and app-local `msvcp140.dll`/`vcruntime140.dll`/`vcruntime140_1.dll` left untouched and re-verified. New host launched via the detached Start-Process pattern (`$env:MARIX_CONFIG='C:\MarixHost\config.toml'`, `-WorkingDirectory C:\MarixHost`, `-RedirectStandardOutput/Error host.out.log/host.err.log`, `-WindowStyle Hidden -PassThru`, no `-Wait`) as PID 5876; it survived PSSession teardown and holds Established `172.22.82.30:51166 -> 43.142.167.218:22346` (verified via `Get-NetTCPConnection -OwningProcess`). `host.out.log` stayed empty; `host.err.log` contained only the benign `telemetry logger unavailable, continuing without it: telemetry I/O error: No connection ... actively refused it. (os error 10061)` line — telemetry port 22347 is refused (RST) by design and the host continues normally (NOT a failure). The new 3.44MB build starts cleanly on the already-present preserved VC++ runtime — no new DLL required.
- 2026-07-07: The repo-wide `.alias/` alias system was removed. VM, host, and CLI deployments no longer include a `.alias/` folder, and `config.toml` is now literal (no `{{...}}` placeholder resolution at deploy time). Do not stage or copy `.alias` into any deploy target — earlier dated notes that list `.alias` describe the old layout and no longer apply.
