"""WASIp1 command modules running against Flatware."""

load("//build:defs.bzl", "fix_procedure")

def flatware_procedure(name, wasm, runnable = False, visibility = None, artifact_visibility = ["//visibility:private"]):
    native.genrule(
        name = name + "_flatware",
        srcs = [wasm, "//flatware:adapter_wasm", "//flatware:control_wasm"],
        tools = ["@local_tools//:wasm-merge", "//flatware:check_resources", "//tools:compile"],
        outs = [name + "_flatware.wasm"],
        cmd = "$(execpath //tools:compile) merge $(execpath @local_tools//:wasm-merge) $(execpath //flatware:check_resources) $(location //flatware:adapter_wasm) $(location //flatware:control_wasm) $(location " + wasm + ") $@",
    )
    fix_procedure(
        name = name + "_elf" if runnable else name,
        wasm = ":" + name + "_flatware",
        visibility = artifact_visibility if runnable else visibility,
        artifact_visibility = artifact_visibility,
    )
