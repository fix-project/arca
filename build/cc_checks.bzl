"""Formatting and static analysis for C and C++."""

load("@rules_shell//shell:sh_binary.bzl", "sh_binary")
load("@rules_shell//shell:sh_test.bzl", "sh_test")

def cc_format(name, srcs, style):
    """Create an autoformatter and a formatting test."""
    for mode in ["fix", "check"]:
        rule = sh_binary if mode == "fix" else sh_test
        rule(
            name = name if mode == "fix" else name + "_test",
            srcs = ["//tools:cc_format.sh"],
            args = [mode, "$(rootpath @local_tools//:clang-format)", "$(rootpath %s)" % style] +
                   ["$(rootpaths %s)" % src for src in srcs],
            data = srcs + [style, "@local_tools//:clang-format"],
        )

def cc_lint_test(name, srcs, hdrs, config, copts):
    """Analyze translation units and their headers with explicit compiler flags."""
    sh_test(
        name = name,
        srcs = ["//tools:cc_lint.sh"],
        args = ["$(rootpath @local_tools//:clang-tidy)", "$(rootpath %s)" % config] +
               ["$(rootpaths %s)" % src for src in srcs] + ["--"] + copts,
        data = srcs + hdrs + [config, "@local_tools//:clang-tidy"],
    )
