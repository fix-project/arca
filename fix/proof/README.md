# Fix proofs

The primary results are soundness and completeness of the Fix evaluation rules,
and correctness of the coupon collector against those rules.

| Result | Statement | Proof |
| --- | --- | --- |
| Value preservation | Successful evaluation produces a Value; evaluating a Value leaves it unchanged | `eval_to_value_handle`, `value_handle_eval_to_itself` in [`Properties.v`](evaluation/Properties.v) |
| Equivalence preservation | Equivalent inputs agree on whether a result exists, and their results are equivalent | [`equiv_eval`](equivalence/Closure.v) |
| Evaluation soundness and completeness | `eval_derivation e r` iff `eval e = Some r` | [`eval_adequacy`](evaluation/Adequacy.v) |
| Blob-content safety | Equivalent Blob Objects have identical contents | [`coupon_blob_same_data`](collector/Logic.v) |
| Full exchange soundness | Every accepted exchange preserves the logical judgement and the assumptions of its premises | `CouponExchanges.check_sound`, `issue_conditional` in [`Exchanges.v`](collector/Exchanges.v) |
| Evaluation certification | Every terminating evaluation has a finite trace through public exchanges; issued claims are sound | [`terminating_evaluation_is_certifiable`, `certification_sound`](collector/Completeness.v) |
| Native validation and issuance | Execution implements the abstract exchange checker and issuer | [`collector_refinement`](collector/wasm/Refinement.v) |

Evaluation is partial and uses big-step rules. Adequacy characterizes successful
runs; it does not assert that every non-Value can take a step. Invalid applications
and selections may fail. Certification completeness says that every terminating
evaluation can be justified through collector exchanges.

`//fix/proof/collector:collector` builds `collector/collector.wat`. Its request is
`[metadata, collector, rule, lhs, rhs, witnesses]`, with an explicit eight-byte
little-endian rule code. The exchange codes and conclusion kinds are defined in
[`fix/runtime/src/coupon.rs`](../runtime/src/coupon.rs).
It exposes equivalence, Eval, Think, Force, Apply, and Select claims.
Witnesses are ordered; the collector does not search for a derivation.

The abstract exchange checker is sound, including conditional authority
propagation. Native validation covers all 26 exchanges, constructor guards, and
Tree loops. The exported entry point implements `check` and `issue`, or traps.
`issue_conditional` supplies authority soundness for the issued claim. These
statements use explicit host-interface and resource contracts.
All 26 success paths also have execution proofs with explicit comparison-readiness
requirements: different representations of the same abstract handle can cause
exact comparison to reject a derivation.

Certification completeness uses the public exchanges, with only executor Apply
and storage Select facts as primitive computation certificates. Think, Force,
and Eval certificates are derived.

In `collector/Logic.v`, `h =[ Rule ]=> r` denotes a coupon judgement. The collector only
validates performed computation; it never evaluates its inputs. Its fixed-order
witnesses are `[Tag, assertion Object]` pairs. Each Tag contains
`[authors, Ref([operation, input, result])]`; checking the supplied assertion
against its Ref requires no storage access. The Ref preserves the claim under
Tag evaluation. Apply certificates require Value combinations; storage Select
certificates do not evaluate their operands.

Conditional validity means that trusting every author implies the claim.
Additional authors remain unrestricted. Preservation across Tag evaluation uses
the stronger issuance invariant that stable authors suffice for the claim.
Evaluation of an authenticated Ref-backed Tag preserves its operation, input,
result, and conditional validity with the same supplied assertion Object.
`collector/Preservation.v` and `shell/TagCreation.v` supply the supporting results.
Native Tag creation and trusted certificate issuers remain host contracts.

The equivalence and indistinguishability proofs check the underlying definitions.
Computational equivalence concerns evaluation; indistinguishability also
preserves observations such as type, contents, comparability, and length.
For Comparable handles the two coincide. Length is available only for Objects
and Refs, and the native ABI returns an unsigned 64-bit integer. Full program
substitution and the intended indistinguishability of computationally equivalent
Thunks remain unfinished.

