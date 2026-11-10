# Bootstrap

The bootstrap builds LLVM, wasm2c, and the Wasm merger from pinned source, then
converts the Fix compiler wrapper, tools, scheduler, and coupon collector into
Fix ELFs on Linux. A seeding tool writes them into a seed
repository. Nothing in this process runs inside Fix.

| Target | What it does |
|---|---|
| `//bootstrap:host_llvm` | Builds native Clang and LLD |
| `//bootstrap:stage0` | Builds the WASIp1 tools and sysroot |
| `//bootstrap:stage1` | Merges the tools with Flatware, translates them to C, and compiles that output as C into native Fix ELFs |
| `//bootstrap` (`:seed`) | Writes the seed repository archive |
| `//bootstrap:compilations` | Rebuilds every seed ELF inside Fix; its archive carries the compile coupons |
| `//bootstrap:stage2` | Opt-in test: the rebuilt certificates equal the seed's |

Every native ELF uses `UNITS` translated C units (`native.bzl`, currently 512);
the generated `compilation_script.fix` uses the same count, so Stage2 reproduces the seed byte for byte.
The native runtime uses C and Rust. Wasm exceptions use numeric tags and
setjmp/longjmp.

`--//bootstrap:debug` builds the seed's dependencies in debug mode.

```sh
bazel build //bootstrap
fix init bazel-bin/bootstrap/seed.tar --precompile
export FIX_TRUST="$(fix repository)/trust"
```

## Seed

`bootstrap/seed.rs` writes the seed: `seed create DIRECTORY COMPILER WASM2C
CLANG TOOLCHAIN [NAME=ELF]...`. The trusted author, and the sole `.fix/trust`
entry, is a Ref to the compiler wrapper ELF. The tool issues directly:

- `fixpoint`, the wrapper's Runnable certificate authored by itself;
- Runnables for clang, wasm2c, and each `NAME=ELF`;
- the `compile-resources` Bootstrap tag, authored by the trusted author, over
  `(wasm2c clang toolchain)`. `toolchain` is the `/toolchain` Flatware tree
  (`usr/` from Stage0 plus `share/fix/` shell resources), imported natively
  with symlinks followed.

The seed has no coupons. It labels `trusted-compiler`, `compile` (the closure
`("" fixpoint compile-resources)`), `scheduler`, `collector`, `clang`, `lld`,
`wasm2c`, and `wasm-merge`. `flatware` labels the adapter and control bridge
merged into one Wasm Blob; `flatware-control` also labels the standalone bridge.
`wasip1-sysroot` labels directory contents, ready to mount at `/usr`.

The Binutils equivalents are listed in `tools.bzl`: `llvm-ar`, `llvm-as`,
`llvm-cvtres`, `llvm-cxxfilt`, `llvm-dis`, `llvm-dwarfdump`, `llvm-dwarfutil`,
`llvm-dwp`, `llvm-link`, `llvm-mc`, `llvm-ml`, `llvm-nm`, `llvm-objcopy`,
`llvm-objdump`, `llvm-readobj`, `llvm-size`, `llvm-strings`, `llvm-symbolizer`,
and `llvm-undname`. To use aliases such as `llvm-ranlib`, `llvm-strip`,
`llvm-readelf`, and `llvm-addr2line`, import the canonical program and pass the
alias as its first argument. For example, run the `llvm-ar` program with
`["llvm-ranlib", "/work/lib.a"]`. Clang and LLD drivers work the same way.
The enabled code-generation targets remain X86 and WebAssembly.

This includes offline object, archive, binary inspection, assembly, and symbol
tools. It excludes JITs and process-launching tools: `llvm-rc` normally launches
a preprocessor, and `llvm-mt` requires XML support absent from this toolchain.

Applying `compile` to a Wasm module, as in `#("" compile module)`, yields a
Runnable for that module, compiled as one translated C unit per 128 KiB of Wasm.
An explicit unit count, as in `#("" compile module 512u64)`, overrides that;
the generated script passes the count the native seed build used.
`seed add DIRECTORY NAME=ELF...` issues further Runnables authored by an
existing seed's `trusted-compiler`; tests use it to add their programs.

## Stage2

The generated `compilation_script.fix` applies `compile` to every canonical
seed module and returns `(label, value)` pairs. Each tool is
compiled once. The compiler entry contains the `compile` closure. Fix can
reproduce Runnable certificates because `compile`
issues them with the trusted author; it cannot issue the Bootstrap tag, so the
closure reuses the seed's. `//bootstrap:stage2` checks that the closure and every
rebuilt Runnable equal the seed's. Import `//bootstrap:compilations` to reuse
those compilations elsewhere.
