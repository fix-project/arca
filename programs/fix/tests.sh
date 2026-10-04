#!/usr/bin/env bash
set -euo pipefail
root="$TEST_SRCDIR/$TEST_WORKSPACE"
"$root/$1" 2u64 3u64 > "$TEST_TMPDIR/addition" 2>&1
grep -F 'as a u64: 5' "$TEST_TMPDIR/addition"
"$root/$1" 18446744073709551615u64 1u64 > "$TEST_TMPDIR/overflow" 2>&1
grep -F 'as a u64: 0' "$TEST_TMPDIR/overflow"
