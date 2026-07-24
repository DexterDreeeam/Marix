param(
    [Parameter(Mandatory)][string] $RepoRoot
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (Get-Variable PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $false
}

. (Join-Path $PSScriptRoot '_deploy-shared.ps1')

# Fixed VM/guest identity, matching 04-ensure-hyperv-vm.ps1's own constants exactly.
$vmName = 'Marix_TestVm'
$guestUserName = 'marixagent'
$guestPassword = '123'
$securePassword = ConvertTo-SecureString -String $guestPassword -AsPlainText -Force
$guestCredential = [pscredential]::new($guestUserName, $securePassword)

$localPackageRoot = Join-Path $RepoRoot '.temp\package\host'
$vmDestRoot = 'C:\MarixHost'
$vmToolDir = 'C:\MarixHost\tool'

Write-Host "Building the local Host package manifest ($localPackageRoot)..."
$localEntries = @(Get-LocalPackageManifestEntries -PackageRoot $localPackageRoot)
$relPaths = @($localEntries | ForEach-Object { $_.RelPath })
Write-Host "  $($localEntries.Count) file(s) in the local package."

Write-Host "Connecting to VM '$vmName' via PowerShell Direct..."
$session = Connect-DeploymentVmSession -VmName $vmName -Credential $guestCredential
try {
    Write-Host "Building the currently-deployed VM manifest ($vmDestRoot)..."
    $vmEntries = @(Get-VmManifestEntries -Session $session -VmDestRoot $vmDestRoot -RelPaths $relPaths)

    $comparison = Test-PackageManifestsMatch -LocalEntries $localEntries -RemoteEntries $vmEntries
    if ($comparison.Matches) {
        Write-Host 'Host package manifest matches the VM deployment exactly; skipping deployment.'
    }
    else {
        Write-Host "Host package differs from the VM deployment ($($comparison.DifferingRelPaths.Count) file(s) changed/new):"
        foreach ($rel in $comparison.DifferingRelPaths) {
            Write-Host "  changed: $rel"
        }

        # Existing documented Marix convention, independent of this task: delete
        # stale tool log files before copying, whenever a deploy actually happens.
        # Step 5 already killed any lingering Tool process that could otherwise hold
        # one of these files open and block its deletion here.
        Write-Host "Deleting existing *.log files under $vmToolDir before deploying..."
        Invoke-Command -Session $session -ScriptBlock {
            param($ToolDir)
            Get-ChildItem -LiteralPath $ToolDir -Filter '*.log' -File -ErrorAction SilentlyContinue |
                Remove-Item -Force -ErrorAction SilentlyContinue
        } -ArgumentList $vmToolDir | Out-Null

        $localByPath = @{}
        foreach ($e in $localEntries) { $localByPath[$e.RelPath] = $e }

        foreach ($rel in $comparison.DifferingRelPaths) {
            $entry = $localByPath[$rel]
            $localFullPath = Join-Path $localPackageRoot ($rel -replace '/', '\')
            $destFullPath = Join-Path $vmDestRoot ($rel -replace '/', '\')
            Write-Host "  Deploying: $rel"
            Sync-FileToVmAtomic -Session $session -LocalPath $localFullPath -DestPath $destFullPath -ExpectedHash $entry.Hash
        }

        Write-Host 'Host package deployment to the VM completed.'
    }
}
finally {
    Remove-PSSession -Session $session -ErrorAction SilentlyContinue
}
