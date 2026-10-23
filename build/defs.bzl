"""Arca artifact and VM rules."""

load("@rules_cc//cc:cc_import.bzl", "cc_import")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//build:cc.bzl", "cc_object")
load("@rules_rust//rust:defs.bzl", "rust_binary")

FIX_COPTS = ["-O2", "-fno-optimize-sibling-calls", "-frounding-math", "-ffreestanding", "-nostdlib", "-nostartfiles", "-mcmodel=large", "-mno-red-zone", "-march=x86-64-v3", "-static"]

ARCA = "//build/platforms:arca"
ARCA_USER = "//build/platforms:arca_user"
WASM = "//build/platforms:wasm"
GUEST_ONLY = ["@platforms//os:none", "@platforms//cpu:x86_64"]

def guest_binary(name, linker_script, rustc_flags = [], platform = ARCA, **kwargs):
    rust_binary(
        name = name,
        platform = platform,
        linker_script = linker_script,
        rustc_flags = rustc_flags + ["-Clink-arg=-no-pie"],
        **kwargs
    )

def kernel_program(name, srcs, image_name = None, linker_script = "//arca/kernel:etc/memmap.ld", visibility = None, image_visibility = ["//visibility:private"], **kwargs):
    """Build a kernel image and its VM launcher."""
    image_name = image_name or name + "_image"
    guest_binary(
        name = image_name,
        visibility = image_visibility,
        srcs = srcs,
        linker_script = linker_script,
        **kwargs
    )
    vm_run(name = name, kernel = ":" + image_name, visibility = visibility)

def kernel_test(name, srcs, image_name = None, linker_script = "//arca/kernel:etc/memmap.ld", rustc_flags = [], visibility = None, image_visibility = ["//visibility:private"], **kwargs):
    """Build a kernel test image and run it under the VMM."""
    image_name = image_name or name + "_image"
    guest_binary(
        name = image_name,
        visibility = image_visibility,
        testonly = True,
        srcs = srcs,
        linker_script = linker_script,
        rustc_flags = rustc_flags + ["--test"],
        **kwargs
    )
    vm_test(
        name = name,
        size = "large",
        timeout = "short",
        kernel = ":" + image_name,
        tags = ["kvm", "local"],
        visibility = visibility,
    )

def wat_module(name, src, out = None, relocatable = False, debug_names = False, visibility = ["//visibility:private"], tags = []):
    """Compile WebAssembly text to a module or relocatable object."""
    native.genrule(
        name = name,
        visibility = visibility,
        srcs = [src],
        tags = tags,
        outs = [out or name + (".o" if relocatable else ".wasm")],
        cmd = "$(execpath //third_party:wat2wasm) " +
              ("--relocatable " if relocatable else "") + ("--debug-names " if debug_names else "") + "$< -o $@",
        tools = ["//third_party:wat2wasm"],
    )

def _wasm_library_impl(ctx):
    archive = ctx.file.archive
    library = cc_common.create_library_to_link(
        actions = ctx.actions,
        static_library = archive,
    )
    linker_input = cc_common.create_linker_input(
        owner = ctx.label,
        libraries = depset([library]),
    )
    return [
        DefaultInfo(files = depset([archive])),
        CcInfo(linking_context = cc_common.create_linking_context(linker_inputs = depset([linker_input]))),
    ]

_wasm_library = rule(
    implementation = _wasm_library_impl,
    attrs = {
        "archive": attr.label(allow_single_file = [".a"], mandatory = True),
    },
)

def wasm_library(name, srcs, **kwargs):
    """Archive relocatable Wasm objects for C/C++ or Rust linkage."""
    native.genrule(
        name = name + "_archive",
        visibility = ["//visibility:private"],
        srcs = srcs,
        outs = ["lib" + name + ".a"],
        cmd = "$(execpath @local_tools//:ar) crs $@ $(SRCS)",
        tools = ["@local_tools//:ar"],
    )
    _wasm_library(name = name, archive = ":" + name + "_archive", **kwargs)

def wat_library(name, src, **kwargs):
    """Compile WebAssembly text into a relocatable Wasm library."""
    wat_module(name = name + "_object", src = src, relocatable = True)
    wasm_library(name = name, srcs = [":" + name + "_object"], **kwargs)