The storage, program, and externref interfaces are assumptions
of the proofs. The native encoding laws, privileged handle-byte identity,
canonicalization certificates, Runnable admission, and ELF loading still have
unverified implementation boundaries. Resource bounds are explicit in the Wasm
execution theorems. No proof of arbitrary program termination is claimed.

[STATUS.md](STATUS.md) records detailed coverage and remaining
gaps. [CORRESPONDENCE.md](CORRESPONDENCE.md) compares the statements
with the original Isabelle development and records assumptions.

## Structure

- `semantics/`: Fix value types, structural relation laws, primitive operations, and fuelled evaluation.
- `storage/`: the immutable Object store interface.
- `evaluation/`: determinism, evaluator equations, and relational adequacy.
- `equivalence/`: computational equivalence and observational indistinguishability.
- `shell/`: the modelled Fix Shell operations, Tag creation, Runnable admission,
  and their Wasm host interpretation.
- `collector/`: coupon judgements, encoding, exchanges, certification completeness,
  and authority preservation, alongside `collector.wat`.
- `collector/wasm/`: binary/layout facts, execution helpers, witness authentication,
  acceptance proofs, output construction, and final implementation refinement.
- `support/`: shared integer and Wasm arithmetic lemmas.

Implementation requirements are owned by the API they describe:

- [`storage/Interface.v`](storage/Interface.v): immutable Object storage,
  comparability, and well-founded Tree containment.
- [`shell/Interface.v`](shell/Interface.v): deterministic programs modelled as
  sequences of Fix Shell operations.
- [`shell/Externref.v`](shell/Externref.v): embedding handles in Wasm externrefs.
- [`shell/Privileged.v`](shell/Privileged.v): privileged exact-handle inspection.
- [`shell/Memory.v`](shell/Memory.v): attachment through WasmCert’s memory interface.

`semantics/Handle.v` contains derived relation laws, not implementation assumptions.
The native modules assemble these interfaces in `collector/wasm/Backend.v`.
`Refinement.v` connects the parsed collector binary to the abstract checker and
issuer; authority soundness follows from `issue_conditional`.

`computational_equiv` is computational equivalence; `handle_eqb` tests exact
handle identity, while `compare_handles` is the Comparable-handle operation.
`eval_derivation` describes evaluation rules; `certifies` describes an issued
certificate. `ShellHost` interprets the Fix Shell API; the `Wasm*` modules prove
execution of the collector binary.

## Exchange interface

Requests are `[metadata, collector, rule, lhs, rhs, witnesses]`. Rule and statement
codes are unsigned 64-bit little-endian Blobs. Each witness is
`[Tag([authors, Ref(assertion)]), assertion]`, where the supplied assertion Object
is `[statement, lhs, rhs]`. The collector authenticates its Ref and checks the
premises in the order below. It never loads Refs or performs evaluation.

The result is `Tag([authors, Ref([statement, lhs, rhs])])`, with the premise
authorities and the collector's issuing identity. Statement codes are
Equivalent `0`, Eval `1`, Apply `2`, Force `3`, Think `4`, Select `6`.
Storage-issued canonicalization facts use ordinary equivalence certificates.

Here `a ≃ b` denotes computational equivalence. `App`, `Sel`, and `Id` construct
Thunks; `Strict` and `Shallow` construct Encodes. All endpoints are supplied in
the request. Lowering turns an Object into its Ref; lifting turns a Ref into
its Object. A lifting exchange requires that Object as an explicit endpoint.

