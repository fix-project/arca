    Theorem terminating_evaluation_is_certifiable_proof value result :
      eval value = Some result -> certifies issuing_author Eval value result.
    Proof.
      (* A successful run has a finite evaluation derivation. The certification
          construction follows that derivation and has kept all intermediate Data
          witnesses needed by the available exchanges, so no additional oracle or
          evaluation operation in the collector is required. *)
      intro Evaluated; apply (proj1 (certify_eval _ _ (EC.eval_derivation_complete _ _ Evaluated))).
    Qed.
