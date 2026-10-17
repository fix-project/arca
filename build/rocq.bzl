"""Compile and kernel-check Rocq libraries using a pinned opam toolchain."""

RocqInfo = provider(fields = ["files", "paths", "module"])

def _rocq_library_impl(ctx):
    source = ctx.file.src
    output = ctx.actions.declare_file(source.basename[:-2] + ".vo")
    paths = depset(
        [(output.dirname, ctx.attr.namespace)],
        transitive = [dep[RocqInfo].paths for dep in ctx.attr.deps],
    )
    dependencies = depset(transitive = [dep[RocqInfo].files for dep in ctx.attr.deps])
    args = ctx.actions.args()
    args.add_all(["compile", "-q", "-noglob", "-Q", source.dirname, ctx.attr.namespace])
    for directory, namespace in paths.to_list():
        args.add_all(["-Q", directory, namespace])
    args.add_all(["-o", output.path, source.path])
    ctx.actions.run(
        executable = ctx.executable._rocq,
        arguments = [args],
        inputs = depset([source] + ctx.files.data, transitive = [dependencies]),
        tools = ctx.files._toolchain,
        outputs = [output],
        env = {"HOME": "."},
        mnemonic = "RocqCompile",
        progress_message = "Compiling Rocq module %s" % source.basename,
        execution_requirements = {"no-remote-exec": "1"},
    )
    return [
        DefaultInfo(files = depset([output])),
        RocqInfo(
            files = depset([output], transitive = [dependencies]),
            paths = paths,
            module = ctx.attr.namespace + "." + source.basename[:-2],
        ),
    ]

rocq_library = rule(
    implementation = _rocq_library_impl,
    attrs = {
        "src": attr.label(allow_single_file = [".v"], mandatory = True),
        "deps": attr.label_list(providers = [RocqInfo]),
        "data": attr.label_list(allow_files = True),
        "namespace": attr.string(mandatory = True),
        "_rocq": attr.label(default = "@rocq_host//:bin/rocq", executable = True, cfg = "exec", allow_single_file = True),
        "_toolchain": attr.label(default = "@rocq_host//:files"),
    },
)

def _runfile(ctx, path):
    return path[3:] if path.startswith("../") else ctx.workspace_name + "/" + path

def _rocq_test_impl(ctx):
    libraries = [dep[RocqInfo] for dep in ctx.attr.deps]
    files = depset(transitive = [library.files for library in libraries])
    paths = depset(transitive = [library.paths for library in libraries])
    directories = {file.dirname: file.short_path.rpartition("/")[0] for file in files.to_list()}
    arguments = []
    for directory, namespace in paths.to_list():
        arguments.extend(['-Q "$TEST_SRCDIR/%s"' % _runfile(ctx, directories[directory]), namespace])
    if ctx.file.modules:
        command = 'mapfile -t modules < "$TEST_SRCDIR/%s"\n' % _runfile(ctx, ctx.file.modules.short_path)
        arguments.append('"${modules[@]}"')
    else:
        if len(libraries) != 1:
            fail("A project check requires exactly one Rocq library")
        command = ""
        arguments.extend(["-norec", libraries[0].module])
    script = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(
        script,
        '#!/usr/bin/env bash\nset -euo pipefail\n%sexec "$TEST_SRCDIR/%s" check -silent %s\n' % (
            command,
            _runfile(ctx, ctx.executable._rocq.short_path),
            " ".join(arguments),
        ),
        is_executable = True,
    )
    return [DefaultInfo(
        executable = script,
        runfiles = ctx.runfiles(files = ctx.files._toolchain + ctx.files.modules, transitive_files = files),
    )]

rocq_test = rule(
    implementation = _rocq_test_impl,
    test = True,
    attrs = {
        "deps": attr.label_list(providers = [RocqInfo]),
        "modules": attr.label(allow_single_file = True),
        "_rocq": attr.label(default = "@rocq_host//:bin/rocq", executable = True, cfg = "exec", allow_single_file = True),
        "_toolchain": attr.label(default = "@rocq_host//:files"),
    },
)

def _rocq_host_impl(ctx):
    rocq = ctx.path(ctx.attr._rocq)
    prefix = rocq.dirname.dirname
    config = ctx.execute([rocq, "compile", "-config"])
    if config.return_code:
        fail(config.stderr)
    ctx.symlink(prefix.get_child("lib"), "lib")
    ctx.symlink(rocq, "bin/rocq")
    ctx.symlink(rocq.dirname.get_child("rocqchk"), "bin/rocqchk")
    wat2wasm = ctx.which("wat2wasm")
    if not wat2wasm:
        fail("Install WABT (wat2wasm); see coupon/proof/README.md")
    ctx.symlink(wat2wasm, "wat2wasm")
    ctx.file("config", config.stdout)
    project = ctx.path(ctx.attr.project).dirname
    sources = project.get_child("wasm-proofs/rocq")
    for line in ctx.read(sources.get_child("_CoqProject")).splitlines():
        if line.endswith(".v"):
            ctx.read(sources.get_child(line))
    dependencies = ctx.execute(["python3", ctx.path(ctx.attr._dependencies), rocq, project])
    if dependencies.return_code:
        fail("Cannot load the managed proof dependencies\n" + dependencies.stderr)
    ctx.file("dependencies", dependencies.stdout)
    ctx.file("BUILD.bazel", '''package(default_visibility = ["//visibility:public"])
exports_files(["bin/rocq", "bin/rocqchk", "wat2wasm", "dependencies"])
filegroup(name = "files", srcs = ["bin/rocq", "bin/rocqchk", "config"] + glob(["lib/**"]))
''')

rocq_host = repository_rule(
    implementation = _rocq_host_impl,
    environ = ["PATH"],
    attrs = {
        "project": attr.label(default = "//coupon/proof:coupon.wat"),
        "_rocq": attr.label(default = "@rocq_toolchain//:opam/proof/bin/rocq"),
        "_dependencies": attr.label(default = "//build:rocq_dependencies.py"),
    },
    local = True,
    configure = True,
)
