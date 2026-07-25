$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# This script never trusts the current working directory. Its directory is
# '<repoRoot>\deployment\win'; derive the deployment and repository roots from
# that fixed on-disk location.
$deploymentRoot = Split-Path -Parent $PSScriptRoot
$repoRoot = (Resolve-Path (Join-Path $deploymentRoot '..')).Path

# Windows step scripts are siblings of this entry point.
$winStepsRoot = $PSScriptRoot
$step1ElevationCheck = Join-Path $winStepsRoot '01-ensure-elevated.ps1'
$step2BuildAndPackage = Join-Path $winStepsRoot '02-build-and-package.ps1'
$step3ResolveConfig = Join-Path $winStepsRoot '03-resolve-config.ps1'
$step4EnsureHost = Join-Path $winStepsRoot '04-ensure-host.ps1'
$step5StopHost = Join-Path $winStepsRoot '05-stop-host.ps1'
$step6DeployHost = Join-Path $winStepsRoot '06-deploy-host.ps1'
$step7StopTelemetry = Join-Path $winStepsRoot '07-stop-telemetry.ps1'
$step8DeployTelemetry = Join-Path $winStepsRoot '08-deploy-telemetry.ps1'
$step9StopServer = Join-Path $winStepsRoot '09-stop-server.ps1'
$step10DeployServer = Join-Path $winStepsRoot '10-deploy-server.ps1'
$step11StartTelemetry = Join-Path $winStepsRoot '11-start-telemetry.ps1'
$step12StartServer = Join-Path $winStepsRoot '12-start-server.ps1'
$step13StartHost = Join-Path $winStepsRoot '13-start-host.ps1'
$step14DeployClient = Join-Path $winStepsRoot '14-deploy-client.ps1'

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
Write-Host '=== Step 4: prepare Host ==='
Invoke-DeploymentStep -StepLabel 'step 4 (prepare Host)' -ScriptPath $step4EnsureHost -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 5: stop Host ==='
Invoke-DeploymentStep -StepLabel 'step 5 (stop Host)' -ScriptPath $step5StopHost -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 6: deploy Host ==='
Invoke-DeploymentStep -StepLabel 'step 6 (deploy Host)' -ScriptPath $step6DeployHost -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 7: stop Telemetry ==='
Invoke-DeploymentStep -StepLabel 'step 7 (stop Telemetry)' -ScriptPath $step7StopTelemetry -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 8: deploy Telemetry ==='
Invoke-DeploymentStep -StepLabel 'step 8 (deploy Telemetry)' -ScriptPath $step8DeployTelemetry -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 9: stop Server ==='
Invoke-DeploymentStep -StepLabel 'step 9 (stop Server)' -ScriptPath $step9StopServer -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 10: deploy Server ==='
Invoke-DeploymentStep -StepLabel 'step 10 (deploy Server)' -ScriptPath $step10DeployServer -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 11: start Telemetry ==='
Invoke-DeploymentStep -StepLabel 'step 11 (start Telemetry)' -ScriptPath $step11StartTelemetry -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 12: start Server ==='
Invoke-DeploymentStep -StepLabel 'step 12 (start Server)' -ScriptPath $step12StartServer -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 13: start Host ==='
Invoke-DeploymentStep -StepLabel 'step 13 (start Host)' -ScriptPath $step13StartHost -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host '=== Step 14: deploy Client ==='
Invoke-DeploymentStep -StepLabel 'step 14 (deploy Client)' -ScriptPath $step14DeployClient -RepoRoot $repoRoot | Out-Null

Write-Host ''
Write-Host 'Deployment script tree completed successfully.'
exit 0
