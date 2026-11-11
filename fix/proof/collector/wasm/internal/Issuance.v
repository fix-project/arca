Module WasmIssuance.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module V := WasmValidation.
  Import V V.V V.V.Support.
  Module Prefix := WasmIssuePrefix.
  Module Resources := WasmResources.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Lemma collector_issue_executes state store locals combination_index lhs_index rhs_index number lhs rhs author :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state combination_index = Some (treeh combination) ->
    G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    nth_error (get_tree_raw combination) 1 = Some author -> G.E.H.T.authorizes (treeh combination) author ->
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
      G.E.H.resolve next (wasm_nat index) = Some output /\ G.E.H.resolve state (wasm_nat index) = None /\
      scope state next 6 /\ G.E.H.observations next = G.E.H.observations state /\
      s_funcs updated = s_funcs store /\ store_typing updated /\ Resources.memory_capacity updated.
  Proof.
    intros Functions Typed Capacity CombinationLive LhsLive RhsLive Author Allowed Room; cbn zeta.
    pose (code := G.E.H.H.HBlobObj (create_blob (serialise_i64 number))).
    pose (claim := create_tree [code; lhs; rhs]).
    pose (fields := create_tree [author;G.E.H.H.HTreeRef claim]).
    pose (output := treeh (create_tree_tagged true [author;G.E.H.H.HTreeRef claim])).
    pose (arguments := [descriptor combination_index;descriptor lhs_index;descriptor rhs_index;VAL_num (VAL_int64 number)]).
    destruct (Prefix.collector_issue_prefix state store combination_index lhs_index rhs_index number lhs rhs author
      Functions Typed Capacity CombinationLive LhsLive RhsLive Author ltac:(lia))
      as [prepared [updated [ref_slot [fields_slot
        [Prepare [Scoped [Obs [SameFunctions [NextTyped [NextCapacity FieldsLive]]]]]]]]]].
    pose (inside := arguments ++ [word ref_slot]).
    pose proof (proj2 Scoped) as Count.
    assert (FunctionsNext : s_funcs updated = s_funcs G.E.allocated_store) by (rewrite SameFunctions; exact Functions).
    assert (Values : G.E.H.host_values combination G.E.collector_instance true true handle_bytes prepared updated
      shell_create_tag [word fields_slot] = G.E.H.allocate prepared output).
    { unfold G.E.H.host_values, G.E.H.transform, word; rewrite FieldsLive.
      cbn [G.E.H.H.obind G.E.H.checked_tag G.E.H.H.HTreeObj].
      rewrite get_tree_raw_create_tree, G.E.H.T.create_tag_authorized by exact Allowed; reflexivity. }
    destruct (new_handle_run prepared updated inside 12%N shell_create_tag [word fields_slot] output
      FunctionsNext eq_refl eq_refl eq_refl ltac:(discriminate) Values ltac:(lia))
      as [next [index [TagRun [TagLive [TagFresh [TagScope TagObs]]]]]].
    exists next, updated, index; split.
    - eapply G.E.collector_call with (code := native_issue)
        (parameters := [T_num T_i32;T_num T_i32;T_num T_i32;T_num T_i64]) (results := [T_num T_i32])
        (defaults := [word 0]) (address := 42%N) (arguments := arguments) (values := [word index])
        (accepted := true) (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
      + rewrite Functions; reflexivity.
      + eapply rt_trans; [exact Prepare|unfold Prefix.issue_tail; exact TagRun].
    - split; [exact TagLive|]; split.
      + eapply fresh_before; [exact (proj1 Scoped)|exact TagFresh].
      + split.
        * replace 6 with (5+1) by lia; eapply scope_trans; eassumption.
        * split; [rewrite TagObs, Obs; reflexivity|auto].
  Qed.

  Lemma collector_issue_validation state store locals combination_index lhs_index rhs_index number lhs rhs author :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state combination_index = Some (treeh combination) ->
    G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    nth_error (get_tree_raw combination) 1 = Some author ->
    G.E.H.D.live_count (G.E.H.slots state) + 6 <= G.E.H.capacity ->
    let code := G.E.H.H.HBlobObj (create_blob (serialise_i64 number)) in
    let claim := create_tree [code;lhs;rhs] in
    let output := treeh (create_tree_tagged true [author;G.E.H.H.HTreeRef claim]) in
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor combination_index;descriptor lhs_index;descriptor rhs_index;VAL_num (VAL_int64 number)] ++
        [AI_basic (BI_call 42%N)])
      (fun final => exists next updated index,
        final = (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [word index]) /\
        G.E.H.resolve next (wasm_nat index) = Some output /\ G.E.H.resolve state (wasm_nat index) = None /\
        scope state next 6 /\ s_funcs updated = s_funcs store /\ store_typing updated /\ Resources.memory_capacity updated).
  Proof.
    intros Functions Typed Capacity CombinationLive LhsLive RhsLive Author Room; cbn zeta.
    destruct (ClassicalDescription.excluded_middle_informative (G.E.H.T.authorizes (treeh combination) author)) as [Allowed|Denied].
    - destruct (collector_issue_executes state store locals combination_index lhs_index rhs_index number lhs rhs author
        Functions Typed Capacity CombinationLive LhsLive RhsLive Author Allowed Room)
        as [next [updated [index [Run [Present [Fresh [Scoped [Obs [SameFunctions [NextTyped NextCapacity]]]]]]]]]].
      eapply checked_result; [exact Run|]; exists next, updated, index.
      split; [reflexivity|]; split; [exact Present|]; split; [exact Fresh|]; auto 8.
    - pose (arguments := [descriptor combination_index;descriptor lhs_index;descriptor rhs_index;VAL_num (VAL_int64 number)]).
      destruct (Prefix.collector_issue_prefix state store combination_index lhs_index rhs_index number lhs rhs author
        Functions Typed Capacity CombinationLive LhsLive RhsLive Author ltac:(lia))
        as [prepared [updated [ref_slot [fields_slot
          [Prepare [Scoped [Obs [SameFunctions [NextTyped [NextCapacity FieldsLive]]]]]]]]]].
      pose (inside := arguments ++ [word ref_slot]).
      assert (FunctionsNext : s_funcs updated = s_funcs G.E.allocated_store) by (rewrite SameFunctions; exact Functions).
      assert (Failed : G.E.H.host_values combination G.E.collector_instance true true handle_bytes prepared updated shell_create_tag [word fields_slot] = None).
      { unfold G.E.H.host_values, G.E.H.transform, word; rewrite FieldsLive.
        cbn [G.E.H.H.obind G.E.H.checked_tag G.E.H.H.HTreeObj].
        rewrite get_tree_raw_create_tree; unfold G.E.H.T.create_tag.
        destruct (ClassicalDescription.excluded_middle_informative _) as [Allowed|]; [contradiction|reflexivity]. }
      eapply checked_trap; eapply G.E.collector_call with (code := native_issue)
        (parameters := [T_num T_i32;T_num T_i32;T_num T_i32;T_num T_i64]) (results := [T_num T_i32])
        (defaults := [word 0]) (address := 42%N) (arguments := arguments) (values := [word 0])
        (accepted := false) (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
      + rewrite Functions; reflexivity.
      + eapply rt_trans with (y := (prepared, updated, Build_frame inside G.E.collector_instance, Prefix.issue_tail fields_slot)); [exact Prepare|].
        unfold Prefix.issue_tail.
        eapply G.E.collector_trap_suffix with (instructions := v_to_e_list [word fields_slot] ++ [AI_basic (BI_call 12%N)]).
        exact (native_import_traps prepared updated inside 12%N shell_create_tag [word fields_slot]
          FunctionsNext eq_refl eq_refl eq_refl ltac:(discriminate) Failed).
  Qed.
End WasmIssuance.
