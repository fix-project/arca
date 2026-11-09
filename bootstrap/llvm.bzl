"""Package native and WASIp1 LLVM tools built by LLVM's Bazel overlay."""

load("@bazel_lib//lib:directory_path.bzl", "directory_path")
load("@bazel_lib//lib:paths.bzl", "to_repository_relative_path")
load(":files.bzl", "COPY_TOOLCHAIN", "mapped_directory", "mapped_directory_action")

def _configuration_impl(settings, attr):
    flags = ["-ffile-prefix-map=.=/llvm-project"]
    return {
        "//command_line_option:compilation_mode": "dbg" if settings["//bootstrap:debug"] else "opt",
        "//command_line_option:platforms": ["//build/platforms:wasip1" if attr.wasi else "//build/platforms:host"],
        "//command_line_option:extra_toolchains": ["//bootstrap:wasi_cc"],
        "//command_line_option:features": ["-supports_start_end_lib"],
        "//command_line_option:host_features": ["-supports_start_end_lib"],
        "//command_line_option:copt": flags + (["-fPIC", "-fwasm-exceptions"] if attr.wasi else []),
        "//command_line_option:cxxopt": ["-std=c++17"] + (["-fno-exceptions"] if attr.wasi else []),
        "//command_line_option:host_copt": flags,
        "//command_line_option:host_cxxopt": ["-std=c++17"],
        "//command_line_option:linkopt": ["-fwasm-exceptions", "-Wl,-z,stack-size=8388608", "--no-wasm-opt"] if attr.wasi else ["-fuse-ld=bfd"],
        "//command_line_option:host_linkopt": ["-fuse-ld=bfd"],
        "@llvm-project//clang:enable_static_analyzer": False,
        "@llvm-project//clang:enable_objc_rewriter": False,
        "@llvm-project//third-party:llvm_enable_zlib": False,
        "@llvm-project//third-party:llvm_enable_zstd": False,
        "@llvm-project//llvm:pfm": "disable",
    }

_configuration = transition(
    implementation = _configuration_impl,
    inputs = ["//bootstrap:debug"],
    outputs = [
        "//command_line_option:compilation_mode",
        "//command_line_option:platforms",
        "//command_line_option:extra_toolchains",
        "//command_line_option:features",
        "//command_line_option:host_features",
        "//command_line_option:copt",
        "//command_line_option:cxxopt",
        "//command_line_option:host_copt",
        "//command_line_option:host_cxxopt",
        "//command_line_option:linkopt",
        "//command_line_option:host_linkopt",
        "@llvm-project//clang:enable_static_analyzer",
        "@llvm-project//clang:enable_objc_rewriter",
        "@llvm-project//third-party:llvm_enable_zlib",
        "@llvm-project//third-party:llvm_enable_zstd",
        "@llvm-project//llvm:pfm",
    ],
)

def _bundle_impl(ctx):
    files = []
    prefixes = {}
    for dep, destination in ctx.attr.files.items() + ctx.attr.native_files.items():
        for file in dep[DefaultInfo].files.to_list():
            path = destination + file.path.split("/staging/include/", 1)[1] if destination.endswith("/include/") else destination
            files.append(file)
            prefixes[to_repository_relative_path(file)] = path
    output = mapped_directory_action(ctx, files, prefixes)
    return [DefaultInfo(files = depset([output]))]

llvm_bundle = rule(
    implementation = _bundle_impl,
    cfg = _configuration,
    toolchains = [COPY_TOOLCHAIN],
    attrs = {
        "files": attr.label_keyed_string_dict(allow_files = True),
        "native_files": attr.label_keyed_string_dict(allow_files = True, cfg = "exec"),
        "wasi": attr.bool(),
        "_allowlist_function_transition": attr.label(default = "@bazel_tools//tools/allowlists/function_transition_allowlist"),
    },
)

def llvm_resources(name, host, builtins, **kwargs):
    directory_path(name = name + "_builtins", directory = builtins, path = "lib/clang/23", tags = ["manual"])
    directory_path(name = name + "_include", directory = host, path = "lib/clang/23/include", tags = ["manual"])
    mapped_directory(
        name = name,
        srcs = {":" + name + "_builtins": ".", ":" + name + "_include": "include"},
        **kwargs
    )
