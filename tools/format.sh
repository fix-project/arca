#!/usr/bin/env bash
set -euo pipefail

cd "$BUILD_WORKSPACE_DIRECTORY"
bazel run @rules_rust//tools/rustfmt:target_aware_rustfmt
bazel run //fix/sdk:format
