<#
.SYNOPSIS
    Build script for Marix GitHub Pages file explorer.
    Scans the repository, computes diffs between marix tags,
    and generates overview/manifest.json for the static site.

.USAGE
    powershell -ExecutionPolicy Bypass -File scripts/build_pages.ps1
#>

param(
    [string]$RepoRoot = ""
)

$ErrorActionPreference = "Stop"

if (-not $RepoRoot) {
    $RepoRoot = (git rev-parse --show-toplevel 2>$null).Replace("/", "\")
    if (-not $RepoRoot) {
        Write-Error "Not in a git repository."
        exit 1
    }
}

$OverviewDir = Join-Path $RepoRoot "overview"

# ── Excluded paths ──
$ExcludeDirs = @(".git", "__pycache__", "node_modules", ".venv", "venv", ".mypy_cache", ".pytest_cache", "target")
$ImageExts = @(".png", ".jpg", ".jpeg", ".gif", ".svg", ".webp", ".ico", ".bmp")
$BinaryExts = $ImageExts + @(".woff", ".woff2", ".ttf", ".eot", ".zip", ".tar", ".gz", ".exe", ".dll", ".so", ".dylib", ".pdb", ".obj")
$MaxFileSize = 100 * 1024  # 100KB

function Test-GeneratedPath {
    param([string]$Path)

    return $Path -eq "overview/manifest.json" `
        -or $Path -eq "docs/manifest.json" `
        -or $Path -like "overview/content/*" `
        -or $Path -like "docs/content/*"
}

# ── Scan Files ──
function Scan-Files {
    $files = @{}
    $allFiles = Get-ChildItem -Path $RepoRoot -Recurse -File -ErrorAction SilentlyContinue

    foreach ($f in $allFiles) {
        $relPath = $f.FullName.Substring($RepoRoot.Length + 1).Replace("\", "/")

        # Skip excluded directories
        $skip = $false
        foreach ($ed in $ExcludeDirs) {
            if ($relPath -like "$ed/*" -or $relPath -like "*/$ed/*") { $skip = $true; break }
        }
        if ($skip) { continue }

        if (Test-GeneratedPath -Path $relPath) { continue }

        $ext = [System.IO.Path]::GetExtension($f.Name).ToLower()
        $entry = @{ size = $f.Length }

        # Large files
        if ($f.Length -gt $MaxFileSize) {
            $entry["content"] = "[File too large: $($f.Length) bytes]"
            $files[$relPath] = $entry
            continue
        }

        # Images: base64
        if ($ImageExts -contains $ext) {
            try {
                $bytes = [System.IO.File]::ReadAllBytes($f.FullName)
                $entry["base64"] = [Convert]::ToBase64String($bytes)
                $mimeMap = @{
                    ".png" = "image/png"; ".jpg" = "image/jpeg"; ".jpeg" = "image/jpeg"
                    ".gif" = "image/gif"; ".svg" = "image/svg+xml"; ".webp" = "image/webp"
                    ".ico" = "image/x-icon"; ".bmp" = "image/bmp"
                }
                $entry["mime"] = if ($mimeMap[$ext]) { $mimeMap[$ext] } else { "image/png" }
            } catch {}
            $files[$relPath] = $entry
            continue
        }

        # Other binary
        if ($BinaryExts -contains $ext) {
            $entry["content"] = "[Binary file: $ext]"
            $files[$relPath] = $entry
            continue
        }

        # Text files
        try {
            $entry["content"] = [System.IO.File]::ReadAllText($f.FullName, [System.Text.Encoding]::UTF8)
        } catch {
            $entry["content"] = "[Unable to read file]"
        }
        $files[$relPath] = $entry
    }
    return $files
}

# ── Get Marix Tags ──
function Get-MarixTags {
    $output = git tag --list "marix_tag_*" --sort=creatordate 2>$null
    if (-not $output) { return @() }
    return @($output -split "`n" | ForEach-Object { $_.Trim() } | Where-Object { $_ })
}

# ── Get Diff ──
function Get-TagDiff {
    param([string]$FromRef, [string]$ToRef)

    $output = git diff --name-status "$FromRef..$ToRef" 2>$null
    $changes = @{}
    if (-not $output) { return $changes }

    foreach ($line in ($output -split "`n")) {
        $line = $line.Trim()
        if (-not $line) { continue }
        $parts = $line -split "`t"
        if ($parts.Count -ge 2) {
            $status = $parts[0][0].ToString()
            $filepath = $parts[$parts.Count - 1].Replace("\", "/")
            if (Test-GeneratedPath -Path $filepath) { continue }
            $changes[$filepath] = @{ status = $status }
        }
    }
    return $changes
}

function Get-FileDiffLines {
    param([string]$FromRef, [string]$ToRef, [string]$FilePath)

    $output = git diff -U3 "$FromRef..$ToRef" -- $FilePath 2>$null
    if (-not $output) {
        return [PSCustomObject]@{
            diff_lines = @()
            hunks = @()
        }
    }

    $lines = $output -split "`n"
    $diffLines = @()
    $hunks = @()

    foreach ($line in $lines) {
        if ($line -match "^diff --git" -or $line -match "^index " -or $line -match "^---" -or $line -match "^\+\+\+") { continue }
        if ($line -match "^@@") {
            $hunks += @{ header = $line; reason = "" }
        }
        $diffLines += $line
    }
    return [PSCustomObject]@{
        diff_lines = @($diffLines)
        hunks = @($hunks)
    }
}

# ── Main ──
Write-Host "Repository root: $RepoRoot"
Write-Host "Scanning repository files..."
$files = Scan-Files
Write-Host "  Found $($files.Count) files"

$tags = @(Get-MarixTags)
Write-Host "  Found $($tags.Count) marix tags: $($tags -join ', ')"

$diffInfo = @{
    prev_tag = $null
    latest_tag = $null
    changes = @{}
}

if ($tags.Count -ge 2) {
    $prevTag = $tags[$tags.Count - 2]
    $latestTag = $tags[$tags.Count - 1]
    $diffInfo.prev_tag = $prevTag
    $diffInfo.latest_tag = $latestTag

    Write-Host "  Computing diff: $prevTag -> $latestTag"
    $changes = Get-TagDiff -FromRef $prevTag -ToRef $latestTag

    foreach ($fp in @($changes.Keys)) {
        $diff = Get-FileDiffLines -FromRef $prevTag -ToRef $latestTag -FilePath $fp
        $changes[$fp]["diff_lines"] = @($diff.diff_lines)
        $changes[$fp]["hunks"] = @($diff.hunks)
    }

    $diffInfo.changes = $changes
    Write-Host "  $($changes.Count) files changed"

} elseif ($tags.Count -eq 1) {
    $tag = $tags[0]
    $diffInfo.latest_tag = $tag
    Write-Host "  Single tag: $tag, computing diff to HEAD"
    $changes = Get-TagDiff -FromRef $tag -ToRef "HEAD"

    foreach ($fp in @($changes.Keys)) {
        $diff = Get-FileDiffLines -FromRef $tag -ToRef "HEAD" -FilePath $fp
        $changes[$fp]["diff_lines"] = @($diff.diff_lines)
        $changes[$fp]["hunks"] = @($diff.hunks)
    }

    $diffInfo.changes = $changes
    Write-Host "  $($changes.Count) files changed since $tag"

} else {
    Write-Host "  No marix tags found, no diff available"
}

$manifest = @{
    generated_at = (Get-Date -Format "o")
    files = $files
    diff = $diffInfo
}

# Write manifest
$manifestPath = Join-Path $OverviewDir "manifest.json"
$json = $manifest | ConvertTo-Json -Depth 10 -Compress:$false
[System.IO.File]::WriteAllText($manifestPath, $json, [System.Text.Encoding]::UTF8)

Write-Host ""
Write-Host "Manifest written to $manifestPath"
Write-Host "  Total files: $($files.Count)"
Write-Host "  Changed files: $($diffInfo.changes.Count)"
Write-Host "Done."
