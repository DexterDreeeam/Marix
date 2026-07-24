param(
    [Parameter(Mandatory)][string] $RepoRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (Get-Variable PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

. (Join-Path $PSScriptRoot '_deploy-shared.ps1')

# Fixed VM/guest identity, matching 04-ensure-hyperv-vm.ps1's own constants exactly.
$vmName = 'Marix_TestVm'
$guestUserName = 'marixagent'
$guestPassword = '123'
$securePassword = ConvertTo-SecureString -String $guestPassword -AsPlainText -Force
$guestCredential = [pscredential]::new($guestUserName, $securePassword)

$hostExePath = 'C:\MarixHost\marix-host.exe'
$toolDirPrefix = 'C:\MarixHost\tool\'

Write-Host "Connecting to VM '$vmName' via PowerShell Direct..."
$session = Connect-DeploymentVmSession -VmName $vmName -Credential $guestCredential
try {
    Write-Host "Checking for a running Host process ($hostExePath)..."
    Stop-VmProcessesByPath -Session $session -ExactPath $hostExePath -Label 'Host (marix-host.exe)' | Out-Null

    # Path-prefix match sweeps every lingering Tool child process under the tool
    # directory in one pass (marix_bash.exe, marix_command_prompt.exe,
    # marix_powershell.exe, every marix_tool_*.exe, and any future addition) without
    # needing to enumerate individual executable names here. This exists so an
    # orphaned Tool process can never hold a C:\MarixHost\tool\*.log file open and
    # block its deletion in step 6.
    Write-Host "Checking for lingering Tool child processes under $toolDirPrefix..."
    Stop-VmProcessesByPath -Session $session -PathPrefix $toolDirPrefix -Label 'Tool child processes under C:\MarixHost\tool\' | Out-Null
}
finally {
    Remove-PSSession -Session $session -ErrorAction SilentlyContinue
}

Write-Host 'Host and Tool process sweep in the VM completed.'