def native_archive(name, srcs, hdrs = [], includes = [], compiler = "@local_tools//:gcc", copts = [], **kwargs):
    """Freestanding x86-64 objects; no Linux CRT or host ABI libraries."""
    objects = []
    for i, src in enumerate(srcs):
        obj = name + "_object_%d" % i
        cc_object(
            name = obj,
            src = src,
            hdrs = hdrs,
            compiler = compiler,
            copts = ["-ffreestanding", "-fno-stack-protector", "-mno-red-zone", "-mcmodel=large", "-fno-pic", "-g"] + ["-I" + path for path in includes] + copts,
            visibility = ["//visibility:private"],
        )
        objects.append(":" + obj)
    native.genrule(
        name = name + "_archive",
        visibility = ["//visibility:private"],
        srcs = objects,
        outs = ["lib" + name + ".a"],
        tools = ["@local_tools//:ar"],
        cmd = "$(execpath @local_tools//:ar) crs $@ $(SRCS)",
    )
    cc_import(name = name, static_library = "lib" + name + ".a", **kwargs)

def _vm_launcher_impl(ctx):
    script = ctx.actions.declare_file(ctx.label.name + ".sh")
    vmm = ctx.executable.vmm
    kernel = ctx.file.kernel
    runfiles = ctx.runfiles(files = [vmm, kernel] + ctx.files.data)
    runfiles = runfiles.merge(ctx.attr.vmm[DefaultInfo].default_runfiles)

    # Resolve runfiles before changing cwd. Relative paths provided by the user
    # remain relative to their workspace; tests use a writable private directory.
    content = """#!/usr/bin/env bash
set -euo pipefail
root="${RUNFILES_DIR:-${TEST_SRCDIR:-$0.runfiles}}/%s"
vmm="$root/%s"
kernel="$root/%s"
if [[ ! -r /dev/kvm || ! -w /dev/kvm ]]; then
    echo "This target requires Linux x86-64 and access to /dev/kvm." >&2
    exit 1
fi
""" % (ctx.workspace_name, vmm.short_path, kernel.short_path)
    if ctx.attr.testonly:
        content += 'cd "${TEST_TMPDIR:?}"\n'
        for f in ctx.files.data:
            content += 'cp "$root/%s" "%s"\n' % (f.short_path, f.basename)
    else:
        content += 'cd "${BUILD_WORKSPACE_DIRECTORY:-$PWD}"\n'

    # Bazel passes the rule's args before command-line arguments.
    command = '"$vmm" "$kernel" --smp %d -- "$@"' % ctx.attr.smp
    if ctx.attr.expected_output:
        content += command + ' 2>&1 | tee "${TEST_TMPDIR:?}/output"\n'
        for expected in ctx.attr.expected_output:
            content += "grep -F -- '" + expected.replace("'", "'\\''") + "' \"${TEST_TMPDIR:?}/output\" >/dev/null\n"
    else:
        content += "exec " + command + "\n"
    ctx.actions.write(script, content, is_executable = True)
    return [DefaultInfo(executable = script, runfiles = runfiles)]

_vm_attrs = {
    "vmm": attr.label(default = "//tools/vmm:vmm", executable = True, cfg = "exec"),
    "kernel": attr.label(mandatory = True, allow_single_file = True),
    "data": attr.label_list(allow_files = True),
    "smp": attr.int(default = 2),
    "expected_output": attr.string_list(),
}

vm_run = rule(implementation = _vm_launcher_impl, executable = True, attrs = _vm_attrs)
vm_test = rule(implementation = _vm_launcher_impl, test = True, attrs = _vm_attrs)

def fix_program_run(name, program, mode = "run", **kwargs):
    rust_binary(
        name = name,
        srcs = ["//build:fix_run.rs"],
        crate_name = "fix_run",
        edition = "2024",
        data = ["//fix", program],
        deps = ["@rules_rust//tools/runfiles"],
        rustc_env = {
            "FIX_CLI": "$(rlocationpath //fix)",
            "FIX_PROGRAM": "$(rlocationpath " + program + ")",
            "FIX_MODE": mode,
        },
        **kwargs
    )

