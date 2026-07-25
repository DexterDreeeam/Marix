$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (Get-Variable PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

$deploymentRoot = Split-Path -Parent $PSScriptRoot
$repoRoot = (Resolve-Path (Join-Path $deploymentRoot '..')).Path

. (Join-Path $PSScriptRoot '_deploy-shared.ps1')

$remoteExecutablePath = '/opt/marix/server-telemetry/marix-server-telemetry'
$remoteDatabasePath = '/opt/marix/server-telemetry/log/telemetry.redb'

Write-Host 'Resolving SSH credentials and opening an SSH context for Telemetry...'
$sshContext = New-DeploymentSshContext -RepoRoot $repoRoot
try {
    Write-Host "Stopping Telemetry ($remoteExecutablePath)..."
    $status = Stop-RemoteProcessByPath `
        -Context $sshContext `
        -ExecutablePath $remoteExecutablePath
    Write-Host "  Telemetry -> $status"

    Write-Host "Deleting the Telemetry database ($remoteDatabasePath)..."
    $remoteCommand = @"
rm -f -- '$remoteDatabasePath'
if [ -e '$remoteDatabasePath' ]; then
  echo DATABASE_STILL_EXISTS
  exit 1
fi
echo DATABASE_REMOVED
"@
    $result = Invoke-DeploymentSsh `
        -Context $sshContext `
        -RemoteCommand $remoteCommand
    if ($result.ExitCode -ne 0) {
        throw "Failed to delete the Telemetry database (exit code $($result.ExitCode)): $($result.StdOutLines -join ' ') $($result.StdErr)"
    }
    Write-Host "  $($result.StdOutLines -join ' ')"
}
finally {
    Remove-DeploymentSshContext -Context $sshContext
}

Write-Host 'Telemetry database cleanup completed. Telemetry remains stopped.'
