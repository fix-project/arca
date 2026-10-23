# This file is licensed under the Apache License v2.0 with LLVM Exceptions.
# See https://llvm.org/LICENSE.txt for license information.
# SPDX-License-Identifier: Apache-2.0 WITH LLVM-exception

"""LLVM overlay configuration from fix-project/llvm-project at
3c24197a97c8f1200596d2dc3dd4335960a3bbc6 (utils/bazel/configure.bzl).

Keeping the rule local avoids fetching LLVM sources during module loading.
"""

# Should be kept in sync with LLVM_ALL_TARGETS in llvm/CMakeLists.txt
DEFAULT_TARGETS = [
    "AArch64",
    "AMDGPU",
    "ARM",
    "AVR",
    "BPF",
    "Hexagon",
    "Lanai",
    "LoongArch",
    "Mips",
    "MSP430",
    "NVPTX",
    "PowerPC",
    "RISCV",
    "Sparc",
    "SPIRV",
    "SystemZ",
    "VE",
    "WebAssembly",
    "X86",
    "XCore",
]

MAX_TRAVERSAL_STEPS = 1000000  # "big number" upper bound on total visited dirs

def _overlay_directories(repository_ctx):
    src_root = repository_ctx.path(Label("@llvm-raw//:WORKSPACE")).dirname
    overlay_root = src_root.get_child("utils/bazel/llvm-project-overlay")
    target_root = repository_ctx.path(".")

    # Tries to minimize the number of symlinks created (that is, does not symlink
    # every single file). Symlinks every file in the overlay directory. Only symlinks
    # individual files in the source directory if their parent directory is also
    # contained in the overlay directory tree.

    stack = ["."]
    for _ in range(MAX_TRAVERSAL_STEPS):
        rel_dir = stack.pop()

        # TODO: `set()` is only available in bazel 8.1.
        # Use `set()` after downstream users are on more recent versions.
        overlay_dirs = {}

        # Symlink overlay files, overlay dirs will be handled in future iterations.
        for entry in overlay_root.get_child(rel_dir).readdir():
            name = entry.basename
            full_rel_path = rel_dir + "/" + name

            if entry.is_dir:
                stack.append(full_rel_path)
                overlay_dirs[name] = None
            else:
                src_path = overlay_root.get_child(full_rel_path)
                dst_path = target_root.get_child(full_rel_path)
                repository_ctx.symlink(src_path, dst_path)

        # Symlink source dirs (if not themselves overlaid) and files.
        src_dir = src_root.get_child(rel_dir)
        if not src_dir.exists:
            continue
        for src_entry in src_dir.readdir():
            name = src_entry.basename
            if name in overlay_dirs.keys():
                # Skip: overlay has a directory with this name
                continue

            repository_ctx.symlink(src_entry, target_root.get_child(rel_dir + "/" + name))

        if not stack:
            return

    fail("overlay_directories: exceeded MAX_TRAVERSAL_STEPS ({}). " +
         "Tree too large or a cycle in the filesystem?".format(
             MAX_TRAVERSAL_STEPS,
         ))

