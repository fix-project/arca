#!/usr/bin/env bash
set -euo pipefail

first=$(cat "$PWD/$1")
second=$(cat "$PWD/$2")
if [[ "$first" != "$second" ]]; then
    echo "self-hosted compiler did not stabilize: $first != $second" >&2
    exit 1
fi
