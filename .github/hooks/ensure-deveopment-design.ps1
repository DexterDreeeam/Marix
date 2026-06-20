$ErrorActionPreference = "Continue"
if (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
  $PSNativeCommandUseErrorActionPreference = $false
}

function Convert-ToRepoPath {
  param([string]$Path)
  $normalized = ($Path -replace "\\", "/")
  $repoRoot = ((Get-Location).Path -replace "\\", "/").TrimEnd("/")
  if ($normalized.StartsWith("$repoRoot/")) {
    return $normalized.Substring($repoRoot.Length + 1)
  }
  return $normalized
}

function Test-NonDotSourcePath {
  param([string]$Path)
  $parts = (Convert-ToRepoPath $Path) -split "/"
  if ($parts.Count -lt 2 -or $parts[0] -ne "src") { return $false }
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

function Get-GitPaths {
  $paths = @()
  $paths += git diff --name-only --diff-filter=ACMRTD 2>$null
  $paths += git diff --cached --name-only --diff-filter=ACMRTD 2>$null
  $paths += git ls-files --others --exclude-standard 2>$null
  return $paths | Where-Object { $_ } | ForEach-Object { Convert-ToRepoPath $_ } | Select-Object -Unique
}

function Get-TranscriptPath {
  $inputJson = [Console]::In.ReadToEnd()
  if ([string]::IsNullOrWhiteSpace($inputJson)) { return "" }
  try {
    $payload = $inputJson | ConvertFrom-Json
    if ($payload.transcriptPath) { return [string]$payload.transcriptPath }
    if ($payload.transcript_path) { return [string]$payload.transcript_path }
  } catch {
    return ""
  }
  return ""
}

function Get-AgentWrittenSourcePaths {
  param([string]$TranscriptPath)
  if ([string]::IsNullOrWhiteSpace($TranscriptPath) -or -not (Test-Path $TranscriptPath)) {
    return @()
  }

  $raw = Get-Content $TranscriptPath -Raw
  $normalized = $raw -replace "\\r\\n", "`n" -replace "\\n", "`n" -replace "\\\\", "/"
  $paths = @()

  $patchMatches = [regex]::Matches($normalized, "\*\*\* (?:Add|Update|Delete) File: ([^\r\n""]+)|\*\*\* Move to: ([^\r\n""]+)")
  foreach ($match in $patchMatches) {
    $path = if ($match.Groups[1].Success) { $match.Groups[1].Value } else { $match.Groups[2].Value }
    $paths += Convert-ToRepoPath $path.Trim()
  }

  return $paths |
    Where-Object { Test-NonDotSourcePath $_ } |
    Select-Object -Unique
}

$changedPaths = @(Get-GitPaths)
$changedPathSet = @{}
foreach ($path in $changedPaths) {
  $changedPathSet[$path] = $true
}

$writtenPaths = @(Get-AgentWrittenSourcePaths (Get-TranscriptPath))
$relevantPaths = @()
foreach ($path in $writtenPaths) {
  if ($changedPathSet.ContainsKey($path)) {
    $relevantPaths += $path
  }
}

$missing = @()
foreach ($path in ($relevantPaths | Select-Object -Unique)) {
  foreach ($designPath in Get-DesignAncestors $path) {
    if (-not $changedPathSet.ContainsKey($designPath)) {
      $missing += "$path -> $designPath"
    }
  }
}

if ($missing.Count -gt 0) {
  $reason = "This agent changed non-dot src files that require updated .design.json in the file folder and every ancestor up to src. Invoke development-designer before finishing. Missing updates: " + (($missing | Select-Object -First 20) -join "; ")
  @{ decision = "block"; reason = $reason } | ConvertTo-Json -Compress
} else {
  @{ decision = "allow" } | ConvertTo-Json -Compress
}
