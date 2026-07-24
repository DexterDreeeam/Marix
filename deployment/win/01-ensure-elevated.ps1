param(
    [Parameter(Mandatory)][string] $RepoRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# Never rely on the current working directory: every path here is built from the
# $RepoRoot value passed explicitly by run.ps1, never re-derived from $PSScriptRoot.

$currentIdentity = [Security.Principal.WindowsIdentity]::GetCurrent()
$currentPrincipal = [Security.Principal.WindowsPrincipal]::new($currentIdentity)
$isElevated = $currentPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

# --- Hard one-shot relaunch guard ---------------------------------------------------
# This script must NEVER relaunch itself more than once, no matter what causes a
# misdetection. A marker FILE (created just before Start-Process, below) is the
# authoritative signal: it is plain filesystem state, so it is visible to any process
# regardless of which OS component ends up creating it. An environment variable is
# also set as a best-effort secondary signal, but it is deliberately NOT relied upon
# alone: a genuine non-elevated -> elevated UAC transition is brokered by the
# Application Information service via a COM elevation moniker -- a fundamentally
# different process-creation path than a normal child process -- and that broker is
# not guaranteed to carry over environment variables mutated in the calling process's
# live memory. Empirically confirmed on this machine: relaunching via '-Verb RunAs'
# from a process that is ALREADY elevated does propagate an env var set beforehand
# (no broker is involved at that point -- Windows just creates a normal child process
# since no privilege change is actually needed), but that is not the scenario that
# matters here, and deliberately forcing a genuine non-elevated -> elevated UAC
# transition to test the opposite case is unsafe to do unattended (a secure-desktop
# consent prompt with nobody able to click it could hang indefinitely). Hence the file
# marker -- unaffected by any of this ambiguity -- is what the guard actually trusts.
$elevationMarkerPath = [IO.Path]::GetFullPath((Join-Path $RepoRoot '.temp\deployment-elevation-attempted.marker'))
$elevationAlreadyAttempted = ($env:MARIX_DEPLOYMENT_ELEVATION_ATTEMPTED -eq '1') -or
    (Test-Path -LiteralPath $elevationMarkerPath -PathType Leaf)

if (-not $isElevated -and $elevationAlreadyAttempted) {
    throw "Elevation was already attempted once and this process is still not running elevated; refusing to relaunch again to avoid an infinite relaunch loop. Run 'deployment\run.ps1' (repo root '$RepoRoot') yourself from an already-elevated PowerShell window instead. This is most likely caused by a known issue: MSIX-packaged (Microsoft Store) builds of PowerShell 7 -- executables whose path contains '\WindowsApps\' -- can fail to actually acquire an elevated token when relaunched via 'Start-Process -Verb RunAs', or can bypass the UAC consent prompt while still landing back in a non-elevated context. If your PowerShell host is an MSIX package, start classic Windows PowerShell ('powershell.exe' under System32\WindowsPowerShell\v1.0) or a non-Store pwsh.exe as Administrator and re-run this script from there."
}

if ($isElevated) {
    # Clear any marker left behind by a past failed attempt so it never wrongly blocks
    # a genuinely fresh future run: this process is proof that elevation now works.
    if (Test-Path -LiteralPath $elevationMarkerPath -PathType Leaf) {
        Remove-Item -LiteralPath $elevationMarkerPath -Force -ErrorAction SilentlyContinue
    }
    Write-Output 'elevated'
    return
}

Write-Host 'Current process is not running elevated; relaunching an elevated PowerShell window...'

$runPs1Path = [IO.Path]::GetFullPath((Join-Path $RepoRoot 'deployment\run.ps1'))
if (-not (Test-Path -LiteralPath $runPs1Path -PathType Leaf)) {
    throw "Main entry point script was not found: $runPs1Path"
}

# Relaunch with whichever PowerShell host is currently running this process (Windows
# PowerShell 5.1's powershell.exe, or PowerShell 7+'s pwsh.exe) so PowerShell 7 users
# stay on pwsh. Reading the running process's own executable path is exact and avoids
# any PATH-resolution ambiguity between the two hosts.
$currentHostPath = (Get-Process -Id $PID).Path
if ([string]::IsNullOrWhiteSpace($currentHostPath)) {
    throw 'Could not determine the current PowerShell host executable path to relaunch elevated.'
}

# MSIX-packaged (Microsoft Store) executables are always installed under a path
# containing '\WindowsApps\', and have documented, unreliable behavior with
# 'Start-Process -Verb RunAs' self-elevation (app-container / execution-alias
# activation quirks can cause the "elevated" relaunch to not actually acquire an
# elevated token). Never use such a host as the relaunch target -- substitute the
# classic, non-MSIX Windows PowerShell instead. A host whose path does not contain
# '\WindowsApps\' is left untouched, preserving the original "stay on whichever host
# is currently running" behavior for the non-MSIX case.
if ($currentHostPath -imatch '\\WindowsApps\\') {
    $classicHostPath = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
    if (-not (Test-Path -LiteralPath $classicHostPath -PathType Leaf)) {
        throw "Current PowerShell host is an MSIX-packaged executable ('$currentHostPath'), which is known to be unreliable for self-elevation, but the classic fallback host was not found: $classicHostPath"
    }
    Write-Host "Current PowerShell host is MSIX-packaged ('$currentHostPath'); relaunching with classic Windows PowerShell instead: $classicHostPath"
    $currentHostPath = $classicHostPath
}

# The elevated window must stay open (-NoExit) so the user can review results and
# close it manually; -ExecutionPolicy Bypass is required to run this unsigned local
# script elevated. -File must stay last since run.ps1 takes no further arguments.
$relaunchArguments = @('-NoExit', '-ExecutionPolicy', 'Bypass', '-File', $runPs1Path)

# Record that a relaunch is being attempted, before actually attempting it, using both
# the marker file (authoritative) and the environment variable (best-effort). See the
# guard comment above for why the file -- not the env var -- is what is actually
# trusted to block a second attempt.
New-Item -ItemType Directory -Path (Split-Path -Parent $elevationMarkerPath) -Force | Out-Null
New-Item -ItemType File -Path $elevationMarkerPath -Force | Out-Null
$env:MARIX_DEPLOYMENT_ELEVATION_ATTEMPTED = '1'

# Fire-and-forget: do NOT use -Wait. The elevated window stays open indefinitely
# (-NoExit), so waiting on it here would hang this (non-elevated) process forever.
# If the user declines the UAC prompt, or Start-Process fails for any other reason,
# that is a real failure -- let the exception propagate rather than swallowing it.
Start-Process -FilePath $currentHostPath -ArgumentList $relaunchArguments -Verb RunAs -WorkingDirectory $RepoRoot

Write-Output 'relaunched'
