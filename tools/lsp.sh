#!/usr/bin/env bash
set -euo pipefail
cd "$BUILD_WORKSPACE_DIRECTORY"
bazel run //:clangd -- "$@"
bazel run @rules_rust//tools/rust_analyzer:gen_rust_project -- //...
