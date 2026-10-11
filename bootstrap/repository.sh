#!/usr/bin/env bash
set -euo pipefail
fix=$(realpath "$1") stage0=$(realpath "$2")
clang=$(realpath "$3") wasm2c=$(realpath "$4") merger=$(realpath "$5")
json=$(realpath "$6") parser=$(realpath "$7") identity=$(realpath "$8")
shell=$(realpath "$9") runtime=$(realpath "${10}") memmap=$(realpath "${11}")
adapter=$(realpath "${12}") control=$(realpath "${13}")
path=$(realpath "${14}")
cpio=$(realpath "${15}") normalize=$(realpath "${16}") compose=$(realpath "${17}")
archive=$(realpath -m "${18}")
shift 18
work=$(mktemp -d)
trap 'rm -rf -- "$work"' EXIT
mkdir -p "$work/toolchain/share/fix"
cp -R -L "$stage0/usr" "$work/toolchain/usr"
chmod -R u+w "$work/toolchain"
cp "$shell" "$work/toolchain/share/fix/libfixshell.a"
cp "$runtime" "$work/toolchain/share/fix/wasm-rt.c"
cp "$memmap" "$work/toolchain/share/fix/memmap.ld"
cp "$adapter" "$work/toolchain/share/fix/adapter.wasm"
cp "$control" "$work/toolchain/share/fix/control.wasm"
for header in "$@"; do cp "$header" "$work/toolchain/share/fix/"; done
cd "$work"
unset BUILD_WORKSPACE_DIRECTORY
"$fix" init
names=(clang wasm2c wasm-merge json parser identity path cpio flatware-normalize compose)
mapfile -t values < <("$fix" create-blob "$clang" "$wasm2c" "$merger" "$json" "$parser" "$identity" "$path" "$cpio" "$normalize" "$compose")
wait "$!"
for i in "${!names[@]}"; do
    "$fix" label set "${names[i]}" "${values[i]}"
    printf '%s\0%s\0' "${names[i]}" "${values[i]}" >> bindings
done
toolchain=$("$fix" flatware-import toolchain /toolchain)
"$fix" label set toolchain "$toolchain"
printf 'toolchain\0%s\0' "$toolchain" >> bindings
"$fix" create-tree --bindings - -l bootstrap < bindings > /dev/null
"$fix" export bootstrap "$archive" clang wasm2c wasm-merge json parser identity path cpio flatware-normalize compose toolchain
