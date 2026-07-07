$ErrorActionPreference = "Continue"
if (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
  $PSNativeCommandUseErrorActionPreference = $false
}

function Convert-ToRepoPath {
  param([string]$Path)
  $normalized = ($Path -replace "\\", "/")
  $repoRoot = ((Get-Location).Path -replace "\\", "/").TrimEnd("/")
  if ($normalized.StartsWith("$repoRoot/")) {
    $normalized = $normalized.Substring($repoRoot.Length + 1)
  }
  return ($normalized -replace "^\./", "")
}

# A design-tracked source file lives under src/, is not src/tests, and has no
# dot-prefixed path segment.
function Test-NonDotSourcePath {
  param([string]$Path)
  $parts = (Convert-ToRepoPath $Path) -split "/"
  if ($parts.Count -lt 2 -or $parts[0] -ne "src") { return $false }
  if ($parts[1] -eq "tests") { return $false }
  foreach ($part in $parts[1..($parts.Count - 1)]) {
    if ($part.StartsWith(".")) { return $false }
  }
  return $true
}

function Get-DesignAncestors {
  param([string]$Path)
  $parts = (Convert-ToRepoPath $Path) -split "/"
  $dirParts = if ($parts.Count -gt 1) { $parts[0..($parts.Count - 2)] } else { @("src") }
  $ancestors = @()
  for ($i = $dirParts.Count; $i -ge 1; $i--) {
    $dir = ($dirParts[0..($i - 1)] -join "/")
    if ($dir -eq "src" -or $dir.StartsWith("src/")) {
      $ancestors += "$dir/.design.json"
    }
  }
  return $ancestors | Select-Object -Unique
}

# Drain stdin so the hook host is never blocked writing to us.
try { [Console]::In.ReadToEnd() | Out-Null } catch { }

# The current turn's change manifest is the lexicographically largest file in
# .temp/changed. Turn names are YYYYMMDD_HHMMSS timestamps, so lexical order is
# chronological order.
$manifestDir = ".temp/changed"
$current = $null
if (Test-Path $manifestDir) {
  $current = Get-ChildItem -Path $manifestDir -Filter *.txt -File -ErrorAction SilentlyContinue |
    Sort-Object Name | Select-Object -Last 1
}

# No manifest recorded for this turn means there is nothing to verify.
if (-not $current) {
  @{ decision = "allow" } | ConvertTo-Json -Compress
  exit 0
}

$changedSet = @{}
foreach ($line in (Get-Content $current.FullName -ErrorAction SilentlyContinue)) {
  $repoPath = Convert-ToRepoPath ([string]$line).Trim()
  if ($repoPath) { $changedSet[$repoPath] = $true }
}

$missing = @()
foreach ($path in @($changedSet.Keys)) {
  if (-not (Test-NonDotSourcePath $path)) { continue }
  foreach ($designPath in Get-DesignAncestors $path) {
    if (-not $changedSet.ContainsKey($designPath)) {
      $missing += "$path -> $designPath"
    }
  }
}

if ($missing.Count -gt 0) {
  $reason = "This turn changed non-dot src files that require updated .design.json in the file folder and every ancestor up to src, listed in the same turn change manifest. Invoke development-designer, then add the updated .design.json paths to the manifest. Missing updates: " + (($missing | Select-Object -First 20) -join "; ")
  @{ decision = "block"; reason = $reason } | ConvertTo-Json -Compress
} else {
  @{ decision = "allow" } | ConvertTo-Json -Compress
}
