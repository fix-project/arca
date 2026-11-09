"""Build Fix repositories and evaluate Fix expressions against them."""

load("@bazel_skylib//lib:shell.bzl", "shell")
load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")

FixRunInfo = provider(fields = ["archive", "handle"])

def _resources(ctx):
    jobs = ctx.attr.jobs or ctx.attr._jobs[BuildSettingInfo].value
    memory = ctx.attr.memory
    if not memory:
        memory = 16384
        for _ in range(32):
            if memory < jobs * 1024:
                memory *= 2
    return jobs, memory

def _run(ctx, commands, inputs, outputs, mnemonic, jobs, memory):
    ctx.actions.run_shell(
        command = "\n".join([
            "set -euo pipefail",
            "execroot=$PWD",
            'fix="$execroot"/%s' % shell.quote(ctx.file.fix.path),
            "work=$(mktemp -d)",
            "trap 'rm -rf \"$work\"' EXIT",
            'cd "$work"',
        ] + commands),
        inputs = inputs + [ctx.file.fix],
        outputs = outputs,
        mnemonic = mnemonic,
        execution_requirements = {
            "no-sandbox": "1",
            "no-remote-exec": "1",
            "resources:fix:1": "",
            "resources:cpu:%d" % jobs: "",
            "resources:memory:%d" % memory: "",
        },
    )

def _archive(path, excludes = ""):
    return 'tar --create --file "$execroot"/%s --mtime=@0 --owner=0 --group=0 --numeric-owner --sort=name %s.fix' % (shell.quote(path), excludes)

def _fix_run_impl(ctx):
    archive = ctx.actions.declare_file(ctx.label.name + ".tar")
    handle = ctx.actions.declare_file(ctx.label.name + ".handle")
    jobs, memory = _resources(ctx)
    commands = ['"$fix" init']
    archives = [file for target in ctx.attr.archives for file in target[DefaultInfo].files.to_list()]
    for file in archives:
        commands.append('"$fix" import "$execroot"/%s' % shell.quote(file.path))
    inputs = []
    for target, name in ctx.attr.data.items():
        file = target[DefaultInfo].files.to_list()[0]
        commands.append('cp "$execroot"/%s %s' % (shell.quote(file.path), shell.quote(name)))
        inputs.append(file)
    bindings = " ".join(["-b " + shell.quote(name + "=" + expression) for name, expression in ctx.attr.bindings.items()])
    commands += [
        'export FIX_TRUST="$work/.fix/trust"',
        '"$fix" -j %d --memory %d %s eval %s > "$execroot"/%s' % (jobs, memory, bindings, shell.quote(ctx.attr.expression), shell.quote(handle.path)),
        _archive(archive.path, "--exclude=.fix/labels "),
    ]
    _run(ctx, commands, inputs + archives, [archive, handle], "FixRun", jobs, memory)
    return [
        DefaultInfo(files = depset([archive])),
        OutputGroupInfo(handle = depset([handle])),
        FixRunInfo(archive = archive, handle = handle),
    ]

fix_run = rule(
    doc = "Imports `archives`, evaluates `expression` with `bindings` (as `fix eval`), and archives the repository without labels. The `handle` output group holds the result handle.",
    implementation = _fix_run_impl,
    attrs = {
        "expression": attr.string(mandatory = True),
        "bindings": attr.string_dict(),
        "data": attr.label_keyed_string_dict(allow_files = True, doc = "Files copied into the working directory under the given names."),
        "archives": attr.label_list(allow_files = [".tar"], doc = "Repository archives to import."),
        "jobs": attr.int(default = 0),
        "memory": attr.int(default = 0, doc = "VM memory in MiB; 0 means 1 GiB per job, at least 16 GiB, rounded up to a power of two."),
        "_jobs": attr.label(default = "//build:fix-jobs"),
        "fix": attr.label(default = "//fix/cli:fix", allow_single_file = True, doc = "The Fix CLI that evaluates."),
    },
)

def _fix_repository_impl(ctx):
    archive = ctx.actions.declare_file(ctx.label.name + ".tar")
    jobs, memory = _resources(ctx)
    commands = [
        '"$fix" -j %d --memory %d init "$execroot"/%s --precompile' % (jobs, memory, shell.quote(ctx.file.seed.path)),
        _archive(archive.path),
    ]
    _run(ctx, commands, [ctx.file.seed], [archive], "FixRepository", jobs, memory)
    return [DefaultInfo(files = depset([archive]))]

fix_repository = rule(
    doc = "Archives a repository made by `fix init SEED --precompile`.",
    implementation = _fix_repository_impl,
    attrs = {
        "seed": attr.label(default = "//bootstrap:seed", allow_single_file = [".tar"]),
        "jobs": attr.int(default = 0),
        "memory": attr.int(default = 0, doc = "VM memory in MiB; 0 means 1 GiB per job, at least 16 GiB, rounded up to a power of two."),
        "_jobs": attr.label(default = "//build:fix-jobs"),
        "fix": attr.label(default = "//fix/cli:fix", allow_single_file = True, doc = "The Fix CLI that evaluates."),
    },
)
