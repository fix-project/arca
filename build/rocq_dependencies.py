"""List the installed libraries needed by the proof's Rocq imports."""

from pathlib import Path
import re
import shlex
import subprocess
import sys
import tempfile

rocq, project = sys.argv[1:]
project = Path(project)
config = dict(line.split("=", 1) for line in subprocess.check_output(
    [rocq, "compile", "-config"], text=True).splitlines())
coqlib = Path(config["COQLIB"]).resolve()
dependencies = subprocess.check_output(
    [rocq, "dep", "-w", "@module-not-found", "-boot", "-R", str(coqlib / "theories"), "Corelib",
     "-R", str(coqlib / "user-contrib"), "", "-f", str(project / "wasm-proofs/rocq/_CoqProject")], text=True)
modules = set()
for line in dependencies.splitlines():
    for name in shlex.split(line.split(": ", 1)[1]):
        path = Path(name).resolve()
        if path.suffix == ".vo" and path.is_relative_to(coqlib):
            parts = path.relative_to(coqlib).with_suffix("").parts
            modules.add(".".join(("Corelib", *parts[1:]) if parts[0] == "theories" else parts[1:]))
with tempfile.TemporaryDirectory(prefix="fix-proof-inventory-") as directory:
    source = Path(directory) / "Inventory.v"
    source.write_text("Require " + " ".join(sorted(modules)) + ".\nPrint Libraries.\n")
    output = subprocess.check_output(
        [rocq, "repl", "-quiet", "-batch", "-q", "-l", str(source)], text=True)
names = set(re.findall(r"^  ([A-Za-z_][A-Za-z_0-9]*(?:\.[A-Za-z_][A-Za-z_0-9]*)*)\s*$",
                       output, re.MULTILINE))
if not modules.issubset(names):
    raise ValueError("Could not inventory every imported library")
print("\n".join(sorted(names)))
