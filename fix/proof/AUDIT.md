# Auditing the Fix proofs

The abstract proof establishes evaluation and certification properties. The Wasm
proof connects the collector binary to the abstract checker. Audit these claims
separately: abstract certification completeness does not imply that every abstract
derivation succeeds in the finite Wasm implementation.

## Source layers

| Layer | Contents | Reading purpose |
| --- | --- | --- |
| External specification | Abstract entry modules and Wasm `specification/` | Check what the definitions mean, what is assumed, and what the results establish |
| Internal structure | Each area's `internal/` directory | Follow the mathematical argument through supporting abstractions and lemmas |
| Proof machinery | Each area's `machinery/` directory | Inspect fuel bookkeeping, guarded coinduction, mutual inductions, instruction traces, and tactics |

The layers organize sources, not trust. Every included source is compiled and
kernel-checked. Public theorem proofs apply checked implementation lemmas; they
introduce no new axioms. Existing module names are preserved. `Load` assembles
the fragments in the entry modules, with module and section boundaries kept in
the entry files when their contents span several fragments. The Wasm helpers
still share one `WasmEntry` context.

## Abstract specification

Read definitions and assumptions before the theorem statements. In particular,
read the semantic interpretation of a coupon, not just its inference rules.

| Subject | Audit source |
| --- | --- |
| Values and handle constructors | [Types.v](semantics/Types.v), [Evaluation.v](semantics/Evaluation.v) |
| Storage assumptions | [STORAGE](storage/Interface.v) |
| Program assumptions and primitive operations | [PROGRAM](shell/Interface.v), [ApplyTree.v](semantics/ApplyTree.v), [Select.v](semantics/Select.v), [Identify.v](semantics/Identify.v) |
| Think, Force, Execute, Eval, and Value preservation | [Properties.v](evaluation/Properties.v) |
| Finite evaluation rules and adequacy | [Adequacy.v](evaluation/Adequacy.v) |
| Internal generating relation and supporting equivalence closure | [Equivalence.v](equivalence/Equivalence.v), [Closure.v](equivalence/Closure.v) |
| Certified equivalence, evaluation, and Value indistinguishability | [Equivalence.v](equivalence/specification/Equivalence.v), assembled inside `Indistinguishability.Certified` |
| Raw observations, equality, comparability, and program substitution | [Indistinguishability.v](equivalence/Indistinguishability.v) |
| Coupon rules and their semantic interpretations | [Logic.v](collector/Logic.v) |
| Encoded claims, exchanges, authorization, and conditional validity | [Encoding.v](collector/Encoding.v), [Exchanges.v](collector/Exchanges.v) |
| Available certificates and certification | [Completeness.v](collector/Completeness.v) |
| Preservation when an authenticated Tag is evaluated | [Preservation.v](collector/Preservation.v) |

The main claims are:

- `eval_adequacy`: the finite rules describe exactly successful evaluation.
- `eval_to_value_handle` and `value_handle_eval_to_itself`: evaluation produces
  Values, and Values evaluate to themselves.
- `Certified.equiv_eval`: equivalent inputs agree on result existence and have
  indistinguishable results. `Certified.equiv_values` identifies equivalence with
  indistinguishability for Values, including Thunks and Refs.
- `indistinguishable_program`: ordinary program execution preserves
  indistinguishability. Thunks expose both strict and shallow evaluation.
- `coupon_sound` and `coupon_blob_same_data`: coupon derivations have their stated
  semantic meanings and cannot equate Blob Objects with different bytes.
- `check_sound` and `issue_conditional`: accepted exchanges preserve claims and
  propagate conditional authority.
- `certification_sound` and `terminating_evaluation_is_certifiable`: issued
  certificates are sound, and every terminating evaluation has a finite issuance
  derivation using public exchanges.
- `evaluation_preserves_authority`: evaluating a Ref-backed coupon preserves its
  claim and stable conditional authority.

Certification's primitive assumptions are actual Apply results for Value
combinations and actual Select results. Think, Force, and Eval certificates must
be derived. `force_result` and `eval_result` allow the equivalent representatives
specified in `Logic.v`; Force compares lowered results using indistinguishability,
and primitive Apply and Select retain exact results. Ref congruence accepts only
Value Objects. `Closure.v` supplies a weaker relation used internally; it is not
the meaning of an `Equivalent` coupon.

