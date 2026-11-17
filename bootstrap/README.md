# Bootstrap

The bootstrap downloads the Fix LLVM SDK and WASIp1 rootfs with pinned SHA-256
hashes. It builds wasm2c and the Wasm merger, then converts the Fix compiler
wrapper, tools, scheduler, and coupon collector into native Fix ELFs. The first
native build uses the host Clang and the released LLD. A seeding tool writes
these programs into a repository.

| Target | What it does |
|---|---|
| `//bootstrap:stage0` | Stages the released WASIp1 tools and sysroot with wasm2c and the Wasm merger |
| `//bootstrap:stage1` | Merges the tools with Flatware, translates them to C, and compiles that output as C into native Fix ELFs |
| `//bootstrap` (`:seed`) | Writes the seed repository archive |
| `//bootstrap:compilations` | Rebuilds the compiler inside Fix; its archive carries the compile coupon |
| `//bootstrap:stability` | Opt-in test: two successive self-hosted compiler generations produce identical certificates |

Every native ELF uses `UNITS` translated C units (`native.bzl`, currently 512).
Each self-hosted compiler generation uses the same count.
The native runtime uses C and Rust. Wasm exceptions use numeric tags and
setjmp/longjmp.

`--//bootstrap:debug` builds the seed's dependencies in debug mode.

```sh
bazel build //fix/cli:fix //bootstrap:seed
fix="$PWD/bazel-bin/fix/cli/fix"
seed="$PWD/$(bazel cquery //bootstrap:seed --output=files)"
"$fix" trust add-agent
mkdir -p fix-repo
cd fix-repo
"$fix" init "$seed" --bootstrap
```

`--bootstrap` trusts the seed compiler, signs the imported Tags, precompiles
the embedded programs, and trusts the collector. The seed supplies a native
scheduler and collector. `--unsigned` is for staging builds; using it with
`--precompile` or `--bootstrap` leaves newly generated Tags and coupons
unsigned, even if the imported seed was signed.

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

Seeds are unsigned staging artifacts. Import stores their Tags without authorizing
them. `--sign` signs the imported Tags with the selected user identities;
`fix sign --all` can sign them later. Compiler trust is a separate per-user setting. Build and precompilation actions use explicit
unsigned staging mode.

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
the stability check passes the count the native seed build used.
`seed add DIRECTORY NAME=ELF...` issues further Runnables authored by an
existing seed's `trusted-compiler`; tests use it to add their programs.

## Stability

Each generation applies `compile` to the compiler module. The first result uses
the host-compiled seed compiler. `seed promote` then makes the rebuilt compiler
the trusted author and issues its Runnable and Bootstrap resource certificates.
The same process is repeated for the next generation. `//bootstrap:stability`
checks that the following generation produces exactly the same result Tree,
including its certificates. Import `//bootstrap:compilations` to reuse the
first compiler compilation coupon elsewhere.
