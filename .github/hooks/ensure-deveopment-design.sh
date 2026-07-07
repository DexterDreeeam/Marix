#!/usr/bin/env bash
set -euo pipefail

# Normalizes any path to a repo-relative, forward-slash path.
to_repo_path() {
  local path repo
  path="$(printf '%s' "$1" | tr '\\' '/')"
  repo="$(pwd | tr '\\' '/')"
  path="${path#"$repo/"}"
  path="${path#./}"
  printf '%s' "$path"
}

# A design-tracked source file lives under src/, is not src/tests, and has no
# dot-prefixed path segment.
is_non_dot_source_path() {
  local path
  path="$(to_repo_path "$1")"
  [[ "$path" == src/* ]] || return 1
  [[ "$path" != src/tests && "$path" != src/tests/* ]] || return 1
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

# Drain stdin so the hook host is never blocked writing to us.
cat >/dev/null 2>&1 || true

# The current turn's change manifest is the lexicographically largest file in
# .temp/changed. Turn names are YYYYMMDD_HHMMSS timestamps, so lexical order is
# chronological order.
manifest_dir=".temp/changed"
current_manifest=""
if [[ -d "$manifest_dir" ]]; then
  current_manifest="$(ls -1 "$manifest_dir"/*.txt 2>/dev/null | LC_ALL=C sort | tail -n 1 || true)"
fi

# No manifest recorded for this turn means there is nothing to verify.
if [[ -z "$current_manifest" || ! -f "$current_manifest" ]]; then
  printf '{"decision":"allow"}\n'
  exit 0
fi

declare -A changed_set=()
while IFS= read -r line || [[ -n "$line" ]]; do
  line="$(to_repo_path "$line")"
  [[ -n "$line" ]] && changed_set["$line"]=1
done < "$current_manifest"

missing=()
for path in "${!changed_set[@]}"; do
  is_non_dot_source_path "$path" || continue
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
  reason="This turn changed non-dot src files that require updated .design.json in the file folder and every ancestor up to src, listed in the same turn change manifest. Invoke development-designer, then add the updated .design.json paths to the manifest. Missing updates: ${joined%; }"
  escaped="$(printf '%s' "$reason" | json_escape)"
  printf '{"decision":"block","reason":"%s"}\n' "$escaped"
else
  printf '{"decision":"allow"}\n'
fi