def _fix_script_test(ctx):
    executable = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.symlink(output = executable, target_file = ctx.executable.driver, is_executable = True)
    def path(file):
        return ctx.workspace_name + "/" + file.short_path
    environment = {
        "FIX_TEST_CLI": path(ctx.executable.fix),
        "FIX_TEST_PARSER": path(ctx.file.parser),
        "FIX_TEST_IDENTITY": path(ctx.file.identity),
        "FIX_TEST_SEED": path(ctx.executable.seed),
        "FIX_TEST_COMPILE": path(ctx.file.compiler),
        "FIX_TEST_DATA": "\n".join([path(file) for file in ctx.files.data]),
        "FIX_TEST_SCRIPT": ctx.attr.script,
        "FIX_TEST_JOBS": str(ctx.attr.jobs),
        "FIX_TEST_EXPECTED": ctx.attr.expected,
    }
    runfiles = ctx.runfiles(files = ctx.files.data + [ctx.executable.fix, ctx.file.parser, ctx.file.identity, ctx.executable.seed, ctx.file.compiler])
    for target in [ctx.attr.fix, ctx.attr.driver]:
        runfiles = runfiles.merge(target[DefaultInfo].default_runfiles)
    return [DefaultInfo(executable = executable, runfiles = runfiles), RunEnvironmentInfo(environment = environment)]

fix_script_test = rule(
    implementation = _fix_script_test,
    test = True,
    attrs = {
        "driver": attr.label(default = "//build:fix_test", executable = True, cfg = "exec"),
        "fix": attr.label(default = "//fix", executable = True, cfg = "target"),
        "parser": attr.label(default = "//programs/fix/parser:parser_elf", allow_single_file = True),
        "identity": attr.label(default = "//programs/fix:identity_elf", allow_single_file = True),
        "seed": attr.label(default = "//tools:seed_repository", executable = True, cfg = "exec"),
        "compiler": attr.label(default = "//programs/fix:compile_elf", allow_single_file = True),
        "script": attr.string(mandatory = True),
        "data": attr.label_list(allow_files = True),
        "jobs": attr.int(default = 1),
        "expected": attr.string(mandatory = True),
    },
)

def fix_procedure(name, wasm, memories = None, tables = None, runnable = False, visibility = None, artifact_visibility = ["//visibility:private"], tags = []):
    """Wasm -> optional resource postprocessing -> C -> native Fix ELF."""
    if memories != None or tables != None:
        counts = (" --memories %d" % memories if memories != None else "") + (" --tables %d" % tables if tables != None else "")
        native.genrule(
            name = name + "_postprocess",
            tags = tags,
            visibility = ["//visibility:private"],
            srcs = [wasm],
            tools = ["//tools:postprocess"],
            outs = [name + ".wasm"],
            cmd = "$(execpath //tools:postprocess) $(location " + wasm + ") $@" + counts,
        )
        wasm = ":" + name + "_postprocess"
    native.genrule(
        name = name + "_translate",
        tags = tags,
        visibility = ["//visibility:private"],
        srcs = [wasm],
        tools = ["//third_party:wasm2c", "//tools:compile"],
        outs = [name + "_translate/module.c", name + "_translate/module.h"],
        cmd = "$(execpath //tools:compile) translate $(execpath //third_party:wasm2c) $(location " + wasm + ") $(location " + name + "_translate/module.c)",
    )
    artifact = name + "_elf" if runnable else name
    flags = FIX_COPTS + ["-Ifix/shell/inc"]
    objects = []
    for index, src in enumerate([name + "_translate/module.c", "//fix/shell:wasm_rt"]):
        obj = name + "_native_object_%d" % index
        cc_object(
            name = obj,
            tags = tags,
            src = src,
            hdrs = [name + "_translate/module.h", "//fix/shell:headers"],
            include_roots = [name + "_translate/module.h"],
            copts = flags,
            visibility = ["//visibility:private"],
        )
        objects.append(":" + obj)
    native.genrule(
        name = artifact,
        tags = tags,
        srcs = objects + ["//fix/shell:memmap", "//fix/shell:shell"],
        tools = ["@local_tools//:gcc", "//tools:compile", "//tools:fix_copts"],
        outs = [name + ".elf"],
        cmd = "$(execpath //tools:compile) link $(execpath @local_tools//:gcc) $(location //tools:fix_copts) $(location //fix/shell:memmap) $(location //fix/shell:shell) $@ " + " ".join(["$(location " + obj + ")" for obj in objects]),
        visibility = artifact_visibility if runnable else visibility,
    )
    if runnable:
        fix_program_run(
            name = name,
            program = ":" + artifact,
            visibility = visibility,
            tags = tags,
        )
