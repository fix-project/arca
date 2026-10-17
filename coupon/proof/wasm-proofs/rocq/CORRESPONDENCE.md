# Isabelle to Rocq correspondence

This audit follows the `Wasm-Proof` session's `isabelle_coupon` and
`make_coupon` roots and their imports. All thirteen constructor executions and the final dispatcher are ported.
The statement/assumption audit and clean pinned-dependency build passed.
The audited reference is the `Wasm-Proof` tree at repository commit
`9e82d4effeec266217a32e52a1325d95554ac605`, with WasmCert-Isabelle at
`4119eaf4433be6358a4dcc4d99cf6b468a809fe2`. The generated `Init` imported `Host`,
so host equations are included even though `Host` was not a direct root import.

## Proof structure

The semantic development retains the sequence of handle operations,
evaluation, congruence, equivalence, equivalence closure, coupon judgements,
guarded constructors, and Wasm executions. Storage, programs, coupon storage,
and externref embeddings are module parameters. Their assumptions are kept
separate from the proved conclusions.

The six evaluation functions still use fuel together. Rocq computes a record
of evaluators by structural recursion on fuel, using separate list recursion.
Padding precedes determinism, which justifies the unbounded option-valued
evaluators. `Forall2` induction replaces `list_all2_induct`; a finite list of
successful evaluations is padded to one common fuel before building a tree.
The new `evals_to_tree_to`, `evals_to_tree_to_exists`,
`eval_entry_to_eval_tree`, and `eval_tree_to_eval_entry` preserve this sequence.
The last theorem supplies explicit output witnesses and their `Forall2`
specification in place of guarded `the (eval x)` applications.

The congruence induction remains coupled across thinking, forcing, and
evaluation. Coinductive equivalence uses a mutual relation over handles and
lists to satisfy Rocq's productivity check. Coupon soundness keeps the mutual
induction over all eight judgements, with nested list induction for trees.

Constructor executions retain table reads, local assignments, host calls,
ordered guards, and the successful return or trap. Their abstract builders
are proved independently and then connected to the actual parsed Wasm
bodies. Shared guard and context lemmas replace Isabelle custom methods;
these lemmas prove reductions, rather than assuming interpreter equations.

## Renamed and combined support results

Exact names already correspond for all named results in `identify`, `slice`,
`digest`, `evaluation_properties`, `equivalence`, `equivalence_closure`,
`coupon`, and `isabelle_coupon`. The statement audit also checked the definitions, premises, and conclusions
under the translations described below.

| Isabelle result | Rocq counterpart | Translation |
| --- | --- | --- |
| `think_with_fuel_padding`, `force_with_fuel_padding`, `execute_with_fuel_padding`, `eval_with_fuel_padding`, `eval_tree_with_fuel_padding` | `think_padding`, `force_padding`, `execute_padding`, `eval_padding`, `eval_tree_padding` | Projections of the coupled `fuel_padding` theorem |
| `thinks_to_deterministic`, `forces_to_deterministic`, `executes_to_deterministic`, `evals_to_deterministic`, `evals_tree_to_deterministic` | `think_deterministic`, `force_deterministic`, `execute_deterministic`, `eval_deterministic`, `eval_tree_deterministic` | Pad both successful fuel witnesses to common fuel |
| `endpoint_some`, `endpoint_unique` | `unfuel_some` and the five `*_some`/`*_unique` results | Classical indefinite choice selects an actual successful witness; determinism identifies it with any other successful witness |
| `rel_state_hpush` | `rel_hpush` | `Forall2` constructor and unchanged data list |
| `rel_state_hs`, `rel_state_ds` | Projections of `rel_state` | Record update reasoning becomes conjunction elimination |
| `rel_state_hs_same_length`, `rel_state_hs_nth` | `Forall2_length`, `Forall2_nth_error`, `Forall2_nth` | Explicit bounded list lookup |
| `create_tree_same_typed`, `get_tree_same_typed` | `typed_tree_complete`, `typed_tree_nth` | Both semantic and same-type relations are transported together |
| `list_all2_append`, `rel_state_hpush_self` | Standard `Forall2_app` with a singleton relation, and specialization of `rel_hpush` | Preserve the original appended-pair fact and reflexive push fact |

## Execution correspondence

