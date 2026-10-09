"""Package native and WASIp1 LLVM tools built by LLVM's Bazel overlay."""

def _configuration_impl(settings, attr):
    flags = ["-ffile-prefix-map=.=/llvm-project"]
    return {
        "//command_line_option:compilation_mode": "dbg" if settings["//bootstrap:debug"] else "opt",
        "//command_line_option:platforms": ["//build/platforms:wasip1" if attr.wasi else "//build/platforms:host"],
        "//command_line_option:extra_toolchains": ["//bootstrap:wasi_cc"],
        "//command_line_option:features": ["-supports_start_end_lib"],
        "//command_line_option:host_features": ["-supports_start_end_lib"],
        "//command_line_option:copt": flags + (["-fPIC", "-fwasm-exceptions"] if attr.wasi else []),
        "//command_line_option:cxxopt": ["-std=c++17"],
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
    output = ctx.actions.declare_directory(ctx.label.name)
    inputs = []
    args = ctx.actions.args()
    args.add(output.path)
    for dep, destination in ctx.attr.files.items() + ctx.attr.native_files.items():
        for file in dep[DefaultInfo].files.to_list():
            path = destination + file.path.split("/staging/include/", 1)[1] if destination.endswith("/include/") else destination
            inputs.append(file)
            args.add_all([file.path, path])
    ctx.actions.run_shell(
        inputs = inputs,
        outputs = [output],
        arguments = [args],
        command = """
set -eu
output="$1"
shift
while (( $# )); do
    mkdir -p "$output/$(dirname "$2")"
    cp -L "$1" "$output/$2"
    shift 2
done
""",
        mnemonic = "BootstrapLlvmBundle",
    )
    return [DefaultInfo(files = depset([output]))]

llvm_bundle = rule(
    implementation = _bundle_impl,
    cfg = _configuration,
    attrs = {
        "files": attr.label_keyed_string_dict(allow_files = True),
        "native_files": attr.label_keyed_string_dict(allow_files = True, cfg = "exec"),
        "wasi": attr.bool(),
        "_allowlist_function_transition": attr.label(default = "@bazel_tools//tools/allowlists/function_transition_allowlist"),
    },
)

def _resources_impl(ctx):
    output = ctx.actions.declare_directory(ctx.label.name)
    ctx.actions.run_shell(
        inputs = [ctx.file.host, ctx.file.builtins],
        outputs = [output],
        arguments = [ctx.file.host.path, ctx.file.builtins.path, output.path],
        command = """
set -eu
mkdir -p "$3"
cp -R -L "$2/lib/clang/23/." "$3/"
cp -R -L "$1/lib/clang/23/include" "$3/"
""",
        mnemonic = "BootstrapLlvmResources",
    )
    return [DefaultInfo(files = depset([output]))]

llvm_resources = rule(
    implementation = _resources_impl,
    attrs = {
        "host": attr.label(allow_single_file = True),
        "builtins": attr.label(allow_single_file = True),
    },
)
