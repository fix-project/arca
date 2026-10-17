# Proof coverage and verification

The Isabelle proof development has been replaced by Rocq, including the
Wasm execution and coupon soundness proofs. The default build compiles the
35 modules below; `make check` additionally checks binary freshness and every
module with `rocq check`. CI uses this same check.
Successful validation is cached by compiled-library and checker content hashes,
with dependency and project results kept separately. `make check-full` forces
validation of both stages; `CHECK_JOBS` controls parallel checker processes.

The dependency is WasmCert/WasmCert-Coq at commit
`5e6df8d60c94aa5dbeff633f5eb48caa6c64c225` (package version 2.2.1).
The local verification environment uses Rocq 9.1.1.
The current `make rocq-check` passes compilation, generated-binary freshness,
and full kernel checking for all 35 modules, including the shared-host refactor
and final dispatcher assembly. A fresh build of the pinned dependency passed;
a clean copy of all 35 port modules also passed compilation and freshness
checks against those isolated libraries. Its full kernel check also passed.

The correspondence audit follows the `Wasm-Proof` session roots
`isabelle_coupon` and `make_coupon`, their imports, and generated `Init`
(which imports `Host`). The unreferenced `storage_coupon` and
`thunkforce_coupon` theories describe older constructors absent from the
current `coupon.wat`; they are outside the built session.

| Isabelle theory | Rocq module | Current coverage |
| --- | --- | --- |
| fix_handle | Handle | Storage signature, all handle types, same-type relations and option relation |
| identify | Identify | Definition and congruence proof |
| slice | Slice | All definitions and list/blob/tree/top-level slice congruence proofs |
| digest | Digest | Definition, type preservation and digest_X |
| apply_tree | ApplyTree | All definitions and step/exec/application congruence, preserving semantic and same-type relations together |
| evaluation | Evaluation, EvaluationProperties | Fuel semantics, list correspondence, padding, determinism, unbounded choice, all unbounded semantic equations, value output and fixed-point properties |
| evaluation_properties | EvaluationProperties, Congruence | Value-to-same-type proofs, finite-fuel tree transport, relaxed/strengthened relations, reference lifting/lowering, coupled eq_forces_to_induct and its forces_X, evals_X and think_X consequences |
| equivalence | Equivalence | Both original coinductive relations, all 79 named supporting lemmas, R' to R coinduction, and evaluation/forcing congruence |
| equivalence_closure | EquivalenceClosure | All 57 named lemmas, corollaries and theorems: closure laws, tree/list congruence, evaluation/forcing congruence, encode and shape transport, lifting/lowering, think_to_eq, force_some_to_eq and blob-data equality |
| coupon | Coupon | All eight mutual judgements and inference rules, the coupled coupon_sound theorem, list support and coupon_blob_same_data |
| isabelle_coupon | CouponApi, CouponConstructors | Abstract coupon storage contract and typeless APIs, coupon_good, all 29 named results, every guarded builder and the thirteen-case make_coupon_good theorem |
| Host | Host | All 21 host operations, WasmCert-Coq host instance, store extension, preservation of store typing, and result typing |
| generated Init | Init, ModuleValidation, ModuleLayout | Actual coupon.wat compiled to bytes and parsed, parser and type-checker success, declarative module typing, verified export indices, dispatch entries and all thirteen constructor function bodies |
| util, generated runtime | ExecutionUtil | Store-preserving reduction composition, labels, frames, host calls, guarded branches, native invocation, executable/declarative module instantiation, explicit dispatch-table initialization reduction, and certified interpreter result tracking |
| self_coupon | SelfCoupon | Constructor success/failure guards, native invocation success returning a good coupon, and failure reaching the Wasm trap instruction |
| eval_blobobj_coupon | EvalBlobCoupon | Both guard failures and successful native execution returning a good evaluation coupon |
| util coupon-table/frame support | CouponTable | Client-supplied coupon table, bounded entry reads, empty-table trap, frame-changing execution and context transport, and native invocation with locals |
| sym_coupon | SymCoupon | Exact parsed body, success/failure guard characterization, native invocation success returning a good coupon, and all guard/table failure traps |
| trans_coupon | TransCoupon | Exact parsed body, both table reads and local updates, all five guard calls, good native return, and empty/short table, type and endpoint failure traps |
| eq_application_coupon, eq_encode_strict_coupon | MappedEqCoupon | Original success/converse/failure guard analyses, exact parsed bodies, endpoint transformations in both operand orders, good native returns, and table, type, transformation and equality failure traps |
| think_to_force_coupon | ThinkToForceCoupon | Original success/failure analyses, exact parsed body, think/data/endpoint checks, good force-coupon return, and every table/guard failure trap |
| force_to_encode_strict_coupon | ForceToEncodeStrictCoupon | Original success/failure analyses, exact parsed body, force/object/endpoint checks, strict-encode host success and failure, good equality-coupon return, and all failure traps |
| eval_eq_coupon | EvalEqCoupon | Original success/failure analyses, exact parsed body, two table/local updates, evaluation/equality tags, three endpoint checks, good evaluation-coupon return and all failure traps |
| think_application_coupon | ThinkApplicationCoupon | Original success/failure analyses, exact parsed body, two table/local updates, evaluation/application tags, middle and endpoint checks, thunk-creation host success/failure, good thinking-coupon return and all failure traps |
| force_result_eq_coupon | ForceResultEqCoupon | Original success/converse/failure analyses, exact parsed body, all three table/local updates, force/force/equality tags, four endpoint checks, good equality-coupon return and all failure traps |
| eq_tree_coupon, eval_tree_coupon | TreeLayout, LoopUtil, TreeCoupon | Exact parsed bodies, original builder success/converse/failure analyses, counter and branch reductions, tag and endpoint loops, size checks, successful good native return and every failure trap under explicit size bounds |
| make_coupon | Dispatcher, MakeCoupon | Original request encodings and thirteen-case split, initialized-table indirect dispatch, both labels, all native executions, good coupon return and failure trap |
| util integer support | WasmNatural | Bounded natural-to-i32 roundtrips, signed and unsigned comparisons, equality, counter increment, and concrete comparison/addition reductions, with explicit size hypotheses |
| make_coupon prologue | Dispatcher | Exact parsed dispatcher body/signature, all thirteen request encodings and function indices, successful indirect-call prologue through the initialized table, and composition of a native constructor return/trap through both labels and the caller's frame |

