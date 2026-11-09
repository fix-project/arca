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

def wasm_tools(name, llvm = ":wasm_llvm", sysroot = ":wasi_sysroot", wasm2c = ":wasm2c", merge = "//tools:wasm_merge_wasm", **kwargs):
    sources = {sysroot: "usr", merge: "commands/wasm-merge.wasm"}
    for role, directory, path, destination in [
        ("clang_resources", llvm, "usr/lib/clang", "usr/lib/clang"),
        ("clang", llvm, "usr/bin/clang", "commands/clang.wasm"),
        ("ld", llvm, "usr/bin/ld.lld", "usr/bin/ld.lld"),
        ("wasm_ld", llvm, "usr/bin/wasm-ld", "usr/bin/wasm-ld"),
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
