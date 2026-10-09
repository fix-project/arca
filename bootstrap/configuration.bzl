"""Bootstrap stage configuration."""

def _mode(settings, _attr):
    return {"//command_line_option:compilation_mode": "dbg" if settings["//bootstrap:debug"] else "opt"}

mode = transition(
    implementation = _mode,
    inputs = ["//bootstrap:debug"],
    outputs = ["//command_line_option:compilation_mode"],
)

ALLOW_TRANSITION = {
    "_allowlist_function_transition": attr.label(default = "@bazel_tools//tools/allowlists/function_transition_allowlist"),
}

def _stage(ctx):
    return [DefaultInfo(files = depset(transitive = [target[DefaultInfo].files for target in ctx.attr.srcs]))]

stage = rule(
    implementation = _stage,
    cfg = mode,
    attrs = dict(ALLOW_TRANSITION, srcs = attr.label_list(allow_files = True)),
)
