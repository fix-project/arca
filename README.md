# Fix and Arca

Fix is a prototype distributed operating system running on the Arca research
kernel.

## Build

Requires Linux x86-64 with x86-64-v3 and XSAVE, Bazelisk, GCC/G++, binutils, CMake, Make,
Clang/libclang, and Bash. Set `LIBCLANG_PATH` or
`BINDGEN_EXTRA_CLANG_ARGS` for nonstandard Clang installations.

```sh
git submodule update --init --recursive
bazel query //... --output=label_kind
bazel build //:artifacts
bazel test //...
bazel test //tests:format //tests:lint
bazel run //programs/kernel:hello -- Ada
bazel run //programs/arca:add -- tuple:word:2,word:3
bazel run //programs/fix:addblob -- 2u64 3u64
bazel run //fix/runtime:fix -- eval path/to/program.fix
```

Arca arguments: `word:`, `blob:`, `tuple:` (comma-separated), `elf:<path>`,
`null`. Fix arguments are Fix expressions. `_elf` targets expose raw artifacts.

Use `--config=release` for optimized builds.

Userspace targets `x86_64-unknown-arca` with the x86-64-v3 baseline and System V
calling conventions. Bazel builds `core`, `alloc`, and `compiler_builtins` from
sources matching the pinned Rust compiler. The kernel uses `x86_64-unknown-none`
with its soft-float ABI.

## rust-analyzer

Generate the ignored `rust-project.json` after changing Rust targets or the
toolchain:

```sh
bazel run @rules_rust//tools/rust_analyzer:gen_rust_project -- //...
```

## Environment-dependent tests

`bazel test //...` requires `/dev/kvm`; without it, use
`bazel test //:host_tests`. The proof test requires Isabelle2025-2, AFP,
and OCaml:

```sh
bazel test --test_tag_filters=proof //coupon:proof
```

See [coupon/proof/README.md](coupon/proof/README.md) for proof setup.

Licensed under [LGPL-2.1-or-later](LICENSE).
