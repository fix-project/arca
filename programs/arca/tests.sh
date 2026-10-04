#!/usr/bin/env bash
set -euo pipefail
root="$TEST_SRCDIR/$TEST_WORKSPACE"
"$root/$1" tuple:word:2,word:3 > "$TEST_TMPDIR/addition" 2>&1
grep -F 'result: 5' "$TEST_TMPDIR/addition"
"$root/$2" blob:hello > "$TEST_TMPDIR/identity" 2>&1
grep -F 'result: "hello"' "$TEST_TMPDIR/identity"
"$root/$3" > "$TEST_TMPDIR/null" 2>&1
grep -F 'result: null' "$TEST_TMPDIR/null"
printf 'copy\0these\nbytes' > "$TEST_TMPDIR/input"
"$root/$4" "blob:$TEST_TMPDIR/input" "blob:$TEST_TMPDIR/output" > "$TEST_TMPDIR/io" 2>&1
cmp "$TEST_TMPDIR/input" "$TEST_TMPDIR/output"
