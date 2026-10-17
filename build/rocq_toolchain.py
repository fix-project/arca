"""Provision the private opam switch used by Bazel's Rocq rules."""

import os
from pathlib import Path
import subprocess
import sys

opam, directory = sys.argv[1:]
root = Path(directory) / "opam"
env = {key: value for key, value in os.environ.items()
       if not key.startswith(("OPAM", "OCAML", "CAML", "COQ", "ROCQ"))}
env.update(OPAMROOT=str(root), OPAMSWITCH="proof", OPAMYES="1")


def run(*args):
    subprocess.run([opam, "--cli=2.1", *args], env=env, check=True, cwd=directory)


if not (root / "config").exists():
    run("init", "--bare", "--no-setup", "--disable-sandboxing", "default",
        "https://opam.ocaml.org")
if not (root / "proof").exists():
    run("switch", "create", "proof", "--empty")
run("repository", "add", "rocq-released", "https://rocq-prover.org/opam/released")
run("install", "./rocq-toolchain.opam", "--locked", "--deps-only", "--no-depexts")
