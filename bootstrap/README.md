# Bootstrap

The bootstrap downloads the Fix LLVM SDK and WASIp1 rootfs with pinned SHA-256
hashes. It builds wasm2c and the Wasm merger, then converts the Fix compiler
wrapper, tools, scheduler, and coupon collector into native Fix ELFs. The first
native build uses Clang and the released LLD. CI selects Clang from the pinned
native LLVM SDK; local builds discover Clang on the host. The SDK and WASIp1
rootfs have separate release pins. A seeding tool writes these programs into a
repository.

| Target | What it does |
|---|---|
| `//bootstrap:stage0` | Stages the released WASIp1 tools and sysroot with wasm2c and the Wasm merger |
| `//bootstrap:stage1` | Merges the tools with Flatware, translates them to C, and compiles that output as C into native Fix ELFs |
| `//bootstrap` (`:seed`) | Writes the seed repository archive |
| `//bootstrap:compilations` | Rebuilds the compiler inside Fix; its archive carries the compile coupon |
| `//bootstrap:stability` | Opt-in test: two successive self-hosted compiler generations produce identical certificates |

Bootstrap unit counts are fixed in `native.bzl`: 512 for the tools, scheduler,
and collector, and one for the compiler wrapper. Both the host-built wrapper
and its self-hosted generations use `COMPILER_UNITS`.
The reference release build measured 46,342 Wasm bytes, 654,313 generated C bytes,
and 101,800 object-file bytes. The Wasm fits below the normal 128 KiB per-unit
threshold. The stability check declares 1 GiB for the wrapper, 1 GiB for wasm2c,
4 GiB per Clang invocation, and an 8 GiB VM.
The native runtime uses C and Rust. Wasm exceptions use numeric tags and
setjmp/longjmp. Fix Shell uses a 32 MiB native stack for translated programs.

`--//bootstrap:debug` builds the seed's dependencies in debug mode.

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
```

`fix bootstrap` creates a store at `$XDG_DATA_HOME/fix/bootstrap/.fix` (default
`~/.local/share/fix/bootstrap/.fix`), imports and signs the
seed's compiler, parser, scheduler, and collector, and trusts the compiler and
collector. `--unsigned` is for isolated staging builds; it leaves newly
generated Tags and coupons unsigned. Bootstrap records these seed
program handles in XDG user configuration. Later label changes do not change
the selections. The seed's scheduler is Hint.

## Seed

`bootstrap/seed.rs` writes the seed: `seed create DIRECTORY COMPILER WASM2C
CLANG TOOLCHAIN [NAME=ELF]...`. The trusted author, and the sole `.fix/trust`
entry, is a Ref to the compiler wrapper ELF. The tool issues directly:

- `fixpoint`, the wrapper's Runnable certificate authored by itself;
- Runnables for clang, wasm2c, and each `NAME=ELF`;
- the `compile-resources` Bootstrap tag, authored by the trusted author, over
  `(wasm2c clang toolchain)`. `toolchain` is the `/toolchain` Flatware tree
  (the linker and required headers from Stage0, plus `share/fix/` shell
  resources), imported natively with symlinks followed.

Seeds are unsigned staging artifacts. Import stores their Tags without authorizing
them. `--sign` signs the imported Tags with the selected user identities;
`fix sign --all` can sign them later. Compiler trust is a separate per-user
setting. Build actions use explicit unsigned staging mode.

The seed has no coupons. It labels `trusted-compiler`, `compile` (the closure
`("" fixpoint compile-resources)`), `scheduler`, `collector`, and `fix-parser`.
Additional tools can be compiled or imported later.
The enabled code-generation targets remain X86 and WebAssembly.

Applying `compile` to a Wasm module, as in `#(536870912u64 compile module)`, yields a
Runnable for that module, compiled as one translated C unit per 128 KiB of Wasm.
An explicit unit count, as in `#(4294967296u64 compile module 512u64)`, overrides that.
An optional final pair `(wasm2c_bytes clang_bytes)` overrides tool memory budgets;
zero selects the default for that tool. For example,
`#(4294967296u64 compile module 512u64 (52613349376u64 17179869184u64))`
gives wasm2c 49 GiB and each Clang invocation 16 GiB. The CLI exposes these as
`fix compile MODULE [UNITS] --wasm2c-memory MIB --clang-memory MIB`.
`--compile-memory MIB` overrides the wrapper's own budget. Its CLI default is
512 MiB plus 4 MiB per C job, including the runtime source. The job count uses
the explicit unit count or the known Blob length; an unevaluated producer with
an unknown output length uses one unit for this estimate.

The default Clang compilation budget is 2 GiB plus 384 bytes per source byte
for the first 16 MiB and 96 bytes per source byte beyond that. The fixed
runtime C source uses the 2 GiB base budget. In an earlier Clang bootstrap,
three partitions of about 14, 19, and 90 MiB ended with charges of about 4.7,
4.7, and 8.7 GiB, respectively. The two rates give smaller jobs more room per
source byte while keeping the largest reservations proportional to their size.

The wasm2c budget is 4 GiB plus 512 times the Wasm byte length. The linking
budget is 4 GiB plus 64 times the total object-file byte length. These
heuristics budget for Fix's conservative memory charges. Individual inputs
can need an explicit override. The VM needs enough available memory to admit
each invocation's declared budget.

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
