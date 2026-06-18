---
name: code-evaluate
description: Evaluate code organization and file size. Use only when explicitly asked to run code-evaluate or evaluate code structure.
---

## Purpose

Evaluate whether code files are too large and should be split into focused modules.

## Workflow

1. **Scan Code Files** — Count lines for repository code files, excluding generated build artifacts.
2. **Find Oversized Files** — Identify any code file above **500 lines**.
3. **Refactor When Needed** — If a code file exceeds 500 lines, split it into smaller focused modules or extract shared functions. Keep each resulting code file at or below 500 lines.
4. **Re-check** — Re-run the line-count scan after refactoring and confirm all code files are at or below 500 lines.
5. **Report** — Summarize oversized files found, modules created, and final line-count status.

## Rules

- This skill runs only when the user explicitly asks for `code-evaluate`.
- Do not apply the 500-line rule automatically during unrelated tasks.
- Every code file evaluated by this skill should be at or below **500 lines** after refactoring.
- Prefer focused modules with clear responsibility over arbitrary file splitting.
- Keep generated files out of the line-count decision unless the user explicitly asks to evaluate generated output.

## Suggested PowerShell Scan

```powershell
Get-ChildItem -Recurse -File |
  Where-Object {
    $_.FullName -notmatch '\\.git\\' -and
    $_.FullName -notmatch '\\overview\\manifest\.json$' -and
    $_.FullName -match '\.(rs|js|ts|tsx|jsx|ps1|html|css)$'
  } |
  ForEach-Object {
    $count = (Get-Content $_.FullName).Count
    if ($count -gt 500) {
      [pscustomobject]@{
        Lines = $count
        Path = $_.FullName.Substring((Get-Location).Path.Length + 1)
      }
    }
  } |
  Sort-Object Lines -Descending |
  Format-Table -AutoSize
```