See [CORRESPONDENCE.md](CORRESPONDENCE.md) for the proof-structure mapping,
renamed support results and assumption accounting.

## Proof-system translations

* Theory imports become module functors sharing an explicit storage backend.
  The storage, coupon storage and externref signatures retain the original
  axioms; theorem conclusions are proved, never added as backend assumptions.
  The same-type relation is a shared manifest inductive definition in the
  storage signature, so separate functor instances use the same predicate.
* `list_all` and `list_all2` become `Forall` and `Forall2`. Option congruence
  stays `rel_opt`; guarded lookup uses `nth_error` where appropriate.
* The six mutually recursive evaluation functions become a record of
  evaluators computed structurally on fuel, with separate list recursion.
  This preserves the original fuel accounting, including execution's use of
  force at the same fuel and evaluation's use of execution at smaller fuel.
* Unbounded option-valued evaluation uses Rocq's classical indefinite choice,
  corresponding to Isabelle's choice operators. It is justified by proved
  fuel padding and determinism. This introduces the standard classical choice
  dependency, not an assumption of evaluator termination or soundness.
  Finite list induction collects successful evaluation witnesses and pads
  them to common fuel before constructing a tree. Tree output characterization
  supplies explicit output lists with a `Forall2` specification, replacing
  guarded uses of Isabelle's `the (eval x)`.
* The original congruence hypotheses are collected in a proposition-valued
  record. Bidirectional transport lemmas split the coupled induction into
  thinking, forcing, and evaluation steps. Thinking preserves the original
  single-step thunk exceptions; forcing consumes these cases explicitly.
  The simultaneous induction and its unbounded option congruence results live
  in `EvaluationProperties`, retaining one evaluator instance for subsequent
  equivalence proofs. `Congruence` is a compatibility entry point.
