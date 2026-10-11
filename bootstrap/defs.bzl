"""Bootstrap tool filesystem."""

load(":configuration.bzl", "ALLOW_TRANSITION", "mode")

def _assemble_impl(ctx):
    output = ctx.actions.declare_directory(ctx.label.name)
    inputs = [ctx.file.llvm, ctx.file.sysroot, ctx.file.wasm2c, ctx.file.merge]
    ctx.actions.run_shell(
        inputs = inputs,
        outputs = [output],
        arguments = [f.path for f in inputs] + [output.path],
        command = """
set -euo pipefail
mkdir -p "$5/usr/bin" "$5/commands"
cp -R -L "$2/." "$5/usr/"
chmod -R u+w "$5/usr"
cp -R -L "$1/usr/lib/clang/." "$5/usr/lib/clang/"
cp "$1/usr/bin/clang" "$5/commands/clang.wasm"
cp "$1/usr/bin/ld.lld" "$1/usr/bin/wasm-ld" "$5/usr/bin/"
cp "$3/wasm2c.wasm" "$5/commands/wasm2c.wasm"
cp "$4" "$5/commands/wasm-merge.wasm"
""",
        mnemonic = "BootstrapWasmTools",
    )
    return [DefaultInfo(files = depset([output]))]

wasm_tools = rule(implementation = _assemble_impl, cfg = mode, attrs = dict(ALLOW_TRANSITION, **{
    "llvm": attr.label(default = "//bootstrap:wasm_llvm", allow_single_file = True),
    "sysroot": attr.label(default = "//bootstrap:wasi_sysroot", allow_single_file = True),
    "wasm2c": attr.label(default = "//bootstrap:wasm2c", allow_single_file = True),
    "merge": attr.label(default = "//tools:wasm_merge_wasm", allow_single_file = True),
}))

def _repository_archive(ctx):
    output = ctx.actions.declare_file("bootstrap.tar.gz")
    inputs = [ctx.file.stage0, ctx.file.clang, ctx.file.wasm2c, ctx.file.merger,
              ctx.file.json, ctx.file.parser, ctx.file.identity, ctx.file.shell, ctx.file.runtime,
              ctx.file.memmap, ctx.file.adapter, ctx.file.control, ctx.file.path,
              ctx.file.cpio, ctx.file.normalize, ctx.file.compose]
    ctx.actions.run_shell(
        command = 'export PATH="$PWD/' + ctx.executable.fix.dirname + '/commands/bin:$PATH"; exec "$@"',
        arguments = [ctx.executable.builder.path, ctx.executable.fix.path] + [file.path for file in inputs] + [output.path] + [file.path for file in ctx.files.headers],
        inputs = inputs + ctx.files.headers,
        tools = [ctx.attr.fix[DefaultInfo].files_to_run, ctx.attr.builder[DefaultInfo].files_to_run],
        outputs = [output],
        mnemonic = "BootstrapRepository",
        use_default_shell_env = True,
        execution_requirements = {"local": "1"},
    )
    return [DefaultInfo(files = depset([output]))]

repository_archive = rule(
    implementation = _repository_archive,
    cfg = mode,
    attrs = {
        "builder": attr.label(default = "//bootstrap:repository.sh", allow_single_file = True, executable = True, cfg = "exec"),
        "fix": attr.label(default = "//fix", executable = True, cfg = "target"),
        "stage0": attr.label(default = "//bootstrap:stage0", allow_single_file = True),
        "clang": attr.label(default = "//bootstrap:stage1_clang_link", allow_single_file = True),
        "wasm2c": attr.label(default = "//bootstrap:stage1_wasm2c_link", allow_single_file = True),
        "merger": attr.label(default = "//bootstrap:stage1_wasm-merge_link", allow_single_file = True),
        "json": attr.label(default = "//programs/fix/json:json_elf", allow_single_file = True),
        "cpio": attr.label(default = "//flatware:cpio_elf", allow_single_file = True),
        "normalize": attr.label(default = "//flatware:normalize_elf", allow_single_file = True),
        "compose": attr.label(default = "//programs/fix:compose_elf", allow_single_file = True),
        "path": attr.label(default = "//programs/fix/rust:path_elf", allow_single_file = True),
        "parser": attr.label(default = "//programs/fix/parser:parser_elf", allow_single_file = True),
        "identity": attr.label(default = "//programs/fix:identity_elf", allow_single_file = True),
        "shell": attr.label(default = "//fix/shell:shell", allow_single_file = True),
        "runtime": attr.label(default = "//fix/shell:wasm_rt", allow_single_file = True),
        "memmap": attr.label(default = "//fix/shell:memmap", allow_single_file = True),
        "adapter": attr.label(default = "//flatware:adapter_wasm", allow_single_file = True),
        "control": attr.label(default = "//flatware:control_wasm", allow_single_file = True),
        "headers": attr.label(default = "//fix/shell:headers", allow_files = True),
        "_allowlist_function_transition": ALLOW_TRANSITION["_allowlist_function_transition"],
    },
)
