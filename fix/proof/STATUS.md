# Proof status

The primary results are:

| Property | Theorem |
| --- | --- |
| Evaluation certification agrees with evaluation | `EvaluationAdequacy.eval_adequacy` |
| Every terminating evaluation has a finite public-exchange derivation | `CertificationCompleteness.terminating_evaluation_is_certifiable` |
| Certification is sound | `CertificationCompleteness.certification_sound` |
| Every accepted exchange is sound | `CouponExchanges.check_sound` |
| Issuance propagates conditional authorities | `CouponExchanges.issue_conditional` |
| Equivalent Blob Objects have identical contents | `Coupon.coupon_blob_same_data` |
| The exported collector implements the abstract checker and issuer | `WasmRefinement.collector_refinement` |

The native proof checks the parsed binary built from `fix/proof/collector/collector.wat`.
All 26 exchanges also have success proofs under explicit resource and comparison
readiness contracts. Exact physical comparison can reject abstractly identical
handles with different representations. The collector never evaluates, loads
Refs, or searches for derivations: it validates ordered supplied witnesses.

Ref-backed claims survive Tag evaluation. Arbitrary additional authors are
allowed; preservation uses the stronger issuance invariant that stable authors
suffice for the claim. `collector/Preservation.v` and `shell/TagCreation.v`
prove these supporting properties.

Computational equivalence and indistinguishability remain distinct.
`Indistinguishability` models observations including comparability, type, contents,
length, and selection. Full program substitution and indistinguishability of
computationally equivalent Thunks remain unfinished. Length is defined only for
Objects and Refs. Digestion remains a planned operation outside the model.

Storage, program decoding, deterministic internal operations, and externref
embedding are interface assumptions. Host byte identity, native Tag creation,
canonicalization certificates, Runnable admission, and ELF loading have unverified
implementation boundaries. Resource limits are explicit execution hypotheses.
Standard classical choice and functional extensionality are used. WasmCert's
general operational semantics includes abstract SIMD operations; the collector
uses no SIMD. No execution oracle or admitted proof is used.

`bazel test //fix/proof:check` compiles and independently
kernel-checks every retained module, embedding the executed binary as a generated
Bazel input. The imported thirteen-exchange implementation and proof are retained in
Git history rather than as a second current collector.
