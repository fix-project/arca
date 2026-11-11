Module WasmForceSuccess.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Import WasmRules WasmSupport WasmRuleSuccess WasmBinaryRules WasmForceRules.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation lift := X.D.Context.EC.E.EP.E.lift.

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

End WasmForceSuccess.
