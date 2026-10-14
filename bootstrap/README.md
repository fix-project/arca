# Bootstrap

The bootstrap builds LLVM, wasm2c, and the Wasm merger from pinned source,
then converts them into Fix executables. The first LLVM build uses the host
C/C++ compiler. The WASIp1 sysroot and C/C++ headers are included.

| Phase | Target | What it does | Minutes on puffer5 |
|---|---|---|---|
| Host LLVM | `//bootstrap:host_llvm` | Builds native Clang and LLD | 5 |
| Stage0 | `//bootstrap:stage0` | Builds the WASIp1 tools and sysroot | 5 |
| Stage1 | `//bootstrap:stage1` | Native tools merge Wasm with Flatware, translate to C, and compile/link Fix ELFs | 5 |
| Stage2 | `//bootstrap:stage2` | Stage1 tools repeat the conversion inside Fix | 10 |
| Stage3 | `//bootstrap:stage3` | Stage2 tools repeat it inside Fix | 10 |

Estimates use release builds on puffer5 with 256 jobs and 256 C units per command,
after prerequisites are built, rounded to the nearest five minutes. With one
repository per stage, Stage2 took 8 minutes 53 seconds and Stage3 took 9 minutes
11 seconds.
Cache hits take seconds.

```sh
bazel build //bootstrap
bazel run //bootstrap:verify
bazel run //bootstrap:verify2
```

The default build stops at Stage1 and packages a minimal `.fix` repository in
`bazel-bin/bootstrap/bootstrap.tar.gz`. `verify` aliases `verify1`, which builds
Stage2 and compares its ELFs with Stage1 byte for byte. `verify2` builds Stage3
and compares it with Stage2. Every stage converts the same original Wasm inputs.
On puffer5, add `--jobs=256`; CMake builds use all available CPUs.
Stage, archive, and verification targets default to release mode. Use
`--//bootstrap:debug` to build their dependencies in debug mode.
Stages 2 and 3 each use one Bazel action and one private Fix repository, with
separate evaluations for preparation, merging, translation, compilation, and
linking. Compilation submits all three tools' 256 C units and runtime units
together. Intermediate values stay in that repository; only the final result
archive and executables are retained. Bazel caches each completed stage. NDJSON
node references share graph construction.
Configure Bazel caches in a user or site Bazelrc.

Verification uses available cores by default. `--//build:fix-jobs=N` sets the
default for all Fix evaluations; `--//bootstrap:jobs=N` overrides it for bootstrap.
Memory defaults to 1 GiB per worker, rounded up to a power of two with a 16 GiB
minimum. Override with `--//bootstrap:memory=MIB`; memory must be a power of two.
A 256-job compilation batch (771 units across all three tools) took about
6 minutes 40 seconds and peaked at 151.7 GiB RSS on puffer5 with a 256 GiB VM.
This diagnostic run included scheduler counters; all objects matched native
compilation byte for byte. It excludes graph construction, translation, linking,
and export.

The complete tool filesystem is `bazel-bin/bootstrap/stage0`; mount its `usr`
directory at `/usr` in Flatware. Each stage exposes its ELF commands through
`//bootstrap:stageN_COMMAND_link` targets.

Build or install the bootstrap repository:

```sh
bazel build //bootstrap
fix init
fix import bazel-bin/bootstrap/bootstrap.tar.gz
```

The archive contains `.fix` with labels `bootstrap`, `json`, `parser`, `identity`, `path`, `clang`, `wasm2c`,
`wasm-merge`, `cpio`, `flatware-normalize`, `compose`, and `toolchain`, plus only the objects and data reachable from
`bootstrap`. System tar can unpack it directly over an empty `.fix` repository.
