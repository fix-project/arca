"""Explicit C-family compilation actions for freestanding builds and tooling."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")

def _cc_object_impl(ctx):
    output = ctx.actions.declare_file(ctx.label.name + ".o")
    arguments = ctx.actions.args()
    arguments.add_all(ctx.attr.copts)
    arguments.add_all(["-I" + file.dirname for file in ctx.files.include_roots])
    headers = []
    for dep in ctx.attr.deps:
        context = dep[CcInfo].compilation_context
        headers.append(context.headers)
        arguments.add_all(context.includes, format_each = "-I%s")
        arguments.add_all(context.quote_includes, before_each = "-iquote")
        arguments.add_all(context.system_includes, before_each = "-isystem")
        arguments.add_all(context.defines, format_each = "-D%s")
    arguments.add_all(["-c", ctx.file.src.path, "-o", output.path])
    ctx.actions.run(
        executable = ctx.executable.compiler,
        arguments = [arguments],
        inputs = depset([ctx.file.src] + ctx.files.hdrs + ctx.files.include_roots, transitive = headers),
        outputs = [output],
        mnemonic = "CppCompile",
        progress_message = "Compiling " + ctx.file.src.short_path,
    )
    return [DefaultInfo(files = depset([output]))]

cc_object = rule(
    implementation = _cc_object_impl,
    attrs = {
        "src": attr.label(allow_single_file = True, mandatory = True),
        "hdrs": attr.label_list(allow_files = True),
        "deps": attr.label_list(providers = [CcInfo]),
        "include_roots": attr.label_list(allow_files = True),
        "copts": attr.string_list(),
        "compiler": attr.label(allow_files = True, executable = True, cfg = "exec", default = "@local_tools//:gcc"),
    },
)