def _extract_cmake_settings(repository_ctx, llvm_cmake):
    # The list to be written to vars.bzl
    # `CMAKE_CXX_STANDARD` may be used from WORKSPACE for the toolchain.
    c = {
        "CMAKE_CXX_STANDARD": None,
        "LLVM_VERSION_MAJOR": None,
        "LLVM_VERSION_MINOR": None,
        "LLVM_VERSION_PATCH": None,
        "LLVM_VERSION_SUFFIX": None,
    }

    # It would be easier to use external commands like sed(1) and python.
    # For portability, the parser should run on Starlark.
    llvm_cmake_path = repository_ctx.path(Label("@llvm-raw//:" + llvm_cmake))
    for line in repository_ctx.read(llvm_cmake_path).splitlines():
        # Extract "set ( FOO bar ... "
        setfoo = line.partition("(")
        if setfoo[1] != "(":
            continue
        if setfoo[0].strip().lower() != "set":
            continue

        # `kv` is assumed as \s*KEY\s+VAL\s*\).*
        # Typical case is like
        #   LLVM_REQUIRED_CXX_STANDARD 17)
        # Possible case -- It should be ignored.
        #   CMAKE_CXX_STANDARD ${...} CACHE STRING "...")
        kv = setfoo[2].strip()
        i = kv.find(" ")
        if i < 0:
            continue
        k = kv[:i]

        # Prefer LLVM_REQUIRED_CXX_STANDARD instead of CMAKE_CXX_STANDARD
        if k == "LLVM_REQUIRED_CXX_STANDARD":
            k = "CMAKE_CXX_STANDARD"
            c[k] = None
        if k not in c:
            continue

        # Skip if `CMAKE_CXX_STANDARD` is set with
        # `LLVM_REQUIRED_CXX_STANDARD`.
        # Then `v` will not be desired form, like "${...} CACHE"
        if c[k] != None:
            continue

        # Pick up 1st word as the value.
        # Note: It assumes unquoted word.
        v = kv[i:].strip().partition(")")[0].partition(" ")[0]
        c[k] = v

    # LLVMVersion.cmake uses set(LLVM_VERSION_SUFFIX) for an empty suffix.
    # The parser above skips settings without a value, so restore CMake's
    # empty-string value before constructing the package version.
    if c["LLVM_VERSION_SUFFIX"] == None:
        c["LLVM_VERSION_SUFFIX"] = ""

    # Synthesize `LLVM_VERSION` for convenience.
    c["LLVM_VERSION"] = "{}.{}.{}".format(
        c["LLVM_VERSION_MAJOR"],
        c["LLVM_VERSION_MINOR"],
        c["LLVM_VERSION_PATCH"],
    )

    c["PACKAGE_VERSION"] = "{}.{}.{}{}".format(
        c["LLVM_VERSION_MAJOR"],
        c["LLVM_VERSION_MINOR"],
        c["LLVM_VERSION_PATCH"],
        c["LLVM_VERSION_SUFFIX"],
    )

    return c

def _write_dict_to_file(repository_ctx, filepath, header, vars):
    # (fci + individual vars) + (fcd + dict items) + (fct)
    fci = header
    fcd = "\nllvm_vars={\n"
    fct = "}\n"

    for k, v in vars.items():
        fci += '{} = "{}"\n'.format(k, v)
        fcd += '    "{}": "{}",\n'.format(k, v)

    repository_ctx.file(filepath, content = fci + fcd + fct)

def _llvm_configure_impl(repository_ctx):
    _overlay_directories(repository_ctx)

    llvm_cmake = "llvm/CMakeLists.txt"
    vars = _extract_cmake_settings(
        repository_ctx,
        llvm_cmake,
    )

    # Grab version info and merge it with the other vars
    version = _extract_cmake_settings(
        repository_ctx,
        "cmake/Modules/LLVMVersion.cmake",
    )
    version = {k: v for k, v in version.items() if v != None}
    vars.update(version)
    vars["LLVM_DEFAULT_TARGET_TRIPLE_OVERRIDE"] = repository_ctx.os.environ.get("LLVM_DEFAULT_TARGET_TRIPLE", "")

    _write_dict_to_file(
        repository_ctx,
        filepath = "vars.bzl",
        header = "# Generated from {}\n\n".format(llvm_cmake),
        vars = vars,
    )

    # Create a starlark file with the requested LLVM targets.
    llvm_targets = repository_ctx.attr.targets
    targets_override = repository_ctx.os.environ.get("LLVM_TARGETS_TO_BUILD", "")
    if targets_override:
        llvm_targets = [target.strip() for target in targets_override.split(";") if target.strip()]
        for target in llvm_targets:
            if target not in DEFAULT_TARGETS:
                fail("Unknown LLVM target in LLVM_TARGETS_TO_BUILD: " + target)
    repository_ctx.file(
        "llvm/targets.bzl",
        content = "llvm_targets = " + str(llvm_targets),
        executable = False,
    )

    # Create a starlark file with the requested BOLT targets.
    bolt_targets = ["AArch64", "X86", "RISCV"]  # Supported targets.
    bolt_targets = [t for t in llvm_targets if t in bolt_targets]
    repository_ctx.file(
        "bolt/targets.bzl",
        content = "bolt_targets = " + str(bolt_targets),
        executable = False,
    )

    if hasattr(repository_ctx, "repo_metadata"):
        return repository_ctx.repo_metadata(reproducible = True)

llvm_configure = repository_rule(
    implementation = _llvm_configure_impl,
    environ = ["LLVM_TARGETS_TO_BUILD", "LLVM_DEFAULT_TARGET_TRIPLE"],
    attrs = {
        "targets": attr.string_list(default = DEFAULT_TARGETS),
    },
)
