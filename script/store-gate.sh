#!/usr/bin/env bash
# Size gate for "storing a heap value hands it over" (C5a): type-check every .ynz file under
# examples/, tests/ and every crates/*/tests/fixtures tree with ONE compiler binary and keep
# the diagnostics, so two binaries (before / after the rule) can be compared file by file.
#
#   script/store-gate.sh <ynz-binary> <output-dir>
#
# Writes <output-dir>/<path with / as __>.ndjson per file, and <output-dir>/summary.tsv with
# `<exit-code>\t<path>` per file (exit 0 = accepted, 1 = rejected; the typeck-only
# `ynz build --json` path, so nothing is linked).
set -euo pipefail
binary=$(realpath "$1")
out=$(mkdir -p "$2" && realpath "$2")
cd "$(git rev-parse --show-toplevel)"
export binary out
{ find examples crates -name '*.ynz'; find tests -name '*.ynz' 2>/dev/null; } | grep -v '/target/' | sort > "$out/files.txt" || true
check_one() {
  file=$1
  name=${file//\//__}
  code=0
  "$binary" build --json "$file" > "$out/$name.ndjson" 2>/dev/null || code=$?
  printf '%s\t%s\n' "$code" "$file"
}
export -f check_one
xargs -a "$out/files.txt" -P 8 -I{} bash -c 'check_one {}' | sort -k2 > "$out/summary.tsv"
