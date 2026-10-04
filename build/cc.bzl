"""Explicit C-family compilation actions for freestanding builds and tooling."""

def _cc_object_impl(ctx):
    output = ctx.actions.declare_file(ctx.label.name + ".o")
    arguments = ctx.actions.args()
    arguments.add_all(ctx.attr.copts)
    arguments.add_all(["-I" + file.dirname for file in ctx.files.include_roots])
    arguments.add_all(["-c", ctx.file.src.path, "-o", output.path])
    ctx.actions.run(
        executable = ctx.executable.compiler,
        arguments = [arguments],
        inputs = depset([ctx.file.src] + ctx.files.hdrs + ctx.files.include_roots),
        outputs = [output],
        mnemonic = "CppCompile",
        progress_message = "Compiling %{input}",
    )
    return [DefaultInfo(files = depset([output]))]

cc_object = rule(
    implementation = _cc_object_impl,
    attrs = {
        "src": attr.label(allow_single_file = True, mandatory = True),
        "hdrs": attr.label_list(allow_files = True),
        "include_roots": attr.label_list(allow_files = True),
        "copts": attr.string_list(),
        "compiler": attr.label(allow_files = True, executable = True, cfg = "exec", default = "@local_tools//:gcc"),
    },
)
