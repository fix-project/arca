"""Provision the pinned Rocq toolchain independently of proof rules."""

def _rocq_toolchain_impl(ctx):
    opam = ctx.which("opam")
    if not opam:
        fail("Install opam to build the pinned Rocq toolchain; see fix/proof/README.md")
    ctx.file("rocq-toolchain.opam", ctx.read(ctx.attr._manifest))
    ctx.file("rocq-toolchain.opam.locked", ctx.read(ctx.attr._lock))
    result = ctx.execute(
        ["python3", ctx.path(ctx.attr._setup), opam, ctx.path(".")],
        timeout = 7200,
        quiet = False,
    )
    if result.return_code:
        fail("Cannot build the pinned Rocq toolchain:\n" + result.stderr)
    ctx.file("BUILD.bazel", 'exports_files(["opam/proof/bin/rocq"])\n')

rocq_toolchain = repository_rule(
    implementation = _rocq_toolchain_impl,
    attrs = {
        "_manifest": attr.label(default = "//build:rocq-toolchain.opam"),
        "_lock": attr.label(default = "//build:rocq-toolchain.opam.locked"),
        "_setup": attr.label(default = "//build:rocq_toolchain.py"),
    },
    configure = True,
)
