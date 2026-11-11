  Lemma initial_descriptor_budget_proof witnesses :
    WasmAuthorityGuard.witnesses_bounded witnesses ->
    G.E.H.D.live_count (G.E.H.slots (G.E.H.initial (treeh combination))) +
      64 * (List.length (get_tree_raw witnesses) + 1) <= G.E.H.capacity.
  Proof.
    intro Bound.
    change (1 + 64 * (List.length (get_tree_raw witnesses) + 1) <= G.E.H.capacity).
    apply Nat2Z.inj_le.
    rewrite Nat2Z.inj_add, Nat2Z.inj_mul, Nat2Z.inj_add, G.E.H.capacity_value.
    change ((1 + 64 * (Z.of_nat (List.length (get_tree_raw witnesses)) + 1)) <= 134217728)%Z.
    unfold WasmAuthorityGuard.witnesses_bounded, WasmAuthorityGuard.witness_limit in Bound; lia.
  Qed.
