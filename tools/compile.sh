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
        "$1" -n module "$2" -o "$3"
        ;;
    link)
        compiler=$1 flags_file=$2 memmap=$3 shell=$4 output=$5
        shift 5
        mapfile -t flags < "$flags_file"
        "$compiler" "${flags[@]}" -T "$memmap" -o "$output" "$@" "$shell"
        ;;
esac
