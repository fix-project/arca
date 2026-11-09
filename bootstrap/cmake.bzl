"""Bundles built with upstream CMake."""

load("@rules_foreign_cc//foreign_cc:defs.bzl", "cmake")
load(":configuration.bzl", "mode")

MAPS = "-ffile-prefix-map=$${EXT_BUILD_ROOT}=/llvm-project -ffile-prefix-map=$${BUILD_TMPDIR}=/build"

def _bundle_impl(ctx):
    return [DefaultInfo(files = ctx.attr.build[0][OutputGroupInfo].gen_dir)]

_bundle = rule(implementation = _bundle_impl, attrs = {
    "build": attr.label(cfg = mode),
    "_allowlist_function_transition": attr.label(default = "@bazel_tools//tools/allowlists/function_transition_allowlist"),
})

def source_build(name, lib_source, targets, options, working_directory = ".", copies = {}, sysroot = None, out_data_dirs = [], out_data_files = [], data = [], tags = []):
    host = "//bootstrap:host_llvm"
    data = data + [host] + ([sysroot] if sysroot else [])
    entries = dict({"CMAKE_C_FLAGS": MAPS, "CMAKE_CXX_FLAGS": MAPS}, **options)
    for key, tool in {"CMAKE_C_COMPILER": "clang", "CMAKE_CXX_COMPILER": "clang++", "CMAKE_ASM_COMPILER": "clang", "CMAKE_AR": "llvm-ar", "CMAKE_RANLIB": "llvm-ranlib"}.items():
        entries[key] = "$${EXT_BUILD_ROOT}/$(execpath %s)/bin/%s" % (host, tool)
    entries.setdefault("CMAKE_ASM_FLAGS", MAPS)
    for key in ["CMAKE_EXE_LINKER_FLAGS", "CMAKE_SHARED_LINKER_FLAGS", "CMAKE_MODULE_LINKER_FLAGS"]:
        entries.setdefault(key, "")
    env = {
        "LC_ALL": "C",
        "SOURCE_DATE_EPOCH": "0",
        "PATH": "$${EXT_BUILD_ROOT}/$$(dirname $(execpath @local_tools//:cmake)):$$PATH",
    }
    generate_args = []
    for key in ["CMAKE_C_FLAGS", "CMAKE_CXX_FLAGS", "CMAKE_ASM_FLAGS", "CMAKE_EXE_LINKER_FLAGS", "CMAKE_SHARED_LINKER_FLAGS", "CMAKE_MODULE_LINKER_FLAGS"]:
        env[key + "_OVERRIDE"] = entries.pop(key)
        generate_args.append('"-D%s=$%s_OVERRIDE"' % (key, key))
    cmake(
        name = name + "_cmake",
        lib_source = lib_source,
        working_directory = working_directory,
        data = data,
        build_data = ["@local_tools//:cmake", "@local_tools//:make", "@local_tools//:nproc"],
        generate_crosstool_file = False,
        cache_entries = select({
            "//bootstrap:debug_enabled": dict(entries, CMAKE_BUILD_TYPE = "Debug"),
            "//conditions:default": dict(entries, CMAKE_BUILD_TYPE = "Release"),
        }),
        generate_args = generate_args,
        targets = targets,
        install = False,
        build_args = ['--parallel "$${FIX_BOOTSTRAP_JOBS:-$$(nproc)}"'],
        env = env,
        postfix_script = "\n".join(['mkdir -p "$${INSTALLDIR}/%s"; cp "%s" "$${INSTALLDIR}/%s"' % (destination.rpartition("/")[0] or ".", source, destination) for source, destination in copies.items()]),
        out_include_dir = "",
        out_headers_only = True,
        out_data_dirs = out_data_dirs,
        out_data_files = out_data_files + list(copies.values()),
        tags = tags + ["no-remote-exec", "no-sandbox"],
    )
    _bundle(name = name, build = ":" + name + "_cmake", tags = tags)
