#!/usr/bin/env bash
set -euo pipefail

# Drain stdin so the hook host is never blocked writing to us.
cat >/dev/null 2>&1 || true

changed_dir=".temp/changed"

if [[ -d "$changed_dir" ]] && find "$changed_dir" -maxdepth 1 -type f -print -quit | grep -q .; then
  printf '{"decision":"block","reason":"Call design-json-update with parameter \\"changed\\" to process .temp\\\\changed."}\n'
else
  printf '{"decision":"allow"}\n'
fi
