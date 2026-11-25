#!/usr/bin/env bash

set -ex

bazel build --profile=release //programs/fix/python
out=$(bazel info --profile=release | grep "bazel-bin" | cut -d' ' -f2)
fix eval -l python "@compile(\"$out/programs/fix/python/python.wasm\")"
