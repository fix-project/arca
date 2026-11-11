Module WasmForceSuccess.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Import WasmRules WasmSupport WasmRuleSuccess WasmBinaryRules WasmForceRules.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation lift := X.D.EC.E.EP.E.lift.

  Lemma force_endpoints_executes state store code lhs rhs witnesses coupons bundle tag claim author code_blob left right :
    In code [7;16] -> s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state witnesses = Some (G.E.H.H.HTreeObj coupons) ->
    get_tree_raw coupons = [G.E.H.H.HTreeObj bundle] ->
    get_tree_raw bundle = [G.E.H.H.HTreeObj tag; G.E.H.H.HTreeObj claim] ->
    get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] -> tree_tag tag = true ->
    get_tree_raw claim = [G.E.H.H.HBlobObj code_blob; left; right] ->
    observed_same state (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim) ->
    G.E.H.D.live_count (G.E.H.slots state) + 7 <= G.E.H.capacity ->
    exists next claim_slot left_slot right_slot,
      reduce_trans
        (state, store, Build_frame (initial_locals code lhs rhs witnesses) G.E.collector_instance,
          to_e_list (firstn 106 rule_body))
        (next, store, Build_frame (endpoint_locals code lhs rhs witnesses claim_slot left_slot right_slot) G.E.collector_instance, []) /\
      G.E.H.resolve next (wasm_nat claim_slot) = Some (G.E.H.H.HTreeObj claim) /\
      G.E.H.resolve next (wasm_nat left_slot) = Some left /\ G.E.H.resolve next (wasm_nat right_slot) = Some right /\
      scope state next 7 /\
      G.E.H.observations next = G.E.H.observations state + 2.
  Proof.
    intros Code Functions Live Coupons Bundle Tag Tagged Claim Same Room.
    assert (Unary : In code unary_codes) by (destruct Code as [Equal|[Equal|Impossible]]; try contradiction; subst code; vm_compute; tauto).
    destruct (unary_prefix_executes state store code lhs rhs witnesses coupons bundle tag claim author
      Unary Functions Live Coupons Bundle Tag Tagged ltac:(rewrite Claim; reflexivity) Same ltac:(lia))
      as [first [claim_slot [Prefix [ClaimLive [ClaimFresh [Scope1 Obs1]]]]]].
    set (locals1 := single_locals code lhs rhs witnesses claim_slot 0).
    destruct (Acceptance.exchange_endpoint_executes first store locals1 (wasm_nat claim_slot) claim left true
      Functions ClaimLive ltac:(rewrite Claim; reflexivity) ltac:(pose proof (proj2 Scope1); lia))
      as [second [left_slot [LeftRun [LeftLive [LeftFresh [Scope2 Obs2]]]]]].
    unfold Acceptance.Support.word, Acceptance.Support.descriptor in LeftRun.
    set (locals2 := scratch_locals code lhs rhs witnesses
      [Some (wasm_nat claim_slot);None;Some (wasm_nat left_slot);None;None;None;None] 0).
    assert (Claim2 : G.E.H.resolve second (wasm_nat claim_slot) = Some (G.E.H.H.HTreeObj claim))
      by (apply (proj1 Scope2); exact ClaimLive).
    destruct (Acceptance.exchange_endpoint_executes second store locals2 (wasm_nat claim_slot) claim right false
      Functions Claim2 ltac:(rewrite Claim; reflexivity)
      ltac:(pose proof (proj2 Scope1); pose proof (proj2 Scope2); lia))
      as [third [right_slot [RightRun [RightLive [RightFresh [Scope3 Obs3]]]]]].
    unfold Acceptance.Support.word, Acceptance.Support.descriptor in RightRun.
    assert (Claim3 : G.E.H.resolve third (wasm_nat claim_slot) = Some (G.E.H.H.HTreeObj claim))
      by (apply (proj1 Scope3); exact Claim2).
    assert (Left3 : G.E.H.resolve third (wasm_nat left_slot) = Some left) by (apply (proj1 Scope3); exact LeftLive).
    assert (Scoped : scope state third 7).
    { replace 7 with (5+1+1) by lia; eapply scope_trans; [eapply scope_trans|]; eassumption. }
    exists third, claim_slot, left_slot, right_slot; split.
    2: { split; [exact Claim3|]; split; [exact Left3|]; split; [exact RightLive|];
         split; [exact Scoped|rewrite Obs3, Obs2, Obs1; reflexivity]. }
    change (reduce_trans
      (state, store, Build_frame (initial_locals code lhs rhs witnesses) G.E.collector_instance,
        to_e_list (firstn 48 rule_body) ++ to_e_list (firstn 58 (skipn 48 rule_body)))
      (third, store, Build_frame (endpoint_locals code lhs rhs witnesses claim_slot left_slot right_slot) G.E.collector_instance, [])).
    eapply rt_trans with (y := (first, store, Build_frame locals1 G.E.collector_instance,
      to_e_list (firstn 58 (skipn 48 rule_body)))).
    - change (reduce_trans
        (state, store, Build_frame (initial_locals code lhs rhs witnesses) G.E.collector_instance,
          to_e_list (firstn 48 rule_body) ++ to_e_list (firstn 58 (skipn 48 rule_body)))
        (first, store, Build_frame locals1 G.E.collector_instance, [] ++ to_e_list (firstn 58 (skipn 48 rule_body)))).
      apply G.E.collector_runs_context with (prefix := []) (suffix := to_e_list (firstn 58 (skipn 48 rule_body)))
        (instructions := to_e_list (firstn 48 rule_body)) (instructions' := []); exact Prefix.
    - unfold rule_body at 1; rewrite native_exchange_body; cbn [nth skipn firstn].
      unfold locals1, single_locals, rule_arguments, result_word in LeftRun |- *.
      cbn in Code.
      destruct Code as [Code|[Code|Impossible]]; try contradiction; subst code.
      all: repeat native_pure; execute LeftRun; native_pure.
      all: native_pure; execute RightRun; native_pure; apply rt_refl.
  Qed.


  Lemma force_constructor_executes state store locals lhs_index input_index th (strict : bool) :
    s_funcs store = s_funcs G.E.allocated_store ->
    lookup_N locals 1%N = Some (descriptor lhs_index) -> lookup_N locals 6%N = Some (descriptor input_index) ->
    G.E.H.resolve state lhs_index = Some (Encode (if strict then G.E.H.H.Strict th else G.E.H.H.Shallow th)) ->
    G.E.H.resolve state input_index = Some (Thunk th) ->
    observed_same state (Encode (if strict then G.E.H.H.Strict th else G.E.H.H.Shallow th))
      (Encode (if strict then G.E.H.H.Strict th else G.E.H.H.Shallow th)) ->
    G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    exists next mapped,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          to_e_list [BI_local_get 1%N; BI_const_num (VAL_int32 (wasm_nat (if strict then 3 else 4)));
            BI_local_get 6%N; BI_call 35%N; BI_call 23%N])
        (next, store, Build_frame locals G.E.collector_instance, []) /\
      G.E.H.resolve next (wasm_nat mapped) = Some (Encode (if strict then G.E.H.H.Strict th else G.E.H.H.Shallow th)) /\
      G.E.H.resolve state (wasm_nat mapped) = None /\ scope state next 1 /\
      G.E.H.observations next = G.E.H.observations state + 2.
  Proof.
    intros Functions LhsLocal InputLocal LhsLive InputLive Observed Room.
    set (value := Encode (if strict then G.E.H.H.Strict th else G.E.H.H.Shallow th)).
    destruct (Acceptance.native_map_thunk_executes state store locals input_index th strict Functions InputLive Room)
      as [allocated [mapped [Created [Present [Fresh [Scoped Obs]]]]]].
    unfold Acceptance.Support.word, Acceptance.Support.descriptor in Created.
    assert (LhsNext : G.E.H.resolve allocated lhs_index = Some value) by (apply (proj1 Scoped); exact LhsLive).
    assert (ObservedNext : observed_same allocated value value).
    { eapply observed_comparison_observations; [exact Obs|exact Observed]. }
    exists (compared allocated), mapped; split.
    2: { split; [exact Present|]; split; [exact Fresh|]; split; [exact Scoped|].
         change (G.E.H.observations allocated + 2 = G.E.H.observations state + 2); rewrite Obs; reflexivity. }
    execute (G.E.collector_local_get combination true true handle_bytes state store locals 1%N (descriptor lhs_index) LhsLocal).
    execute_under [descriptor lhs_index; word (if strict then 3 else 4)]
      (G.E.collector_local_get combination true true handle_bytes state store locals 6%N (descriptor input_index) InputLocal).
    execute_under [descriptor lhs_index] Created.
    exact (same_run allocated store locals lhs_index (wasm_nat mapped) value value true Functions LhsNext Present ObservedNext).
  Qed.

  Lemma native_force_strict state store locals lhs_index rhs_index witnesses_index th datum witnesses bundle tag claim author code :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Trees.Resources.memory_capacity store ->
    G.E.H.resolve state lhs_index = Some (Encode (G.E.H.H.Strict th)) ->
    G.E.H.resolve state rhs_index = Some (lift (Data datum)) ->
    G.E.H.resolve state witnesses_index = Some (G.E.H.H.HTreeObj witnesses) ->
    get_tree_raw witnesses = [G.E.H.H.HTreeObj bundle] ->
    get_tree_raw bundle = [G.E.H.H.HTreeObj tag; G.E.H.H.HTreeObj claim] ->
    get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] -> tree_tag tag = true ->
    get_tree_raw claim = [G.E.H.H.HBlobObj code; Thunk th; Data datum] -> decode_u64 (get_blob_data code) = Some 3 ->
    observed_same state (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim) ->
    observed_same (at_observation state 2) (Encode (G.E.H.H.Strict th)) (Encode (G.E.H.H.Strict th)) ->
    observed_same (at_observation state 4) (Data datum) (Data datum) ->
    observed_same (at_observation state 6) (Thunk th) (Thunk th) ->
    observed_same (at_observation state 8) (Data datum) (Data datum) ->
    G.E.H.D.live_count (G.E.H.slots state) + 12 <= G.E.H.capacity ->
    exists next updated,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list (rule_arguments 7 lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
        (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [result_word 0]) /\
      scope state next 12 /\ s_funcs updated = s_funcs store /\ store_typing updated /\ Trees.Resources.memory_capacity updated.
  Proof.
    intros Functions Typed Capacity LhsLive RhsLive WitnessesLive Witnesses Bundle Tag Tagged Claim Decoded
      SameRef SameLhs SameRhs SameThunk SameData Room.
    destruct (force_endpoints_executes state store 7 lhs_index rhs_index witnesses_index witnesses bundle tag claim author code (Thunk th) (Data datum)
      ltac:(vm_compute; tauto) Functions WitnessesLive Witnesses Bundle Tag Tagged Claim SameRef ltac:(lia))
      as [first [p [a [b [Prefix [PLive [ALive [BLive [ScopePrefix Obs1]]]]]]]]].
    set (locals1 := endpoint_locals 7 lhs_index rhs_index witnesses_index p a b).
    pose proof ScopePrefix as Scope1.
    assert (Lhs1 : G.E.H.resolve first lhs_index = Some (Encode (G.E.H.H.Strict th)))
      by (apply (proj1 Scope1); exact LhsLive).
    assert (ObservedLhs1 : observed_same first (Encode (G.E.H.H.Strict th)) (Encode (G.E.H.H.Strict th)))
      by (eapply ready_at; [|exact SameLhs]; exact Obs1).
    destruct (force_constructor_executes first store locals1 lhs_index (wasm_nat a) th true Functions eq_refl eq_refl
      Lhs1 ALive ObservedLhs1
      ltac:(let Count := fresh "Count" in pose proof (proj2 Scope1) as Count; cbn [List.length] in Count; lia))
      as [second [mapped [Constructor [MappedLive [MappedFresh [Scope2 Obs2]]]]]].
    set (slots := [Some (wasm_nat p);None;Some (wasm_nat a);Some (wasm_nat b);None;Some (wasm_nat mapped);None]).
    set (inside := scratch_locals 7 lhs_index rhs_index witnesses_index slots 0).
    assert (Budget2 : scope state second 8).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    pose proof Budget2 as Scope02.
    assert (Rhs2 : G.E.H.resolve second rhs_index = Some (lift (Data datum))) by (apply (proj1 Scope02); exact RhsLive).
    assert (B2 : G.E.H.resolve second (wasm_nat b) = Some (Data datum)) by (apply (proj1 Scope2); exact BLive).
    assert (ObservedB2 : observed_same second (Data datum) (Data datum))
      by (eapply ready_at; [|exact SameRhs]; rewrite Obs2, Obs1; lia).
    destruct (strict_rhs_executes second store inside rhs_index (wasm_nat b) datum Functions eq_refl eq_refl Rhs2 B2 ObservedB2
      ltac:(let Count := fresh "Count" in pose proof (proj2 Scope02) as Count; cbn [List.length] in Count; lia))
      as [third [RhsRun [Scope3 Obs3]]].
    assert (Budget3 : scope state third 9).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    pose proof Budget3 as Scope03.
    assert (P3 : G.E.H.resolve third (wasm_nat p) = Some (G.E.H.H.HTreeObj claim))
      by (apply (proj1 Scope3), (proj1 Scope2); exact PLive).
    assert (A3 : G.E.H.resolve third (wasm_nat a) = Some (Thunk th))
      by (apply (proj1 Scope3), (proj1 Scope2); exact ALive).
    assert (B3 : G.E.H.resolve third (wasm_nat b) = Some (Data datum)) by (apply (proj1 Scope3); exact B2).
    assert (ObservedA3 : observed_same third (Thunk th) (Thunk th))
      by (eapply ready_at; [|exact SameThunk]; rewrite Obs3, Obs2, Obs1; lia).
    assert (ObservedB3 : observed_same (compared third) (Data datum) (Data datum)).
    { eapply ready_at; [|exact SameData]; change (G.E.H.observations third + 2 = G.E.H.observations state + 8).
      rewrite Obs3, Obs2, Obs1; lia. }
    destruct (Acceptance.Witnesses.collector_premise_executes third store inside (wasm_nat p) (wasm_nat a) (wasm_nat b)
      claim code 3 (Thunk th) (Data datum) Functions Typed Capacity P3 A3 B3 Claim Decoded ObservedA3 ObservedB3
      ltac:(let Count := fresh "Count" in pose proof (proj2 Scope03) as Count; cbn [List.length] in Count; lia))
      as [fourth [updated [Premise [Scope4 [Obs4 [Functions4 [Typed4 Capacity4]]]]]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in Premise.
    assert (Scoped : scope state fourth 12).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    exists fourth, updated; split; [|split; [exact Scoped|split; [exact Functions4|auto]]].
    eapply rule_call_executes with (next := fourth) (updated := updated)
      (following := Build_frame inside G.E.collector_instance)
      (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn 114 rule_body))); try exact Functions.
    eapply rule_prefix_continue; [exact Prefix|].
    unfold rule_body at 1; rewrite native_exchange_body; cbn [nth skipn].
    unfold locals1, endpoint_locals, scratch_locals, optional_descriptor, rule_arguments, result_word; cbn [firstn map].
    repeat scratch_pure; cbn [lfill].
    eapply G.E.collector_runs_context with (prefix := [])
      (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]); apply G.E.collector_runs_label.
    unfold locals1, endpoint_locals, scratch_locals, rule_arguments, result_word in Constructor.
    execute Constructor; unfold strict_rhs_code in RhsRun; execute RhsRun.
    do 3 scratch_pure; execute Premise; apply rt_refl.
  Qed.

  Lemma native_force_shallow state store locals lhs_index rhs_index witnesses_index th datum witnesses bundle tag claim author code :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Trees.Resources.memory_capacity store ->
    G.E.H.resolve state lhs_index = Some (Encode (G.E.H.H.Shallow th)) ->
    G.E.H.resolve state rhs_index = Some (Data (Ref (G.E.H.H.data_reference datum))) ->
    G.E.H.resolve state witnesses_index = Some (G.E.H.H.HTreeObj witnesses) ->
    get_tree_raw witnesses = [G.E.H.H.HTreeObj bundle] ->
    get_tree_raw bundle = [G.E.H.H.HTreeObj tag; G.E.H.H.HTreeObj claim] ->
    get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] -> tree_tag tag = true ->
    get_tree_raw claim = [G.E.H.H.HBlobObj code; Thunk th; Data datum] -> decode_u64 (get_blob_data code) = Some 3 ->
    observed_same state (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim) ->
    observed_same (at_observation state 2) (Encode (G.E.H.H.Shallow th)) (Encode (G.E.H.H.Shallow th)) ->
    observed_same (at_observation state 4) (Data (Ref (G.E.H.H.data_reference datum))) (Data (Ref (G.E.H.H.data_reference datum))) ->
    observed_same (at_observation state 6) (Thunk th) (Thunk th) ->
    observed_same (at_observation state 8) (Data datum) (Data datum) ->
    G.E.H.D.live_count (G.E.H.slots state) + 12 <= G.E.H.capacity ->
    exists next updated,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list (rule_arguments 16 lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
        (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [result_word 0]) /\
      scope state next 12 /\ s_funcs updated = s_funcs store /\ store_typing updated /\ Trees.Resources.memory_capacity updated.
  Proof.
    intros Functions Typed Capacity LhsLive RhsLive WitnessesLive Witnesses Bundle Tag Tagged Claim Decoded
      SameRef SameLhs SameRhs SameThunk SameData Room.
    destruct (force_endpoints_executes state store 16 lhs_index rhs_index witnesses_index witnesses bundle tag claim author code (Thunk th) (Data datum)
      ltac:(vm_compute; tauto) Functions WitnessesLive Witnesses Bundle Tag Tagged Claim SameRef ltac:(lia))
      as [first [p [a [b [Prefix [PLive [ALive [BLive [ScopePrefix Obs1]]]]]]]]].
    set (locals1 := endpoint_locals 16 lhs_index rhs_index witnesses_index p a b).
    pose proof ScopePrefix as Scope1.
    assert (Lhs1 : G.E.H.resolve first lhs_index = Some (Encode (G.E.H.H.Shallow th))) by (apply (proj1 Scope1); exact LhsLive).
    assert (ObservedLhs1 : observed_same first (Encode (G.E.H.H.Shallow th)) (Encode (G.E.H.H.Shallow th)))
      by (eapply ready_at; [|exact SameLhs]; exact Obs1).
    destruct (force_constructor_executes first store locals1 lhs_index (wasm_nat a) th false Functions eq_refl eq_refl
      Lhs1 ALive ObservedLhs1
      ltac:(let Count := fresh "Count" in pose proof (proj2 Scope1) as Count; cbn [List.length] in Count; lia))
      as [second [mapped [Constructor [MappedLive [MappedFresh [Scope2 Obs2]]]]]].
    set (slots2 := [Some (wasm_nat p);None;Some (wasm_nat a);Some (wasm_nat b);None;Some (wasm_nat mapped);None]).
    set (locals2 := scratch_locals 16 lhs_index rhs_index witnesses_index slots2 0).
    assert (Budget2 : scope state second 8).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    assert (B2 : G.E.H.resolve second (wasm_nat b) = Some (Data datum)) by (apply (proj1 Scope2); exact BLive).
    destruct (Acceptance.native_lower_executes second store locals2 (wasm_nat b) datum Functions B2
      ltac:(let Count := fresh "Count" in pose proof (proj2 Budget2) as Count; cbn [List.length] in Count; lia))
      as [third [lowered [LowerRun [LowerLive [Scope3 Obs3]]]]].
    unfold Acceptance.Support.word, Acceptance.Support.descriptor in LowerRun.
    set (slots := [Some (wasm_nat p);None;Some (wasm_nat a);Some (wasm_nat b);None;Some (wasm_nat mapped);Some lowered]).
    set (inside := scratch_locals 16 lhs_index rhs_index witnesses_index slots 0).
    assert (Budget3 : scope state third 9).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    pose proof Budget3 as Scope03.
    assert (Rhs3 : G.E.H.resolve third rhs_index = Some (Data (Ref (G.E.H.H.data_reference datum))))
      by (apply (proj1 Scope03); exact RhsLive).
    assert (ObservedRhs3 : observed_same third (Data (Ref (G.E.H.H.data_reference datum))) (Data (Ref (G.E.H.H.data_reference datum))))
      by (eapply ready_at; [|exact SameRhs]; rewrite Obs3, Obs2, Obs1; lia).
    pose proof (same_run third store inside rhs_index lowered (Data (Ref (G.E.H.H.data_reference datum)))
      (Data (Ref (G.E.H.H.data_reference datum))) true Functions Rhs3 LowerLive ObservedRhs3) as Compare.
    assert (BudgetCompared : scope state (compared third) 9).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    assert (P3 : G.E.H.resolve (compared third) (wasm_nat p) = Some (G.E.H.H.HTreeObj claim))
      by (apply (proj1 Scope3), (proj1 Scope2); exact PLive).
    assert (A3 : G.E.H.resolve (compared third) (wasm_nat a) = Some (Thunk th))
      by (apply (proj1 Scope3), (proj1 Scope2); exact ALive).
    assert (B3 : G.E.H.resolve (compared third) (wasm_nat b) = Some (Data datum)) by (apply (proj1 Scope3); exact B2).
    assert (ObservedA3 : observed_same (compared third) (Thunk th) (Thunk th)).
    { eapply ready_at; [|exact SameThunk]; change (G.E.H.observations third + 2 = G.E.H.observations state + 6); rewrite Obs3, Obs2, Obs1; lia. }
    assert (ObservedB3 : observed_same (compared (compared third)) (Data datum) (Data datum)).
    { eapply ready_at; [|exact SameData]; change (G.E.H.observations third + 2 + 2 = G.E.H.observations state + 8); rewrite Obs3, Obs2, Obs1; lia. }
    destruct (Acceptance.Witnesses.collector_premise_executes (compared third) store inside (wasm_nat p) (wasm_nat a) (wasm_nat b)
      claim code 3 (Thunk th) (Data datum) Functions Typed Capacity P3 A3 B3 Claim Decoded ObservedA3 ObservedB3
      ltac:(let Count := fresh "Count" in pose proof (proj2 Scope03) as Count; cbn [List.length] in Count;
        change (G.E.H.D.live_count (G.E.H.slots third) + 3 <= G.E.H.capacity); lia))
      as [fourth [updated [Premise [Scope4 [Obs4 [Functions4 [Typed4 Capacity4]]]]]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in Premise.
    assert (Scoped : scope state fourth 12).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    exists fourth, updated; split; [|split; [exact Scoped|split; [exact Functions4|auto]]].
    eapply rule_call_executes with (next := fourth) (updated := updated)
      (following := Build_frame inside G.E.collector_instance)
      (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn 126 rule_body))); try exact Functions.
    eapply rule_prefix_continue; [exact Prefix|].
    unfold rule_body at 1; rewrite native_exchange_body; cbn [nth skipn].
    unfold locals1, endpoint_locals, scratch_locals, optional_descriptor, rule_arguments, result_word; cbn [firstn map].
    repeat scratch_pure; cbn [lfill].
    eapply G.E.collector_runs_context with (prefix := [])
      (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]); apply G.E.collector_runs_label.
    unfold locals1, endpoint_locals, scratch_locals, rule_arguments, result_word in Constructor.
    execute Constructor; do 2 scratch_pure; execute_under [descriptor rhs_index] LowerRun.
    execute Compare.
    do 3 scratch_pure; execute Premise; apply rt_refl.
  Qed.

  Lemma native_force_results state store locals lhs_index rhs_index witnesses_index lhs rhs left right witnesses
      bundle0 tag0 claim0 author0 code0 bundle1 tag1 claim1 author1 code1 bundle2 tag2 claim2 author2 code2 :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Trees.Resources.memory_capacity store ->
    G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state witnesses_index = Some (G.E.H.H.HTreeObj witnesses) ->
    get_tree_raw witnesses = [G.E.H.H.HTreeObj bundle0;G.E.H.H.HTreeObj bundle1;G.E.H.H.HTreeObj bundle2] ->
    get_tree_raw bundle0 = [G.E.H.H.HTreeObj tag0;G.E.H.H.HTreeObj claim0] ->
    get_tree_raw tag0 = [author0;G.E.H.H.HTreeRef claim0] -> tree_tag tag0 = true ->
    get_tree_raw claim0 = [G.E.H.H.HBlobObj code0;lhs;left] -> decode_u64 (get_blob_data code0) = Some 3 ->
    get_tree_raw bundle1 = [G.E.H.H.HTreeObj tag1;G.E.H.H.HTreeObj claim1] ->
    get_tree_raw tag1 = [author1;G.E.H.H.HTreeRef claim1] -> tree_tag tag1 = true ->
    get_tree_raw claim1 = [G.E.H.H.HBlobObj code1;rhs;right] -> decode_u64 (get_blob_data code1) = Some 3 ->
    get_tree_raw bundle2 = [G.E.H.H.HTreeObj tag2;G.E.H.H.HTreeObj claim2] ->
    get_tree_raw tag2 = [author2;G.E.H.H.HTreeRef claim2] -> tree_tag tag2 = true ->
    get_tree_raw claim2 = [G.E.H.H.HBlobObj code2;left;right] -> decode_u64 (get_blob_data code2) = Some 0 ->
    observed_same state (G.E.H.H.HTreeRef claim0) (G.E.H.H.HTreeRef claim0) ->
    observed_same (at_observation state 2) (G.E.H.H.HTreeRef claim1) (G.E.H.H.HTreeRef claim1) ->
    observed_same (at_observation state 4) lhs lhs -> observed_same (at_observation state 6) left left ->
    observed_same (at_observation state 8) rhs rhs -> observed_same (at_observation state 10) right right ->
    observed_same (at_observation state 12) (G.E.H.H.HTreeRef claim2) (G.E.H.H.HTreeRef claim2) ->
    observed_same (at_observation state 14) left left -> observed_same (at_observation state 16) right right ->
    G.E.H.D.live_count (G.E.H.slots state) + 26 <= G.E.H.capacity ->
    exists next updated,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list (rule_arguments 1 lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
        (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [result_word 0]) /\
      scope state next 26 /\ s_funcs updated = s_funcs store /\ store_typing updated /\ Trees.Resources.memory_capacity updated.
  Proof.
    intros Functions Typed Capacity LhsLive RhsLive WitnessesLive Witnesses
      Bundle0 Tag0 Tagged0 Claim0 Decoded0 Bundle1 Tag1 Tagged1 Claim1 Decoded1 Bundle2 Tag2 Tagged2 Claim2 Decoded2
      SameRef0 SameRef1 SameLhs SameLeft SameRhs SameRight SameRef2 SameLeft2 SameRight2 Room.
    set (initial := initial_locals 1 lhs_index rhs_index witnesses_index).
    destruct (Acceptance.Witnesses.witness_claim_executes state store initial witnesses_index witnesses 0 bundle0 tag0 claim0 author0
      Functions WitnessesLive ltac:(rewrite Witnesses; reflexivity) ltac:(lia) Bundle0 Tag0 Tagged0
      ltac:(rewrite Claim0; reflexivity) SameRef0 ltac:(lia))
      as [first [p [Reopen0 [PLive [PFresh [Scope1 Obs1]]]]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in Reopen0.
    assert (Budget1 : scope state first 5).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    set (locals1 := single_locals 1 lhs_index rhs_index witnesses_index p 0).
    assert (Witnesses1 : G.E.H.resolve first witnesses_index = Some (G.E.H.H.HTreeObj witnesses))
      by (apply (proj1 Scope1); exact WitnessesLive).
    assert (ObservedRef1 : observed_same first (G.E.H.H.HTreeRef claim1) (G.E.H.H.HTreeRef claim1))
      by (eapply ready_at; [|exact SameRef1]; exact Obs1).
    destruct (Acceptance.Witnesses.witness_claim_executes first store locals1 witnesses_index witnesses 1 bundle1 tag1 claim1 author1
      Functions Witnesses1 ltac:(rewrite Witnesses; reflexivity) ltac:(lia) Bundle1 Tag1 Tagged1
      ltac:(rewrite Claim1; reflexivity) ObservedRef1 ltac:(let Count := fresh "Count" in pose proof (proj2 Scope1) as Count; lia))
      as [second [q [Reopen1 [QLive [QFresh [Scope2 Obs2]]]]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in Reopen1.
    assert (Budget2 : scope state second 10).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    set (locals2 := scratch_locals 1 lhs_index rhs_index witnesses_index
      [Some (wasm_nat p);Some (wasm_nat q);None;None;None;None;None] 0).
    assert (P2 : G.E.H.resolve second (wasm_nat p) = Some (G.E.H.H.HTreeObj claim0)) by (apply (proj1 Scope2); exact PLive).
    destruct (Acceptance.exchange_endpoint_executes second store locals2 (wasm_nat p) claim0 left false Functions P2
      ltac:(rewrite Claim0; reflexivity) ltac:(let Count := fresh "Count" in pose proof (proj2 Budget2) as Count; cbn [List.length] in Count; lia))
      as [third [a [LeftRun [ALive [AFresh [Scope3 Obs3]]]]]].
    unfold Acceptance.Support.word, Acceptance.Support.descriptor in LeftRun.
    assert (Budget3 : scope state third 11).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    set (locals3 := scratch_locals 1 lhs_index rhs_index witnesses_index
      [Some (wasm_nat p);Some (wasm_nat q);Some (wasm_nat a);None;None;None;None] 0).
    assert (Q3 : G.E.H.resolve third (wasm_nat q) = Some (G.E.H.H.HTreeObj claim1)) by (apply (proj1 Scope3); exact QLive).
    destruct (Acceptance.exchange_endpoint_executes third store locals3 (wasm_nat q) claim1 right false Functions Q3
      ltac:(rewrite Claim1; reflexivity) ltac:(let Count := fresh "Count" in pose proof (proj2 Budget3) as Count; cbn [List.length] in Count; lia))
      as [fourth [b [RightRun [BLive [BFresh [Scope4 Obs4]]]]]].
    unfold Acceptance.Support.word, Acceptance.Support.descriptor in RightRun.
    assert (Budget4 : scope state fourth 12).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    pose proof Budget4 as Scope04.
    set (locals4 := scratch_locals 1 lhs_index rhs_index witnesses_index
      [Some (wasm_nat p);Some (wasm_nat q);Some (wasm_nat a);Some (wasm_nat b);None;None;None] 0).
    assert (P4 : G.E.H.resolve fourth (wasm_nat p) = Some (G.E.H.H.HTreeObj claim0)) by (apply (proj1 Scope4), (proj1 Scope3); exact P2).
    assert (A4 : G.E.H.resolve fourth (wasm_nat a) = Some left) by (apply (proj1 Scope4); exact ALive).
    assert (Lhs4 : G.E.H.resolve fourth lhs_index = Some lhs) by (apply (proj1 Scope04); exact LhsLive).
    assert (ObservedLhs4 : observed_same fourth lhs lhs) by (eapply ready_at; [|exact SameLhs]; rewrite Obs4, Obs3, Obs2, Obs1; lia).
    assert (ObservedA4 : observed_same (compared fourth) left left).
    { eapply ready_at; [|exact SameLeft]; change (G.E.H.observations fourth + 2 = G.E.H.observations state + 6).
      rewrite Obs4, Obs3, Obs2, Obs1; lia. }
    destruct (Acceptance.Witnesses.collector_premise_executes fourth store locals4 (wasm_nat p) lhs_index (wasm_nat a)
      claim0 code0 3 lhs left Functions Typed Capacity P4 Lhs4 A4 Claim0 Decoded0 ObservedLhs4 ObservedA4
      ltac:(let Count := fresh "Count" in pose proof (proj2 Scope04) as Count; cbn [List.length] in Count; lia))
      as [fifth [store1 [Premise0 [Scope5 [Obs5 [Functions5 [Typed5 Capacity5]]]]]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in Premise0.
    assert (Budget5 : scope state fifth 15).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    pose proof Budget5 as Scope05.
    assert (FunctionsStore1 : s_funcs store1 = s_funcs G.E.allocated_store) by (rewrite Functions5; exact Functions).
    assert (Q5 : G.E.H.resolve fifth (wasm_nat q) = Some (G.E.H.H.HTreeObj claim1)) by (apply (proj1 Scope5), (proj1 Scope4); exact Q3).
    assert (B5 : G.E.H.resolve fifth (wasm_nat b) = Some right) by (apply (proj1 Scope5); exact BLive).
    assert (Rhs5 : G.E.H.resolve fifth rhs_index = Some rhs) by (apply (proj1 Scope05); exact RhsLive).
    assert (ObservedRhs5 : observed_same fifth rhs rhs) by (eapply ready_at; [|exact SameRhs]; rewrite Obs5, Obs4, Obs3, Obs2, Obs1; lia).
    assert (ObservedB5 : observed_same (compared fifth) right right).
    { eapply ready_at; [|exact SameRight]; change (G.E.H.observations fifth + 2 = G.E.H.observations state + 10).
      rewrite Obs5, Obs4, Obs3, Obs2, Obs1; lia. }
    destruct (Acceptance.Witnesses.collector_premise_executes fifth store1 locals4 (wasm_nat q) rhs_index (wasm_nat b)
      claim1 code1 3 rhs right FunctionsStore1 Typed5 Capacity5 Q5 Rhs5 B5 Claim1 Decoded1 ObservedRhs5 ObservedB5
      ltac:(let Count := fresh "Count" in pose proof (proj2 Scope05) as Count; cbn [List.length] in Count; lia))
      as [sixth [store2 [Premise1 [Scope6 [Obs6 [Functions6 [Typed6 Capacity6]]]]]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in Premise1.
    assert (Budget6 : scope state sixth 18).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    pose proof Budget6 as Scope06.
    assert (FunctionsStore2 : s_funcs store2 = s_funcs G.E.allocated_store) by (rewrite Functions6; exact FunctionsStore1).
    assert (Witnesses6 : G.E.H.resolve sixth witnesses_index = Some (G.E.H.H.HTreeObj witnesses)) by (apply (proj1 Scope06); exact WitnessesLive).
    assert (ObservedRef6 : observed_same sixth (G.E.H.H.HTreeRef claim2) (G.E.H.H.HTreeRef claim2)).
    { eapply ready_at; [|exact SameRef2]; rewrite Obs6, Obs5, Obs4, Obs3, Obs2, Obs1; lia. }
    destruct (Acceptance.Witnesses.witness_claim_executes sixth store2 locals4 witnesses_index witnesses 2 bundle2 tag2 claim2 author2
      FunctionsStore2 Witnesses6 ltac:(rewrite Witnesses; reflexivity) ltac:(lia) Bundle2 Tag2 Tagged2
      ltac:(rewrite Claim2; reflexivity) ObservedRef6
      ltac:(let Count := fresh "Count" in pose proof (proj2 Scope06) as Count; cbn [List.length] in Count; lia))
      as [seventh [t [Reopen2 [TLive [TFresh [Scope7 Obs7]]]]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in Reopen2.
    set (slots := [Some (wasm_nat p);Some (wasm_nat q);Some (wasm_nat a);Some (wasm_nat b);Some (wasm_nat t);None;None]).
    set (inside := scratch_locals 1 lhs_index rhs_index witnesses_index slots 0).
    assert (Budget7 : scope state seventh 23).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    pose proof Budget7 as Scope07.
    assert (A7 : G.E.H.resolve seventh (wasm_nat a) = Some left)
      by (apply (proj1 Scope7), (proj1 Scope6), (proj1 Scope5); exact A4).
    assert (B7 : G.E.H.resolve seventh (wasm_nat b) = Some right) by (apply (proj1 Scope7), (proj1 Scope6); exact B5).
    assert (ObservedA7 : observed_same seventh left left).
    { eapply ready_at; [|exact SameLeft2]; rewrite Obs7, Obs6, Obs5, Obs4, Obs3, Obs2, Obs1; lia. }
    assert (ObservedB7 : observed_same (compared seventh) right right).
    { eapply ready_at; [|exact SameRight2]; change (G.E.H.observations seventh + 2 = G.E.H.observations state + 16).
      rewrite Obs7, Obs6, Obs5, Obs4, Obs3, Obs2, Obs1; lia. }
    destruct (Acceptance.Witnesses.collector_premise_executes seventh store2 inside (wasm_nat t) (wasm_nat a) (wasm_nat b)
      claim2 code2 0 left right FunctionsStore2 Typed6 Capacity6 TLive A7 B7 Claim2 Decoded2 ObservedA7 ObservedB7
      ltac:(let Count := fresh "Count" in pose proof (proj2 Scope07) as Count; cbn [List.length] in Count; lia))
      as [eighth [updated [Premise2 [Scope8 [Obs8 [Functions8 [Typed8 Capacity8]]]]]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in Premise2.
    assert (Scoped : scope state eighth 26).
    { eapply scope_weaken; [solve [eauto 10 using scope_trans, compared_scope, scope_refl]|lia]. }
    exists eighth, updated; split; [|split; [exact Scoped|split; [rewrite Functions8, Functions6, Functions5; reflexivity|auto]]].
    eapply rule_call_executes with (next := eighth) (updated := updated)
      (following := Build_frame inside G.E.collector_instance)
      (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn 24 rule_body))); try exact Functions.
    unfold rule_body at 1; rewrite native_exchange_body; cbn [nth].
    unfold initial, initial_locals, rule_arguments, result_word.
    do 18 scratch_pure; cbn [lfill].
    eapply G.E.collector_runs_context with (prefix := [])
      (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]); apply G.E.collector_runs_label.
    scratch_pure; execute (arity_run state store initial witnesses_index witnesses 3 Functions WitnessesLive
      ltac:(rewrite Witnesses; reflexivity) ltac:(vm_compute; congruence)).
    scratch_pure; execute Reopen0; scratch_pure.
    scratch_pure; execute Reopen1; scratch_pure.
    scratch_pure; execute LeftRun; scratch_pure.
    scratch_pure; execute RightRun; scratch_pure.
    do 3 scratch_pure; execute Premise0.
    do 3 scratch_pure; execute Premise1.
    scratch_pure; execute_under [VAL_num (VAL_int64 (wasm_nat64 0))] Reopen2.
    do 2 scratch_pure; execute Premise2; apply rt_refl.
  Qed.
End WasmForceSuccess.
