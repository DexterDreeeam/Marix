$ErrorActionPreference = "SilentlyContinue"

function Write-AllowDecision {
  @{ decision = "allow" } | ConvertTo-Json -Compress
}

$repoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$soundPath = Join-Path $repoRoot ".asset\agent_stop.mp3"
if (-not (Test-Path -LiteralPath $soundPath -PathType Leaf)) {
  Write-AllowDecision
  exit 0
}

try {
  Add-Type -AssemblyName PresentationCore
  $player = New-Object System.Windows.Media.MediaPlayer
  $player.Open([Uri]$soundPath)
  Start-Sleep -Milliseconds 200
  $player.Play()
  Start-Sleep -Milliseconds 3000
  $player.Close()
} catch {
  Write-AllowDecision
  exit 0
}

Write-AllowDecision
