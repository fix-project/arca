#!/usr/bin/env bash
set -euo pipefail
root="$TEST_SRCDIR/$TEST_WORKSPACE"
runner="$root/$1" probe="$root/$2"
cd "$TEST_TMPDIR"
export TMPDIR="$TEST_TMPDIR"
mkdir input
printf original > input/input.txt
printf preserved > input/untouched
ln -s input.txt input/link
printf 'buffered\0input' > stdin
status=0
"$runner" --dir "$PWD/input::/work" --env GREETING=hello --stdin "$PWD/stdin" "$probe" argument > stdout 2> stderr || status=$?
test "$status" = 23
cmp stdin stdout
grep -Fx 'guest diagnostic' stderr
output=$(sed -n 's/^output directory: //p' stderr)
cmp input/input.txt "$output/work/renamed.txt"
cmp input/untouched "$output/work/untouched"
test "$(readlink "$output/work/link")" = input.txt
test ! -e "$output/work/input.txt"
printf 'changed\0bytes' > expected
cmp expected "$output/work/result"
test "$(cat input/input.txt)" = original
