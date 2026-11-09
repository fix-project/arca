#!/usr/bin/env bash
# Checks that compiling every seed module inside Fix reproduces the seed's certificates.
set -euo pipefail
fix=$PWD/$1
seed=$PWD/$2
archive=$PWD/$3
result=$(cat "$4")
cd "$TEST_TMPDIR"
"$fix" init
"$fix" import "$seed"
"$fix" import "$archive"
mapfile -t rebuilt < <("$fix" read-tree "$result")
mapfile -t closure < <("$fix" read-tree compile)
mapfile -t certificate < <("$fix" read-tree "${closure[2]}")
mapfile -t resources < <("$fix" read-tree "${certificate[1]}")
check() {
    [[ $2 == "$3" ]] || { echo "$1: rebuilt $2, seed $3" >&2; exit 1; }
}
for entry in "${rebuilt[@]}"; do
    mapfile -t fields < <("$fix" read-tree "$entry")
    name=$("$fix" read-blob "${fields[0]}")
    check "$name" "${fields[1]}" "$("$fix" resolve "$name")"
done
check wasm2c-resource "${resources[0]}" "$("$fix" resolve wasm2c)"
check clang-resource "${resources[1]}" "$("$fix" resolve clang)"