| Isabelle theory | Rocq module | Correspondence |
| --- | --- | --- |
| `self_coupon` | `SelfCoupon` | Guard characterization, successful invocation, trap, coupon goodness |
| `eval_blobobj_coupon` | `EvalBlobCoupon` | Blob-type and equality guards, successful invocation, trap, coupon goodness |
| `sym_coupon` | `SymCoupon` | `make_some`/`make_none` analysis, table/local prefix, guards, success and trap |
| `trans_coupon` | `TransCoupon` | Both table/local prefixes, five guards, success and every failure class |
| `eq_application_coupon`, `eq_encode_strict_coupon` | `MappedEqCoupon` | Shared `make_some`, `make_some_rev`, `make_none`, host transformations, endpoint guards, success and trap; opposite equality operand orders are justified by Boolean symmetry |
| `think_to_force_coupon` | `ThinkToForceCoupon` | `make_some`/`make_none`, think tag, data check, endpoints, successful force coupon and trap |
| `force_to_encode_strict_coupon` | `ForceToEncodeStrictCoupon` | `make_some`/`make_none`, force tag, object check, endpoints, strict encoding, successful equality coupon and trap |
| `eval_eq_coupon` | `EvalEqCoupon` | `make_some`/`make_none`, two reads and assignments, evaluation/equality tags, three endpoint guards, good native return and all failure traps |
| `think_application_coupon` | `ThinkApplicationCoupon` | `make_some`/`make_none`, two reads and assignments, ordered tag/middle/endpoint guards, application-thunk creation success/failure, good native return and all failure traps |
| `force_result_eq_coupon` | `ForceResultEqCoupon` | `mt_some`/`mt_some_rev`/`mt_none`, three reads and assignments, three tag and four endpoint guards, good native return and all failure traps |
| `eq_tree_coupon`, `eval_tree_coupon` | `TreeLayout`, `LoopUtil`, `TreeCoupon` | Exact parsed bodies; `make_some`/`make_some_rev`/`make_none`; counter, stop and branch reductions; tag and endpoint loops; both size checks; native success and every failure trap, with explicit signed-size hypotheses |
| `make_coupon` prologue | `Dispatcher` | Exact parsed body/signature, all thirteen request encodings and indices, indirect-call reduction through the initialized table, both original labels, and native return/trap composition |
| `make_coupon` final conclusions | `MakeCoupon` | Original thirteen-case split, native child execution under both dispatcher labels, successful good coupon return, and failure trap |

Isabelle's fixed `run_iter` step-count equalities become finite reflexive
transitive WasmCert-Coq reductions. Interpreter instruction accounting differs
between the two libraries; the old `plus_*` and `fuel50` arithmetic support
is not an assertion about the Rocq interpreter's step count. Execution
composition under instruction, label, and frame contexts is proved in
`ExecutionUtil` and `CouponTable`.

## Assumptions and final checks

The Rocq backend signatures contain the original six storage laws, four
coupon storage laws, and two externref roundtrip laws. Program decoding and
the deterministic internal operation remain abstract parameters. The
typeless API and matching host equations are executable definitions, rather
than newly assumed execution facts. Classical choice is used for unbounded
evaluation and equality decisions, with successful-result correctness proved.

The original tree theories' global size bounds and the dispatcher's coupon
count bound are explicit execution hypotheses. They are not global axioms
about every tree or list. The old, unimported `storage_coupon`
and `thunkforce_coupon` describe constructors absent from the current WAT and
are outside this session.

`WasmNatural` proves natural counter representation, signed/unsigned
comparison, equality, and increment under explicit bounds. Its comparison and
increment lemmas also produce the corresponding Wasm `reduce_simple` steps.
`LoopUtil` uses these steps to prove the loop invariants. `TreeCoupon` retains
the tag loop, left/right size checks, left/right endpoint loop, and creation
in that order. Suffix induction replaces the induction on an Isabelle
interpreter iteration count; the coupon local is tracked explicitly in frames.
Success derives input-size bounds from the checked sizes. Failure states both
input-size bounds as premises, matching the original global restriction.

The host-operation datatype is shared outside the backend functor.
`MakeCoupon` identifies the independently constructed host proof certificates
using standard proof irrelevance, then transports each finite reduction. The
host equations and operational behavior are unchanged. Self and blob execution
also apply to the store containing the client-supplied coupon table.

The final audit checked the six fuel equations and their unbounded relations,
the 31 congruence properties and their R'/R instances, the rules of both
coinductive relations, the reflexive/symmetric/transitive closure, all eight
coupon judgements and their soundness conclusions, the typeless API equations,
and the guarded builder definitions. It also checked all thirteen parsed native
bodies and the dispatcher against the same WAT, including request numbering,
caller-frame preservation, and both labels on the indirect-call path.

The original success/failure helper facts sometimes include a true-tag premise
inside a failure witness. Rocq can first split on that Boolean tag and then use
the endpoint witness; its execution proofs keep this split and guard order.
Finite indexed tree quantifiers become `forallb` over `seq`, with explicit
`nth_error` and endpoint witnesses. The final success and trap conclusions cover
all thirteen requests rather than a selection of constructors.

The final success and failure theorems were checked with `Print Assumptions`.
Their context contains backend parameters and laws, standard classical
choice/description, functional extensionality, and proof irrelevance. It also
contains WasmCert-Coq's five abstract SIMD string operations because these
occur in its general operational semantics; the coupon program uses no SIMD
instructions. The port introduces no execution axioms or admitted proofs.

The 35-module build passed full kernel checking after final assembly. A fresh
WasmCert-Coq build from the pinned commit passed, and a clean copy of the port
compiled against its isolated Wasm and CompCert libraries with generated-byte
freshness checking. The clean copy's full kernel check also passed. Both compilation and kernel
checking explicitly bound `Wasm` and `compcert` to the newly built isolated
libraries; library lookup verified those paths before the check.
