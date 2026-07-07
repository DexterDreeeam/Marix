$ErrorActionPreference = "Continue"
if (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
  $PSNativeCommandUseErrorActionPreference = $false
}

# Drain stdin so the hook host is never blocked writing to us.
try { [Console]::In.ReadToEnd() | Out-Null } catch { }

$changedDir = Join-Path ".temp" "changed"
$hasPendingFiles = $false

if (Test-Path -LiteralPath $changedDir -PathType Container) {
  $hasPendingFiles = [bool](Get-ChildItem -LiteralPath $changedDir -File -ErrorAction SilentlyContinue | Select-Object -First 1)
}

if ($hasPendingFiles) {
  @{ decision = "block"; reason = 'Call design-json-update with parameter "changed" to process .temp\changed.' } | ConvertTo-Json -Compress
} else {
  @{ decision = "allow" } | ConvertTo-Json -Compress
}
