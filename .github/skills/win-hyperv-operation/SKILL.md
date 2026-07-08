---
name: win-hyperv-operation
description: Common skill for Windows Hyper-V VM operations, including VM provisioning, file copy, PowerShell Direct execution, deployment checks, and diagnostics.
---
You are using the Windows Hyper-V operation skill for Marix.

## Scope

Operate the local Hyper-V guest used in the Marix workflow. Provide generic ways to interact with the VM: reachability checks, host-to-guest file transfer, PowerShell Direct execution, guest diagnostics, and copying Marix client artifacts into the guest.

This skill only covers how to interact with the Hyper-V VM. It does not configure Marix, does not describe how any deployed client connects to a server, and does not deal with remote hosts, IPs, ports, or runtime config.

Do not change source code unless the user explicitly asks for a code change. Do not manage overview UI or source design metadata except to report facts relevant to VM operations.


## Current Context

- Target Hyper-V VM: `Marix_TestVm`. Reuse it when it already exists; otherwise provision it fully unattended (see **Zero-Touch VM Provisioning**) — no manual steps inside the guest.
- `Marix_TestVm` is the only valid target VM for Marix operations. Never substitute another existing VM name; if this VM cannot be found, verified, or provisioned because host permissions are insufficient, report the operation as blocked.
- Guest Service Interface is enabled and supports `Copy-VMFile` from host to guest.
- Guest login credentials are fixed: username `marix-client`, password `123`. PowerShell Direct builds a `PSCredential` from these.
- All Hyper-V host artifacts (Windows ISO, VHD, provisioning work files) live under `C:\marix\hyperv\`.
- Client deployment inside the guest lives under `C:\MarixClient\`:
  - CLI client -> `C:\MarixClient\Cli\`
  - Web client -> `C:\MarixClient\Web\`
  - App client -> `C:\MarixClient\App\`
- PowerShell Direct with `Invoke-Command -VMName Marix_TestVm` runs over VMBus and needs no guest network, NIC, WinRM, or SSH — only the fixed guest credential above.

## Responsibilities

- Ensure the target VM exists before any other operation: look it up with `Get-VM -Name Marix_TestVm`. If it is missing, provision it fully unattended (see **Zero-Touch VM Provisioning**) so it boots already controllable via PowerShell Direct — never ask the user to sign into the guest or run anything inside it. If it already exists, reuse it and start it only when it is not running.
- Verify VM state with Hyper-V cmdlets such as `Get-VM` and `Get-VMIntegrationService`.
- Copy deployment files into the guest with `Copy-VMFile`, placing each client under its `C:\MarixClient\` subfolder (`Cli`, `Web`, or `App`).
- Use PowerShell Direct for guest command execution:
  - build a `PSCredential` from the fixed `marix-client` / `123` credentials,
  - call `Invoke-Command -VMName Marix_TestVm -Credential $credential -ScriptBlock { ... }`.
- If an operation fails, distinguish between:
  - Hyper-V host or permission issues,
  - guest credential issues,
  - Guest Service copy issues,
  - guest VM state issues.

## Zero-Touch VM Provisioning

Creating the VM is fully hands-off — the skill obtains the Windows ISO itself and installs unattended; never ask the user to place files, sign into the guest, or run anything inside it. Windows is installed from an `Autounattend.xml` answer file that creates the fixed `marix-client` local administrator account this skill uses for PowerShell Direct. PowerShell Direct rides the Hyper-V VMBus, so the guest needs no network, NIC, WinRM, or SSH; the moment Windows finishes installing, the host can control the VM.

Prerequisites:

- A Windows installation ISO at `C:\marix\hyperv\ISOs\Windows.iso` — the procedure downloads an official Windows 11 Pro retail ISO there automatically (via `pbatard/Fido`, the resolver Rufus uses) whenever the file is missing, so nothing is placed by hand. The host only needs internet access.
- A Generation 2 / UEFI-capable Hyper-V host, run from an elevated session.

Run the whole procedure on the host; it needs no guest interaction:

```powershell
$vmName = "Marix_TestVm"
$winIso = "C:\marix\hyperv\ISOs\Windows.iso"
$vhd    = "C:\marix\hyperv\Marix_TestVm.vhdx"
$work   = "C:\marix\hyperv\work\$vmName-unattend"
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

# 1. Fixed guest admin account this skill connects with.
$guestUser = "marix-client"
$guestPass = "123"

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

# 6. Remove the answer-file ISO and work files once the guest is reachable.
Get-VMDvdDrive -VMName $vmName | Where-Object Path -eq $unattendIso | Remove-VMDvdDrive
Remove-Item $work -Recurse -Force
```

After this, the VM is fully under host control with nothing performed inside the guest. On later runs, reuse it (`Get-VM`; `Start-VM` only when it is off).

This recipe mirrors the patterns in `fdcastel/Hyper-V-Automation`, `StefanScherer/packer-windows`, and `alissonsol/yuruna`, plus Microsoft's PowerShell Direct requirements.

## Safety Rules

- Do not use broad VM-destructive operations.
- Creating the target VM when it is absent and starting it when it is off are allowed as part of ensuring availability; do not stop, restart, checkpoint, or delete a VM unless the user explicitly asks.
- Write `Autounattend.xml` only under `C:\marix\hyperv\work` and delete it once the guest is reachable; never place it in the repo.
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

Credential construction pattern (fixed credentials):

```powershell
$credential = [pscredential]::new("marix-client", (ConvertTo-SecureString "123" -AsPlainText -Force))
```

Deploy a client into the guest (example — CLI; use `Web` or `App` for the other clients):

```powershell
Copy-VMFile -Name Marix_TestVm -FileSource Host -SourcePath $cliArtifact -DestinationPath "C:\MarixClient\Cli\$fileName" -CreateFullPath -Force
```

## Operational Notes

- Hyper-V cmdlets need real elevation: a non-elevated host token (where `whoami /groups` shows `BUILTIN\Administrators` as "Group used for deny only") makes `Get-VM`, `Copy-VMFile`, and PowerShell Direct fail with permission errors. Run from an elevated session before retrying.
- During zero-touch provisioning, a Gen2 Windows ISO can stall at the UEFI "press any key to boot" prompt. Send a keystroke to the guest right after starting the VM (for example `Msvm_Keyboard.TypeKey` with Enter/Space) so unattended setup proceeds without guest interaction.
