  Lemma collector_output_executes_proof state store locals combination_index lhs_index rhs_index number lhs rhs author :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state combination_index = Some (treeh combination) ->
    G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    nth_error (get_tree_raw combination) 1 = Some author ->
    G.E.H.T.authorizes (treeh combination) author ->
    G.E.H.D.live_count (G.E.H.slots state) + 6 <= G.E.H.capacity ->
    let code := G.E.H.H.HBlobObj (create_blob (serialise_i64 number)) in
    let claim := create_tree [code; lhs; rhs] in
    let output := treeh (create_tree_tagged true [author; G.E.H.H.HTreeRef claim]) in
    exists next updated index,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [descriptor combination_index; descriptor lhs_index; descriptor rhs_index; VAL_num (VAL_int64 number)] ++
            [AI_basic (BI_call 42%N)])
        (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [word index]) /\
      G.E.H.resolve next (wasm_nat index) = Some output /\
      G.E.H.resolve state (wasm_nat index) = None /\ scope state next 6 /\
      G.E.H.observations next = G.E.H.observations state /\
      s_funcs updated = s_funcs store /\ store_typing updated /\ Resources.memory_capacity updated.
  Proof.
    exact (Issuance.collector_issue_executes state store locals combination_index lhs_index rhs_index number lhs rhs author).
  Qed.
