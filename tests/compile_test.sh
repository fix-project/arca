#!/usr/bin/env bash
set -euo pipefail
root="$TEST_SRCDIR/$TEST_WORKSPACE"
export PATH="$root/fix/cli/commands/bin:$PATH"
cd "$TEST_TMPDIR"
unset BUILD_WORKSPACE_DIRECTORY
"$root/$1" init
"$root/$1" import "$root/$4"
wasm=$("$root/$1" create-blob "$root/$2")
thunk=$(FIX_COMPILE_PARTITIONS=2 "$root/$1" compile "$wasm")
encoded=$("$root/$1" create-strict-encode "$thunk")
fib=$("$root/$1" eval -j 4 "$encoded")
add=$("$root/$1" create-blob "$root/$3")
n=$("$root/$1" create-blob --u64 5)
combination=$("$root/$1" create-tree "$fib" "$n" "$add")
thunk=$("$root/$1" create-application-thunk "$combination")
result=$("$root/$1" eval -j 4 "$("$root/$1" create-strict-encode "$thunk")")
test "$("$root/$1" read-blob --u64 "$result")" = 8
