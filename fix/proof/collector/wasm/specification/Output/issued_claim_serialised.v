  Lemma issued_claim_serialised lhs rhs number :
    (Z.of_nat number < 2 ^ 64)%Z ->
    create_tree [G.E.H.H.HBlobObj (create_blob (serialise_i64 (wasm_nat64 number)));lhs;rhs] =
      issued_claim lhs rhs number.
  Proof. apply issued_claim_serialised_proof. Qed.
