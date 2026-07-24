param(
    [Parameter(Mandatory)][string] $RepoRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (Get-Variable PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

. (Join-Path $PSScriptRoot '_deploy-shared.ps1')

$localPackageRoot = Join-Path $RepoRoot '.temp\package\server'
$remoteDestRoot = '/opt/marix/server'

Write-Host "Building the local Server package manifest ($localPackageRoot)..."
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
        Write-Host 'Server package manifest matches the Ubuntu deployment exactly; skipping deployment.'
    }
    else {
        Write-Host "Server package differs from the Ubuntu deployment ($($comparison.DifferingRelPaths.Count) file(s) changed/new):"
        foreach ($rel in $comparison.DifferingRelPaths) {
            Write-Host "  changed: $rel"
        }

        # Per-file atomic replace, never a whole-directory swap, for the same
        # reasons as the Server Telemetry deploy step (consistency, and to avoid
        # clobbering anything unexpected under /opt/marix/server/ outside the known
        # package file set). Includes prompt/*.prompt files via the same relpath
        # join used for every other file.
        $localByPath = @{}
        foreach ($e in $localEntries) { $localByPath[$e.RelPath] = $e }
        $executableRelPath = 'marix-server'

        foreach ($rel in $comparison.DifferingRelPaths) {
            $entry = $localByPath[$rel]
            $localFullPath = Join-Path $localPackageRoot ($rel -replace '/', '\')
            $remoteDestPath = "$remoteDestRoot/$rel"
            Write-Host "  Deploying: $rel"
            Sync-FileToRemoteAtomic -Context $sshContext -LocalPath $localFullPath -RemoteDestPath $remoteDestPath -ExpectedHash $entry.Hash -MakeExecutable:($rel -eq $executableRelPath)
        }

        Write-Host 'Server package deployment to Ubuntu completed.'
    }
}
finally {
    Remove-DeploymentSshContext -Context $sshContext
}
