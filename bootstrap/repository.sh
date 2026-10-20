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
seed=$(realpath "${19}") compiler=$(realpath "${20}")
compiler_wasm=$(realpath "${21}")
result=$(realpath "${22}") trust=$(realpath -m "${23}")
handles=$(realpath -m "${24}")
shift 24
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
cp "$compiler_wasm" "$work/toolchain/share/fix/compile.wasm"
for header in "$@"; do cp "$header" "$work/toolchain/share/fix/"; done
cd "$work"
unset BUILD_WORKSPACE_DIRECTORY
"$fix" init
names=(clang wasm2c wasm-merge json parser identity path cpio flatware-normalize compose flatware-result)
"$seed" seed .fix "$compiler" clang "$clang" wasm2c "$wasm2c" wasm-merge "$merger" \
    json "$json" parser "$parser" identity "$identity" path "$path" cpio "$cpio" \
    flatware-normalize "$normalize" compose "$compose" flatware-result "$result"
export FIX_TRUST="$work/.fix/trust"
toolchain=$("$fix" flatware-import toolchain /toolchain)
"$fix" label set toolchain "$toolchain"
resources=$("$fix" create-tree wasm2c clang toolchain)
resources=$("$seed" tag .fix Bootstrap "$resources")
"$fix" label set compile-resources "$resources"
names+=(compile fixpoint compile-resources toolchain)
for name in "${names[@]}"; do
    printf '%s\0%s\0' "$name" "$("$fix" resolve "$name")" >> bindings
done
environment=$("$fix" create-tree --bindings - -l bootstrap < bindings)
python3 - "$handles" "$environment" <<'PY'
import json
from pathlib import Path
import sys
fields = Path("bindings").read_bytes().decode().split("\0")[:-1]
handles = dict(zip(fields[::2], fields[1::2]))
handles["bootstrap"] = sys.argv[2]
Path(sys.argv[1]).write_text(json.dumps(handles) + "\n")
PY
"$fix" export bootstrap "$work/bootstrap.tar" "${names[@]}"
tar --directory "$work" --append --file "$work/bootstrap.tar" --mtime=@0 \
    --owner=0 --group=0 --numeric-owner .fix/trust
gzip -n < "$work/bootstrap.tar" > "$archive"
cp .fix/trust "$trust"
