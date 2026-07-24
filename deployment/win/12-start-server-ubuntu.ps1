param(
    [Parameter(Mandatory)][string] $RepoRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (Get-Variable PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

. (Join-Path $PSScriptRoot '_deploy-shared.ps1')

$remoteDeployDir = '/opt/marix/server'
$remoteExecutablePath = '/opt/marix/server/marix-server'
$localConfigPath = Join-Path $RepoRoot '.temp\package\server\config.toml'

# Started unconditionally, regardless of whether step 10 actually redeployed anything -
# step 9 already guarantees nothing is running under this path beforehand.
$hostPort = Get-ConfigTomlValue -Path $localConfigPath -Key 'host_port'

Write-Host 'Resolving SSH credentials and opening an SSH context to the Ubuntu server...'
$sshContext = New-DeploymentSshContext -RepoRoot $RepoRoot
try {
    Write-Host "Starting Server as a detached background process ($remoteExecutablePath)..."
    Start-RemoteProcessDetached -Context $sshContext -RemoteDir $remoteDeployDir -ExecutablePath $remoteExecutablePath -OutputLogName 'server.out'
    Write-Host '  Process started.'

    # Server listens on both client_port and host_port; probing host_port specifically
    # validates the exact channel Host will use next in step 13. Same wildcard-bind /
    # loopback-probe reasoning as step 11 (see src\common\structure\channel.rs), and
    # likewise no systemctl is-active fallback - Server is no longer managed by
    # systemd, so the bounded TCP probe is the sole readiness gate.
    Write-Host "Waiting for the Server TCP listener (host_port) on 127.0.0.1:$hostPort to become ready..."
    Wait-RemoteTcpReady -Context $sshContext -ProbeHost '127.0.0.1' -Port $hostPort -LivenessExecutablePath $remoteExecutablePath
    Write-Host '  Server TCP listener (host_port) is ready.'
}
finally {
    Remove-DeploymentSshContext -Context $sshContext
}

Write-Host 'Server start-up and readiness confirmation on Ubuntu completed.'
