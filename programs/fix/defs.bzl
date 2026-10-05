"""Fix programs written in WebAssembly text or Rust."""

load("@rules_rust//rust:defs.bzl", "rust_shared_library")
load("//build:defs.bzl", "WASM", "fix_procedure", "wat_module")

FIX_PROGRAMS = ["identity", "addblob", "slowaddblob"]
FIX_RUST_PROGRAMS = ["map", "mapreduce", "count_words", "bptree_get", "bptree_get_n", "fib"]

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

def fix_rust_program(name):
    """Build a Rust procedure using fixutils, a native Fix ELF, and a Fix runtime launcher."""
    rust_shared_library(
        name = name + "_wasm",
        visibility = ["//tests:__pkg__"],
        srcs = ["rust/" + name + ".rs"],
        platform = WASM,
        deps = [
            "//fix/sdk",
            "@crates//:dlmalloc",
        ],
    )
    fix_procedure(
        name = name,
        wasm = ":" + name + "_wasm",
        postprocess = True,
        runnable = True,
        visibility = ["//visibility:public"],
        artifact_visibility = ["//tests/fix:__pkg__"],
    )
