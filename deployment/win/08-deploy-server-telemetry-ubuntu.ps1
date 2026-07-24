param(
    [Parameter(Mandatory)][string] $RepoRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (Get-Variable PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

. (Join-Path $PSScriptRoot '_deploy-shared.ps1')

$localPackageRoot = Join-Path $RepoRoot '.temp\package\server_telemetry'
$remoteDestRoot = '/opt/marix/server-telemetry'

Write-Host "Building the local Server Telemetry package manifest ($localPackageRoot)..."
$localEntries = @(Get-LocalPackageManifestEntries -PackageRoot $localPackageRoot)
$relPaths = @($localEntries | ForEach-Object { $_.RelPath })
Write-Host "  $($localEntries.Count) file(s) in the local package."

Write-Host 'Resolving SSH credentials and opening an SSH context to the Ubuntu server...'
$sshContext = New-DeploymentSshContext -RepoRoot $RepoRoot
try {
    Write-Host "Building the currently-deployed remote manifest ($remoteDestRoot)..."
    $remoteEntries = @(Get-SshManifestEntries -Context $sshContext -RemoteDestRoot $remoteDestRoot -RelPaths $relPaths)

    $comparison = Test-PackageManifestsMatch -LocalEntries $localEntries -RemoteEntries $remoteEntries
    if ($comparison.Matches) {
        Write-Host 'Server Telemetry package manifest matches the Ubuntu deployment exactly; skipping deployment.'
    }
    else {
        Write-Host "Server Telemetry package differs from the Ubuntu deployment ($($comparison.DifferingRelPaths.Count) file(s) changed/new):"
        foreach ($rel in $comparison.DifferingRelPaths) {
            Write-Host "  changed: $rel"
        }

        # Per-file atomic replace, never a whole-directory swap: the Telemetry redb
        # store lives at <this same directory>/log/*.redb (see
        # src\common\logging\store.rs), so a directory-level rename-swap would strand
        # or lose that live data on every redeploy. Per-file replace never touches
        # sibling files/directories it doesn't explicitly know about.
        $localByPath = @{}
        foreach ($e in $localEntries) { $localByPath[$e.RelPath] = $e }
        $executableRelPath = 'marix-server-telemetry'

        foreach ($rel in $comparison.DifferingRelPaths) {
            $entry = $localByPath[$rel]
            $localFullPath = Join-Path $localPackageRoot ($rel -replace '/', '\')
            $remoteDestPath = "$remoteDestRoot/$rel"
            Write-Host "  Deploying: $rel"
            Sync-FileToRemoteAtomic -Context $sshContext -LocalPath $localFullPath -RemoteDestPath $remoteDestPath -ExpectedHash $entry.Hash -MakeExecutable:($rel -eq $executableRelPath)
        }

        Write-Host 'Server Telemetry package deployment to Ubuntu completed.'
    }
}
finally {
    Remove-DeploymentSshContext -Context $sshContext
}
