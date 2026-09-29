#!/usr/bin/env bash
set -euo pipefail
command -v isabelle >/dev/null || { echo "Install Isabelle2025-2 and AFP; see coupon/proof/README.md" >&2; exit 1; }
command -v ocamlc >/dev/null || { echo "Install the proof's OCaml dependencies; see coupon/proof/README.md" >&2; exit 1; }
# Bzlmod's canonical repository name is supplied by rlocation's repo mapping.
source "${RUNFILES_DIR:-$0.runfiles}/bazel_tools/tools/bash/runfiles/runfiles.bash"
proof="$(dirname "$(rlocation "$1")")"
cp -R -L "$proof" "${TEST_TMPDIR:?}/proof"
chmod -R u+w "$TEST_TMPDIR/proof"
cd "$TEST_TMPDIR/proof"
make
