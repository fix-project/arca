# Fix and Arca

Fix is a prototype distributed operating system running on the Arca research
kernel.

## Build

Requires Linux x86-64 with x86-64-v3 and XSAVE, Bazelisk, GCC/G++, binutils, CMake, Make,
Clang/libclang, clang-format, clang-tidy, Binaryen 120 (`wasm-merge`), Bash, GNU coreutils 9.3+, and Python 3.
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
bazel run //programs/fix:addblob -- 2u64 3u64
```

## Run Fix

Build the Fix binary and bootstrap seed.

```sh
bazel build //fix/cli:fix //bootstrap:seed
fix="$PWD/bazel-bin/fix/cli/fix"
seed="$PWD/$(bazel cquery //bootstrap:seed --output=files)"
"$fix" trust add-agent
"$fix" bootstrap "$seed"
mkdir -p fix-repo
cd fix-repo
"$fix" init
"$fix" eval 42u64
"$fix" run path/to/program.fix
```

`trust add-agent` requires an active `ssh-agent` with an Ed25519 key; load one
with `ssh-add ~/.ssh/id_ed25519` if needed. The
`bootstrap` imports the seed's compiler, scheduler, collector, and parser into
`$XDG_DATA_HOME/fix/bootstrap/.fix` (default `~/.local/share/fix/bootstrap/.fix`)
and signs its Tags using the identities configured by `trust add-agent`.
Trust and global program settings live in `$XDG_CONFIG_HOME/fix` (default
`~/.config/fix`). New repositories start empty and have a `bootstrap` file remote.
Use `fix sync bootstrap` to fetch its label refs, then `fix remote labels bootstrap`
to list them or `fix pull bootstrap:compile compile` to create a local label.
`fix bootstrap` records the seed's program handles in user
configuration, so moving a label does not change the defaults. Hint is the
seed's default scheduler. Use
`fix config scheduler serial` to select a local label in the current repository.
Command-line options such as `--scheduler` take precedence. Bootstrap backing
data is available to every repository when a configured program is persisted.
A home repository is optional; create one with `fix init` in your home directory
and add it as an ordinary `file://` remote where needed.
Use `--unsigned` only for isolated staging builds. Their Tags and coupons need
signatures before normal use.

## Clone and evaluate remotely

Bootstrap Fix on both machines, then initialize a repository on the server.
The client can clone its label names without downloading their data and use the
distributed scheduler to delegate evaluations over SSH. See the
[two-machine setup](fix/remote/README.md#ssh-clone-and-distributed-evaluation)
for the trust setup, scheduler build, and commands.

Arca arguments: `word:`, `blob:`, `tuple:` (comma-separated), `elf:<path>`,
`null`. Fix arguments are Fix expressions. `_elf` targets expose raw artifacts.
The `map` and `curry` combinators provide only `_elf` targets. Test fixtures live
under `//tests`; SIMD and combinator tests run with `bazel test //arca/kernel:tests`.
Integration tests for standalone programs can live beside those programs.

Arca and Fix unit tests use Rust's host libtest runner with a fresh VM per case.
List cases with `bazel test //fix/runtime:tests --test_arg=--list --test_output=all`,
or select them with `bazel test //fix/runtime:tests --test_filter=continuations`.
Fixtures are loaded from Bazel runfiles rather than embedded in test images.

Use `--config=release` for optimized builds.

Userspace targets `x86_64-unknown-arca` with the x86-64-v3 baseline and System V
calling conventions. Bazel builds `core`, `alloc`, and `compiler_builtins` from
sources matching the pinned Rust compiler. The kernel uses `x86_64-unknown-none`
with its soft-float ABI.

The [bootstrap process](bootstrap/README.md) starts from pinned LLVM release
artifacts, rebuilds the compiler inside Fix, and checks byte-identical
generations. Shared caches use ordinary Bazel configuration.

## Benchmarks

[Microbenchmarks](build/benchmarks/README.md) cover Arca lifecycle and continuation
operations, Fix invocation, and resident, copied, and persisted continuations.
Run `bazel run --config=release //:benchmarks` for all numeric benchmark results,
or select a leaf suite such as `//fix/benchmarks:continuations`.

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

The [Fix CLI](fix/cli/README.md) also provides persistent object construction,
reading, inspection, and labels.

Licensed under [LGPL-2.1-or-later](LICENSE).
