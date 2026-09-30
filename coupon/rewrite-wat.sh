#!/usr/bin/bash

# Adjust the soundness proof's coupon collector into an executable version

if [[ $# -ne 4 ]]; then
    echo "Usage: $0 ORIGINAL.wat PREAMBLE.wat EXTRA_FUNCS.wat OUTPUT.wat" >&2
    exit 1
fi

original=$1
preamble=$2
extra_funcs=$3
output=$4

for file in "$original" "$preamble" "$extra_funcs"; do
    if [[ ! -f "$file" ]]; then
        echo "Error: file not found: $file" >&2
        exit 1
    fi
done

tmp=$(mktemp)
trap 'rm -f "$tmp"' EXIT

cat "$preamble" > "$tmp"

# Keep the original coupon.wat once the type declarations start
awk '/^[[:space:]]*\(type([[:space:](]|$)/, 0' "$original" >> "$tmp"

cat "$extra_funcs" >> "$tmp"

mv "$tmp" "$output"
