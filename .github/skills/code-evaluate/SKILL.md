---
name: code-evaluate
description: Evaluate code organization and file size. Use only when explicitly asked to run code-evaluate or evaluate code structure.
---

## Purpose

Evaluate code organization, file size, and lightweight style health. This skill is explicit opt-in: it should help identify refactor opportunities and style concerns, but only the 500-line file-size rule is a required refactor trigger.

## Workflow

1. **Scan Code Files** — Count lines for repository source files, excluding generated/build/vendor artifacts and dot-prefixed companion paths under `src/`.
2. **Find Oversized Files** — Identify any evaluated code file above **500 lines**.
3. **Refactor Required Violations** — If a code file exceeds 500 lines, split it into smaller focused modules or extract shared functions. Keep each resulting code file at or below 500 lines.
4. **Run Lightweight Style Review** — For source files covered by `.github/coding_style/`, collect advisory style findings from the matching language document. Do not automatically refactor for advisory findings unless the user explicitly asks.
5. **Re-check Required Rules** — Re-run the line-count scan after any required refactoring and confirm all evaluated code files are at or below 500 lines.
6. **Report** — Summarize required violations, any modules created, final line-count status, and advisory style findings.

## Rules

- This skill runs only when the user explicitly asks for `code-evaluate`.
- Do not apply the 500-line rule automatically during unrelated tasks.
- Every code file evaluated by this skill should be at or below **500 lines** after refactoring.
- Prefer focused modules with clear responsibility over arbitrary file splitting.
- Keep generated files out of the line-count decision unless the user explicitly asks to evaluate generated output.
- Ignore every dot-prefixed file or folder under `src/`. Those paths are companion metadata maintained by `development-designer`, not normal source files.
- Treat `.github/coding_style/*.md` guidance as advisory unless that document marks a rule as required. Report advisory findings as suggestions or notes; do not hard-fail or auto-fix them by default.
- Do not install new tools for this skill. Use existing repository tooling when available.

## Coding Style Sources

- Rust: `.github/coding_style/rust.md`
- JavaScript: `.github/coding_style/js.md`
- If a language has no matching style document, evaluate only file size and obvious organization issues.

## Report Severity

- **Required** — Files over 500 lines. These should be refactored when running this skill.
- **Suggested** — Language style findings that are likely worth addressing but should not be auto-fixed by default.
- **Note** — Context-dependent observations, tool availability, or style tradeoffs.

## Suggested PowerShell Scan

```powershell
function Test-IgnoredCodeEvaluatePath {
  param([string]$RelativePath)

  $parts = $RelativePath -split '[\\/]'
  if ($parts | Where-Object { $_ -in @('.git', '__pycache__', 'node_modules', '.venv', 'venv', '.mypy_cache', '.pytest_cache', 'target') }) {
    return $true
  }
  if ($RelativePath -match '^(overview|docs)[\\/]content[\\/]') {
    return $true
  }
  if ($RelativePath -match '(^|[\\/])manifest\.json$') {
    return $true
  }
  if ($parts.Count -gt 1 -and $parts[0] -eq 'src' -and ($parts | Where-Object { $_.StartsWith('.') })) {
    return $true
  }
  return $false
}

Get-ChildItem -Recurse -File |
  ForEach-Object {
    $relative = $_.FullName.Substring((Get-Location).Path.Length + 1)
    if (Test-IgnoredCodeEvaluatePath $relative) { return }
    if ($relative -notmatch '\.(rs|js|ts|tsx|jsx|ps1|html|css)$') { return }
    $count = (Get-Content $_.FullName).Count
    if ($count -gt 500) {
      [pscustomobject]@{
        Lines = $count
        Path = $relative
      }
    }
  } |
  Sort-Object Lines -Descending |
  Format-Table -AutoSize
```
