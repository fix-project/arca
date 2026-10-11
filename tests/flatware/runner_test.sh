#!/usr/bin/env bash
set -euo pipefail
root="$TEST_SRCDIR/$TEST_WORKSPACE"
export PATH="$root/fix/cli/commands/bin:$PATH"
runner="$root/$1" probe="$root/$2"
cd "$TEST_TMPDIR"
unset BUILD_WORKSPACE_DIRECTORY
"$runner" init
"$runner" create-blob "$root/$3" -l json
"$runner" create-blob "$root/$4" -l cpio
"$runner" create-blob "$root/$5" -l flatware-normalize
"$runner" create-blob "$root/$6" -l compose
"$runner" create-blob "$probe" -l probe
greeting=$("$runner" create-blob --string GREETING=hello)
environment=$("$runner" create-tree "$greeting")
mkdir input
printf original > input/input.txt
printf preserved > input/untouched
ln -s input.txt input/link
printf 'buffered\0input' > stdin
for attempt in 1 2; do
    status=0
    directory=$("$runner" flatware-import input /work)
    fds=$("$runner" create-tree "$directory")
    FIX_FDS="$fds" FIX_ENV="$environment" FIX_RESULT_LABEL="run-$attempt" "$runner" flatware-run probe argument < stdin > stdout 2> stderr || status=$?
    mapfile -t output < <("$runner" read-tree "run-$attempt")
    mapfile -t descriptors < <("$runner" read-tree "${output[0]}")
    mkdir result
    "$runner" flatware-export "${descriptors[3]}" result/work
    test "$status" = 23
    cmp stdin stdout
    grep -Fx 'guest diagnostic' stderr
    cmp input/input.txt result/work/renamed.txt
    cmp input/untouched result/work/untouched
    test "$(readlink result/work/link)" = input.txt
    test ! -e result/work/input.txt
    printf 'changed\0bytes' > expected
    cmp expected result/work/result
    test "$(cat input/input.txt)" = original
    mv result "result-$attempt"
done
cmp result-1/work/settings result-2/work/settings
