"""Fix programs written in WebAssembly text."""

load("//build:defs.bzl", "fix_procedure", "wat_module")

FIX_PROGRAMS = ["identity", "addblob", "slowaddblob"]

def fix_wat_program(name):
    """Build a WAT module, a native Fix ELF, and a Fix runtime launcher."""
    wat_module(name = name + "_wasm", src = name + ".wat", out = name + ".wasm")
    fix_procedure(
        name = name,
        wasm = ":" + name + "_wasm",
        runnable = True,
        visibility = ["//visibility:public"],
        artifact_visibility = ["//fix/runtime:__pkg__", "//tests/fix:__pkg__"],
    )
