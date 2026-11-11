# Proof status

[AUDIT.md](AUDIT.md) separates the audit surface from the internal argument
and checker machinery. The source organization preserves the theorem statements
and assumptions.

The primary results are:

| Property | Theorem |
| --- | --- |
| Evaluation certification agrees with evaluation | `EvaluationAdequacy.eval_adequacy` |
| Every terminating evaluation has a finite public-exchange derivation | `CertificationCompleteness.terminating_evaluation_is_certifiable` |
| Certification is sound | `CertificationCompleteness.certification_sound` |
| Every accepted exchange is sound | `CouponExchanges.check_sound` |
| Issuance propagates conditional authorities | `CouponExchanges.issue_conditional` |
| Equivalent Blob Objects have identical contents | `Coupon.coupon_blob_same_data` |
| Native descriptor allocation, reuse, and closing | `Descriptors.allocate_spec`, `close_lookup`, `close_live` |
| Witness claim access preserves open descriptors | `WasmWitnesses.witness_claim_executes` |
| Tree traversal bounds descriptor allocation | `WasmTreeLoop.native_tree_rule_executes` |
| Issuance returns a fresh descriptor | `WasmOutput.collector_output_executes` |
| All 26 primitive exchanges refine the abstract checker | `WasmNativeDispatch.collector_dispatch_validation` |
| Exported execution authorizes witnesses and issues the checked claim | `WasmEntry.exported_exchange_refinement` |
| Exported Tag reconstruction is sound | `WasmEntry.exported_tree_exchange_sound` |

The native proof checks the parsed binary built from `fix/proof/collector/collector.wat`.
The descriptor allocator has 134,217,728 slots, selects the lowest free slot,
and permits reuse after closing. Host calls and Wasm execution preserve its
capacity invariant. The collector never closes descriptors. Claim access allocates
at most five slots; a Tree iteration allocates at most ten. Authority traversal
allocates at most `2 + 7 * witness_count` slots. Its author guard accepts only
kernel or current-collector authors. Issuance allocates at most six slots and
returns a fresh descriptor.

All 26 primitive exchange success paths and Tag reconstruction have execution
proofs. Checked validation covers witness authentication, authority traversal,
integer decoding, premise checks, constructors, mapped congruence, Tree loops,
and all primitive dispatch cases. These proofs include rejected inputs and
finite allocation failure.

The exported entry proof connects the parsed binary to the abstract checker
and issuer, preserving existing descriptors. Each invocation allocates at most
`64 * (witness_count + 1)` descriptors. The collector rejects more than 1,048,576
witnesses. `WasmEntry.initial_descriptor_budget` proves that this limit fits the
finite descriptor capacity when starting with one request descriptor. Descriptor
closing and reuse remain available in the Shell model, independently of the
collector's immutable allocation discipline.

The execution-refinement results permit traps. The primitive success proofs
require explicit representation readiness and resource bounds; abstract
certification completeness remains a separate result.

Ref-backed claims survive Tag evaluation; preservation uses the issuance
invariant that a stable author suffices for the claim.
`collector/Preservation.v` proves preservation. `shell/TagCreation.v` proves
that checked creation preserves the supplied fields, requires a comparable
Object or Ref author on the callable path, and respects indistinguishability.

Computational equivalence and indistinguishability remain distinct.
`Indistinguishability` models observations including comparability, type, contents,
length, and selection; matching observations also agree on full evaluation. Full program substitution and indistinguishability of
computationally equivalent Thunks remain unfinished. Length is defined only for
Objects and Refs. Digestion remains a planned operation outside the model.

Storage, program decoding, and deterministic internal operations are interface
assumptions. Host byte identity, native Tag creation,
canonicalization certificates, Runnable admission, and ELF loading have unverified
implementation boundaries. Resource limits are explicit execution hypotheses.
Standard classical choice and functional extensionality are used. WasmCert's
general operational semantics includes abstract SIMD operations; the collector
uses no SIMD. No execution oracle or admitted proof is used.

`bazel test //fix/proof:check` compiles and independently
kernel-checks the proof modules, embedding the executed binary as a generated
Bazel input. The imported thirteen-exchange implementation and proof are retained in
Git history rather than as a second current collector.
