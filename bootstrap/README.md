# Bootstrap

The bootstrap builds LLVM, wasm2c, and the Wasm merger from pinned source.
Native tools convert the WASIp1 commands into Stage1 Fix executables.
The first LLVM build uses the host C/C++ compiler; the WASIp1 sysroot and
C/C++ headers are included.

| Phase | Target | What it does | Minutes on puffer5 |
|---|---|---|---|
| Host LLVM | `//bootstrap:host_llvm` | Builds native Clang and LLD | 5 |
| Stage0 | `//bootstrap:stage0` | Builds the WASIp1 tools and sysroot | 5 |
| Stage1 | `//bootstrap:stage1` | Merges Wasm with Flatware, translates to C, and compiles/links Fix ELFs | 5 |

```sh
bazel build //bootstrap --jobs=256
```

The default build stops at Stage1 and packages its tools in
`bazel-bin/bootstrap/bootstrap.tar.gz`. Each command's ELF is exposed through
`//bootstrap:stage1_COMMAND_link`; the tool filesystem is `//bootstrap:stage0`.

Estimates are uncached release builds on puffer5 with 256 jobs and 256 C units
per command, after prerequisites are built, rounded to five minutes.
CMake uses all available CPUs. Cache hits take seconds; configure shared caches
in a user or site Bazelrc. Use `--//bootstrap:debug` to opt into debug builds.

Install the bootstrap repository:

```sh
fix init
fix import bazel-bin/bootstrap/bootstrap.tar.gz
```

The archive contains a `.fix` repository with `bootstrap`, `json`, `parser`,
`identity`, `path`, `clang`, `wasm2c`, `wasm-merge`, `cpio`, `flatware-normalize`, `compose`, and `toolchain` labels.
System tar can also unpack it over an empty `.fix` repository.
