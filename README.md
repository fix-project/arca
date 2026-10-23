# Fix and Arca

Fix is a prototype distributed operating system running on the Arca research
kernel.

## Build

Requires Linux x86-64 with x86-64-v3 and XSAVE, Bazelisk, GCC/G++, binutils, CMake, Make,
Clang/libclang, clang-format, clang-tidy, Binaryen 120 (`wasm-merge`), Bash, GNU coreutils 9.3+, GNU cpio, and Python 3.
Set `LIBCLANG_PATH` or `BINDGEN_EXTRA_CLANG_ARGS` for nonstandard Clang installations.

```sh
git submodule update --init --recursive
bazel query //... --output=label_kind
bazel build //:artifacts
bazel test //:tests
bazel test //tests:format //tests:lint
bazel run //:format
bazel run //programs/kernel:hello -- Ada
bazel run //programs/arca:add -- tuple:word:2,word:3
bazel run //fix -- init
bazel run //fix -- import /path/to/bootstrap.tar.gz
bazel run //programs/fix:addblob -- 2u64 3u64
bazel run //fix -- run path/to/program.fix
```

Arca arguments: `word:`, `blob:`, `tuple:` (comma-separated), `elf:<path>`,
`null`. Fix arguments are Fix expressions. `_elf` targets expose raw artifacts.
The `map` and `curry` combinators provide only `_elf` targets. Test fixtures live
under `//tests`; SIMD and combinator tests run with `bazel test //arca/kernel:tests`.
Integration tests for standalone programs can live beside those programs.

Use `--config=release` for optimized builds.

Userspace targets `x86_64-unknown-arca` with the x86-64-v3 baseline and System V
calling conventions. Bazel builds `core`, `alloc`, and `compiler_builtins` from
sources matching the pinned Rust compiler. The kernel uses `x86_64-unknown-none`
with its soft-float ABI.

The optional [bootstrap process](bootstrap/README.md) builds self-hosted Wasm-to-ELF
toolchains from pinned sources and checks byte-identical generations. Shared
caches use ordinary Bazel configuration.

## Formatting

Format Rust throughout the codebase and C SDK sources:

```sh
bazel run //:format
```

## Language servers

Generate `compile_commands.json` for clangd and `rust-project.json` for rust-analyzer:

```sh
bazel run //:lsp
```

## Environment-dependent tests

`bazel test //:tests` requires `/dev/kvm`; without it, use
`bazel test //:host_tests`. The optional proof test requires opam, Python 3.11+,
and the native build prerequisites listed in [fix/proof/README.md](fix/proof/README.md).
Bazel provisions its own pinned OCaml/Rocq toolchain on the first run:

```sh
bazel test //fix/proof:check
```

Bazel compiles each module and caches its kernel check separately. The proof embeds
the Wasm binary built from `fix/proof/collector/collector.wat` and verifies the installed
library dependencies. Individual module checks assume their imports; the full
suite verifies the complete dependency closure. CI runs it separately from the
default tests.
Theory directories declare their Bazel module dependencies and Rocq namespaces.

The [Fix CLI](fix/runtime/README.md) also provides persistent object construction,
reading, inspection, and labels.

Licensed under [LGPL-2.1-or-later](LICENSE).
