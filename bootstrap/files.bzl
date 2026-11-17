"""Bootstrap tool filesystems."""

load("@bazel_lib//lib:copy_to_directory.bzl", "copy_to_directory_bin_action")
load("@bazel_lib//lib:directory_path.bzl", "DirectoryPathInfo", "directory_path")
load("@bazel_lib//lib:paths.bzl", "to_repository_relative_path")
load(":configuration.bzl", "ALLOW_TRANSITION", "mode")

COPY_TOOLCHAIN = "@bazel_lib//lib:copy_to_directory_toolchain_type"

def mapped_directory_action(ctx, files, prefixes, targets = [], allow_overwrites = False):
    output = ctx.actions.declare_directory(ctx.label.name)
    copy_to_directory_bin_action(
        ctx,
        name = ctx.label.name,
        dst = output,
        copy_to_directory_bin = ctx.toolchains[COPY_TOOLCHAIN].copy_to_directory_info.bin,
        files = files,
        targets = targets,
        root_paths = [],
        include_external_repositories = ["**"],
        replace_prefixes = prefixes,
        hardlink = "auto",
        allow_overwrites = allow_overwrites,
    )
    return output

def _mapped_directory_impl(ctx):
    files = []
    targets = []
    prefixes = {}
    for target, destination in ctx.attr.srcs.items():
        if DirectoryPathInfo in target:
            source = target[DirectoryPathInfo]
            prefixes[to_repository_relative_path(source.directory) + "/" + source.path] = destination
            targets.append(target)
        else:
            for source in target[DefaultInfo].files.to_list():
                prefixes[to_repository_relative_path(source)] = destination + source.basename if destination.endswith("/") else destination
                files.append(source)
    output = mapped_directory_action(ctx, files, prefixes, targets, ctx.attr.allow_overwrites)
    return [DefaultInfo(files = depset([output]))]

mapped_directory = rule(
    implementation = _mapped_directory_impl,
    cfg = mode,
    attrs = dict(ALLOW_TRANSITION, **{
        "srcs": attr.label_keyed_string_dict(allow_files = True),
        "allow_overwrites": attr.bool(),
    }),
    toolchains = [COPY_TOOLCHAIN],
)

def _archive_directory_impl(ctx):
    files = ctx.attr.src[DefaultInfo].files.to_list()
    root = ctx.attr.src.label.workspace_root + "/" + ctx.attr.strip_prefix
    paths = []
    for file in files:
        if not file.path.startswith(root):
            fail("archive file is outside its repository: " + file.path)
        paths.append(file.path[len(root):])
    manifest = ctx.actions.declare_file(ctx.label.name + ".manifest")
    ctx.actions.write(manifest, "\n".join(sorted(paths)) + "\n")
    output = ctx.actions.declare_directory(ctx.label.name)
    ctx.actions.run_shell(
        inputs = files + [manifest],
        outputs = [output],
        command = 'set -euo pipefail; mkdir -p "$1"; tar -C "$2" -cf - -T "$3" | tar -C "$1" -xf -',
        arguments = [output.path, root, manifest.path],
        mnemonic = "StageRelease",
    )
    return [DefaultInfo(files = depset([output]))]

archive_directory = rule(
    implementation = _archive_directory_impl,
    attrs = {"src": attr.label(mandatory = True), "strip_prefix": attr.string()},
)


def wasm_tools(name, rootfs = ":release_rootfs", wasm2c = ":wasm2c", merge = "//tools:wasm_merge_wasm", **kwargs):
    sources = {rootfs: ".", merge: "commands/wasm-merge.wasm"}
    for role, directory, path, destination in [
        ("clang", rootfs, "usr/bin/clang", "commands/clang.wasm"),
        ("wasm2c", wasm2c, "wasm2c.wasm", "commands/wasm2c.wasm"),
    ]:
        target = name + "_" + role
        directory_path(name = target, directory = directory, path = path, tags = ["manual"])
        sources[":" + target] = destination
    mapped_directory(name = name, srcs = sources, allow_overwrites = True, **kwargs)

def toolchain_files(name, stage0, resources, **kwargs):
    directory_path(name = name + "_usr", directory = stage0, path = "usr", tags = ["manual"])
    sources = {":" + name + "_usr": "usr"}
    sources.update(resources)
    mapped_directory(name = name, srcs = sources, **kwargs)
