"""List external libraries imported by compiled Rocq modules."""

from pathlib import Path
import re
import subprocess
import sys
import tempfile

output, rocq, *arguments = sys.argv[1:]
separator = arguments.index("--")
loadpath = arguments[:separator]
modules = set(arguments[separator + 1:])
with tempfile.TemporaryDirectory(prefix="fix-proof-inventory-") as directory:
    source = Path(directory) / "Inventory.v"
    source.write_text("Require " + " ".join(sorted(modules)) + ".\nPrint Libraries.\n")
    libraries = subprocess.check_output(
        [rocq, "repl", "-quiet", "-batch", "-q", *loadpath, "-l", str(source)], text=True)
names = set(re.findall(r"^  ([A-Za-z_][A-Za-z_0-9]*(?:\.[A-Za-z_][A-Za-z_0-9]*)*)\s*$",
                       libraries, re.MULTILINE))
if not modules.issubset(names):
    raise ValueError("Could not inventory every proof module")
Path(output).write_text("\n".join(sorted(names - modules)) + "\n")
