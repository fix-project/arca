  Theorem collector_dispatch_validation state store locals code rule lhs_index rhs_index witnesses_index lhs rhs witnesses :
    X.decode_exchange code = Some rule ->
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.within_capacity state -> G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state witnesses_index = Some (G.E.H.H.HTreeObj witnesses) ->
    lengths_bounded 1 lhs -> lengths_bounded 1 rhs -> lengths_bounded 1 (G.E.H.H.HTreeObj witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + (32 * (List.length (get_tree_raw witnesses) + 1)) <= G.E.H.capacity ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list (Base.rule_arguments code lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
      (fun final => exists next updated result kind,
        final = (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [Base.result_word result]) /\
        scope state next (32 * (List.length (get_tree_raw witnesses) + 1)) /\ s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Claims.Code.statement_kind result = Some kind /\ X.check rule (get_tree_raw witnesses) lhs rhs = Some kind).
  Proof. apply collector_dispatch_validation_proof. Qed.
