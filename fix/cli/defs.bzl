"""Fix command environment."""

load("@rules_rust//rust:defs.bzl", "rust_binary")

def _commands(ctx):
    files = []
    for source in ctx.files.scripts + [ctx.executable.preprocess]:
        name = source.basename
        if source == ctx.executable.preprocess:
            name = "fix-preprocess"
        output = ctx.actions.declare_file(ctx.label.name + "/bin/" + name)
        ctx.actions.symlink(output = output, target_file = source)
        files.append(output)
    runfiles = ctx.runfiles(files = files + ctx.files.scripts)
    for tool in [ctx.attr.preprocess]:
        runfiles = runfiles.merge(tool[DefaultInfo].default_runfiles)
    return [DefaultInfo(files = depset([files[0]]), runfiles = runfiles)]

commands = rule(
    implementation = _commands,
    attrs = {
        "scripts": attr.label_list(allow_files = True),
        "preprocess": attr.label(default = ":preprocess", executable = True, cfg = "exec"),
    },
)

def launcher(name, scripts):
    commands(name = "commands", scripts = scripts)
    rust_binary(
        name = name,
        srcs = ["main.rs", "trust.rs"],
        edition = "2024",
        deps = ["@crates//:clap", "@crates//:env_logger", "//tools/vmm:lib"],
        compile_data = ["//fix/runtime:image_elf"],
        data = [":commands"],
        toolchains = ["//build:fix-jobs"],
        env = {"FIX_JOBS": "$(FIX_JOBS)"},
        rustc_env = {
            "FIX_KERNEL": "$(execpath //fix/runtime:image_elf)",
        },
    )
