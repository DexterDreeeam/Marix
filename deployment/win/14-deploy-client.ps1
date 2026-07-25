param(
    [Parameter(Mandatory)][string] $RepoRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (Get-Variable PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

. (Join-Path $PSScriptRoot '_deploy-shared.ps1')

$clientPackageRoot = Join-Path $RepoRoot '.temp\package\client\Cli'
$clientDestinationRoot = 'C:\MarixClient\Cli'
$clientExecutablePath = Join-Path $clientDestinationRoot 'marix-client-cli.exe'
$requiredClientFiles = @('marix-client-cli.exe', 'config.toml')

Write-Host "Building the local Client CLI package manifest ($clientPackageRoot)..."
if (Test-Path -LiteralPath (Join-Path $clientPackageRoot 'tool') -PathType Container) {
    throw "Client CLI package must not contain a tool directory: $clientPackageRoot\tool"
}
if (Test-Path -LiteralPath (Join-Path $clientPackageRoot 'App') -PathType Container) {
    throw "Client CLI package must not contain an App directory: $clientPackageRoot\App"
}
$clientPackageEntries = @(Get-LocalPackageManifestEntries -PackageRoot $clientPackageRoot)
$clientPackagePaths = @($clientPackageEntries | ForEach-Object { $_.RelPath })
$clientPackagePathSet = [Collections.Generic.HashSet[string]]::new(
    [StringComparer]::OrdinalIgnoreCase
)
foreach ($rel in $clientPackagePaths) {
    if ($rel -ieq 'tool' -or $rel.StartsWith('tool/', [StringComparison]::OrdinalIgnoreCase) -or
        $rel -ieq 'App' -or $rel.StartsWith('App/', [StringComparison]::OrdinalIgnoreCase)) {
        throw "Client CLI package must not contain App or tool content: $rel"
    }
    $clientPackagePathSet.Add($rel) | Out-Null
}
foreach ($requiredFile in $requiredClientFiles) {
    if (-not $clientPackagePathSet.Contains($requiredFile)) {
        throw "Client CLI package is missing required file: $requiredFile"
    }
}

Write-Host "Building the current local Client CLI manifest ($clientDestinationRoot)..."
$deployedClientEntries = @(
    Get-LocalManifestEntriesForPaths `
        -DestinationRoot $clientDestinationRoot `
        -RelPaths $clientPackagePaths
)
$clientComparison = Test-PackageManifestsMatch `
    -LocalEntries $clientPackageEntries `
    -RemoteEntries $deployedClientEntries

if ($clientComparison.Matches) {
    Write-Host 'Client CLI package manifest matches the local deployment; skipping deployment.'
}
else {
    $runningClientProcesses = @(Get-LocalProcessesByExactPath -ExactPath $clientExecutablePath)
    if ($runningClientProcesses.Count -gt 0) {
        $runningIds = ($runningClientProcesses | ForEach-Object { $_.Id }) -join ', '
        throw "Client CLI is running from the exact deployment path '$clientExecutablePath' (PID(s): $runningIds). Stop it manually before deployment."
    }

    New-Item -ItemType Directory -Path $clientDestinationRoot -Force | Out-Null
    $clientEntriesByPath = @{}
    foreach ($entry in $clientPackageEntries) {
        $clientEntriesByPath[$entry.RelPath] = $entry
    }

    $replacedClientFiles = New-Object System.Collections.Generic.List[object]
    try {
        foreach ($rel in $clientComparison.DifferingRelPaths) {
            $sourcePath = Join-Path $clientPackageRoot ($rel -replace '/', '\')
            $destinationPath = Join-Path $clientDestinationRoot ($rel -replace '/', '\')
            Write-Host "  Deploying Client CLI file: $rel"
            $replacement = Sync-FileToLocalAtomic `
                -LocalPath $sourcePath `
                -DestPath $destinationPath `
                -ExpectedHash $clientEntriesByPath[$rel].Hash
            $replacedClientFiles.Add($replacement) | Out-Null
        }

        $verifiedClientEntries = @(
            Get-LocalManifestEntriesForPaths `
                -DestinationRoot $clientDestinationRoot `
                -RelPaths $clientPackagePaths
        )
        $verification = Test-PackageManifestsMatch `
            -LocalEntries $clientPackageEntries `
            -RemoteEntries $verifiedClientEntries
        if (-not $verification.Matches) {
            throw "Client CLI manifest verification failed for: $($verification.DifferingRelPaths -join ', ')"
        }
    }
    catch {
        $deploymentFailure = $_
        for ($index = $replacedClientFiles.Count - 1; $index -ge 0; $index--) {
            $replacement = $replacedClientFiles[$index]
            try {
                Restore-LocalFileFromOld `
                    -DestPath $replacement.DestPath `
                    -HadOriginal ([bool]$replacement.HadOriginal)
            }
            catch {
                Write-Warning "Best-effort Client CLI rollback failed for '$($replacement.DestPath)': $($_.Exception.Message)"
            }
        }
        throw $deploymentFailure
    }

    Write-Host 'Client CLI deployment and complete manifest verification succeeded.'
}

Write-Host 'Client CLI is ready. Run:'
Write-Host "  & 'C:\MarixClient\Cli\marix-client-cli.exe' --oneshot '<your task>'"