* Isabelle's nested coinduction through `list_all2` becomes a mutual
  coinductive relation over handles and handle lists. For finite lists, the
  list relation is proved equivalent to `Forall2 R`, preserving the original
  tree rule. A productive unfolding relation supports Rocq's guarded
  cofixpoint for `R'` to `R`.
* Equivalence-closure proofs use induction on `clos_refl_sym_trans`.
  Executing encodings once yields a closure whose relation excludes encoded
  endpoints; this proved invariant handles encoded intermediate steps in
  shape, tree-to-thunk and blob-data arguments. Identification thunks provide
  the intermediate steps for composing lifted-data thinking relations.
* Coupon soundness uses mutual structural recursion on the derivations,
  with explicit nested list induction for tree evaluation and equality.
  Isabelle's proposition-valued `coupon_good` becomes a Rocq proposition;
  runtime constructor guards remain booleans. Guarded `the` applications
  become explicit `Some` witnesses, and bounded universal tree checks use
  `forallb` over finite indices with `nth_error` lookup.
* Wasm values and integer operations come from `Wasm.datatypes` and
  `Wasm.numerics`. Externrefs are WasmCert-Coq addresses, with the original
  abstract roundtrip laws stated over those addresses.
* Individual Wasm function proofs retain success and trap cases. Isabelle's
  `run_iter` equalities become finite reflexive transitive Wasm reductions,
  with lemmas for stepping under instruction, label and frame contexts.
  Function bodies, exports and dispatch references are checked against the
  parsed binary. A fuel runner retains the interpreter's final store and
  frame, with a proof that every successful run gives a Wasm reduction.
  It uses the executable context reformation function directly, avoiding
  computation through WasmCert-Coq's opaque proof of context reformation.
  Host execution rejects mismatched signatures and retains all 21 original
  equations on matching signatures.
  Runtime initialization fills each dispatch entry using table initialization
  and table assignment reductions, then drops the element segment. The final
  store is thus proved reachable from the executable module allocation.
  Client-supplied coupons populate the exported externref table in the input
  configuration. Executions that update locals use a relation over frames
  and instructions; lifting it under native frames preserves the caller's
  frame. This replaces the original mutable interpreter frame contexts.
  Shared table/local reductions retain each updated frame explicitly. Guard
  prefix failure is proved without demanding execution of later host calls.
  This preserves the original guard order when a later host operation can fail.
* The original slice endpoint checks and the distinct opcode/digest type
  numberings are preserved. The typeless application-thunk API accepts tree
  objects only, while the application opcode also accepts tree references.

## Completion audit

| Requirement | Verified evidence |
| --- | --- |
| Evaluation and congruence | Original fuel equations retained; padding, determinism, successful-result choice, value properties, and the coupled transport theorem are proved; R' and R have proved property records |
| Equivalence and closure | Original coinductive rules retained through the proved finite-list lifting; all 79 equivalence and 57 closure results present, including forcing/evaluation transport and shape/blob conclusions |
| Coupon soundness and builders | All eight mutual judgements, their soundness conclusions, original API contracts, all 29 constructor results, and every guarded builder are proved |
| Actual Wasm executions | Exact generated AST and signatures checked; all thirteen native constructors have success and trap proofs; signed-size restrictions are explicit premises |
| Final dispatcher | Original thirteen-case split, initialized-table indirect call, both labels, caller-frame preservation, coupon goodness and trap conclusions proved |
| Assumptions | `Print Assumptions` reviewed for final success/failure; only backend contracts, standard logical principles and inherited WasmCert-Coq parameters; no admitted proofs or new execution axioms |
| Reproducible dependency/build | Pinned source archive rebuilt and installed in isolation; all 35 modules compiled and passed full kernel and freshness checks against those libraries |
| Default build and CI | Root `make` compiles Rocq; `make check` and CI check all modules; obsolete Isabelle sources, dependencies and tooling removed |

The historical Isabelle reference and scope are recorded in CORRESPONDENCE.md.
Execution step counts are replaced by finite WasmCert-Coq reductions, rather
than assertions about the old interpreter's instruction accounting.
