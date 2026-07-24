$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# This script never trusts the current working directory. The repository root is
# derived purely from this file's own on-disk location: run.ps1 lives at
# '<repoRoot>\deployment\run.ps1', so one level up from $PSScriptRoot is the root.
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

$winStepsRoot = Join-Path $repoRoot 'deployment\win'
$step1ElevationCheck = Join-Path $winStepsRoot '01-ensure-elevated.ps1'
$step2BuildAndPackage = Join-Path $winStepsRoot '02-build-and-package.ps1'
$step3ResolveConfig = Join-Path $winStepsRoot '03-resolve-config.ps1'
$step4EnsureHyperVVm = Join-Path $winStepsRoot '04-ensure-hyperv-vm.ps1'
$step5StopHostVm = Join-Path $winStepsRoot '05-stop-host-vm.ps1'
$step6DeployHostVm = Join-Path $winStepsRoot '06-deploy-host-vm.ps1'
$step7StopServerTelemetryUbuntu = Join-Path $winStepsRoot '07-stop-server-telemetry-ubuntu.ps1'
$step8DeployServerTelemetryUbuntu = Join-Path $winStepsRoot '08-deploy-server-telemetry-ubuntu.ps1'
$step9StopServerUbuntu = Join-Path $winStepsRoot '09-stop-server-ubuntu.ps1'
$step10DeployServerUbuntu = Join-Path $winStepsRoot '10-deploy-server-ubuntu.ps1'
$step11StartServerTelemetryUbuntu = Join-Path $winStepsRoot '11-start-server-telemetry-ubuntu.ps1'
$step12StartServerUbuntu = Join-Path $winStepsRoot '12-start-server-ubuntu.ps1'
$step13StartHostVm = Join-Path $winStepsRoot '13-start-host-vm.ps1'

function Invoke-DeploymentStep {
    param(
        [Parameter(Mandatory)][string] $StepLabel,
        [Parameter(Mandatory)][string] $ScriptPath,
        [Parameter(Mandatory)][string] $RepoRoot
    )

    if (-not (Test-Path -LiteralPath $ScriptPath -PathType Leaf)) {
        Write-Host "Deployment failed before $StepLabel : step script was not found: $ScriptPath" -ForegroundColor Red
        exit 1
    }

    try {
        return & $ScriptPath -RepoRoot $RepoRoot
    }
    catch {
        Write-Host "Deployment failed at $StepLabel :" -ForegroundColor Red
        Write-Host $_.Exception.Message -ForegroundColor Red
        exit 1
    }
}

Write-Host '=== Step 1: elevation check ==='
$elevationOutput = Invoke-DeploymentStep -StepLabel 'step 1 (elevation check)' -ScriptPath $step1ElevationCheck -RepoRoot $repoRoot
$elevationSignal = $elevationOutput | Select-Object -Last 1

switch ($elevationSignal) {
    'relaunched' {
        Write-Host ''
        Write-Host 'Not running elevated; launched an elevated PowerShell window to continue. Review that window for results.'
        exit 0
    }
    'elevated' {
        Write-Host 'Already running elevated; continuing in this process.'
    }
    default {
        Write-Host "Deployment failed at step 1 (elevation check): unexpected result '$elevationSignal'." -ForegroundColor Red
        exit 1
    }
}

Write-Host ''
Write-Host '=== Step 2: build and package ==='
Invoke-DeploymentStep -StepLabel 'step 2 (build and package)' -ScriptPath $step2BuildAndPackage -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 3: resolve and copy config.toml ==='
Invoke-DeploymentStep -StepLabel 'step 3 (resolve config)' -ScriptPath $step3ResolveConfig -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 4: Hyper-V VM readiness ==='
Invoke-DeploymentStep -StepLabel 'step 4 (Hyper-V VM readiness)' -ScriptPath $step4EnsureHyperVVm -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 5: stop Host and Tool processes in the VM ==='
Invoke-DeploymentStep -StepLabel 'step 5 (stop Host and Tool processes in the VM)' -ScriptPath $step5StopHostVm -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 6: deploy Host package to the VM ==='
Invoke-DeploymentStep -StepLabel 'step 6 (deploy Host package to the VM)' -ScriptPath $step6DeployHostVm -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 7: stop Server Telemetry on Ubuntu ==='
Invoke-DeploymentStep -StepLabel 'step 7 (stop Server Telemetry on Ubuntu)' -ScriptPath $step7StopServerTelemetryUbuntu -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 8: deploy Server Telemetry package to Ubuntu ==='
Invoke-DeploymentStep -StepLabel 'step 8 (deploy Server Telemetry package to Ubuntu)' -ScriptPath $step8DeployServerTelemetryUbuntu -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 9: stop Server on Ubuntu ==='
Invoke-DeploymentStep -StepLabel 'step 9 (stop Server on Ubuntu)' -ScriptPath $step9StopServerUbuntu -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 10: deploy Server package to Ubuntu ==='
Invoke-DeploymentStep -StepLabel 'step 10 (deploy Server package to Ubuntu)' -ScriptPath $step10DeployServerUbuntu -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 11: start Server Telemetry on Ubuntu ==='
Invoke-DeploymentStep -StepLabel 'step 11 (start Server Telemetry on Ubuntu)' -ScriptPath $step11StartServerTelemetryUbuntu -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 12: start Server on Ubuntu ==='
Invoke-DeploymentStep -StepLabel 'step 12 (start Server on Ubuntu)' -ScriptPath $step12StartServerUbuntu -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 13: start Host in the VM ==='
Invoke-DeploymentStep -StepLabel 'step 13 (start Host in the VM)' -ScriptPath $step13StartHostVm -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host 'Deployment script tree completed successfully.'
exit 0
