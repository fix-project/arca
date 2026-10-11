#!/usr/bin/env bash
set -euo pipefail
root="$TEST_SRCDIR/$TEST_WORKSPACE"
export PATH="$root/fix/cli/commands/bin:$PATH"
cd "$TEST_TMPDIR"
unset BUILD_WORKSPACE_DIRECTORY
"$root/$1" init
"$root/$1" import "$root/$3"
wasm=$("$root/$1" create-blob "$root/$2")
linked=$("$root/$1" flatware-link "$wasm")
FIX_COMPILE_PARTITIONS=2 "$root/$1" compile "$linked" > thunk
thunk=$(cat thunk)
test "$("$root/$1" get-type "$thunk")" = thunk
encoded=$("$root/$1" create-strict-encode "$thunk")
for mode in serial parallel; do
    if [[ $mode == serial ]]; then jobs=1; else jobs=4; fi
    executable=$("$root/$1" eval -j "$jobs" "$encoded")
    "$root/$1" read-blob "$executable" > "$mode.elf"
    FIX_JOBS="$jobs" "$root/$1" flatware-run "$executable" < /dev/null > "$mode.stdout" 2> "$mode.stderr"
done
cmp serial.elf parallel.elf
cmp serial.stdout parallel.stdout
