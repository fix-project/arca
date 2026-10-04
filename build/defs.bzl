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

def wat_module(name, src, out = None, relocatable = False, debug_names = False, visibility = ["//visibility:private"]):
    """Compile WebAssembly text to a module or relocatable object."""
    native.genrule(
        name = name,
        visibility = visibility,
        srcs = [src],
        outs = [out or name + (".o" if relocatable else ".wasm")],
        cmd = "$(execpath //third_party:wat2wasm) --enable-multi-memory --enable-exceptions " +
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
    command = '"$vmm" "$kernel" --smp %d "$@"' % ctx.attr.smp
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

def _fix_program_launcher_impl(ctx):
    script = ctx.actions.declare_file(ctx.label.name + ".sh")
    vmm = ctx.executable.vmm
    kernel = ctx.file.kernel
    program = ctx.file.program
    runfiles = ctx.runfiles(files = [vmm, kernel, program])
    runfiles = runfiles.merge(ctx.attr.vmm[DefaultInfo].default_runfiles)
    content = """#!/usr/bin/env bash
set -euo pipefail
root="${RUNFILES_DIR:-${TEST_SRCDIR:-$0.runfiles}}/%s"
vmm="$root/%s"
kernel="$root/%s"
program="$root/%s"
if [[ ! -r /dev/kvm || ! -w /dev/kvm ]]; then
    echo "This target requires Linux x86-64 and access to /dev/kvm." >&2
    exit 1
fi
cd "${BUILD_WORKSPACE_DIRECTORY:-$PWD}"
work=$(mktemp -d)
trap 'rm -f "$work/program.elf" "$work/program.fix"; rmdir "$work"' EXIT
cp "$program" "$work/program.elf"
printf 'program = @"%%s"\\n*#(program' "$work/program.elf" > "$work/program.fix"
for arg in "$@"; do
    printf ' %%s' "$arg" >> "$work/program.fix"
done
printf ')\\n' >> "$work/program.fix"
"$vmm" "$kernel" --smp 2 eval "$work/program.fix"
""" % (ctx.workspace_name, vmm.short_path, kernel.short_path, program.short_path)
    ctx.actions.write(script, content, is_executable = True)
    return [DefaultInfo(executable = script, runfiles = runfiles)]

fix_program_run = rule(
    implementation = _fix_program_launcher_impl,
    executable = True,
    attrs = {
        "vmm": attr.label(default = "//tools/vmm:vmm", executable = True, cfg = "exec"),
        "kernel": attr.label(default = "//fix/runtime:image_elf", allow_single_file = True),
        "program": attr.label(mandatory = True, allow_single_file = True),
    },
)

def fix_procedure(name, wasm, memories = None, tables = None, runnable = False, visibility = None, artifact_visibility = ["//visibility:private"]):
    """Wasm -> optional resource postprocessing -> C -> native Fix ELF."""
    if memories != None or tables != None:
        counts = (" --memories %d" % memories if memories != None else "") + (" --tables %d" % tables if tables != None else "")
        native.genrule(
            name = name + "_postprocess",
            visibility = ["//visibility:private"],
            srcs = [wasm],
            tools = ["//tools:postprocess"],
            outs = [name + ".wasm"],
            cmd = "$(execpath //tools:postprocess) $(location " + wasm + ") $@" + counts,
        )
        wasm = ":" + name + "_postprocess"
    native.genrule(
        name = name + "_translate",
        visibility = ["//visibility:private"],
        srcs = [wasm],
        tools = ["//third_party:wasm2c", "//tools:compile"],
        outs = [name + "/module.c", name + "/module.h"],
        cmd = "$(execpath //tools:compile) translate $(execpath //third_party:wasm2c) $(location " + wasm + ") $(location " + name + "/module.c)",
    )
    artifact = name + "_elf" if runnable else name
    flags = FIX_COPTS + ["-Ifix/shell/inc"]
    objects = []
    for index, src in enumerate([name + "/module.c", "//fix/shell:wasm_rt"]):
        obj = name + "_native_object_%d" % index
        cc_object(
            name = obj,
            src = src,
            hdrs = [name + "/module.h", "//fix/shell:headers"],
            include_roots = [name + "/module.h"],
            copts = flags,
            visibility = ["//visibility:private"],
        )
        objects.append(":" + obj)
    native.genrule(
        name = artifact,
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
        )
