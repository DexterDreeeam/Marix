---
name: code-evaluate
description: Evaluate code organization and file size. Use only when explicitly asked to run code-evaluate or evaluate code structure.
---

## Purpose

Evaluate code organization, file size, and lightweight style health. This skill is explicit opt-in: it should help identify refactor opportunities and Rust/style concerns, but only the 500-line file-size rule is a required refactor trigger.

## Workflow

1. **Scan Code Files** — Count lines for repository source files, excluding generated/build/vendor artifacts and dot-prefixed companion paths under `src/`.
2. **Find Oversized Files** — Identify any evaluated code file above **500 lines**.
3. **Refactor Required Violations** — If a code file exceeds 500 lines, split it into smaller focused modules or extract shared functions. Keep each resulting code file at or below 500 lines.
4. **Run Lightweight Style Review** — For Rust and other source files, collect advisory style findings. Do not automatically refactor for advisory findings unless the user explicitly asks.
5. **Re-check Required Rules** — Re-run the line-count scan after any required refactoring and confirm all evaluated code files are at or below 500 lines.
6. **Report** — Summarize required violations, any modules created, final line-count status, and advisory style findings.

## Rules

- This skill runs only when the user explicitly asks for `code-evaluate`.
- Do not apply the 500-line rule automatically during unrelated tasks.
- Every code file evaluated by this skill should be at or below **500 lines** after refactoring.
- Prefer focused modules with clear responsibility over arbitrary file splitting.
- Keep generated files out of the line-count decision unless the user explicitly asks to evaluate generated output.
- Ignore every dot-prefixed file or folder under `src/`. Those paths are companion metadata maintained by `development-designer`, not normal source files.
- Treat the Rust/style checklist as advisory. Report findings as suggestions or notes; do not hard-fail or auto-fix them by default.
- Do not install new tools for this skill. Use existing repository tooling when available.

## Rust Advisory Checklist

Use these checks as lightweight guidance when evaluating Rust code:

- **Formatting and linting**
  - Prefer `cargo fmt` / rustfmt formatting when a Cargo project is present.
  - Prefer `cargo clippy --all-targets --all-features` when a Cargo project is present.
  - If the repository does not support those commands, report that they were not run rather than adding tooling.
- **Naming**
  - Types and traits should use `CamelCase`.
  - Functions, methods, variables, modules, and fields should use `snake_case`.
  - Constants and statics should use `SCREAMING_SNAKE_CASE`.
  - Prefer clear words over unclear abbreviations.
- **Public API documentation**
  - Public, library-like items should have rustdoc comments.
  - Document public errors, panics, edge cases, and examples when they matter to callers.
  - Any public `unsafe` item must document its safety contract.
- **Error handling**
  - Prefer `Result<T, E>` for recoverable failures.
  - Avoid using `panic!` for normal control flow or recoverable errors.
  - Prefer meaningful error types and actionable error messages.
- **Production hygiene**
  - Flag non-test uses of `unwrap()`, `expect()`, `panic!`, `todo!`, `unimplemented!`, `dbg!`, `println!`, and `eprintln!` unless locally justified.
  - Prefer logging or structured diagnostics over raw print macros in production code.
- **Readability and API shape**
  - Flag very long functions, deeply nested logic, and repeated blocks as refactor candidates.
  - Prefer cohesive modules and helper functions over large mixed-responsibility files.
  - Prefer enums or typed options over unclear boolean mode parameters.
  - Keep visibility private by default; use `pub` only for intentional API surfaces.

## Report Severity

- **Required** — Files over 500 lines. These should be refactored when running this skill.
- **Suggested** — Rust/style checklist findings that are likely worth addressing but should not be auto-fixed by default.
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

## Suggested Rust Advisory Scan

```powershell
Get-ChildItem -Recurse -File -Filter *.rs |
  ForEach-Object {
    $relative = $_.FullName.Substring((Get-Location).Path.Length + 1)
    if (Test-IgnoredCodeEvaluatePath $relative) { return }
    Select-String -Path $_.FullName -Pattern '\.(unwrap|expect)\s*\(|\b(panic|todo|unimplemented|dbg|println|eprintln)!\s*\(' |
      ForEach-Object {
        [pscustomobject]@{
          Path = $relative
          Line = $_.LineNumber
          Match = $_.Matches.Value
          Severity = 'Suggested'
        }
      }
  } |
  Format-Table -AutoSize
```
