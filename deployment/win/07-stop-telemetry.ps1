param(
    [Parameter(Mandatory)][string] $RepoRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (Get-Variable PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

. (Join-Path $PSScriptRoot '_deploy-shared.ps1')

$remoteExecutablePath = '/opt/marix/server-telemetry/marix-server-telemetry'

Write-Host 'Resolving SSH credentials and opening an SSH context for Telemetry...'
$sshContext = New-DeploymentSshContext -RepoRoot $RepoRoot
try {
    Write-Host "Checking for a running Telemetry process ($remoteExecutablePath)..."
    $status = Stop-RemoteProcessByPath -Context $sshContext -ExecutablePath $remoteExecutablePath
    Write-Host "  Telemetry -> $status"
}
finally {
    Remove-DeploymentSshContext -Context $sshContext
}

Write-Host 'Telemetry process check completed.'
