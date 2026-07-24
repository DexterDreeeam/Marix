param(
    [Parameter(Mandatory)][string] $RepoRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (Get-Variable PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

. (Join-Path $PSScriptRoot '_deploy-shared.ps1')

$remoteExecutablePath = '/opt/marix/server/marix-server'

Write-Host 'Resolving SSH credentials and opening an SSH context to the Ubuntu server...'
$sshContext = New-DeploymentSshContext -RepoRoot $RepoRoot
try {
    Write-Host "Checking for a running Server process ($remoteExecutablePath)..."
    $status = Stop-RemoteProcessByPath -Context $sshContext -ExecutablePath $remoteExecutablePath
    Write-Host "  Server -> $status"
}
finally {
    Remove-DeploymentSshContext -Context $sshContext
}

Write-Host 'Server process check on Ubuntu completed.'
