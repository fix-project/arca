"""Embed a Wasm binary in a parsed WasmCert-Coq module."""

from pathlib import Path
import sys

binary, output = map(Path, sys.argv[1:])
data = binary.read_bytes()
rows = ["  " + "; ".join(f"x{b:02x}" for b in data[i:i + 16])
        for i in range(0, len(data), 16)]
byte_list = ";\n".join(rows)
output.write_text(f"""(** Embeds the collector binary and parses its WasmCert module. A successful
    parse ties the execution proofs to the exact binary built for execution. *)

From Stdlib Require Import Strings.Byte List.
From Wasm Require Import datatypes binary_format_parser.
Import ListNotations.

Definition evaluation_binary : list Byte.byte := [
{byte_list}
].

(** Compute the actual module AST. Parsing failure is a compilation error,
    rather than a default module or an assumption about the input. *)
Definition evaluation_module : module.
Proof.
  let parsed := eval vm_compute in (run_parse_module evaluation_binary) in
  lazymatch parsed with
  | Some ?m => exact m
  | None => fail "WasmCert-Coq could not parse collector binary"
  end.
Defined.

Lemma evaluation_module_parsed : run_parse_module evaluation_binary = Some evaluation_module.
Proof. vm_compute; reflexivity. Qed.
""")