For the internal argument, start with [collector construction](collector/internal/Completeness/Construction/certify_leaf.v)
and [issuance](collector/internal/Completeness/Issuance/read_encoded.v),
[rule soundness](collector/internal/Logic/Rules.v), and
[evaluation congruence](evaluation/internal/Properties/Congruence.v).
The fuel evaluator, transport inductions, auxiliary coinductive relation, and
mutual coupon/certification inductions live in `machinery/`.

## Wasm specification

[Backend.v](collector/wasm/Backend.v) lists the context assumptions;
[Entry.v](collector/wasm/Entry.v) assembles the proof and retains its public names.
The following predicates and results are the audit surface:

| Contract | Audit source |
| --- | --- |
| A checked execution may trap or satisfy its success postcondition | [checked](collector/wasm/specification/Checked/checked.v) |
| Existing descriptors survive; additional allocation is bounded | [retains and scope](collector/wasm/specification/Support/word.v) |
| Exact-comparison readiness | [observed_comparison and observed_same](collector/wasm/specification/Support/compared.v) |
| Memory and nested-length bounds | [memory_capacity](collector/wasm/specification/Resources/memory_capacity.v), [lengths_bounded](collector/wasm/specification/Validation/Lengths.v) |
| Witness authorization and witness-count limit | [authorized](collector/wasm/specification/AuthorityValidation/Authorized.v), [witness_limit](collector/wasm/specification/AuthorityGuard/witness_limit.v) |
| Requests requiring Tag reconstruction | [tree_request](collector/wasm/specification/EntryFields/TreeRequest.v) |
| Fresh output descriptor, claim, and certificate shape | [collector_output_executes](collector/wasm/specification/Output/collector_output_executes.v) |
| Primitive dispatch refines the abstract checker | [collector_dispatch_validation](collector/wasm/specification/NativeDispatch/collector_dispatch_validation.v) |
| Exported ordinary execution authenticates witnesses and issues the checked claim | [exported_exchange_refinement](collector/wasm/specification/Export/exported_exchange_refinement.v) |
| Exported Tag reconstruction establishes the Eval claim | [exported_tree_exchange_sound](collector/wasm/specification/Export/exported_tree_exchange_sound.v) |
| The witness limit fits descriptor capacity from the initial request | [initial_descriptor_budget](collector/wasm/specification/Export/initial_descriptor_budget.v) |

`checked` has an existential finite execution ending in either a trap or the
postcondition. Its refinement results do not guarantee successful issuance.
The separate primitive success arguments in
[NativeSuccess.v](collector/wasm/internal/NativeSuccess.v),
[BinarySuccess.v](collector/wasm/internal/BinarySuccess.v), and
[ForceSuccess.v](collector/wasm/internal/ForceSuccess.v) require comparison
readiness and resource bounds. The descriptor model permits closing and reuse;
the collector retains its descriptors and bounds its witness count instead.

The internal argument follows witness authentication, ordered premise checking,
Tree traversal, dispatch, authority, and issuance. Instruction reductions,
guard traces, continuation prefixes, binary layout, and tactic definitions are
in Wasm `machinery/`. The embedded `Binary.v` is generated from the built
`collector.wat`, not a separately maintained program model.

## Assumptions and limits

Storage and program interfaces are abstract contracts. Unbounded evaluators use
classical choice; functional extensionality is also used. The native proof adds
WasmCert's operational semantics, the Shell host model, and the privileged
handle-observation contract. Audit [Descriptors.v](shell/Descriptors.v),
[Memory.v](shell/Memory.v), [Host.v](shell/Host.v), and
[Privileged.v](shell/Privileged.v) when assessing that boundary.

Native encoding, canonicalization certificates, trusted issuance, Tag creation,
Runnable admission, and ELF loading still have unverified implementation
boundaries. Program substitution and indistinguishability of equivalent Values
are checked within the abstract program model. No arbitrary-program termination
theorem is claimed. [STATUS.md](STATUS.md) records detailed coverage;
[CORRESPONDENCE.md](CORRESPONDENCE.md) records the historical correspondence.

Run `bazel test //fix/proof:check` to compile and independently kernel-check the
development. Bazel lists every fragment as an input of its existing module.
The interactive Makefile compiles only entry modules; Rocq's dependency analysis
tracks their included fragments.
