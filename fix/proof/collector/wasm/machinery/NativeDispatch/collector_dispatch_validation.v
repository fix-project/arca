  Theorem collector_dispatch_validation_proof state store locals code rule lhs_index rhs_index witnesses_index lhs rhs witnesses :
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
  Proof.
    intros Code Functions Typed Capacity Within Left Right Witnesses LeftBound RightBound WitnessBound WitnessBounds Bytes Room.
    let finish := ltac:(
      first [discriminate Code|
      cbn [X.decode_exchange] in Code; inversion Code; subst rule; clear Code;
      eapply checked_weaken;
      [first
        [eapply Dispatcher.tree_validation with (evaluate := false)
        |eapply Dispatcher.tree_validation with (evaluate := true)
        |eapply Dispatcher.self_validation
        |eapply Dispatcher.eval_leaf_validation
        |eapply Dispatcher.identification_validation
        |eapply Direct.direct_validation with (kind := Direct.Dispatcher.D.Base.Symmetry)
        |eapply Direct.direct_validation with (kind := Direct.Dispatcher.D.Base.EvalResult)
        |eapply Direct.direct_validation with (kind := Direct.Dispatcher.D.Base.ThinkData)
        |eapply Direct.direct_validation with (kind := Direct.Dispatcher.D.Base.ThunkStep)
        |eapply Congruence.congruence_validation with (kind := X.Application)
        |eapply Congruence.congruence_validation with (kind := X.Selection)
        |eapply Congruence.congruence_validation with (kind := X.Identification)
        |eapply Congruence.congruence_validation with (kind := X.Strict)
        |eapply Congruence.congruence_validation with (kind := X.Shallow)
        |eapply Congruence.congruence_validation with (kind := X.TreeReference)
        |eapply Congruence.congruence_validation with (kind := X.BlobReference)
        |eapply Thunks.thunk_encode_validation with (strict := true)
        |eapply Thunks.thunk_encode_validation with (strict := false)
        |eapply Substitution.substitution_validation with (kind := Substitution.B.Transitive)
        |eapply Substitution.substitution_validation with (kind := Substitution.B.EvalSubstitution)
        |eapply Substitution.substitution_validation with (kind := Substitution.B.ForceSubstitution)
        |eapply Forces.force_strict_validation
        |eapply Forces.force_shallow_validation
        |eapply Thoughts.think_map_validation with (application := true)
        |eapply Thoughts.think_map_validation with (application := false)
        |eapply Results.force_results_validation];
      try eassumption; lia
      | intros final [next [updated [result [kind [Final [Scoped Rest]]]]]];
        exists next,updated,result,kind; split; [exact Final|]; split;
        [eapply scope_weaken; [exact Scoped|lia]|exact Rest] ] ]) in
    do 27 (destruct code as [|code]; [solve [finish]|]); discriminate.
  Qed.
