"""Compile C Wasm modules and synthesize requested Wasm operations."""

load("//build:cc.bzl", "cc_object")

SDK_WARNINGS = ["-Wall", "-Wextra", "-Werror"]

def sdk_copts():
    return ["--target=wasm32", "-mreference-types", "-mbulk-memory", "-O2", "-ffreestanding", "-fno-builtin", "-nostdlib", "-Ifix/sdk/include", "-x", "c"]

def fix_cc_wasm(name, srcs, hdrs = [], link_inputs = [], copts = [], memories = None, tables = None, visibility = None):
    """Resource counts include existing Memories and Tables."""
    compiled = name + "_compiled"
    objects = []
    for index, src in enumerate(srcs):
        obj = compiled + "_object_%d" % index
        cc_object(
            name = obj,
            src = src,
            hdrs = hdrs + ["//fix/sdk:headers"],
            compiler = "@local_tools//:clang",
            copts = sdk_copts() + copts,
            visibility = ["//visibility:private"],
        )
        objects.append(":" + obj)
    native.genrule(
        name = compiled,
        srcs = objects + link_inputs,
        outs = [compiled + ".wasm"],
        tools = ["@local_tools//:clang"],
        cmd = "$(execpath @local_tools//:clang) --target=wasm32 -mreference-types -mbulk-memory -O2 -nostdlib -Wl,--no-entry -Wl,--export-memory $(SRCS) -o $@",
        visibility = ["//visibility:private"],
    )
    counts = (" --memories %d" % memories if memories != None else "") + (" --tables %d" % tables if tables != None else "")
    native.genrule(
        name = name,
        srcs = [":" + compiled],
        outs = [name + ".wasm"],
        tools = ["//tools:postprocess"],
        cmd = "$(execpath //tools:postprocess) $< $@" + counts,
        visibility = visibility,
    )
