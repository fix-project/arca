#!/usr/bin/env bash
set -euo pipefail
mode=$1
shift
case "$mode" in
    merge)
        merge=$1 checker=$2 adapter=$3 control=$4 input=$5 output=$6
        "$merge" "$adapter" wasi_snapshot_preview1 "$input" flatware_guest "$control" flatware_control \
            --all-features --disable-gc --rename-export-conflicts -o "$output"
        "$checker" "$output" "$output"
        ;;
    translate)
        "$1" -n module --enable-multi-memory --enable-exceptions "$2" -o "$3"
        ;;
    link|compile)
        compiler=$1 flags_file=$2 memmap=$3 shell=$4 output=$5
        shift 5
        mapfile -t flags < "$flags_file"
        if [[ "$mode" == compile ]]; then
            wasm2c=$1 input=$2 runtime=$3 header=$4
            source="$(dirname "$output")/module.c"
            "$0" translate "$wasm2c" "$input" "$source"
            set -- "-I$(dirname "$header")" "-I$(dirname "$source")" "$source" "$runtime"
        fi
        "$compiler" "${flags[@]}" -T "$memmap" -o "$output" "$@" "$shell"
        ;;
esac
