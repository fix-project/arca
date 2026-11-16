#!/usr/bin/env bash
set -euo pipefail

configure=$1
sdk=$(dirname "$(dirname "$2")")
output=$3
build=$(mktemp -d "${output}.build.XXXXXX")
trap 'rm -rf "$build"' EXIT

options=(
    --without-x
    --disable-shared
    --disable-all-pkgs
    --disable-web-progs
    --enable-tex
    --disable-synctex
    --disable-xetex
    --enable-missing
)

mkdir "$build/native" "$build/wasi"
cd "$build/native"
"$configure" "${options[@]}" CFLAGS="-O2 -g0" CXXFLAGS="-O2 -g0"
"$MAKE" recurse CONF_SUBDIRS=texk/kpathsea
"$MAKE" -C texk recurse CONF_SUBDIRS=web2c
"$MAKE" -j2 -C texk/web2c tangle ctangle tie otangle

native_web2c="$build/native/texk/web2c"
export PATH="$native_web2c:$PATH"
export TANGLE="$native_web2c/tangle"
export TANGLEBOOT="$TANGLE"
export CTANGLE="$native_web2c/ctangle"
export CTANGLEBOOT="$CTANGLE"
export TIE="$native_web2c/tie"
export OTANGLE="$native_web2c/otangle"

cd "$build/wasi"
"$configure" "${options[@]}" \
    --build="$("$CC" -dumpmachine)" \
    --host=wasm32-wasip1 \
    BUILDCC="$CC" \
    CC="$sdk/bin/clang" \
    CFLAGS="--target=wasm32-wasip1 --sysroot=$sdk/share/wasi-sysroot -O2 -g0 -D_WASI_EMULATED_SIGNAL=1 -D_WASI_EMULATED_GETPID=1" \
    LDFLAGS="-lwasi-emulated-signal -lwasi-emulated-getpid" \
    AR="$sdk/bin/llvm-ar" \
    RANLIB="$sdk/bin/llvm-ranlib"

"$MAKE" recurse CONF_SUBDIRS=texk/kpathsea
"$MAKE" -C texk recurse CONF_SUBDIRS=web2c
"$MAKE" -j2 -C texk/web2c tex
cp texk/web2c/tex "$output"
