#!/usr/bin/env bash
set -euo pipefail

linter=$1
config=$2
shift 2
exec "$linter" --config-file="$config" --header-filter='.*' "$@"
