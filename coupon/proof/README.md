# Fix coupon proofs

This development proves the Fix evaluation and coupon rules in Rocq, and proves
that the constructors in `coupon.wat` return good coupons or trap as specified.
The development uses Rocq 9.1.1 and
[WasmCert-Coq](https://github.com/WasmCert/WasmCert-Coq), pinned to commit
`5e6df8d60c94aa5dbeff633f5eb48caa6c64c225` (package version 2.2.1).

The 35 proof modules are in `wasm-proofs/rocq`. They retain the development's
structure: handles, evaluation, congruence, coinductive equivalence, equivalence
closure, coupon judgements, guarded constructors, and Wasm execution.
[STATUS.md](wasm-proofs/rocq/STATUS.md) records coverage and verification;
[CORRESPONDENCE.md](wasm-proofs/rocq/CORRESPONDENCE.md) explains the Isabelle-to-Rocq
translations and assumptions.

## Checking with Bazel

Install opam, Python 3.11+, WABT (`wat2wasm`), and native build tools
(Clang, make, m4, pkg-config, and GMP development headers), then run:

```sh
bazel test //coupon:proof
```

Bazel builds a private opam switch in its external repository cache using
[`build/rocq-toolchain.opam.locked`](../../build/rocq-toolchain.opam.locked).
It pins OCaml, Rocq, WasmCert, and transitive package versions without changing
or using the active opam switch. Initial provisioning builds these dependencies;
subsequent runs reuse them. Opam builds the third-party toolchain; Bazel caches
our proof modules individually.

This compiles the development, checks generated binary freshness, and independently
kernel-checks every proof module. Separate Bazel targets let unchanged modules
reuse their cached checks; they do not define separate public proof obligations.

After changing `coupon/collector.wat`, regenerate its binary snapshot from the
repository root with:

```sh
python3 coupon/proof/scripts/generate-rocq-init.py
```

Bazel checks the generated snapshot against the collector binary.

Execution theorems use WasmCert-Coq's finite reduction semantics. Tree execution
states the original i32 size restrictions as explicit premises. The storage,
program, coupon-storage, and externref backends remain abstract interfaces;
their contracts and standard logical assumptions are documented in the
correspondence audit.
