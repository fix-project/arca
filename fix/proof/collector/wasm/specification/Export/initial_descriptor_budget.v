  Lemma initial_descriptor_budget witnesses :
    WasmAuthorityGuard.witnesses_bounded witnesses ->
    G.E.H.D.live_count (G.E.H.slots (G.E.H.initial (treeh combination))) +
      64 * (List.length (get_tree_raw witnesses) + 1) <= G.E.H.capacity.
  Proof. apply initial_descriptor_budget_proof. Qed.
