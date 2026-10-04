#!/usr/bin/env bash
set -euo pipefail
root="$TEST_SRCDIR/$TEST_WORKSPACE"
"$root/$1" > "$TEST_TMPDIR/blob" 2>&1
grep -F 'result is a Blob: [104, 101, 108, 108, 111, 44, 32, 119, 111, 114, 108, 100]' "$TEST_TMPDIR/blob"
"$root/$2" 2u64 3u64 > "$TEST_TMPDIR/add" 2>&1
grep -F 'as a u64: 5' "$TEST_TMPDIR/add"