| Code | Rule | Conclusion | Ordered premises |
| --- | --- | --- | --- |
| 0 | TreeEq | `Tree(xs) ≃ Tree(ys)` | `xs[i] ≃ ys[i]` for each child |
| 1 | ForceResultEq | `a ≃ b` | `Force(a, x)`, `Force(b, y)`, `x ≃ y` |
| 2 | ThinkApplication | `Think(App(t), r)` | `Eval(t, u)`, `Apply(u, r)` |
| 3 | ThinkToForce | `Force(t, d)` | `Think(t, d)`, with Data result `d` |
| 4 | EqApplication | `App(t) ≃ App(u)` | `t ≃ u` |
| 5 | EvalLeaf | `Eval(x, x)` | None; `x` is a Blob Object, Ref, or bare Thunk |
| 6 | EvalTree | `Eval(Tree(xs), Tree(ys))` | `Eval(xs[i], ys[i])` for each child |
| 7 | ForceToEncodeStrict | `Strict(t) ≃ lift(d)` | `Force(t, d)` |
| 8 | EvalEq | `Eval(b, r)` | `Eval(a, r)`, `a ≃ b` |
| 9 | EqEncodeStrict | `Strict(a) ≃ Strict(b)` | `a ≃ b` |
| 10 | Symmetry | `a ≃ b` | `b ≃ a` |
| 11 | Transitivity | `a ≃ c` | `a ≃ b`, `b ≃ c` |
| 12 | Reflexivity | `a ≃ a` | None |
| 13 | ThunkEq | `a ≃ b` | `Think(a, b)`, with bare Thunk result `b` |
| 14 | ThunkStrictEq | `a ≃ b` | `Think(a, Strict(b))` |
| 15 | ThunkShallowEq | `a ≃ b` | `Think(a, Shallow(b))` |
| 16 | ForceToEncodeShallow | `Shallow(t) ≃ lower(d)` | `Force(t, d)` |
| 17 | EqSelection | `Sel(t) ≃ Sel(u)` | `t ≃ u` |
| 18 | EqIdentification | `Id(a) ≃ Id(b)` | `a ≃ b`, with Ref endpoints |
| 19 | EqEncodeShallow | `Shallow(a) ≃ Shallow(b)` | `a ≃ b` |
| 20 | EvalResultEq | `a ≃ b` | `Eval(a, b)` |
| 21 | TreeRefEq | `Ref(t) ≃ Ref(u)` | `t ≃ u`, with Tree Object endpoints |
| 22 | BlobRefEq | `Ref(a) ≃ Ref(b)` | `a ≃ b`, with Blob Object endpoints |
| 24 | ForceEq | `Force(b, r)` | `a ≃ b`, `Force(a, r)` |
| 25 | ThinkSelection | `Think(Sel(t), r)` | `Eval(t, u)`, `Select(u, r)` |
| 26 | ThinkIdentification | `Think(Id(r), r)` | None; `r` is a Ref |

Tree rules require equally sized Object Trees with matching Tag markers.
Application and selection congruence take Tree Objects. Encode congruence takes
bare Thunks. Apply and Select premises are issued by the kernel and storage,
respectively. All exchanges validate supplied facts without performing computation.

## Checking

The development uses Rocq 9.1.1, RocqHammer tactics 1.3.2+9.1, and
[WasmCert-Coq](https://github.com/WasmCert/WasmCert-Coq), pinned to
`5e6df8d60c94aa5dbeff633f5eb48caa6c64c225` (package version 2.2.1).
Install opam, Python 3.11+, and native build tools
(Clang, make, m4, pkg-config, and GMP development headers), then run:

```sh
bazel test //fix/proof:check
```

Bazel builds a private opam switch in its external repository cache using
[`build/rocq-toolchain.opam.locked`](../../build/rocq-toolchain.opam.locked).
It pins OCaml, Rocq, WasmCert, and transitive package versions without changing
or using the active opam switch. Initial provisioning builds these dependencies;
subsequent runs reuse them. Opam builds the third-party toolchain; Bazel caches
our proof modules individually.

This compiles the development and independently
kernel-checks every proof module. Separate Bazel targets let unchanged modules
reuse their cached checks; they do not define separate public proof obligations.

Bazel generates `collector/wasm:binary_source` from the same Wasm artifact used
by `//fix/proof/collector:collector`. Changes to the collector automatically
regenerate the embedded binary and rebuild the affected proofs.

For interactive editing, install host Rocq 9.1.1, Stdlib 9.0.0, and the proof
libraries listed above, then run from the repository root:

```sh
make -f fix/proof/Makefile -j
```

This uses Bazel only to generate the collector's `Binary.v`. The host Rocq reads
that source in place and builds its own proof modules beside the sources, using
a generated `Makefile.rocq`. `_CoqProject` exposes only these host modules to
editors. Rebuild after changing dependencies, then restart affected sessions.

The normal Bazel build continues to use its private pinned toolchain and
separate compiled modules; it does not require a host Rocq installation.
