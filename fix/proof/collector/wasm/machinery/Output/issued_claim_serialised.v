  Lemma issued_claim_serialised_proof lhs rhs number :
    (Z.of_nat number < 2 ^ 64)%Z ->
    create_tree [G.E.H.H.HBlobObj (create_blob (serialise_i64 (wasm_nat64 number)));lhs;rhs] =
      issued_claim lhs rhs number.
  Proof.
    intro Bound; rewrite serialise_i64_encode_u64, wasm_nat64_unsigned by exact Bound.
    rewrite Nat2Z.id; reflexivity.
  Qed.
