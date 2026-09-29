"""Discover native build tools on the host."""

def _local_tools_impl(ctx):
    tools = ["bash", "gcc", "g++", "clang", "ar", "ranlib", "cmake", "make"]
    for tool in tools:
        path = ctx.which(tool)
        if not path:
            fail("Install %s before building Fix (see README.md)" % tool)
        ctx.symlink(path, tool)
    ctx.file("BUILD.bazel", "exports_files(%s, visibility = [\"//visibility:public\"])\n" % repr(tools))

local_tools = repository_rule(implementation = _local_tools_impl, local = True, configure = True)
