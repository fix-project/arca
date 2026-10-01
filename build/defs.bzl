"""Arca artifact and VM rules."""

load("@rules_cc//cc:cc_import.bzl", "cc_import")
load("@rules_rust//rust:defs.bzl", "rust_binary")

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

def wat_module(name, src, out = None):
    """Compile WebAssembly text to a binary module."""
    native.genrule(
        name = name,
        visibility = ["//visibility:private"],
        srcs = [src],
        outs = [out or name + ".wasm"],
        cmd = "$(execpath //third_party:wat2wasm) --enable-multi-memory $< -o $@",
        tools = ["//third_party:wat2wasm"],
    )

def native_archive(name, srcs, hdrs = [], includes = [], compiler = "@local_tools//:gcc", copts = [], **kwargs):
    """Freestanding x86-64 objects; no Linux CRT or host ABI libraries."""
    objects = []
    for i, src in enumerate(srcs):
        obj = name + "_%d.o" % i
        native.genrule(
            name = name + "_object_%d" % i,
            visibility = ["//visibility:private"],
            srcs = [src] + hdrs,
            outs = [obj],
            tools = [compiler],
            cmd = "$(execpath " + compiler + ") -c -ffreestanding -fno-stack-protector -mno-red-zone -mcmodel=large -fno-pic -g " +
                  " ".join(["-I" + p for p in includes] + copts) +
                  " $(location " + src + ") -o $@",
        )
        objects.append(obj)
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
if [[ "$#" -eq 0 ]]; then
    echo "Pass one or more Fix expressions after --." >&2
    exit 2
fi
printf '(let ((program @"%%s")) *#(program' "$work/program.elf" > "$work/program.fix"
for arg in "$@"; do
    printf ' %%s' "$arg" >> "$work/program.fix"
done
printf '))\\n' >> "$work/program.fix"
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

def fix_procedure(name, wasm, postprocess = False, runnable = False, visibility = None, artifact_visibility = ["//visibility:private"]):
    """Wasm -> optional Fix memory imports -> C -> native Fix ELF."""
    if postprocess:
        native.genrule(
            name = name + "_postprocess",
            visibility = ["//visibility:private"],
            srcs = [wasm],
            tools = ["//tools:postprocess"],
            outs = [name + ".wasm"],
            cmd = "$(execpath //tools:postprocess) $(location " + wasm + ") $@",
        )
        wasm = ":" + name + "_postprocess"
    native.genrule(
        name = name + "_translate",
        visibility = ["//visibility:private"],
        srcs = [wasm],
        tools = ["//third_party:wasm2c"],
        outs = [name + "/module.c", name + "/module.h"],
        cmd = "$(execpath //third_party:wasm2c) -n module --enable-multi-memory $(location " + wasm + ") -o $(location " + name + "/module.c)",
    )
    artifact = name + "_elf" if runnable else name
    native.genrule(
        name = artifact,
        srcs = [name + "/module.c", name + "/module.h", "//fix/shell:headers", "//fix/shell:wasm_rt", "//fix/shell:memmap", "//fix/shell:shell"],
        tools = ["@local_tools//:gcc"],
        outs = [name + ".elf"],
        cmd = "$(execpath @local_tools//:gcc) -o $@ -T $(location //fix/shell:memmap) -O2 -fno-optimize-sibling-calls -frounding-math -ffreestanding -nostdlib -nostartfiles -mcmodel=large -mno-red-zone -march=x86-64-v3 -static -Ifix/shell/inc " +
              "-I$$(dirname $(location " + name + "/module.h)) $(location " + name + "/module.c) $(location //fix/shell:wasm_rt) $(location //fix/shell:shell)",
        visibility = artifact_visibility if runnable else visibility,
    )
    if runnable:
        fix_program_run(
            name = name,
            program = ":" + artifact,
            visibility = visibility,
        )
