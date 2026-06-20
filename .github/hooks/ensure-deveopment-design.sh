#!/usr/bin/env bash
set -euo pipefail

to_repo_path() {
  local path repo
  path="$(printf '%s' "$1" | tr '\\' '/')"
  repo="$(pwd | tr '\\' '/')"
  path="${path#"$repo/"}"
  printf '%s' "$path"
}

is_non_dot_source_path() {
  local path
  path="$(to_repo_path "$1")"
  [[ "$path" == src/* ]] || return 1
  IFS='/' read -r -a parts <<< "$path"
  local part
  for part in "${parts[@]:1}"; do
    [[ "$part" != .* ]] || return 1
  done
  return 0
}

json_escape() {
  sed 's/\\/\\\\/g; s/"/\\"/g; s/\r//g'
}

read_hook_input() {
  cat || true
}

get_transcript_path() {
  local input="$1"
  printf '%s' "$input" |
    sed -n 's/.*"transcriptPath"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p; s/.*"transcript_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' |
    head -n 1
}

mapfile -t changed_paths < <(
  {
    git diff --name-only --diff-filter=ACMRTD 2>/dev/null || true
    git diff --cached --name-only --diff-filter=ACMRTD 2>/dev/null || true
    git ls-files --others --exclude-standard 2>/dev/null || true
  } | sed 's#\\#/#g' | awk 'NF' | sort -u
)

declare -A changed_set=()
for path in "${changed_paths[@]}"; do
  changed_set["$(to_repo_path "$path")"]=1
done

hook_input="$(read_hook_input)"
transcript_path="$(get_transcript_path "$hook_input")"
written_paths=()
if [[ -n "$transcript_path" && -f "$transcript_path" ]]; then
  while IFS= read -r path; do
    repo_path="$(to_repo_path "$path")"
    is_non_dot_source_path "$repo_path" || continue
    written_paths+=("$repo_path")
  done < <(
    sed 's/\\r\\n/\n/g; s/\\n/\n/g; s#\\\\#/#g' "$transcript_path" |
      sed -n 's/.*\*\*\* \(Add\|Update\|Delete\) File: \([^"\r\n]*\).*/\2/p; s/.*\*\*\* Move to: \([^"\r\n]*\).*/\1/p'
  )
fi

missing=()
for path in "${written_paths[@]}"; do
  [[ -n "${changed_set[$path]+x}" ]] || continue
  dir="${path%/*}"
  [[ "$dir" == "$path" ]] && dir="src"
  while [[ "$dir" == src* ]]; do
    design_path="$dir/.design.json"
    if [[ -z "${changed_set[$design_path]+x}" ]]; then
      missing+=("$path -> $design_path")
    fi
    [[ "$dir" == "src" ]] && break
    dir="${dir%/*}"
  done
done

if (( ${#missing[@]} > 0 )); then
  joined="$(printf '%s; ' "${missing[@]:0:20}")"
  reason="This agent changed non-dot src files that require updated .design.json in the file folder and every ancestor up to src. Invoke development-designer before finishing. Missing updates: ${joined%; }"
  escaped="$(printf '%s' "$reason" | json_escape)"
  printf '{"decision":"block","reason":"%s"}\n' "$escaped"
else
  printf '{"decision":"allow"}\n'
fi
