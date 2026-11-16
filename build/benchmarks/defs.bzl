"""Criterion benchmarks with optimized hosts, guest workers, and fixtures."""

load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library")
load("//build:defs.bzl", "guest_binary")

_ARGS = ["--discard-baseline", "--noplot", "--bench"]

def _release_impl(settings, attr):
    return {"//command_line_option:compilation_mode": "opt"}

_release = transition(
    implementation = _release_impl,
    inputs = [],
    outputs = ["//command_line_option:compilation_mode"],
)

def _benchmark_impl(ctx):
    actual = ctx.attr.actual[0][DefaultInfo]
    executable = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.symlink(output = executable, target_file = actual.files_to_run.executable, is_executable = True)
    return [DefaultInfo(
        executable = executable,
        runfiles = actual.default_runfiles.merge(ctx.runfiles(files = [actual.files_to_run.executable])),
    )]

_benchmark = rule(
    implementation = _benchmark_impl,
    executable = True,
    attrs = {
        "actual": attr.label(cfg = _release, executable = True, mandatory = True),
        "_allowlist_function_transition": attr.label(default = "@bazel_tools//tools/allowlists/function_transition_allowlist"),
    },
)

def guest_benchmark(name, src, cases, fixtures, deps = []):
    rust_library(
        name = name + "_cases",
        srcs = [cases],
        crate_name = "benchmark_cases",
        deps = ["//build/benchmarks:protocol"],
    )
    guest_binary(
        name = name + "_image",
        srcs = [src],
        crate_root = src,
        linker_script = "//arca/kernel:etc/memmap.ld",
        proc_macro_deps = ["//tools/macros"],
        deps = [
            ":" + name + "_cases",
            "//build/benchmarks:guest",
            "//arca/api",
            "//arca/kernel",
            "//lib/common",
        ] + deps,
    )
    rust_binary(
        name = name + "_host",
        visibility = ["//tests:__pkg__"],
        srcs = ["//build/benchmarks:host.rs"],
        crate_name = "benchmark_host",
        args = _ARGS,
        data = [":" + name + "_image", "//tools/vmm:vmm"] + fixtures.values(),
        rustc_env = {
            "BENCH_WORKER": "$(rlocationpath :" + name + "_image)",
            "BENCH_VMM": "$(rlocationpath //tools/vmm:vmm)",
            "BENCH_FIXTURES": "\n".join([key + "=$(rlocationpath " + value + ")" for key, value in fixtures.items()]),
        },
        deps = [
            ":" + name + "_cases",
            "//build/benchmarks:protocol",
            "@crates//:criterion",
            "@rules_rust//tools/runfiles",
        ],
    )
    _benchmark(name = name, actual = ":" + name + "_host", args = _ARGS)

def benchmark_suite(name, suites):
    rust_binary(
        name = name + "_runner",
        visibility = ["//tests:__pkg__"],
        srcs = ["//build/benchmarks:suite.rs"],
        crate_name = "benchmark_suite",
        args = _ARGS,
        data = suites,
        rustc_env = {
            "BENCHMARK_SUITES": "\n".join(["$(rlocationpath " + suite + ")" for suite in suites]),
        },
        deps = ["@rules_rust//tools/runfiles"],
    )
    _benchmark(name = name, actual = ":" + name + "_runner", args = _ARGS)
