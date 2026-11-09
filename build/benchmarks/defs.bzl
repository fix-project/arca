"""Criterion benchmark suites backed by guest workers."""

load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library")
load("//build:defs.bzl", "guest_binary")

_ARGS = ["--discard-baseline", "--noplot", "--bench"]

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
        name = name,
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

def benchmark_suite(name, suites):
    rust_binary(
        name = name + "_runner",
        visibility = ["//visibility:private"],
        srcs = ["//build/benchmarks:suite.rs"],
        crate_name = "benchmark_suite",
        args = _ARGS,
        data = suites,
        rustc_env = {
            "BENCHMARK_SUITES": "\n".join(["$(rlocationpath " + suite + ")" for suite in suites]),
        },
        deps = ["@rules_rust//tools/runfiles"],
    )
    native.alias(name = name, actual = ":" + name + "_runner")
