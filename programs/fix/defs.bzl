"""Build and test standalone Fix programs."""

load("@rules_rust//rust:defs.bzl", "rust_shared_library")
load("//build:defs.bzl", "WASM", "fix_procedure", "fix_script_test", "wat_module")
load("//fix/sdk:defs.bzl", "fix_cc_wasm")

FIX_PROGRAMS = ["identity", "addblob"]
FIX_RUST_PROGRAMS = ["map", "mapreduce", "count_words", "bptree_get", "bptree_get_n", "fib"]

def fix_wat_program(name):
    """Build a WAT module, a native Fix ELF, and a Fix runtime launcher."""
    wat_module(name = name + "_wasm", src = name + ".wat", out = name + ".wasm")
    fix_procedure(
        name = name,
        wasm = ":" + name + "_wasm",
        runnable = True,
        visibility = ["//visibility:public"],
        artifact_visibility = ["//visibility:public"],
    )

def fix_rust_program(name, alloc = False, importers = False, memories = 3, tables = 3, artifact_visibility = ["//tests/fix:__pkg__"], deps = []):
    """Build a Rust procedure using the Fix SDK, a native Fix ELF, and a Fix runtime launcher."""
    sdk = "//fix/sdk"
    if importers:
        sdk = "//fix/sdk:sdk_import"
    elif alloc:
        sdk = "//fix/sdk:sdk_alloc"
    rust_shared_library(
        name = name + "_wasm",
        visibility = ["//tests:__pkg__"],
        srcs = [name + ".rs"] + native.glob([name + "/**/*.rs"], allow_empty = True),
        crate_root = name + ".rs",
        platform = WASM,
        deps = [sdk] + deps + (["@crates//:dlmalloc"] if alloc or importers else []),
    )
    fix_procedure(
        name = name,
        wasm = ":" + name + "_wasm",
        memories = memories,
        tables = tables,
        runnable = True,
        visibility = ["//visibility:public"],
        artifact_visibility = artifact_visibility,
    )

FIX_LANGUAGE_PROGRAMS = ["blob", "add"]

def fix_language_programs(language):
    """Build the same standalone programs and integration tests in each language."""
    for name in FIX_LANGUAGE_PROGRAMS:
        wasm = name + "_wasm"
        if language == "rust":
            rust_shared_library(
                name = wasm,
                srcs = [name + ".rs"],
                crate_name = name,
                crate_root = name + ".rs",
                platform = WASM,
                deps = ["//fix/sdk"],
            )
        elif language == "wat":
            wat_module(name = wasm, src = name + ".wat")
        else:
            fix_cc_wasm(
                name = wasm,
                srcs = [name + ".c"],
                memories = 2,
            )
        fix_procedure(
            name = name,
            wasm = ":" + wasm,
            memories = 3 if language == "rust" else None,
            tables = 3 if language == "rust" else None,
            runnable = True,
            artifact_visibility = ["//visibility:public"],
        )

    native.filegroup(name = "programs", srcs = [":" + name for name in FIX_LANGUAGE_PROGRAMS])
