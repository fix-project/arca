"""Merge, translate, compile, and link actions for each native generation."""

load(":configuration.bzl", "stage")
load(":tools.bzl", "COMMANDS")
load("@bazel_skylib//lib:shell.bzl", "shell")

# C translation units per program; the in-Fix rebuild must use the same count.
UNITS = 512

def _resources(_os, _inputs):
    return {"cpu": 1, "memory": 4096}

def _native_compile_resources(_os, _inputs):
    return {"cpu": 1, "memory": 1024}

def _invoke(ctx, inputs, staged, outputs, arguments, mnemonic, resource_set = _resources):
    args = ctx.actions.args()
    args.add_all(arguments)
    directories = {name.rsplit("/", 1)[0]: None for _, name in staged if "/" in name}
    copies = ['mkdir -p "$work"/%s' % shell.quote(name) for name in sorted(directories)]
    copies += ['ln -s "$execroot"/%s "$work"/%s' % (shell.quote(source), shell.quote(name)) for source, name in staged]
    exports = ['cp "$work"/%s "$execroot"/%s' % (shell.quote(file.basename), shell.quote(file.path)) for file in outputs]
    script = ctx.actions.declare_file(ctx.label.name + "/" + outputs[0].basename + ".sh")
    ctx.actions.write(
        script,
        """#!/bin/bash
set -euo pipefail
execroot=$PWD
tool="$execroot/"%s
usr="$execroot/"%s/usr
tool_dir=$(dirname "$tool")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
%s
args=()
for arg in "$@"; do
    arg=${arg//\\{work\\}/$work}
    arg=${arg//\\{usr\\}/$usr}
    args+=("${arg//\\{tool_dir\\}/$tool_dir}")
done
(cd "$work"; env -i PATH=/usr/bin:/bin LC_ALL=C SOURCE_DATE_EPOCH=0 BINARYEN_CORES=1 "$tool" "${args[@]}")
%s
""" % (shell.quote(ctx.file.tool.path + ctx.attr.tool_path), shell.quote(ctx.file.wasm.path), "\n".join(copies), "\n".join(exports)),
        is_executable = True,
    )
    ctx.actions.run(
        executable = script,
        arguments = [args],
        inputs = inputs + [ctx.file.tool, ctx.file.wasm],
        outputs = outputs,
        mnemonic = mnemonic,
        resource_set = resource_set,
        execution_requirements = {"no-remote-exec": "1", "no-sandbox": "1"},
    )

_EXECUTION = {
    "tool": attr.label(mandatory = True, allow_single_file = True),
    "tool_path": attr.string(),
    "wasm": attr.label(default = "//bootstrap:stage0", allow_single_file = True),
}

def _merge_impl(ctx):
    output = ctx.actions.declare_file(ctx.attr.output)
    merged = ctx.actions.declare_file(ctx.label.name + "/module.wasm")
    rootfs = ctx.file.wasm
    staged = [(rootfs.path + "/" + ctx.attr.command_path, "guest.wasm"), (ctx.file.adapter.path, "adapter.wasm"), (ctx.file.control.path, "control.wasm")]
    _invoke(ctx, [ctx.file.adapter, ctx.file.control], staged, [merged], ["{work}/adapter.wasm", "wasi_snapshot_preview1", "{work}/guest.wasm", "flatware_guest", "{work}/control.wasm", "flatware_control", "--all-features", "--disable-gc", "--rename-export-conflicts", "-o", "{work}/module.wasm"], "BootstrapMerge")
    ctx.actions.run(executable = ctx.executable.checker, arguments = [merged.path, output.path], inputs = [merged], outputs = [output], mnemonic = "BootstrapCheck")
    return [DefaultInfo(files = depset([output]))]

merge = rule(
    implementation = _merge_impl,
    attrs = dict(
        _EXECUTION,
        command_path = attr.string(mandatory = True),
        output = attr.string(),
        adapter = attr.label(default = "//flatware:adapter_wasm", allow_single_file = True),
        control = attr.label(default = "//flatware:control_wasm", allow_single_file = True),
        checker = attr.label(default = "//flatware:check_resources", executable = True, cfg = "exec"),
    ),
)

def _translate_impl(ctx):
    count = ctx.attr.partitions
    if count < 1:
        fail("Partition count must be positive")
    prefix = ctx.attr.directory + "/"
    sources = [ctx.actions.declare_file(prefix + ("module.c" if count == 1 else "module_%d.c" % i)) for i in range(count)]
    headers = [ctx.actions.declare_file(prefix + name) for name in (["module.h"] if count == 1 else ["module.h", "module-impl.h"])]
    _invoke(ctx, [ctx.file.src], [(ctx.file.src.path, "module.wasm")], sources + headers, ["--enable-threads", "--enable-custom-page-sizes", "--enable-compact-imports", "-n", "module", "--num-outputs=" + str(count), "{work}/module.wasm", "-o", "{work}/module.c"], "BootstrapTranslate")
    return [DefaultInfo(files = depset(sources + headers)), OutputGroupInfo(sources = depset(sources), headers = depset(headers))]

