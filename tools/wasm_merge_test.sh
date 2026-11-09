#!/usr/bin/env bash
set -euo pipefail
root="$TEST_SRCDIR/$TEST_WORKSPACE"
merge="$root/$1" library="$root/$2" app="$root/$3"
cd "$TEST_TMPDIR"
if "$merge" "$app" app --no-resource-imports -o unresolved.wasm 2> unresolved.stderr; then exit 1; fi
grep -F 'unresolved resource import lib::functions' unresolved.stderr
"$merge" "$library" lib "$app" app --no-resource-imports --rename-export-conflicts -o merged.wasm
test -s merged.wasm
