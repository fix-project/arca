#!/usr/bin/env bash
set -euo pipefail
root="$TEST_SRCDIR/$TEST_WORKSPACE"
export PATH="$root/fix/cli/commands/bin:$PATH"
merge="$root/$1" runner="$root/$2" library="$root/$3" app="$root/$4"
cd "$TEST_TMPDIR"
unset BUILD_WORKSPACE_DIRECTORY
"$runner" init
"$runner" import "$root/$5"
trap 'status=$?; if ((status)); then for log in *.stderr; do [[ ! -f $log ]] || tail -80 "$log" >&2; done; fi; exit "$status"' EXIT
mkdir input
cp "$library" input/library.wasm
cp "$app" input/app.wasm
if "$merge" input/app.wasm app --no-resource-imports -o unresolved.wasm 2> unresolved.stderr; then exit 1; fi
grep -F 'unresolved resource import lib::functions' unresolved.stderr
"$merge" input/library.wasm lib input/app.wasm app --no-resource-imports --rename-export-conflicts -o expected.wasm
directory=$("$runner" flatware-import input /work)
fds=$("$runner" create-tree "$directory")
FIX_JOBS=2 FIX_FDS="$fds" FIX_RESULT_LABEL=merged "$runner" flatware-run wasm-merge /work/library.wasm lib /work/app.wasm app --rename-export-conflicts -o /work/merged.wasm < /dev/null > merge.stdout 2> merge.stderr
mapfile -t output < <("$runner" read-tree merged)
mapfile -t descriptors < <("$runner" read-tree "${output[0]}")
"$runner" flatware-export "${descriptors[3]}" result
cmp expected.wasm result/merged.wasm
wasm=$("$runner" create-blob expected.wasm)
linked=$("$runner" flatware-link "$wasm")
thunk=$(FIX_COMPILE_PARTITIONS=2 "$runner" compile "$linked")
encoded=$("$runner" create-strict-encode "$thunk")
executable=$("$runner" eval -j 2 "$encoded")
"$runner" flatware-run "$executable" < /dev/null > execution.stdout 2> execution.stderr
printf OK > expected.stdout
cmp expected.stdout execution.stdout
