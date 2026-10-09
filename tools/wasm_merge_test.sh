#!/usr/bin/env bash
set -euo pipefail
root="$TEST_SRCDIR/$TEST_WORKSPACE"
merge="$root/$1" guest_merge="$root/$2" runner="$root/$3" library="$root/$4" app="$root/$5"
cd "$TEST_TMPDIR"
trap 'status=$?; if ((status)); then for log in *.stderr; do [[ ! -f $log ]] || tail -80 "$log" >&2; done; fi; exit "$status"' EXIT
mkdir input
cp "$library" input/library.wasm
cp "$app" input/app.wasm
"$merge" input/library.wasm lib input/app.wasm app --rename-export-conflicts -o expected.wasm
"$runner" --jobs 2 --dir "$PWD/input::/work" "$guest_merge" /work/library.wasm lib /work/app.wasm app --rename-export-conflicts -o /work/merged.wasm > merge.stdout 2> merge.stderr
cmp expected.wasm result/work/merged.wasm
rm -rf result
"$runner" expected.wasm > execution.stdout 2> execution.stderr
printf OK > expected.stdout
cmp expected.stdout execution.stdout
