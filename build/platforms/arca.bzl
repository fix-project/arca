"""The freestanding userspace target."""

ARCA_TARGET = {
    "arch": "x86_64",
    "code-model": "large",
    "cpu": "x86-64-v3",
    "data-layout": "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
    "disable-redzone": True,
    "linker": "rust-lld",
    "linker-flavor": "gnu-lld",
    "llvm-target": "x86_64-unknown-none-elf",
    "max-atomic-width": 64,
    "os": "arca",
    "panic-strategy": "abort",
    "plt-by-default": False,
    "relocation-model": "static",
    "stack-probes": {"kind": "inline"},
    "target-pointer-width": 64,
}

def _target_impl(ctx):
    output = ctx.actions.declare_file("stdlib/x86_64-unknown-arca.json")
    # Rust hashes the target JSON bytes; match the toolchain rule's formatting.
    ctx.actions.write(output, json.encode_indent(ARCA_TARGET, indent = " " * 4))
    return [DefaultInfo(files = depset([output]))]

arca_target = rule(implementation = _target_impl)
