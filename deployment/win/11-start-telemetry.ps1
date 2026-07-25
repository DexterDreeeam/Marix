param(
    [Parameter(Mandatory)][string] $RepoRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (Get-Variable PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

. (Join-Path $PSScriptRoot '_deploy-shared.ps1')

$remoteDeployDir = '/opt/marix/server-telemetry'
$remoteExecutablePath = '/opt/marix/server-telemetry/marix-server-telemetry'
$localConfigPath = Join-Path $RepoRoot '.temp\package\server_telemetry\config.toml'

# Started unconditionally, regardless of whether step 8 actually redeployed anything -
# step 7 already guarantees nothing is running under this path beforehand.
$telemetryPort = Get-ConfigTomlValue -Path $localConfigPath -Key 'telemetry_port'

Write-Host 'Resolving SSH credentials and opening an SSH context for Telemetry...'
$sshContext = New-DeploymentSshContext -RepoRoot $RepoRoot
try {
    Write-Host "Starting Telemetry as a detached background process ($remoteExecutablePath)..."
    Start-RemoteProcessDetached -Context $sshContext -RemoteDir $remoteDeployDir -ExecutablePath $remoteExecutablePath -OutputLogName 'telemetry.out'
    Write-Host '  Process started.'

    # src\common\structure\channel.rs confirms every channel endpoint (Client, Host,
    # and Telemetry alike) binds its LISTEN side to the wildcard address
    # (Ipv4Addr::UNSPECIFIED), while only the CONNECT side dials config.server.ip.
    # Since this probe runs on the remote host itself (over this same SSH
    # session), loopback correctly reaches that wildcard listener without depending
    # on the host's own public IP being self-routable. The bounded probe uses an
    # anchored pgrep -f liveness check because no process here is managed by systemd.
    Write-Host "Waiting for the Telemetry TCP listener on 127.0.0.1:$telemetryPort to become ready..."
    Wait-RemoteTcpReady -Context $sshContext -ProbeHost '127.0.0.1' -Port $telemetryPort -LivenessExecutablePath $remoteExecutablePath
    Write-Host '  Telemetry TCP listener is ready.'
}
finally {
    Remove-DeploymentSshContext -Context $sshContext
}

Write-Host 'Telemetry start-up and readiness confirmation completed.'
