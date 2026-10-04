"""Flatware launchers with the compiler and VM in their runfiles."""

def _launcher(ctx):
    script = ctx.actions.declare_file(ctx.label.name + ".sh")
    resources = {
        "VMM": ctx.executable.vmm,
        "KERNEL": ctx.file.kernel,
    }
    files = [ctx.executable.runner]
    dependencies = [ctx.attr.runner, ctx.attr.vmm]
    if not ctx.file.program:
        resources.update({
            "ADAPTER": ctx.file.adapter,
            "CONTROL": ctx.file.control,
            "WASM_MERGE": ctx.file.wasm_merge,
            "WASM2C": ctx.file.wasm2c,
            "GCC": ctx.file.gcc,
            "COMPILE": ctx.executable.compile,
            "FLAGS": ctx.file.flags,
            "CHECKER": ctx.executable.checker,
            "SHELL": ctx.file.shell,
            "WASM_RT": ctx.file.wasm_rt,
            "MEMMAP": ctx.file.memmap,
            "HEADER": [file for file in ctx.files.headers if file.basename == "wasm-rt.h"][0],
        })
        files += ctx.files.headers
        dependencies += [ctx.attr.checker, ctx.attr.compile]
    files += resources.values()
    content = """#!/usr/bin/env bash
set -euo pipefail
root="${RUNFILES_DIR:-${TEST_SRCDIR:-$0.runfiles}}/%s"
root="$(cd "$root" && pwd)"
cd "${BUILD_WORKSPACE_DIRECTORY:-$PWD}"
""" % ctx.workspace_name
    for name, file in resources.items():
        content += 'export FLATWARE_%s="$root/%s"\n' % (name, file.short_path)
    if ctx.file.program:
        files.append(ctx.file.program)
        content += 'exec "$root/%s" run --native "$root/%s" "$@"\n' % (ctx.executable.runner.short_path, ctx.file.program.short_path)
    else:
        content += 'exec "$root/%s" run "$@"\n' % ctx.executable.runner.short_path
    ctx.actions.write(script, content, is_executable = True)
    runfiles = ctx.runfiles(files = files)
    for target in dependencies:
        runfiles = runfiles.merge(target[DefaultInfo].default_runfiles)
    return [DefaultInfo(executable = script, runfiles = runfiles)]

_runtime_attrs = {
    "program": attr.label(allow_single_file = True),
    "runner": attr.label(default = "//flatware/runner:binary", executable = True, cfg = "exec"),
    "kernel": attr.label(default = "//flatware/runner:kernel_elf", allow_single_file = True),
    "vmm": attr.label(default = "//tools/vmm:vmm", executable = True, cfg = "exec"),
}

_native_run = rule(implementation = _launcher, executable = True, attrs = _runtime_attrs)
_wasm_run = rule(
    implementation = _launcher,
    executable = True,
    attrs = dict(_runtime_attrs, **{
        "adapter": attr.label(default = "//flatware:adapter_wasm", allow_single_file = True),
        "control": attr.label(default = "//flatware:control_wasm", allow_single_file = True),
        "wasm_merge": attr.label(default = "@local_tools//:wasm-merge", allow_single_file = True, cfg = "exec"),
        "wasm2c": attr.label(default = "//third_party:wasm2c", allow_single_file = True, cfg = "exec"),
        "compile": attr.label(default = "//tools:compile", executable = True, cfg = "exec"),
        "flags": attr.label(default = "//tools:fix_copts", allow_single_file = True),
        "gcc": attr.label(default = "@local_tools//:gcc", allow_single_file = True, cfg = "exec"),
        "checker": attr.label(default = "//flatware:check_resources", executable = True, cfg = "exec"),
        "shell": attr.label(default = "//fix/shell:shell", allow_single_file = True),
        "wasm_rt": attr.label(default = "//fix/shell:wasm_rt", allow_single_file = True),
        "memmap": attr.label(default = "//fix/shell:memmap", allow_single_file = True),
        "headers": attr.label(default = "//fix/shell:headers"),
    }),
)

def flatware_run(name, program = None, **kwargs):
    """Create a launcher, adding compiler resources for uncompiled Wasm."""
    launcher = _native_run if program != None else _wasm_run
    launcher(name = name, program = program, **kwargs)