translate = rule(
    implementation = _translate_impl,
    attrs = dict(
        _EXECUTION,
        src = attr.label(allow_single_file = True),
        partitions = attr.int(default = UNITS),
        directory = attr.string(),
    ),
)

CFLAGS = [
    "--target=x86_64-unknown-linux-gnu",
    "-O2",
    "-fno-optimize-sibling-calls",
    "-frounding-math",
    "-ffreestanding",
    "-nostdlib",
    "-nostdinc",
    "-isystem",
    "{usr}/lib/clang/23/include",
    "-isystem",
    "{usr}/include/wasm32-wasip1",
    "-isystem",
    "{usr}/include",
    "-mcmodel=small",
    "-fPIC",
    "-fvisibility=hidden",
    "-mno-red-zone",
    "-march=x86-64-v3",
    "-static",
    "-resource-dir={usr}/lib/clang/23",
    "-no-canonical-prefixes",
]

_NATIVE_FLAGS = CFLAGS + [
    "-ffile-prefix-map={work}=/work",
    "-ffile-prefix-map={work}/toolchain=/toolchain",
    "-ffile-prefix-map={usr}=/toolchain/usr",
    "-I{work}/toolchain/share/fix",
]

def _compile_impl(ctx):
    translation = ctx.attr.translation[OutputGroupInfo]
    generated = translation.sources.to_list() + translation.headers.to_list()
    sources = translation.sources.to_list() + [ctx.file.runtime]
    headers = translation.headers.to_list() + ctx.files.headers
    def location(file):
        if file in generated:
            return file.basename
        return "toolchain/share/fix/" + file.basename
    objects = []
    for i, source in enumerate(sources):
        output = ctx.actions.declare_file(ctx.attr.directory + "/object_%d.o" % i)
        staged = [(file.path, location(file)) for file in [source] + headers]
        flags = _NATIVE_FLAGS + (["-O3"] if source == ctx.file.runtime else [])
        _invoke(ctx, [source] + headers, staged, [output], flags + ["-I{work}/.", "-c", "{work}/" + location(source), "-o", "{work}/" + output.basename], "BootstrapCompile", resource_set = _native_compile_resources)
        objects.append(output)
    return [DefaultInfo(files = depset(objects))]

compile = rule(
    implementation = _compile_impl,
    attrs = dict(
        _EXECUTION,
        translation = attr.label(providers = [OutputGroupInfo]),
        directory = attr.string(),
        runtime = attr.label(default = "//fix/shell:wasm_rt", allow_single_file = True),
        headers = attr.label(default = "//fix/shell:headers"),
    ),
)

def _link_impl(ctx):
    output = ctx.actions.declare_file(ctx.attr.output)
    staged = [(file.path, file.basename) for file in ctx.files.objects] + [(ctx.file.shell.path, "libfixshell.a"), (ctx.file.memmap.path, "memmap.ld")]
    arguments = _NATIVE_FLAGS + ["-fuse-ld={tool_dir}/ld.lld", "-Wl,--build-id=none", "-Wl,--threads=1", "-Wl,-T,{work}/memmap.ld", "-o", "{work}/" + output.basename] + ["{work}/" + file.basename for file in ctx.files.objects] + ["{work}/libfixshell.a"]
    _invoke(ctx, ctx.files.objects + [ctx.file.shell, ctx.file.memmap], staged, [output], arguments, "BootstrapLink")
    return [DefaultInfo(files = depset([output]))]

link = rule(
    implementation = _link_impl,
    attrs = dict(
        _EXECUTION,
        objects = attr.label(),
        output = attr.string(),
        shell = attr.label(default = "//fix/shell:shell", allow_single_file = True),
        memmap = attr.label(default = "//fix/shell:memmap", allow_single_file = True),
    ),
)

def program(name, src, directory, partitions = UNITS, output = None):
    tools = {
        "clang": {"tool": ":host_llvm", "tool_path": "/bin/clang"},
        "wasm2c": {"tool": ":host_wasm2c", "tool_path": "/bin/wasm2c"},
    }
    translate(name = name + "_translate", src = src, directory = directory, partitions = partitions, tags = ["manual"], **tools["wasm2c"])
    compile(name = name + "_compile", translation = ":" + name + "_translate", directory = directory, tags = ["manual"], **tools["clang"])
    link(name = name + "_link", objects = ":" + name + "_compile", output = output or directory + "/program.elf", tags = ["manual"], **tools["clang"])

def generation(name):
    outputs = []
    for command, command_path in COMMANDS.items():
        prefix = name + "_" + command
        directory = name + "/" + command
        merge(name = prefix + "_merge", command_path = command_path, output = directory + "/module.wasm", tags = ["manual"], tool = "//tools:wasm_merge")
        program(name = prefix, src = ":" + prefix + "_merge", directory = directory, output = name + "/commands/" + command + ".elf")
        outputs.append(":" + prefix + "_link")
    stage(name = name, srcs = outputs + [":stage0", "//fix/shell:shell", "//fix/shell:wasm_rt", "//fix/shell:headers", "//fix/shell:memmap", "//flatware:adapter_wasm", "//flatware:control_wasm"], tags = ["manual"])
