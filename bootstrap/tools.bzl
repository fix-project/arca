"""LLVM utilities and the programs carried by the bootstrap seed."""

COMMANDS = {
    "clang": "commands/clang.wasm",
    "lld": "usr/bin/ld.lld",
    "wasm2c": "commands/wasm2c.wasm",
    "wasm-merge": "commands/wasm-merge.wasm",
}

SEED_MODULES = {
    "compile": "//programs/fix:compile_module",
    "scheduler": "//programs/fix:scheduler_module",
    "collector": "//fix/proof/collector:module",
} | {name: ":stage1_" + name + "_merge" for name in COMMANDS}

def _compilation_script(ctx):
    lines = ["units = %du64" % ctx.attr.units, "compile = $compile"]
    values = {}
    for index, name in enumerate(ctx.attr.modules):
        value = "rebuilt%d" % index
        values[name] = value
        lines.append('%s = *#("" compile @file(%s) units)' % (value, json.encode(name + ".wasm")))
    closure = '("" %s *[compile 2u64])' % values["compile"]
    entries = ["(%s %s)" % (json.encode(name), closure if name == "compile" else values[name]) for name in ctx.attr.modules]
    lines.append("(" + " ".join(entries) + ")")
    output = ctx.actions.declare_file(ctx.label.name + ".fix")
    ctx.actions.write(output, "\n".join(lines) + "\n")
    return [DefaultInfo(files = depset([output]))]

compilation_script = rule(
    implementation = _compilation_script,
    attrs = {
        "modules": attr.string_list(mandatory = True),
        "units": attr.int(mandatory = True),
    },
)
