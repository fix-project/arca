#!/usr/bin/env bash
set -euo pipefail
printf '\006\000\000\000\000\000\000\000' > "$TEST_TMPDIR/expected"
for file in $1; do
    cmp "$TEST_TMPDIR/expected" "$TEST_SRCDIR/$TEST_WORKSPACE/$file"
done
