"""Standalone Arca programs and their kernel runners."""

load("//build:defs.bzl", "ARCA_USER", "guest_binary", "kernel_program")

ARCA_PROGRAMS = ["add", "curry", "identity", "io", "map", "null", "simd"]

def arca_program(name):
    """Build a userspace ELF, a kernel image embedding it, and a VM launcher."""
    guest_binary(
        name = name + "_elf",
        visibility = ["//arca/kernel:__pkg__", "//tests:__pkg__"],
        srcs = ["src/" + name + ".rs"],
        linker_script = "//arca/user:etc/memmap.ld",
        platform = ARCA_USER,
        deps = [
            "//arca/abi",
            "//arca/api",
            "//arca/user",
        ],
    )
    kernel_program(
        name = name,
        visibility = ["//visibility:public"],
        image_visibility = ["//tests:__pkg__"],
        srcs = ["//programs/arca:run.rs"],
        compile_data = [":" + name + "_elf"],
        crate_name = name + "_runner",
        linker_script = "//arca/kernel:etc/memmap.ld",
        rustc_env = {"ARCA_PROGRAM_NAME": name},
        deps = [
            "//arca/api",
            "//arca/kernel",
            "//lib/common",
            "@crates//:log",
            "@crates//:postcard",
        ],
    )
