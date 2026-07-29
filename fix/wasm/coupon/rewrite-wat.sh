#!/usr/bin/bash

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

awk '
    /^[[:space:]]*\(type([[:space:](]|$)/ {
        found = 1
    }

    found {
        lines[++count] = $0
    }

    END {
        if (!found) {
            print "Error: no type declaration found" > "/dev/stderr"
            exit 1
        }

        # Find the last nonblank line, which should close the module.
        last = count
        while (last > 0 && lines[last] ~ /^[[:space:]]*$/) {
            last--
        }

        if (last == 0 || lines[last] !~ /^[[:space:]]*\)[[:space:]]*$/) {
            print "Error: module does not end with a standalone closing parenthesis" \
                > "/dev/stderr"
            exit 1
        }

        # Print everything except the module closing parenthesis.
        for (i = 1; i < last; i++) {
            print lines[i]
        }
    }
' "$original" >> "$tmp"

cat "$extra_funcs" >> "$tmp"
printf ')\n' >> "$tmp"

mv "$tmp" "$output"
