#!/usr/bin/env bash
set -euo pipefail
root="$TEST_SRCDIR/$TEST_WORKSPACE"
export PATH="$root/fix/cli/commands/bin:$PATH"
cd "$TEST_TMPDIR"
unset BUILD_WORKSPACE_DIRECTORY
"$root/$1" init
"$root/$1" import "$root/$4"
export FIX_TRUST="$TEST_TMPDIR/.fix/trust"
wasm=$("$root/$1" create-blob "$root/$2")
thunk=$(FIX_COMPILE_PARTITIONS=2 "$root/$1" compile "$wasm")
encoded=$("$root/$1" create-strict-encode "$thunk")
fib=$("$root/$1" eval -j 4 "$encoded")
"$root/$1" is-tag "$fib"
add_wasm=$("$root/$1" create-blob "$root/$3")
add_thunk=$(FIX_COMPILE_PARTITIONS=2 "$root/$1" compile "$add_wasm")
add=$("$root/$1" eval -j 4 "$("$root/$1" create-strict-encode "$add_thunk")")
n=$("$root/$1" create-blob --u64 5)
metadata=$("$root/$1" create-blob --string "")
combination=$("$root/$1" create-tree "$metadata" "$fib" "$n" "$add")
thunk=$("$root/$1" create-application-thunk "$combination")
result=$("$root/$1" eval -j 4 "$("$root/$1" create-strict-encode "$thunk")")
test "$("$root/$1" read-blob --u64 "$result")" = 8
source=$("$root/$1" parse - bootstrap <<'FIX'
[*#("" $path $toolchain ("share" "fix" "compile.wasm")) 2u64]
FIX
)
thunk=$(FIX_COMPILE_PARTITIONS=8 "$root/$1" compile "$source")
self=$("$root/$1" eval -j 4 "$("$root/$1" create-strict-encode "$thunk")")
fixpoint=$("$root/$1" resolve fixpoint)
if [[ $self != "$fixpoint" ]]; then
    printf 'compiler fixed point mismatch: expected %s, got %s\n' "$fixpoint" "$self" >&2
    exit 1
fi
invalid=$("$root/$1" create-blob --string 'not WebAssembly')
thunk=$("$root/$1" compile "$invalid")
error=$("$root/$1" eval -j 4 "$("$root/$1" create-strict-encode "$thunk")")
"$root/$1" is-tag "$error"
mapfile -t fields < <("$root/$1" read-tree "$error")
test "$("$root/$1" read-blob --string "${fields[2]}")" = Error
