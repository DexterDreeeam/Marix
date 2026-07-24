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

$hostExePath = 'C:\MarixHost\marix-host.exe'
$hostWorkDir = 'C:\MarixHost'
$hostStdOutLog = 'C:\MarixHost\host.stdout.log'
$hostStdErrLog = 'C:\MarixHost\host.stderr.log'

# Host's source was changed so that, if it cannot connect to Server, it now exits the
# whole process within 30 seconds instead of retrying forever. Waiting ~35 seconds
# (that 30-second connect timeout plus a small buffer) before checking therefore makes
# "process still running" a reliable success signal rather than an ambiguous one.
$waitSeconds = 35

Write-Host "Connecting to VM '$vmName' via PowerShell Direct..."
$session = Connect-DeploymentVmSession -VmName $vmName -Credential $guestCredential
try {
    Write-Host "Starting Host as a detached process ($hostExePath)..."
    Invoke-Command -Session $session -ScriptBlock {
        param($ExePath, $WorkDir, $StdOutLog, $StdErrLog)
        if (-not (Test-Path -LiteralPath $ExePath -PathType Leaf)) {
            throw "Host executable was not found in the VM: $ExePath"
        }
        Remove-Item -LiteralPath $StdOutLog -Force -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $StdErrLog -Force -ErrorAction SilentlyContinue
        # Start-Process launches a genuinely independent Windows process: it is not
        # tied to the lifetime of this PowerShell Direct session/runspace, so it
        # keeps running after this Invoke-Command call returns and after the
        # session is later closed by this script's own finally block.
        Start-Process -FilePath $ExePath -WorkingDirectory $WorkDir -WindowStyle Hidden `
            -RedirectStandardOutput $StdOutLog -RedirectStandardError $StdErrLog | Out-Null
    } -ArgumentList $hostExePath, $hostWorkDir, $hostStdOutLog, $hostStdErrLog | Out-Null
    Write-Host '  Start-Process issued.'

    Write-Host "Waiting $waitSeconds second(s) for Host to either exit (e.g. Server connect failure) or remain running (success)..."
    Start-Sleep -Seconds $waitSeconds

    Write-Host 'Confirming the Host process is still running...'
    $stillRunning = @(Get-VmProcessesByPath -Session $session -ExactPath $hostExePath)
    if ($stillRunning.Count -ge 1) {
        $pidList = ($stillRunning | ForEach-Object { $_.Id }) -join ', '
        Write-Host "  Host is still running after $waitSeconds second(s) (PID(s): $pidList) -> startup confirmed."
    }
    else {
        throw "Host process ($hostExePath) is not running $waitSeconds second(s) after start; it likely exited early (for example because it could not connect to Server within its own connect timeout). See '$hostStdOutLog' and '$hostStdErrLog' inside the VM for details."
    }
}
finally {
    Remove-PSSession -Session $session -ErrorAction SilentlyContinue
}

Write-Host 'Host start-up and readiness confirmation in the VM completed.'
