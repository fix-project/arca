"""The Linux-issued seed repository."""

load(":configuration.bzl", "ALLOW_TRANSITION", "mode")
load("//build:fix.bzl", "FixRunInfo")

def _seed_impl(ctx):
    archive = ctx.actions.declare_file(ctx.label.name + ".tar")
    programs = {name: target.files.to_list()[0] for target, name in ctx.attr.programs.items()}
    blobs = {name: target.files.to_list()[0] for target, name in ctx.attr.blobs.items()}
    directories = {name: target.files.to_list()[0] for target, name in ctx.attr.directories.items()}
    tool = ctx.attr._tool[DefaultInfo].files_to_run
    inputs = [ctx.file.compiler, ctx.file.wasm2c, ctx.file.clang, ctx.file.toolchain]
    arguments = ["create", "$work"] + [file.path for file in inputs] + [name + "=" + file.path for name, file in programs.items()]
    for name, file in blobs.items():
        arguments.extend(["--blob", name + "=" + file.path])
    for name, file in directories.items():
        arguments.extend(["--directory", name + "=" + file.path])
    ctx.actions.run_shell(
        command = 'set -euo pipefail\nwork=$(mktemp -d)\ntrap \'rm -rf "$work"\' EXIT\n"$1" %s\ntar --create --file "$2" --mtime=@0 --owner=0 --group=0 --numeric-owner --sort=name --directory "$work" .fix\n' % " ".join(['"%s"' % argument for argument in arguments]),
        arguments = [tool.executable.path, archive.path],
        inputs = inputs + programs.values() + blobs.values() + directories.values(),
        tools = [tool],
        outputs = [archive],
        mnemonic = "BootstrapSeed",
    )
    return [DefaultInfo(files = depset([archive]))]

seed_repository = rule(
    implementation = _seed_impl,
    cfg = mode,
    attrs = dict(ALLOW_TRANSITION, **{
        "compiler": attr.label(mandatory = True, allow_single_file = True),
        "wasm2c": attr.label(mandatory = True, allow_single_file = True),
        "clang": attr.label(mandatory = True, allow_single_file = True),
        "toolchain": attr.label(mandatory = True, allow_single_file = True),
        "programs": attr.label_keyed_string_dict(allow_files = True),
        "blobs": attr.label_keyed_string_dict(allow_files = True),
        "directories": attr.label_keyed_string_dict(allow_files = True),
        "_tool": attr.label(default = "//bootstrap:seed_tool", executable = True, cfg = "exec"),
    }),
)

def _promote_seed_impl(ctx):
    archive = ctx.actions.declare_file(ctx.label.name + ".tar")
    compilation = ctx.attr.compilation[FixRunInfo]
    ctx.actions.run_shell(
        command = """set -euo pipefail
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
execroot=$PWD
cd "$work"
export FIX_UNSIGNED=true
"$execroot/$1" init
"$execroot/$1" import "$execroot/$2"
"$execroot/$1" import "$execroot/$3"
"$execroot/$4" promote "$work" "$execroot/$5"
tar --create --file "$execroot/$6" --mtime=@0 --owner=0 --group=0 --numeric-owner --sort=name .fix
""",
        arguments = [
            ctx.file.fix.path,
            ctx.file.seed.path,
            compilation.archive.path,
            ctx.executable._tool.path,
            compilation.handle.path,
            archive.path,
        ],
        inputs = [ctx.file.seed, compilation.archive, compilation.handle],
        tools = [ctx.file.fix, ctx.executable._tool],
        outputs = [archive],
        mnemonic = "PromoteBootstrap",
    )
    return [DefaultInfo(files = depset([archive]))]

promote_seed = rule(
    implementation = _promote_seed_impl,
    attrs = {
        "seed": attr.label(mandatory = True, allow_single_file = [".tar"]),
        "compilation": attr.label(mandatory = True, providers = [FixRunInfo]),
        "fix": attr.label(default = "//fix/cli:fix", allow_single_file = True),
        "_tool": attr.label(default = "//bootstrap:seed_tool", executable = True, cfg = "exec"),
    },
)
