#!/bin/bash
# Build the freestanding libraries with the same target specification as userspace.
set -euo pipefail
rustc=$1
sources=$(dirname "$(dirname "$(dirname "$2")")")
target=$3
out=$4
host_std=$(dirname "$5")
mkdir -p "$out"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
flags=(--edition=2024 --crate-type=rlib --target "$target" -Zunstable-options
       -Copt-level=3 -Cforce-frame-pointers=on -Cpanic=abort --cap-lints=allow)
"$rustc" "${flags[@]}" --crate-name core "$sources/core/src/lib.rs" -o "$out/libcore.rlib"

# Use compiler-builtins' own configuration rather than duplicating its cfgs.
builtins="$sources/compiler-builtins/compiler-builtins"
"$rustc" --edition=2024 -L "$host_std" "$builtins/build.rs" -o "$work/configure"
export TARGET=x86_64-unknown-arca CARGO_MANIFEST_DIR="$builtins" OUT_DIR="$work" OPT_LEVEL=3
export CARGO_CFG_TARGET_ARCH=x86_64 CARGO_CFG_TARGET_ENV='' CARGO_CFG_TARGET_OS=arca CARGO_CFG_TARGET_VENDOR=unknown
export CARGO_FEATURE_ARCH=1 CARGO_FEATURE_COMPILER_BUILTINS=1 CARGO_FEATURE_UNMANGLED_NAMES=1 CARGO_FEATURE_MEM=1
"$rustc" --target "$target" -Zunstable-options --print cfg > "$work/target.cfg"
target_features=()
while IFS= read -r cfg; do
    case "$cfg" in
        target_feature=*)
            feature=${cfg#target_feature=}
            target_features+=("${feature//\"/}")
            ;;
        target_has_reliable_*) export "CARGO_CFG_${cfg^^}=1" ;;
    esac
done < "$work/target.cfg"
export CARGO_CFG_TARGET_FEATURE
CARGO_CFG_TARGET_FEATURE=$(IFS=,; echo "${target_features[*]}")
builtin_flags=(--cfg 'feature="arch"' --cfg 'feature="compiler-builtins"'
               --cfg 'feature="unmangled-names"' --cfg 'feature="mem"')
"$work/configure" > "$work/builtins.cfg"
while IFS= read -r line; do
    case "$line" in
        cargo:rustc-cfg=*) builtin_flags+=(--cfg "${line#cargo:rustc-cfg=}") ;;
        cargo::rustc-cfg=*) builtin_flags+=(--cfg "${line#cargo::rustc-cfg=}") ;;
    esac
done < "$work/builtins.cfg"
"$rustc" "${flags[@]}" "${builtin_flags[@]}" --crate-name compiler_builtins \
    --extern "core=$out/libcore.rlib" "$builtins/src/lib.rs" -o "$out/libcompiler_builtins.rlib"
"$rustc" "${flags[@]}" --crate-name alloc --extern "core=$out/libcore.rlib" \
    --extern "compiler_builtins=$out/libcompiler_builtins.rlib" -L "dependency=$out" \
    "$sources/alloc/src/lib.rs" -o "$out/liballoc.rlib"
