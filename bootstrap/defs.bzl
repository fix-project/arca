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
