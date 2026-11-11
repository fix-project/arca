Module WasmBinarySuccess.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Import WasmRules WasmSupport WasmRuleSuccess WasmBinaryRules.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Lemma native_substitution state store locals kind lhs_index rhs_index witnesses_index lhs rhs pivot witnesses
      bundle tag claim author code bundle2 tag2 claim2 author2 code2 :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Trees.Resources.memory_capacity store ->
    G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state witnesses_index = Some (G.E.H.H.HTreeObj witnesses) ->
    get_tree_raw witnesses = [G.E.H.H.HTreeObj bundle; G.E.H.H.HTreeObj bundle2] ->
    get_tree_raw bundle = [G.E.H.H.HTreeObj tag; G.E.H.H.HTreeObj claim] ->
    get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] -> tree_tag tag = true ->
    get_tree_raw claim = [G.E.H.H.HBlobObj code; substitution_left kind lhs pivot; substitution_right kind lhs rhs pivot] ->
    decode_u64 (get_blob_data code) = Some (substitution_first_kind kind) ->
    get_tree_raw bundle2 = [G.E.H.H.HTreeObj tag2; G.E.H.H.HTreeObj claim2] ->
    get_tree_raw tag2 = [author2; G.E.H.H.HTreeRef claim2] -> tree_tag tag2 = true ->
    get_tree_raw claim2 = [G.E.H.H.HBlobObj code2; pivot; substitution_last kind lhs rhs] ->
    decode_u64 (get_blob_data code2) = Some (substitution_second_kind kind) ->
    observed_same state (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim) ->
    observed_same (at_observation state 2) (substitution_left kind lhs pivot) (substitution_left kind lhs pivot) ->
    observed_same (at_observation state 4) (substitution_right kind lhs rhs pivot) (substitution_right kind lhs rhs pivot) ->
    observed_same (at_observation state 6) (G.E.H.H.HTreeRef claim2) (G.E.H.H.HTreeRef claim2) ->
    observed_same (at_observation state 8) pivot pivot ->
    observed_same (at_observation state 10) (substitution_last kind lhs rhs) (substitution_last kind lhs rhs) ->
    G.E.H.D.live_count (G.E.H.slots state) + 18 <= G.E.H.capacity ->
    exists next updated,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list (rule_arguments (substitution_code kind) lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
        (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [result_word (substitution_result kind)]) /\
      scope state next 18 /\ s_funcs updated = s_funcs store /\ store_typing updated /\ Trees.Resources.memory_capacity updated.
  Proof.
    intros Functions Typed Capacity LhsLive RhsLive WitnessesLive Witnesses Bundle Tag Tagged Claim Decoded
      Bundle2 Tag2 Tagged2 Claim2 Decoded2 SameRef SameLeft SameRight SameRef2 SamePivot SameLast Room.
    set (input_code := substitution_code kind).
    assert (Binary : In input_code binary_codes) by (destruct kind; vm_compute; tauto).
    destruct (binary_endpoints_executes state store input_code lhs_index rhs_index witnesses_index witnesses bundle tag claim author
      code (substitution_left kind lhs pivot) (substitution_right kind lhs rhs pivot) (G.E.H.H.HTreeObj bundle2)
      Binary Functions WitnessesLive Witnesses Bundle Tag Tagged Claim SameRef ltac:(lia))
      as [first [p [a [b [Prefix [PLive [ALive [BLive [ScopePrefix Obs1]]]]]]]]].
    set (locals1 := endpoint_locals input_code lhs_index rhs_index witnesses_index p a b).
    pose proof ScopePrefix as Scope1.
    assert (BorrowedLeft : G.E.H.resolve first lhs_index = Some lhs) by (apply (proj1 Scope1); exact LhsLive).
    assert (BorrowedRight : G.E.H.resolve first rhs_index = Some rhs) by (apply (proj1 Scope1); exact RhsLive).
    assert (LeftLive : G.E.H.resolve first (substitution_left_slot kind lhs_index (wasm_nat a)) = Some (substitution_left kind lhs pivot))
      by (destruct kind; assumption).
    assert (RightLive : G.E.H.resolve first (substitution_right_slot kind lhs_index rhs_index (wasm_nat b)) =
      Some (substitution_right kind lhs rhs pivot)) by (destruct kind; assumption).
    assert (SameLeft1 : observed_same first (substitution_left kind lhs pivot) (substitution_left kind lhs pivot))
      by (eapply ready_at; [|exact SameLeft]; exact Obs1).
    assert (SameRight1 : observed_same (compared first) (substitution_right kind lhs rhs pivot) (substitution_right kind lhs rhs pivot)).
    { eapply ready_at; [|exact SameRight]; change (G.E.H.observations first + 2 = G.E.H.observations state + 4); rewrite Obs1; lia. }
    destruct (Acceptance.Witnesses.collector_premise_executes first store locals1 (wasm_nat p)
      (substitution_left_slot kind lhs_index (wasm_nat a)) (substitution_right_slot kind lhs_index rhs_index (wasm_nat b))
      claim code (substitution_first_kind kind) (substitution_left kind lhs pivot) (substitution_right kind lhs rhs pivot)
      Functions Typed Capacity PLive LeftLive RightLive Claim Decoded SameLeft1 SameRight1 ltac:(let Count := fresh "Count" in pose proof (proj2 Scope1) as Count; cbn [List.length] in Count; lia))
      as [second [intermediate [Premise1 [Scope2 [Obs2 [Functions2 [Typed2 Capacity2]]]]]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in Premise1.
    assert (FunctionsIntermediate : s_funcs intermediate = s_funcs G.E.allocated_store) by (rewrite Functions2; exact Functions).
    assert (Budget2 : scope state second 10).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    pose proof Budget2 as Scope02.
    assert (Witnesses2 : G.E.H.resolve second witnesses_index = Some (G.E.H.H.HTreeObj witnesses))
      by (apply (proj1 Scope02); exact WitnessesLive).
    assert (ObservedRef2 : observed_same second (G.E.H.H.HTreeRef claim2) (G.E.H.H.HTreeRef claim2)).
    { eapply ready_at; [|exact SameRef2]; rewrite Obs2, Obs1; lia. }
    destruct (Acceptance.Witnesses.witness_claim_executes second intermediate locals1 witnesses_index witnesses 1 bundle2 tag2 claim2 author2
      FunctionsIntermediate Witnesses2 ltac:(rewrite Witnesses; reflexivity) ltac:(lia) Bundle2 Tag2 Tagged2
      ltac:(rewrite Claim2; reflexivity) ObservedRef2 ltac:(let Count := fresh "Count" in pose proof (proj2 Scope02) as Count; cbn [List.length] in Count; lia))
      as [third [q [Reopen [QLive [QFresh [Scope3 Obs3]]]]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in Reopen.
    set (slots := [Some (wasm_nat p);Some (wasm_nat q);Some (wasm_nat a);Some (wasm_nat b);None;None;None]).
    set (inside := scratch_locals input_code lhs_index rhs_index witnesses_index slots 0).
    assert (Budget3 : scope state third 15).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    pose proof Budget3 as Scope03.
    assert (PivotLive : G.E.H.resolve third (substitution_pivot_slot kind (wasm_nat a) (wasm_nat b)) = Some pivot).
    { destruct kind; cbn [substitution_pivot_slot].
      - apply (proj1 Scope3), (proj1 Scope2); exact BLive.
      - apply (proj1 Scope3), (proj1 Scope2); exact ALive.
      - apply (proj1 Scope3), (proj1 Scope2); exact ALive. }
    assert (LastLive : G.E.H.resolve third (substitution_last_slot kind lhs_index rhs_index) = Some (substitution_last kind lhs rhs)).
    { destruct kind; cbn [substitution_last_slot substitution_last]; apply (proj1 Scope03); assumption. }
    assert (SamePivot3 : observed_same third pivot pivot) by (eapply ready_at; [|exact SamePivot]; rewrite Obs3, Obs2, Obs1; lia).
    assert (SameLast3 : observed_same (compared third) (substitution_last kind lhs rhs) (substitution_last kind lhs rhs)).
    { eapply ready_at; [|exact SameLast]; change (G.E.H.observations third + 2 = G.E.H.observations state + 10).
      rewrite Obs3, Obs2, Obs1; lia. }
    destruct (Acceptance.Witnesses.collector_premise_executes third intermediate inside (wasm_nat q)
      (substitution_pivot_slot kind (wasm_nat a) (wasm_nat b)) (substitution_last_slot kind lhs_index rhs_index)
      claim2 code2 (substitution_second_kind kind) pivot (substitution_last kind lhs rhs)
      FunctionsIntermediate Typed2 Capacity2 QLive PivotLive LastLive Claim2 Decoded2 SamePivot3 SameLast3
      ltac:(let Count := fresh "Count" in pose proof (proj2 Scope03) as Count; cbn [List.length] in Count; lia))
      as [fourth [updated [Premise2 [Scope4 [Obs4 [Functions4 [Typed4 Capacity4]]]]]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in Premise2.
    assert (Scoped : scope state fourth 18).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    exists fourth, updated; split; [|split; [exact Scoped|split; [rewrite Functions4, Functions2; reflexivity|auto]]].
    eapply rule_call_executes with (next := fourth) (updated := updated)
      (following := Build_frame inside G.E.collector_instance)
      (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn (substitution_tail kind) rule_body)));
      try exact Functions.
    eapply rule_prefix_continue; [exact Prefix|].
    unfold rule_body at 1; rewrite native_exchange_body; cbn [nth skipn].
    unfold locals1, endpoint_locals, scratch_locals, optional_descriptor, rule_arguments, result_word; cbn [firstn map].
    destruct kind; unfold input_code in *; cbn [substitution_code substitution_tail substitution_first_kind substitution_second_kind
      substitution_result substitution_left_slot substitution_right_slot substitution_pivot_slot substitution_last_slot] in *;
      repeat scratch_pure; cbn [lfill].
    all: eapply G.E.collector_runs_context with (prefix := [])
      (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]); apply G.E.collector_runs_label.
    all: do 3 scratch_pure; execute Premise1.
    all: scratch_pure; first [execute_under [VAL_num (VAL_int64 (wasm_nat64 0))] Reopen
      | execute_under [VAL_num (VAL_int64 (wasm_nat64 3))] Reopen].
    all: scratch_pure; do 2 scratch_pure; execute Premise2; apply rt_refl.
  Qed.

  Lemma native_think_map state store locals lhs_index rhs_index witnesses_index argument ready rhs witnesses
      bundle tag claim author code bundle2 tag2 claim2 author2 code2 (application : bool) :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Trees.Resources.memory_capacity store ->
    G.E.H.resolve state lhs_index = Some (Thunk (if application then G.E.H.H.Application argument else G.E.H.H.Selection argument)) ->
    G.E.H.resolve state rhs_index = Some rhs -> G.E.H.resolve state witnesses_index = Some (G.E.H.H.HTreeObj witnesses) ->
    get_tree_raw witnesses = [G.E.H.H.HTreeObj bundle; G.E.H.H.HTreeObj bundle2] ->
    get_tree_raw bundle = [G.E.H.H.HTreeObj tag; G.E.H.H.HTreeObj claim] ->
    get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] -> tree_tag tag = true ->
    get_tree_raw claim = [G.E.H.H.HBlobObj code; G.E.H.H.HTreeObj argument; ready] -> decode_u64 (get_blob_data code) = Some 1 ->
    get_tree_raw bundle2 = [G.E.H.H.HTreeObj tag2; G.E.H.H.HTreeObj claim2] ->
    get_tree_raw tag2 = [author2; G.E.H.H.HTreeRef claim2] -> tree_tag tag2 = true ->
    get_tree_raw claim2 = [G.E.H.H.HBlobObj code2; ready; rhs] -> decode_u64 (get_blob_data code2) = Some (if application then 2 else 6) ->
    observed_same state (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim) ->
    observed_same (at_observation state 2) (Thunk (if application then G.E.H.H.Application argument else G.E.H.H.Selection argument))
      (Thunk (if application then G.E.H.H.Application argument else G.E.H.H.Selection argument)) ->
    observed_same (at_observation state 4) (G.E.H.H.HTreeObj argument) (G.E.H.H.HTreeObj argument) ->
    observed_same (at_observation state 6) ready ready ->
    observed_same (at_observation state 8) (G.E.H.H.HTreeRef claim2) (G.E.H.H.HTreeRef claim2) ->
    observed_same (at_observation state 10) ready ready -> observed_same (at_observation state 12) rhs rhs ->
    G.E.H.D.live_count (G.E.H.slots state) + 19 <= G.E.H.capacity ->
    exists next updated,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list (rule_arguments (if application then 2 else 25) lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
        (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [result_word 4]) /\
      scope state next 19 /\ s_funcs updated = s_funcs store /\ store_typing updated /\ Trees.Resources.memory_capacity updated.
  Proof.
    intros Functions Typed Capacity LhsLive RhsLive WitnessesLive Witnesses Bundle Tag Tagged Claim Decoded
      Bundle2 Tag2 Tagged2 Claim2 Decoded2 SameRef SameLhs SameArgument SameReady SameRef2 SameReady2 SameRhs Room.
    set (input_code := if application then 2 else 25).
    set (thought := Thunk (if application then G.E.H.H.Application argument else G.E.H.H.Selection argument)).
    assert (Binary : In input_code binary_codes) by (destruct application; vm_compute; tauto).
    destruct (binary_endpoints_executes state store input_code lhs_index rhs_index witnesses_index witnesses bundle tag claim author code
      (G.E.H.H.HTreeObj argument) ready (G.E.H.H.HTreeObj bundle2) Binary Functions WitnessesLive Witnesses Bundle Tag Tagged Claim SameRef ltac:(lia))
      as [first [p [a [b [Prefix [PLive [ALive [BLive [ScopePrefix Obs1]]]]]]]]].
    set (locals1 := endpoint_locals input_code lhs_index rhs_index witnesses_index p a b).
    pose proof ScopePrefix as Scope1.
    destruct (Acceptance.native_map_tree_executes first store locals1 (wasm_nat a) argument application Functions ALive
      ltac:(let Count := fresh "Count" in pose proof (proj2 Scope1) as Count; cbn [List.length] in Count; lia))
      as [second [mapped [MapRun [MappedLive [MappedFresh [Scope2 Obs2]]]]]].
    unfold Acceptance.Support.word, Acceptance.Support.descriptor in MapRun.
    set (slots1 := [Some (wasm_nat p);None;Some (wasm_nat a);Some (wasm_nat b);None;Some (wasm_nat mapped);None]).
    set (locals2 := scratch_locals input_code lhs_index rhs_index witnesses_index slots1 0).
    assert (Budget2 : scope state second 8).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    pose proof Budget2 as Scope02.
    assert (Lhs2 : G.E.H.resolve second lhs_index = Some thought) by (apply (proj1 Scope02); exact LhsLive).
    assert (ObservedLhs2 : observed_same second thought thought).
    { eapply ready_at; [|exact SameLhs]; rewrite Obs2, Obs1; reflexivity. }
    pose proof (same_run second store locals2 lhs_index (wasm_nat mapped) thought thought true Functions Lhs2 MappedLive ObservedLhs2) as Compare.
    assert (BudgetCompared : scope state (compared second) 8).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    assert (P2 : G.E.H.resolve (compared second) (wasm_nat p) = Some (G.E.H.H.HTreeObj claim)) by (apply (proj1 Scope2); exact PLive).
    assert (A2 : G.E.H.resolve (compared second) (wasm_nat a) = Some (G.E.H.H.HTreeObj argument)) by (apply (proj1 Scope2); exact ALive).
    assert (B2 : G.E.H.resolve (compared second) (wasm_nat b) = Some ready) by (apply (proj1 Scope2); exact BLive).
    assert (ObservedA2 : observed_same (compared second) (G.E.H.H.HTreeObj argument) (G.E.H.H.HTreeObj argument)).
    { eapply ready_at; [|exact SameArgument]; change (G.E.H.observations second + 2 = G.E.H.observations state + 4); rewrite Obs2, Obs1; lia. }
    assert (ObservedB2 : observed_same (compared (compared second)) ready ready).
    { eapply ready_at; [|exact SameReady]; change (G.E.H.observations second + 2 + 2 = G.E.H.observations state + 6); rewrite Obs2, Obs1; lia. }
    destruct (Acceptance.Witnesses.collector_premise_executes (compared second) store locals2 (wasm_nat p) (wasm_nat a) (wasm_nat b)
      claim code 1 (G.E.H.H.HTreeObj argument) ready Functions Typed Capacity P2 A2 B2 Claim Decoded ObservedA2 ObservedB2
      ltac:(let Count := fresh "Count" in pose proof (proj2 Scope02) as Count; cbn [List.length] in Count; change (G.E.H.D.live_count (G.E.H.slots second) + 3 <= G.E.H.capacity); lia))
      as [third [intermediate [Premise1 [Scope3 [Obs3 [Functions3 [Typed3 Capacity3]]]]]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in Premise1.
    assert (FunctionsIntermediate : s_funcs intermediate = s_funcs G.E.allocated_store) by (rewrite Functions3; exact Functions).
    assert (Budget3 : scope state third 11).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    pose proof Budget3 as Scope03.
    assert (Witnesses3 : G.E.H.resolve third witnesses_index = Some (G.E.H.H.HTreeObj witnesses)) by (apply (proj1 Scope03); exact WitnessesLive).
    assert (ObservedRef3 : observed_same third (G.E.H.H.HTreeRef claim2) (G.E.H.H.HTreeRef claim2)).
    { eapply ready_at; [|exact SameRef2].
      rewrite Obs3; change (G.E.H.observations second + 2 + 4 = G.E.H.observations state + 8); rewrite Obs2, Obs1; lia. }
    destruct (Acceptance.Witnesses.witness_claim_executes third intermediate locals2 witnesses_index witnesses 1 bundle2 tag2 claim2 author2
      FunctionsIntermediate Witnesses3 ltac:(rewrite Witnesses; reflexivity) ltac:(lia) Bundle2 Tag2 Tagged2
      ltac:(rewrite Claim2; reflexivity) ObservedRef3
      ltac:(let Count := fresh "Count" in pose proof (proj2 Scope03) as Count; cbn [List.length] in Count; lia))
      as [fourth [q [Reopen [QLive [QFresh [Scope4 Obs4]]]]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in Reopen.
    set (slots := [Some (wasm_nat p);Some (wasm_nat q);Some (wasm_nat a);Some (wasm_nat b);None;Some (wasm_nat mapped);None]).
    set (inside := scratch_locals input_code lhs_index rhs_index witnesses_index slots 0).
    assert (Budget4 : scope state fourth 16).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    pose proof Budget4 as Scope04.
    assert (B4 : G.E.H.resolve fourth (wasm_nat b) = Some ready) by (apply (proj1 Scope4), (proj1 Scope3); exact B2).
    assert (Rhs4 : G.E.H.resolve fourth rhs_index = Some rhs) by (apply (proj1 Scope04); exact RhsLive).
    assert (ObservedB4 : observed_same fourth ready ready).
    { eapply ready_at; [|exact SameReady2].
      rewrite Obs4, Obs3; change (G.E.H.observations second + 2 + 4 + 2 = G.E.H.observations state + 10); rewrite Obs2, Obs1; lia. }
    assert (ObservedRhs4 : observed_same (compared fourth) rhs rhs).
    { eapply ready_at; [|exact SameRhs].
      change (G.E.H.observations fourth + 2 = G.E.H.observations state + 12).
      rewrite Obs4, Obs3; change (G.E.H.observations second + 2 + 4 + 2 + 2 = G.E.H.observations state + 12); rewrite Obs2, Obs1; lia. }
    destruct (Acceptance.Witnesses.collector_premise_executes fourth intermediate inside (wasm_nat q) (wasm_nat b) rhs_index
      claim2 code2 (if application then 2 else 6) ready rhs FunctionsIntermediate Typed3 Capacity3 QLive B4 Rhs4 Claim2 Decoded2 ObservedB4 ObservedRhs4
      ltac:(let Count := fresh "Count" in pose proof (proj2 Scope04) as Count; cbn [List.length] in Count; lia))
      as [fifth [updated [Premise2 [Scope5 [Obs5 [Functions5 [Typed5 Capacity5]]]]]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in Premise2.
    assert (Scoped : scope state fifth 19).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    exists fifth, updated; split; [|split; [exact Scoped|split; [rewrite Functions5, Functions3; reflexivity|auto]]].
    eapply rule_call_executes with (next := fifth) (updated := updated)
      (following := Build_frame inside G.E.collector_instance)
      (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn (if application then 110 else 134) rule_body)));
      try exact Functions.
    eapply rule_prefix_continue; [exact Prefix|].
    unfold rule_body at 1; rewrite native_exchange_body; cbn [nth skipn].
    unfold locals1, endpoint_locals, scratch_locals, optional_descriptor, rule_arguments, result_word; cbn [firstn map].
    destruct application; unfold input_code in *; repeat scratch_pure; cbn [lfill].
    all: eapply G.E.collector_runs_context with (prefix := [])
      (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]); apply G.E.collector_runs_label.
    all: do 2 scratch_pure; execute_under [descriptor lhs_index] MapRun.
    all: execute Compare.
    all: do 3 scratch_pure; execute Premise1.
    all: scratch_pure; first [execute_under [VAL_num (VAL_int64 (wasm_nat64 2))] Reopen
      | execute_under [VAL_num (VAL_int64 (wasm_nat64 6))] Reopen].
    all: scratch_pure; do 2 scratch_pure; execute Premise2; apply rt_refl.
  Qed.
End WasmBinarySuccess.
