# Fix and Arca

Fix is a prototype distributed operating system running on the Arca research
kernel.

## Build

Requires Linux x86-64, Bazelisk (or the version in `.bazelversion`), GCC/G++,
binutils, CMake, Make, Clang/libclang, and Bash. Rust is downloaded by Bazel;
native C/C++ tools come from the host. Nonstandard libclang installations can
set `LIBCLANG_PATH` and `BINDGEN_EXTRA_CLANG_ARGS`.

```sh
git submodule update --init --recursive
bazel build //:all
bazel test //...
bazel test //tests:format //tests:lint
bazel run //programs/kernel:hello -- Ada
bazel run //programs/arca:add -- tuple:word:2,word:3
bazel run //programs/fix:addblob -- 2u64 3u64
bazel run //fix/runtime:fix -- eval path/to/program.fix
```

Arca program arguments use `word:`, `blob:`, `tuple:` (comma-separated values),
`elf:` (a path to a built Arca ELF), or `null`. Fix program arguments are Fix
expressions. Use `_elf` targets when another target needs the raw artifact.

Use `--config=release` for optimized builds. Kernel and user-space platforms are
separate because only the kernel may use common's core-local allocator cache.

## Environment-dependent tests

`bazel test //...` includes kernel and Fix guest tests and requires readable/writable
`/dev/kvm`. Without KVM, run `bazel test //:host_tests`. The proof test requires
Isabelle2025-2, AFP, and OCaml and is run separately:

```sh
bazel test --test_tag_filters=proof //coupon:proof
```

See [coupon/proof/README.md](coupon/proof/README.md) for proof setup. Tests write
to private temporary directories; `bazel run` uses the workspace directory.

`programs/legacy-c` is not built: it requires the removed arca-musl port.

Licensed under [LGPL-2.1-or-later](LICENSE).
