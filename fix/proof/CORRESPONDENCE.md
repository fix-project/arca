# Isabelle to Rocq correspondence

This audit follows the `Wasm-Proof` session's `isabelle_coupon` and
`make_coupon` roots and their imports. The original constructor executions are preserved in the imported history.
The statement/assumption audit and clean pinned-dependency build passed.
The audited reference is the `Wasm-Proof` tree at repository commit
`9e82d4effeec266217a32e52a1325d95554ac605`, with WasmCert-Isabelle at
`4119eaf4433be6358a4dcc4d99cf6b468a809fe2`. The generated `Init` imported `Host`,
so host equations are included even though `Host` was not a direct root import.

## Proof structure

Digestion has since been removed from the current model. Its original Isabelle
theory remains historical; the Rocq interpreter and congruence proofs cover the
three implemented thunk constructors.

The semantic development retains the sequence of handle operations,
evaluation, congruence, equivalence, equivalence closure, coupon judgements,
guarded constructors, and Wasm executions. Storage, programs,
and externref embeddings are module parameters. Their assumptions are kept
separate from the proved conclusions.

Tree evaluation preserves its Tag marker. Tree equivalence and the Tree
coupon rules require matching markers.

Identification Thunks enclose Refs, and both the program opcode and typeless
constructor normalize Object inputs to Refs. This refines Isabelle's
Data-parameterized constructor to match the specification and native shell.

The six evaluation functions still use fuel together. Rocq computes a record
of evaluators by structural recursion on fuel, using separate list recursion.
Padding precedes determinism, which justifies the unbounded option-valued
evaluators. `Forall2` induction replaces `Forall2_induct`; a finite list of
successful evaluations is padded to one common fuel before building a tree.
`evals_to_tree_to`, `eval_entry_to_eval_tree`, and
`eval_tree_to_eval_entry` preserve this sequence.
The last theorem supplies explicit output witnesses and their `Forall2`
specification in place of guarded `the (eval x)` applications.

The congruence induction remains coupled across thinking, forcing, and
evaluation. Coinductive equivalence uses a mutual relation over handles and
lists to satisfy Rocq's productivity check. Coupon soundness uses mutual
induction over six judgements, with nested list induction for Trees.

Constructor executions retain table reads, local assignments, host calls,
ordered guards, and the successful return or trap. Their abstract builders
are proved independently and then connected to the actual parsed Wasm
bodies. Shared guard and context lemmas replace Isabelle custom methods;
these lemmas prove reductions, rather than assuming interpreter equations.

The current collector adds the explicit exchanges needed to certify terminating
Fix evaluations. `collector/wasm/Refinement.v` connects the parsed 26-exchange binary
to `CouponExchanges.check` and `issue`, including conditional authority
propagation. The current native proof states host-interface and resource assumptions;
exact-comparison success additionally requires representation readiness.

## Renamed and combined support results

The statement audit checked definitions, premises, and conclusions under the
translations below. The Fix operation names are retained. Computational
equivalence is named `computational_equiv`, exact identity testing `handle_eqb`,
and relational evaluation `eval_derivation`; these distinguish semantic relations
from handle comparison and issued certificates.

| Isabelle result | Rocq counterpart | Translation |
| --- | --- | --- |
| `think_with_fuel_padding`, `force_with_fuel_padding`, `execute_with_fuel_padding`, `eval_with_fuel_padding`, `eval_tree_with_fuel_padding` | `think_padding`, `force_padding`, `execute_padding`, `eval_padding`, `eval_tree_padding` | Projections of the coupled `fuel_padding` theorem |
| `thinks_to_deterministic`, `forces_to_deterministic`, `executes_to_deterministic`, `evals_to_deterministic`, `evals_tree_to_deterministic` | `think_deterministic`, `force_deterministic`, `execute_deterministic`, `eval_deterministic`, `eval_tree_deterministic` | Pad both successful fuel witnesses to common fuel |
| `endpoint_some`, `endpoint_unique` | `unfuel_some` and the five `*_some`/`*_unique` results | Classical indefinite choice selects an actual successful witness; determinism identifies it with any other successful witness |
| `rel_state_hpush` | `rel_hpush` | `Forall2` constructor and unchanged data list |
| `rel_state_hs`, `rel_state_ds` | Projections of `rel_state` | Record update reasoning becomes conjunction elimination |
| `rel_state_hs_same_length`, `rel_state_hs_nth` | `Forall2_length`, `Forall2_nth_error`, `Forall2_nth` | Explicit bounded list lookup |
| `create_tree_same_typed`, `get_tree_same_typed` | `typed_tree_complete`, `typed_tree_nth` | Both semantic and same-type relations are transported together |
| `Forall2_append`, `rel_state_hpush_self` | Standard `Forall2_app` with a singleton relation, and specialization of `rel_hpush` | Preserve the original appended-pair fact and reflexive push fact |

## Native collector

The original thirteen-exchange execution proof is preserved in the imported
history. The current collector replaces it with the explicit exchange interface
and Ref-backed witnesses. `collector/wasm/Refinement.v` proves native validation, success
under comparison-readiness contracts, and conditional issuance against
`CouponExchanges`. `WasmExecution` supplies finite Wasm reductions under
instruction, label, and frame contexts. `WasmGuards` supplies guard and
witness-authentication proofs.

The native proof assumes the storage and program interfaces, externref embedding,
Fix host operations, byte-identity contracts, and explicit resource bounds.
The concrete Arca implementations of these interfaces are not formally verified.
Independent Rocq kernel checking covers the retained proof modules. Bazel generates
the embedded binary from the same Wasm artifact used by the collector. No execution axioms or admitted proofs are introduced.
