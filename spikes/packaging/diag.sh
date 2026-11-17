#!/bin/sh
# Trial only (Task 14): print each file's size in bytes, type and SHA-256. Arguments are files or folders (searched
# recursively). Exits 1 if an argument does not exist or holds no file, so a missing output is never silent.
set -eu
status=0
for target in "$@"; do
  if [ ! -e "$target" ]; then
    echo "diag: missing: $target" >&2
    status=1
    continue
  fi
  find "$target" -type f | sort | while IFS= read -r f; do
    size=$(wc -c < "$f" | tr -d ' ')
    if command -v sha256sum >/dev/null 2>&1; then
      sum=$(sha256sum "$f" | cut -d' ' -f1)
    else
      sum=$(shasum -a 256 "$f" | cut -d' ' -f1)
    fi
    kind=$( (file -b "$f" 2>/dev/null || echo "file(1) not available") | cut -c1-110)
    printf '%12s  %s  %s  [%s]\n' "$size" "$sum" "$f" "$kind"
  done
  if [ -z "$(find "$target" -type f | head -n 1)" ]; then
    echo "diag: no files under: $target" >&2
    status=1
  fi
done
exit "$status"
