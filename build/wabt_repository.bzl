"""Local WABT sources with runtime hooks."""

def _wabt_repository_impl(ctx):
    source = ctx.workspace_root.get_child("third_party/wabt")
    files = ["src/c-writer.cc", "src/template/wasm2c.top.h", "src/template/wasm2c.declarations.c", "src/template/wasm2c_simd.declarations.c"]
    directories = ["", "src", "src/template"]
    for name in files:
        ctx.file(name, ctx.read(source.get_child(name)), executable = False)
    for directory in directories:
        for child in source.get_child(directory).readdir():
            name = directory + "/" + child.basename if directory else child.basename
            if name not in files and name not in directories and name != ".git":
                ctx.symlink(child, name)
    ctx.patch(ctx.attr.patch, strip = 1)
    ctx.symlink(ctx.attr.build_file, "BUILD.bazel")

wabt_repository = repository_rule(
    implementation = _wabt_repository_impl,
    attrs = {
        "patch": attr.label(mandatory = True, allow_single_file = True),
        "build_file": attr.label(mandatory = True, allow_single_file = True),
    },
    local = True,
)
