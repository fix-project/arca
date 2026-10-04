#!/usr/bin/env bash
set -euo pipefail

mode=$1
formatter=$(realpath "$2")
style=$(realpath "$3")
shift 3

case "$mode" in
    fix)
        cd "$BUILD_WORKSPACE_DIRECTORY"
        exec "$formatter" --style="file:$style" -i "$@"
        ;;
    check)
        exec "$formatter" --style="file:$style" --dry-run --Werror "$@"
        ;;
esac
