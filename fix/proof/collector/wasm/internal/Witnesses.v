Module WasmWitnesses.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Support := WasmSupport.
  Import Support.
  Module Resources := WasmResources.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Definition authenticated_claim witness claim :=
    exists bundle tag author,
      witness = treeh bundle /\ get_tree_raw bundle = [treeh tag; treeh claim] /\
      get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] /\ tree_tag tag = true /\
      List.length (get_tree_raw claim) = 3.

  Lemma collector_claim_executes state store locals root bundle tag claim author :
    s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state root = Some (treeh bundle) ->
    get_tree_raw bundle = [treeh tag; treeh claim] ->
    get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] -> tree_tag tag = true ->
    List.length (get_tree_raw claim) = 3 ->
    observed_same state (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim) ->
    G.E.H.D.live_count (G.E.H.slots state) + 4 <= G.E.H.capacity ->
    exists next output,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [descriptor root] ++ [AI_basic (BI_call 30%N)])
        (next, store, Build_frame locals G.E.collector_instance, v_to_e_list [word output]) /\
      G.E.H.resolve next (wasm_nat output) = Some (treeh claim) /\
      G.E.H.resolve state (wasm_nat output) = None /\ scope state next 4 /\
      G.E.H.observations next = G.E.H.observations state + 2.
  Proof.
    intros Functions Root Bundle Tag Tagged Length Observed Room.
    set (initial := [descriptor root; word 0; word 0]).
    destruct (child_run state store initial root bundle 0 (treeh tag) Functions Root
      ltac:(rewrite Bundle; reflexivity) ltac:(lia) ltac:(lia))
      as [first [coupon [CouponRun [CouponLive [CouponFresh [Scope1 Obs1]]]]]].
    set (locals1 := [descriptor root; word coupon; word 0]).
    assert (Root1 : G.E.H.resolve first root = Some (treeh bundle)) by (apply (proj1 Scope1); exact Root).
    destruct (child_run first store locals1 root bundle 1 (treeh claim) Functions Root1
      ltac:(rewrite Bundle; reflexivity) ltac:(lia) ltac:(destruct Scope1; lia))
      as [second [assertion [ClaimRun [ClaimLive [ClaimFresh [Scope2 Obs2]]]]]].
    set (locals2 := [descriptor root; word coupon; word assertion]).
    assert (Coupon2 : G.E.H.resolve second (wasm_nat coupon) = Some (treeh tag))
      by (apply (proj1 Scope2); exact CouponLive).
    destruct (child_run second store locals2 (wasm_nat coupon) tag 1 (G.E.H.H.HTreeRef claim)
      Functions Coupon2 ltac:(rewrite Tag; reflexivity) ltac:(lia)
      ltac:(destruct Scope1; destruct Scope2; lia))
      as [third [reference [RefRun [RefLive [RefFresh [Scope3 Obs3]]]]]].
    set (inside := locals2).
    assert (Claim3 : G.E.H.resolve third (wasm_nat assertion) = Some (treeh claim))
      by (apply (proj1 Scope3); exact ClaimLive).
    assert (Observed3 : observed_same third (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim)).
    { eapply observed_comparison_observations; [rewrite Obs3, Obs2, Obs1; reflexivity|exact Observed]. }
    destruct (object_run third store locals2 (wasm_nat reference) (wasm_nat assertion)
      (G.E.H.H.HTreeRef claim) (TreeObj claim) Functions RefLive Claim3 Observed3
      ltac:(destruct Scope1; destruct Scope2; destruct Scope3; lia))
      as [fourth [ObjectRun [Scope4 Obs4]]].
    assert (Scoped : scope state fourth 4).
    { replace 4 with (1 + (1 + (1 + 1))) by lia.
      eapply scope_trans; [exact Scope1|]; eapply scope_trans; [exact Scope2|].
      eapply scope_trans; eassumption. }
    assert (ClaimNext : G.E.H.resolve fourth (wasm_nat assertion) = Some (treeh claim))
      by (apply (proj1 Scope4); exact Claim3).
    exists fourth, assertion; split.
    2: { split; [exact ClaimNext|]; split.
         - eapply fresh_before; [exact (proj1 Scope1)|exact ClaimFresh].
         - split; [exact Scoped|rewrite Obs4, Obs3, Obs2, Obs1; reflexivity]. }
    eapply G.E.collector_call with (code := evaluation_claim)
      (parameters := [T_num T_i32]) (results := [T_num T_i32])
      (defaults := [word 0; word 0]) (address := 30%N)
      (arguments := [descriptor root]) (values := [word assertion]) (accepted := true)
      (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
    - rewrite Functions; reflexivity.
    - change (reduce_trans (state, store, Build_frame initial G.E.collector_instance,
        to_e_list (modfunc_body evaluation_claim))
        (fourth, store, Build_frame inside G.E.collector_instance, v_to_e_list [word assertion])).
      rewrite evaluation_claim_body.
      native_pure.
      execute (arity_run state store initial root bundle 2 Functions Root
        ltac:(rewrite Bundle; reflexivity) ltac:(vm_compute; congruence)).
      native_pure; execute CouponRun; native_pure.
      native_pure; execute ClaimRun; native_pure.
      native_pure.
      execute (tag_run second store locals2 (wasm_nat coupon) (treeh tag) true Functions Coupon2 Tagged).
      native_pure.
      native_pure.
      execute (arity_run second store locals2 (wasm_nat coupon) tag 2 Functions Coupon2
        ltac:(rewrite Tag; reflexivity) ltac:(vm_compute; congruence)).
      native_pure.
      execute (arity_run second store locals2 (wasm_nat assertion) claim 3 Functions ClaimLive Length
        ltac:(vm_compute; congruence)).
      native_pure; execute RefRun.
      native_pure; execute ObjectRun.
      native_pure; apply rt_refl.
  Qed.

  Lemma witness_claim_executes state store locals root witnesses index bundle tag claim author :
    s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state root = Some (treeh witnesses) ->
    nth_error (get_tree_raw witnesses) index = Some (treeh bundle) ->
    (Z.of_nat index < 2 ^ 32)%Z ->
    get_tree_raw bundle = [treeh tag; treeh claim] ->
    get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] -> tree_tag tag = true ->
    List.length (get_tree_raw claim) = 3 ->
    observed_same state (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim) ->
    G.E.H.D.live_count (G.E.H.slots state) + 5 <= G.E.H.capacity ->
    exists next output,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [descriptor root; word index] ++ [AI_basic (BI_call 46%N)])
        (next, store, Build_frame locals G.E.collector_instance, v_to_e_list [word output]) /\
      G.E.H.resolve next (wasm_nat output) = Some (treeh claim) /\
      G.E.H.resolve state (wasm_nat output) = None /\ scope state next 5 /\
      G.E.H.observations next = G.E.H.observations state + 2.
  Proof.
    intros Functions Root Field Bound Bundle Tag Tagged Length Observed Room.
    set (inside := [descriptor root; word index]).
    destruct (child_run state store inside root witnesses index (treeh bundle)
      Functions Root Field Bound ltac:(lia))
      as [first [witness [WitnessRun [WitnessLive [WitnessFresh [Scope1 Obs1]]]]]].
    assert (Observed1 : observed_same first (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim)).
    { eapply observed_comparison_observations; eassumption. }
    destruct (collector_claim_executes first store inside (wasm_nat witness) bundle tag claim author
      Functions WitnessLive Bundle Tag Tagged Length Observed1 ltac:(destruct Scope1; lia))
      as [next [assertion [ClaimRun [ClaimLive [ClaimFresh [Scope2 Obs2]]]]]].
    exists next, assertion; split.
    - eapply G.E.collector_call with (code := native_witness_claim)
        (parameters := [T_num T_i32; T_num T_i32]) (results := [T_num T_i32])
        (defaults := []) (address := 46%N) (arguments := inside) (values := [word assertion])
        (accepted := true) (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
      + rewrite Functions; reflexivity.
      + rewrite native_witness_claim_body.
        do 2 native_pure; execute WitnessRun; exact ClaimRun.
    - split; [exact ClaimLive|]; split.
      + eapply fresh_before; [exact (proj1 Scope1)|exact ClaimFresh].
      + split.
        * replace 5 with (1+4) by lia; eapply scope_trans; eassumption.
        * rewrite Obs2,Obs1; reflexivity.
  Qed.

  Definition claim_ready state witnesses index :=
    exists bundle tag claim author,
      nth_error (get_tree_raw witnesses) index = Some (treeh bundle) /\
      get_tree_raw bundle = [treeh tag; treeh claim] /\
      get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] /\ tree_tag tag = true /\
      List.length (get_tree_raw claim) = 3 /\
      observed_same state (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim).

  Lemma collector_premise_executes state store locals claim_index lhs_index rhs_index claim code kind lhs rhs :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state claim_index = Some (treeh claim) ->
    G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    get_tree_raw claim = [G.E.H.H.HBlobObj code; lhs; rhs] -> decode_u64 (get_blob_data code) = Some kind ->
    observed_same state lhs lhs -> observed_same (compared state) rhs rhs ->
    G.E.H.D.live_count (G.E.H.slots state) + 3 <= G.E.H.capacity ->
    exists next updated,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 kind)); descriptor claim_index;
            descriptor lhs_index; descriptor rhs_index] ++ [AI_basic (BI_call 31%N)])
        (next, updated, Build_frame locals G.E.collector_instance, []) /\
      scope state next 3 /\ G.E.H.observations next = G.E.H.observations state + 4 /\
      s_funcs updated = s_funcs store /\ store_typing updated /\ Resources.memory_capacity updated.
  Proof.
    intros Functions Typed Capacity ClaimLive LhsLive RhsLive Fields Code LhsObserved RhsObserved Room.
    set (inside := [VAL_num (VAL_int64 (wasm_nat64 kind)); descriptor claim_index;
      descriptor lhs_index; descriptor rhs_index]).
    destruct (child_run state store inside claim_index claim 0 (G.E.H.H.HBlobObj code)
      Functions ClaimLive ltac:(rewrite Fields; reflexivity) ltac:(lia) ltac:(lia))
      as [first [code_slot [CodeRun [CodeLive [CodeFresh [Scope1 Obs1]]]]]].
    destruct (Resources.integer_available first store inside (wasm_nat code_slot) code kind
      Functions Typed Capacity CodeLive Code)
      as [updated [IntegerRun [SameFunctions [TypedNext CapacityNext]]]].
    assert (FunctionsNext : s_funcs updated = s_funcs G.E.allocated_store) by (rewrite SameFunctions; exact Functions).
    assert (Claim1 : G.E.H.resolve first claim_index = Some (treeh claim))
      by (apply (proj1 Scope1); exact ClaimLive).
    destruct (child_run first updated inside claim_index claim 1 lhs FunctionsNext Claim1
      ltac:(rewrite Fields; reflexivity) ltac:(lia) ltac:(destruct Scope1; lia))
      as [second [left_slot [LeftRun [LeftLive [LeftFresh [Scope2 Obs2]]]]]].
    assert (Scope02 : scope state second 2).
    { replace 2 with (1+1) by lia; eapply scope_trans; eassumption. }
    assert (Lhs2 : G.E.H.resolve second lhs_index = Some lhs) by (apply (proj1 Scope02); exact LhsLive).
    assert (LeftObserved2 : observed_same second lhs lhs).
    { eapply observed_comparison_observations; [rewrite Obs2, Obs1; reflexivity|exact LhsObserved]. }
    pose proof (same_run second updated inside (wasm_nat left_slot) lhs_index lhs lhs true
      FunctionsNext LeftLive Lhs2 LeftObserved2) as SameLeft.
    assert (Claim2 : G.E.H.resolve (compared second) claim_index = Some (treeh claim))
      by (apply (proj1 Scope02); exact ClaimLive).
    destruct (child_run (compared second) updated inside claim_index claim 2 rhs FunctionsNext Claim2
      ltac:(rewrite Fields; reflexivity) ltac:(lia) ltac:(pose proof (proj2 Scope02) as Count; change (G.E.H.D.live_count (G.E.H.slots second) < G.E.H.capacity); lia))
      as [third [right_slot [RightRun [RightLive [RightFresh [Scope3 Obs3]]]]]].
    assert (Scope03 : scope state third 3).
    { replace 3 with (2+1) by lia; eapply scope_trans; [exact Scope02|exact Scope3]. }
    assert (Rhs3 : G.E.H.resolve third rhs_index = Some rhs) by (apply (proj1 Scope03); exact RhsLive).
    assert (RightObserved3 : observed_same third rhs rhs).
    { eapply observed_comparison_observations; [|exact RhsObserved].
      rewrite Obs3; change (G.E.H.observations second + 2 = G.E.H.observations state + 2).
      rewrite Obs2, Obs1; reflexivity. }
    pose proof (same_run third updated inside (wasm_nat right_slot) rhs_index rhs rhs true
      FunctionsNext RightLive Rhs3 RightObserved3) as SameRight.
    exists (compared third), updated; split.
    - eapply G.E.collector_call_void with (code := evaluation_premise)
        (parameters := [T_num T_i64; T_num T_i32; T_num T_i32; T_num T_i32])
        (defaults := []) (address := 31%N) (arguments := inside) (accepted := true)
        (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
      + rewrite Functions; reflexivity.
      + rewrite evaluation_premise_header.
        native_pure; execute CodeRun; execute IntegerRun; native_pure.
        execute (equal64_run first updated inside (wasm_nat64 kind)).
        execute (native_require first updated inside true FunctionsNext).
        native_pure; execute LeftRun; native_pure; execute SameLeft.
        native_pure; execute RightRun; native_pure; exact SameRight.
    - split; [exact Scope03|]; split.
      + change (G.E.H.observations third + 2 = G.E.H.observations state + 4).
        rewrite Obs3; change (G.E.H.observations second + 2 + 2 = G.E.H.observations state + 4).
        rewrite Obs2, Obs1; lia.
      + auto.
  Qed.

  Definition decoded_claim number witness left right :=
    exists bundle tag claim author code,
      witness = treeh bundle /\ get_tree_raw bundle = [treeh tag; treeh claim] /\
      get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] /\ tree_tag tag = true /\
      get_tree_raw claim = [G.E.H.H.HBlobObj code; left; right] /\ decode_u64 (get_blob_data code) = Some number.

  Lemma read_decoded_claim kind number witness left right :
    Claims.Code.statement_kind number = Some kind -> X.read kind witness = Some (left, right) ->
    decoded_claim number witness left right.
  Proof.
    intros Kind Read; unfold X.read in Read.
    destruct (X.Wire.decode witness) as [[[[decoded author] a] b]|] eqn:Decoded; [|discriminate].
    destruct (X.API.statement_kind_eq_dec decoded kind); [subst decoded|discriminate].
    inversion Read; subst a b.
    destruct (X.Wire.decode_authenticates _ _ _ _ _ Decoded)
      as [bundle [tag [claim [code [number0 [Witness [Bundle [Tag [Tagged [Claim [Number Kind0]]]]]]]]]]].
    pose proof (Claims.Code.statement_kind_roundtrip _ _ Kind) as Code.
    pose proof (Claims.Code.statement_kind_roundtrip _ _ Kind0) as Code0.
    assert (Equal : number0 = number) by congruence; subst number0.
    exists bundle, tag, claim, author, code; repeat split; assumption.
  Qed.
End WasmWitnesses.
