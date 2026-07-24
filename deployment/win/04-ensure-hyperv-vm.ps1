param(
    [Parameter(Mandatory)][string] $RepoRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# Unlike steps 2 and 3, this step's paths are all fixed, absolute Hyper-V host
# locations under 'C:\marix\hyperv\' -- not relative to the repository -- per this
# tree's established Hyper-V conventions (see the win-hyperv-operation skill).
# $RepoRoot is still accepted (and required) so this script's calling convention
# matches every other step script's; it is simply not otherwise used in this
# script's own body.

function Test-HyperVReady {
    <#
    Confirms Hyper-V is genuinely enabled and functional, WITHOUT relying on
    Get-WindowsOptionalFeature -- empirically confirmed unreliable on this class of
    machine (fails with a DISM/COM "class not registered" error even when Hyper-V is
    fully functional). Instead requires BOTH: the Hyper-V PowerShell module is
    present, and the real Virtual Machine Management Service (vmms) is running.
    #>
    $hyperVModule = Get-Module -ListAvailable -Name Hyper-V
    $vmmsService = Get-Service -Name vmms -ErrorAction SilentlyContinue

    $moduleAvailable = ($null -ne $hyperVModule)
    # Short-circuit ordering matters under Set-StrictMode -Version Latest: accessing
    # '.Status' on a $null service object throws, so the null check must come first.
    $vmmsRunning = ($null -ne $vmmsService) -and ($vmmsService.Status -eq 'Running')

    return ($moduleAvailable -and $vmmsRunning)
}

function Wait-GuestReachable {
    <#
    Bounded poll for genuine PowerShell Direct reachability using the fixed guest
    credential. This -- not Wait-VM's heartbeat, not $vm.State, and not systemd-style
    service-active checks -- is the authoritative readiness gate: a VM can report a
    healthy heartbeat while Windows is still mid-boot and not yet accepting logons.
    #>
    param(
        [Parameter(Mandatory)][string] $VmName,
        [Parameter(Mandatory)][pscredential] $Credential,
        [Parameter(Mandatory)][int] $TimeoutSeconds,
        [int] $RetryDelaySeconds = 10
    )

    $stopwatch = [Diagnostics.Stopwatch]::StartNew()
    do {
        $session = $null
        try {
            $session = New-PSSession -VMName $VmName -Credential $Credential -ErrorAction SilentlyContinue
        }
        catch {
            # Empirically confirmed: some failure modes (e.g. a momentary "VM not
            # found" race, or other transient WMI/VMMS hiccups) are terminating
            # errors that -ErrorAction SilentlyContinue alone does not suppress.
            # Treat any such attempt as simply "not reachable yet" and keep retrying
            # within the bound below, exactly like a plain connection failure.
            $session = $null
        }

        if ($session) {
            Remove-PSSession -Session $session
            return
        }

        Start-Sleep -Seconds $RetryDelaySeconds
    } while ($stopwatch.Elapsed.TotalSeconds -lt $TimeoutSeconds)

    throw "Guest '$VmName' did not become reachable via PowerShell Direct with the fixed credential within $TimeoutSeconds seconds."
}

function Send-VmBootKeystroke {
    <#
    Best-effort nudge only -- NOT a correctness guarantee, and deliberately does not
    throw on failure. A Generation 2 Windows installer ISO can stall at the firmware's
    "press any key to boot from CD or DVD..." prompt; sending a keystroke to the guest
    shortly after Start-VM lets unattended setup proceed without a human at the
    console. Uses Get-CimInstance/Get-CimAssociatedInstance/Invoke-CimMethod (not
    Get-WmiObject, which is unavailable in PowerShell 7) so this works identically on
    both required hosts. If every attempt fails, this only logs a warning: the bounded
    Wait-VM heartbeat wait and Wait-GuestReachable check that follow it are the real,
    authoritative gates for provisioning success, not this nudge.
    #>
    param(
        [Parameter(Mandatory)][string] $VmName
    )

    $escapedVmName = $VmName -replace "'", "''"
    $attemptDeadline = (Get-Date).AddSeconds(20)
    $sentAny = $false

    while ((Get-Date) -lt $attemptDeadline) {
        try {
            $computerSystem = Get-CimInstance -Namespace 'root\virtualization\v2' -ClassName Msvm_ComputerSystem -Filter "ElementName='$escapedVmName'" -ErrorAction Stop
            if ($null -eq $computerSystem) {
                throw "No Msvm_ComputerSystem instance found for VM '$VmName'."
            }
            $keyboard = Get-CimAssociatedInstance -InputObject $computerSystem -ResultClassName Msvm_Keyboard -ErrorAction Stop
            if ($null -eq $keyboard) {
                throw "No associated Msvm_Keyboard instance found for VM '$VmName'."
            }
            Invoke-CimMethod -InputObject $keyboard -MethodName TypeKey -Arguments @{ keyCode = 13 } -ErrorAction Stop | Out-Null
            $sentAny = $true
        }
        catch {
            # Swallow and retry within the bounded window above; see comment header.
        }
        Start-Sleep -Seconds 3
    }

    if (-not $sentAny) {
        Write-Host "Warning: could not confirm any boot keystroke was sent to VM '$VmName' via WMI/CIM (continuing regardless; the bounded heartbeat and reachability waits below remain the authoritative readiness gates)." -ForegroundColor Yellow
    }
}

function Get-AutounattendXmlContent {
    <#
    Produces the Autounattend.xml answer-file content: Gen2/UEFI partitions, accept
    EULA, skip OOBE, create the fixed guest admin account, allow scripts. The
    W269N-... key is the Windows Pro GVLK (no activation prompt).
    #>
    param(
        [Parameter(Mandatory)][string] $GuestUserName,
        [Parameter(Mandatory)][string] $GuestPassword
    )

    return @"
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
      <UserAccounts><LocalAccounts><LocalAccount wcm:action="add"><Name>$GuestUserName</Name><Group>Administrators</Group><Password><Value>$GuestPassword</Value><PlainText>true</PlainText></Password></LocalAccount></LocalAccounts></UserAccounts>
      <AutoLogon><Enabled>true</Enabled><Username>$GuestUserName</Username><Password><Value>$GuestPassword</Value><PlainText>true</PlainText></Password><LogonCount>1</LogonCount></AutoLogon>
      <TimeZone>UTC</TimeZone>
    </component>
  </settings>
</unattend>
"@
}

function New-AutounattendIso {
    <#
    Packs a directory (expected to contain Autounattend.xml at its root) into a small
    ISO 9660 image via IMAPI2FS -- built into Windows, no Windows ADK required.
    #>
    param(
        [Parameter(Mandatory)][string] $SourceDirectory,
        [Parameter(Mandatory)][string] $DestinationIsoPath
    )

    if (-not ('MarixIsoHelper' -as [type])) {
        try {
            Add-Type -CompilerOptions '/unsafe' -TypeDefinition @'
public class MarixIsoHelper {
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
        catch {
            throw "Failed to compile the ISO-packing helper type: $_"
        }
    }

    try {
        $fileSystemImage = New-Object -ComObject IMAPI2FS.MsftFileSystemImage
        $fileSystemImage.VolumeName = 'UNATTEND'
        $fileSystemImage.ChooseImageDefaultsForMediaType(2) | Out-Null   # 2 = CDR
        $fileSystemImage.Root.AddTree($SourceDirectory, $false) | Out-Null
        $resultImage = $fileSystemImage.CreateResultImage()
        [MarixIsoHelper]::Save($DestinationIsoPath, $resultImage.ImageStream, $resultImage.BlockSize, $resultImage.TotalBlocks)
    }
    catch {
        throw "Failed to pack '$SourceDirectory' into answer-file ISO '$DestinationIsoPath': $_"
    }

    if (-not (Test-Path -LiteralPath $DestinationIsoPath -PathType Leaf)) {
        throw "Answer-file ISO was not created: $DestinationIsoPath"
    }
}

function Invoke-ZeroTouchVmProvisioning {
    <#
    Fully, automatically provisions VmName from scratch: downloads the Windows
    installer ISO if missing, generates an Autounattend.xml answer file baking in
    the fixed guest credential, packs it into a tiny ISO, creates a Generation 2 VM
    that boots both ISOs, nudges it past the UEFI "press any key" prompt, waits
    (bounded) for first-boot heartbeat and PowerShell Direct reachability, then
    cleans up the temporary answer-file ISO and work directory. Purely additive and
    non-destructive: only ever creates new files/VMs, never touches anything
    pre-existing.
    #>
    param(
        [Parameter(Mandatory)][string] $VmName,
        [Parameter(Mandatory)][string] $GuestUserName,
        [Parameter(Mandatory)][string] $GuestPassword,
        [Parameter(Mandatory)][pscredential] $GuestCredential,
        [Parameter(Mandatory)][string] $WinIsoPath,
        [Parameter(Mandatory)][string] $VhdPath,
        [Parameter(Mandatory)][string] $WorkRoot
    )

    $stageDirectory = Join-Path $WorkRoot 'iso'
    New-Item -ItemType Directory -Path $stageDirectory -Force | Out-Null

    # 0. Ensure the Windows installation ISO exists; download it automatically when
    #    missing. Fido is the script Rufus uses to resolve official Microsoft retail
    #    ISO download links.
    if (-not (Test-Path -LiteralPath $WinIsoPath -PathType Leaf)) {
        Write-Host "Windows installation ISO was not found at '$WinIsoPath'; resolving and downloading it automatically..."
        try {
            [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
            New-Item -ItemType Directory -Path (Split-Path -Parent $WinIsoPath) -Force | Out-Null
            $fidoPath = Join-Path $WorkRoot 'Fido.ps1'
            Invoke-WebRequest -Uri 'https://raw.githubusercontent.com/pbatard/Fido/master/Fido.ps1' -OutFile $fidoPath -UseBasicParsing
        }
        catch {
            throw "Failed to download the Fido ISO-resolver script: $_"
        }

        try {
            $isoUrl = (& $fidoPath -Win 11 -Ed Pro -Lang English -Arch x64 -GetUrl) | Select-Object -Last 1
        }
        catch {
            throw "Fido failed to resolve a Windows installation ISO download URL: $_"
        }
        if ([string]::IsNullOrWhiteSpace($isoUrl)) {
            throw 'Fido returned an empty Windows installation ISO download URL.'
        }

        Write-Host 'Downloading Windows installation ISO (official retail ISO, ~6 GB; resumable)...'
        try {
            Start-BitsTransfer -Source $isoUrl -Destination $WinIsoPath
        }
        catch {
            throw "Failed to download the Windows installation ISO from '$isoUrl' to '$WinIsoPath': $_"
        }
        if (-not (Test-Path -LiteralPath $WinIsoPath -PathType Leaf)) {
            throw "Windows installation ISO download reported success but the file is missing: $WinIsoPath"
        }
    }

    # 1/2. Fixed guest admin account baked into the answer file at install time.
    try {
        Get-AutounattendXmlContent -GuestUserName $GuestUserName -GuestPassword $GuestPassword |
            Set-Content -LiteralPath (Join-Path $stageDirectory 'Autounattend.xml') -Encoding UTF8
    }
    catch {
        throw "Failed to generate the Autounattend.xml answer file: $_"
    }

    # 3. Pack Autounattend.xml into a tiny ISO (it must sit at the ISO root).
    $unattendIsoPath = Join-Path $WorkRoot 'unattend.iso'
    New-AutounattendIso -SourceDirectory $stageDirectory -DestinationIsoPath $unattendIsoPath

    # 4. Build the Generation 2 VM and boot the Windows + answer-file ISOs.
    Write-Host "Creating VM '$VmName' (Generation 2, 4GB RAM, 80GB VHD, switch 'Default Switch')..."
    try {
        New-VM -Name $VmName -Generation 2 -MemoryStartupBytes 4GB -NewVHDPath $VhdPath -NewVHDSizeBytes 80GB -SwitchName 'Default Switch' | Out-Null
    }
    catch {
        throw "Failed to create VM '$VmName' (VHD '$VhdPath', switch 'Default Switch'): $_"
    }

    try {
        Set-VMProcessor -VMName $VmName -Count 2 | Out-Null
    }
    catch {
        throw "Failed to configure the processor count for VM '$VmName': $_"
    }

    try {
        Get-VMIntegrationService -VMName $VmName -Name 'Guest Service Interface' | Enable-VMIntegrationService | Out-Null
    }
    catch {
        throw "Failed to enable the Guest Service Interface integration service for VM '$VmName': $_"
    }

    try {
        Set-VM -Name $VmName -AutomaticCheckpointsEnabled $false | Out-Null
    }
    catch {
        throw "Failed to disable automatic checkpoints for VM '$VmName': $_"
    }

    try {
        $winDvdDrive = Add-VMDvdDrive -VMName $VmName -Path $WinIsoPath -Passthru
    }
    catch {
        throw "Failed to attach the Windows installation ISO to VM '$VmName': $_"
    }

    try {
        Add-VMDvdDrive -VMName $VmName -Path $unattendIsoPath | Out-Null
    }
    catch {
        throw "Failed to attach the Autounattend answer-file ISO to VM '$VmName': $_"
    }

    try {
        Set-VMFirmware -VMName $VmName -SecureBootTemplate 'MicrosoftWindows' -FirstBootDevice $winDvdDrive | Out-Null
    }
    catch {
        throw "Failed to configure firmware and boot order for VM '$VmName': $_"
    }

    Write-Host "Starting VM '$VmName' for unattended installation..."
    try {
        Start-VM -Name $VmName | Out-Null
    }
    catch {
        throw "Failed to start VM '$VmName' after provisioning: $_"
    }

    # 5. A Gen2 Windows ISO can stall at the UEFI "press any key to boot" prompt --
    #    nudge it past that so unattended setup proceeds without manual interaction.
    Send-VmBootKeystroke -VmName $VmName

    # 6. Wait for the OS, then control it over PowerShell Direct (VMBus, no network).
    Write-Host 'Waiting for the first-boot heartbeat (up to 1800 seconds for Windows setup and first boot)...'
    Wait-VM -Name $VmName -For Heartbeat -Timeout 1800 | Out-Null

    Write-Host 'Confirming guest reachability via PowerShell Direct (up to 600 seconds)...'
    Wait-GuestReachable -VmName $VmName -Credential $GuestCredential -TimeoutSeconds 600

    # 7. Remove the answer-file ISO and work files once the guest is reachable. This
    #    is best-effort cleanup after the substantive goal has already been achieved;
    #    a leftover temp file must never be reported as a provisioning failure.
    Write-Host 'Cleaning up the temporary answer-file ISO and provisioning work directory...'
    try {
        Get-VMDvdDrive -VMName $VmName | Where-Object { $_.Path -eq $unattendIsoPath } | Remove-VMDvdDrive
    }
    catch {
        Write-Host "Warning: could not remove the temporary answer-file DVD drive from VM '$VmName': $_" -ForegroundColor Yellow
    }
    try {
        Remove-Item -LiteralPath $WorkRoot -Recurse -Force -ErrorAction SilentlyContinue
    }
    catch {
        Write-Host "Warning: could not remove the temporary provisioning work directory '$WorkRoot': $_" -ForegroundColor Yellow
    }
}

$vmName = 'Marix_TestVm'
$guestUserName = 'marixagent'
$guestPassword = '123'
$securePassword = ConvertTo-SecureString -String $guestPassword -AsPlainText -Force
$guestCredential = [pscredential]::new($guestUserName, $securePassword)

$hyperVRoot = 'C:\marix\hyperv'
$winIsoPath = Join-Path $hyperVRoot 'ISOs\Windows.iso'
$vmVhdPath = Join-Path $hyperVRoot "$vmName.vhdx"
$provisionWorkRoot = Join-Path $hyperVRoot "work\$vmName-unattend"

if (-not (Test-HyperVReady)) {
    throw "Hyper-V does not appear to be enabled or running on this machine (the Hyper-V PowerShell module and/or the Virtual Machine Management Service 'vmms' were not found or not running). Enabling Hyper-V requires enabling the 'Microsoft-Hyper-V-All' Windows optional feature (e.g. via 'Enable-WindowsOptionalFeature -Online -FeatureName Microsoft-Hyper-V-All -All' run as Administrator, or via Control Panel > Turn Windows features on or off), which requires restarting this physical machine to take effect. This script does not do this automatically because it would force a disruptive, un-consented reboot. Please enable Hyper-V, restart this machine, and re-run this script."
}
Write-Host 'Hyper-V is enabled: the Hyper-V PowerShell module is present and the vmms service is running.'

$vm = Get-VM -Name $vmName -ErrorAction SilentlyContinue
if (-not $vm) {
    Write-Host "VM '$vmName' was not found; provisioning it from scratch (zero-touch; this can take up to ~35 minutes for Windows setup and first boot)..."
    Invoke-ZeroTouchVmProvisioning -VmName $vmName -GuestUserName $guestUserName -GuestPassword $guestPassword -GuestCredential $guestCredential -WinIsoPath $winIsoPath -VhdPath $vmVhdPath -WorkRoot $provisionWorkRoot
    $vm = Get-VM -Name $vmName
    Write-Host "VM '$vmName' was provisioned from scratch and is reachable via PowerShell Direct."
}
else {
    if ($vm.State -ne 'Running') {
        Write-Host "VM '$vmName' exists but is not running (state: $($vm.State)); starting it..."
        Start-VM -Name $vmName | Out-Null
    }
    else {
        Write-Host "VM '$vmName' already exists and is running."
    }

    # Always run this wait+reachability check even if $vm.State was already 'Running'
    # at entry -- a VM can report State=Running while Windows is still mid-boot and
    # not yet accepting logons, so State alone is never trusted. Wait-VM on an
    # already-live heartbeat returns near-instantly, so this costs nothing extra in
    # the common already-ready case.
    Wait-VM -Name $vmName -For Heartbeat -Timeout 120 | Out-Null

    try {
        Wait-GuestReachable -VmName $vmName -Credential $guestCredential -TimeoutSeconds 600
    }
    catch {
        throw "VM '$vmName' exists and is running, but the fixed guest credential ('$guestUserName') could not be used to connect via PowerShell Direct within the timeout. This script cannot safely create or repair a guest-side account without already having a working way into that same guest. Please either fix the '$guestUserName' account manually inside the guest (e.g. via Hyper-V Manager's 'Connect' / Enhanced Session console), or delete this VM and re-run this script so it can zero-touch-provision a fresh replacement. Original error: $_"
    }
    Write-Host "VM '$vmName' is reachable via PowerShell Direct."
}

Write-Host ''
Write-Host "Hyper-V VM readiness confirmed: '$vmName' is running and reachable via PowerShell Direct with the fixed guest credential ('$guestUserName')."
