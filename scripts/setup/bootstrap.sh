#!/usr/bin/env bash

set -ex

bazel build --config=release //bootstrap 
out=$(bazel info --config=release | grep "bazel-bin" | cut -d' ' -f2)
bazel run --config=release //fix -- bootstrap "$out/bazel-bin/bootstrap/seed.tar"
