#!/usr/bin/env bash
set -euo pipefail
root="$TEST_SRCDIR/$TEST_WORKSPACE"
fix="$root/$1"
cd "$TEST_TMPDIR"
"$fix" init
"$fix" import "$root/$4"
export FIX_TRUST="$TEST_TMPDIR/.fix/trust"
fib=$("$fix" -j 4 compile "@file(\"$root/$2\")" 2)
"$fix" is-tag "$fib"
add=$("$fix" -j 4 compile "@file(\"$root/$3\")" 2)
result=$("$fix" -j 4 "*#(\"\" @handle(\"$fib\") 5u64 @handle(\"$add\"))")
test "$("$fix" read-blob --u64 "$result")" = 8
error=$("$fix" -j 4 compile '"not WebAssembly"')
"$fix" is-tag "$error"
mapfile -t fields < <("$fix" read-tree "$error")
test "$("$fix" read-blob --string "${fields[2]}")" = Error
