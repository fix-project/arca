"""Build LLVM's freestanding C++ exception runtime."""

load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")

def _cpp_runtime_impl(ctx):
    abi = ctx.actions.declare_file(ctx.label.name + "/libc++abi.a")
    unwind = ctx.actions.declare_file(ctx.label.name + "/libunwind.a")
    headers = ctx.actions.declare_directory(ctx.label.name + "/headers")
    tools = [ctx.file._cmake, ctx.file._clang, ctx.file._make, ctx.file._ar, ctx.file._ranlib]
    ctx.actions.run_shell(
        inputs = depset(ctx.files.srcs + [ctx.file.root]),
        tools = tools,
        outputs = [abi, unwind, headers],
        arguments = [ctx.file.root.dirname, abi.path, unwind.path, headers.path] + [tool.path for tool in tools],
        command = """
set -euo pipefail
root="$PWD/$1"
abi="$PWD/$2"
unwind="$PWD/$3"
headers="$PWD/$4"
cmake="$PWD/$5"
compiler="$PWD/$6"
make="$PWD/$7"
ar="$PWD/$8"
ranlib="$PWD/$9"
work="$abi.build"
"$cmake" -S "$root/runtimes" -B "$work" \\
    -DCMAKE_BUILD_TYPE=Release -DCMAKE_TRY_COMPILE_TARGET_TYPE=STATIC_LIBRARY \\
    -DCMAKE_C_COMPILER="$compiler" -DCMAKE_CXX_COMPILER="$compiler" \\
    -DCMAKE_MAKE_PROGRAM="$make" -DCMAKE_AR="$ar" -DCMAKE_RANLIB="$ranlib" \\
    '-DLLVM_ENABLE_RUNTIMES=libunwind;libcxxabi;libcxx' -DLLVM_INCLUDE_TESTS=OFF \\
    -DLIBUNWIND_ENABLE_SHARED=OFF -DLIBUNWIND_ENABLE_THREADS=OFF \\
    -DLIBUNWIND_IS_BAREMETAL=ON -DLIBUNWIND_USE_COMPILER_RT=ON -DLIBUNWIND_ENABLE_ASSERTIONS=OFF \\
    -DLIBCXXABI_ENABLE_SHARED=OFF -DLIBCXXABI_ENABLE_THREADS=OFF \\
    -DLIBCXXABI_BAREMETAL=ON -DLIBCXXABI_USE_COMPILER_RT=ON -DLIBCXXABI_ENABLE_ASSERTIONS=OFF \\
    -DLIBCXXABI_SILENT_TERMINATE=ON -DLIBCXXABI_ENABLE_NEW_DELETE_DEFINITIONS=OFF \\
    -DLIBCXX_ENABLE_SHARED=OFF -DLIBCXX_ENABLE_THREADS=OFF -DLIBCXX_INCLUDE_TESTS=OFF \\
    '-DCMAKE_C_FLAGS=-ffreestanding -mno-red-zone -fPIC -mcmodel=small' \\
    '-DCMAKE_CXX_FLAGS=-ffreestanding -mno-red-zone -fPIC -mcmodel=small'
"$cmake" --build "$work" --target cxxabi_static unwind_static --parallel 4
cp "$work/lib/libc++abi.a" "$abi"
cp "$work/lib/libunwind.a" "$unwind"
mkdir -p "$headers"
cp -R -L "$work/include/c++/v1/." "$headers"
rm -rf "$work"
""",
        mnemonic = "CppRuntime",
        progress_message = "Building LLVM C++ exception runtime",
    )
    return [
        DefaultInfo(files = depset([abi, unwind])),
        OutputGroupInfo(headers = depset([headers])),
        CcInfo(compilation_context = cc_common.create_compilation_context(
            headers = depset([headers]),
            system_includes = depset([headers.path]),
        )),
    ]

cpp_runtime = rule(
    implementation = _cpp_runtime_impl,
    attrs = {
        "srcs": attr.label_list(allow_files = True),
        "root": attr.label(allow_single_file = True),
        "_cmake": attr.label(default = "@local_tools//:cmake", allow_single_file = True),
        "_clang": attr.label(default = "@local_tools//:clang", allow_single_file = True),
        "_make": attr.label(default = "@local_tools//:make", allow_single_file = True),
        "_ar": attr.label(default = "@local_tools//:ar", allow_single_file = True),
        "_ranlib": attr.label(default = "@local_tools//:ranlib", allow_single_file = True),
    },
)
