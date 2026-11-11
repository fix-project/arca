Module WasmTreeLoop.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Support := WasmSupport.
  Import Support.
  Module Resources := WasmResources.
  Module Witnesses := WasmWitnesses.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Definition native_tree_loop := match nth 15 (modfunc_body native_tree_rule) BI_unreachable with
    | BI_block _ [BI_loop _ body] => body | _ => [] end.

  Lemma native_tree_loop_body : native_tree_loop =
    ltac:(let code := eval cbv [native_tree_loop native_tree_rule modfunc_body mod_funcs evaluation_module nth] in
      native_tree_loop in exact code).
  Proof. reflexivity. Qed.

  Definition iteration_ready state lhs rhs witnesses kind index :=
    exists bundle tag claim author code left right,
      nth_error (get_tree_raw witnesses) index = Some (treeh bundle) /\
      get_tree_raw bundle = [treeh tag; treeh claim] /\
      get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] /\ tree_tag tag = true /\
      get_tree_raw claim = [G.E.H.H.HBlobObj code; left; right] /\ decode_u64 (get_blob_data code) = Some kind /\
      nth_error (get_tree_raw lhs) index = Some left /\ nth_error (get_tree_raw rhs) index = Some right /\
      observed_same state (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim) /\
      observed_same (compared state) left left /\ observed_same (compared (compared state)) right right.

  Definition loop_locals kind lhs rhs witnesses count index :=
    [VAL_num (VAL_int64 (wasm_nat64 kind)); descriptor lhs; descriptor rhs; descriptor witnesses;
      word count; word index].

  Lemma tree_iteration_executes state store lhs_index rhs_index witnesses_index lhs rhs witnesses kind count index
 :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state lhs_index = Some (treeh lhs) -> G.E.H.resolve state rhs_index = Some (treeh rhs) ->
    G.E.H.resolve state witnesses_index = Some (treeh witnesses) ->
    index < count -> (Z.of_nat count < 2 ^ 32)%Z -> iteration_ready state lhs rhs witnesses kind index ->
    G.E.H.D.live_count (G.E.H.slots state) + 10 <= G.E.H.capacity ->
    exists next updated,
      reduce_trans
        (state, store, Build_frame (loop_locals kind lhs_index rhs_index witnesses_index count index) G.E.collector_instance, to_e_list native_tree_loop)
        (next, updated, Build_frame (loop_locals kind lhs_index rhs_index witnesses_index count (Datatypes.S index)) G.E.collector_instance, [AI_basic (BI_br 0%N)]) /\
      scope state next 10 /\ G.E.H.observations next = G.E.H.observations state + 6 /\
      s_funcs updated = s_funcs store /\ store_typing updated /\ Resources.memory_capacity updated.
  Proof.
    intros Functions Typed Capacity LhsLive RhsLive WitnessesLive Index Count Ready Room.
    destruct Ready as [bundle [tag [claim [author [code [left [right Ready]]]]]]].
    destruct Ready as [Field [Bundle [Tag [Tagged [Claim [Decoded [Left [Right [Observed [SameLeft SameRight]]]]]]]]]].
    set (initial := loop_locals kind lhs_index rhs_index witnesses_index count index).
    assert (IndexBound : (Z.of_nat index < 2 ^ 32)%Z) by lia.
    destruct (Witnesses.witness_claim_executes state store initial witnesses_index witnesses index bundle tag claim author
      Functions WitnessesLive Field IndexBound Bundle Tag Tagged ltac:(rewrite Claim; reflexivity) Observed ltac:(lia))
      as [first [claim_slot [ClaimRun [ClaimLive [ClaimFresh [Scope1 Obs1]]]]]].
    unfold Witnesses.Support.word, Witnesses.Support.descriptor in ClaimRun.
    pose proof (proj2 Scope1) as Count1.
    set (locals1 := loop_locals kind lhs_index rhs_index witnesses_index count index).
    assert (Lhs1 : G.E.H.resolve first lhs_index = Some (treeh lhs)) by (apply (proj1 Scope1); exact LhsLive).
    destruct (child_run first store locals1 lhs_index lhs index left Functions Lhs1 Left IndexBound ltac:(lia))
      as [second [left_slot [LeftRun [LeftLive [LeftFresh [Scope2 Obs2]]]]]].
    pose proof (proj2 Scope2) as Count2.
    set (locals2 := loop_locals kind lhs_index rhs_index witnesses_index count index).
    assert (Scope02 : scope state second 6).
    { replace 6 with (5+1) by lia; eapply scope_trans; eassumption. }
    assert (Rhs2 : G.E.H.resolve second rhs_index = Some (treeh rhs)) by (apply (proj1 Scope02); exact RhsLive).
    destruct (child_run second store locals2 rhs_index rhs index right Functions Rhs2 Right IndexBound ltac:(lia))
      as [third [right_slot [RightRun [RightLive [RightFresh [Scope3 Obs3]]]]]].
    pose proof (proj2 Scope3) as Count3.
    set (inside := loop_locals kind lhs_index rhs_index witnesses_index count index).
    assert (Scope03 : scope state third 7).
    { replace 7 with (6+1) by lia; eapply scope_trans; eassumption. }
    assert (Claim3 : G.E.H.resolve third (wasm_nat claim_slot) = Some (treeh claim))
      by (apply (proj1 Scope3), (proj1 Scope2); exact ClaimLive).
    assert (Left3 : G.E.H.resolve third (wasm_nat left_slot) = Some left) by (apply (proj1 Scope3); exact LeftLive).
    assert (SameLeft3 : observed_same third left left).
    { eapply observed_comparison_observations; [|exact SameLeft].
      change (G.E.H.observations third = G.E.H.observations state + 2); rewrite Obs3, Obs2, Obs1; reflexivity. }
    assert (SameRight3 : observed_same (compared third) right right).
    { eapply observed_comparison_observations; [|exact SameRight].
      change (G.E.H.observations third + 2 = G.E.H.observations state + 2 + 2); rewrite Obs3, Obs2, Obs1; reflexivity. }
    destruct (Witnesses.collector_premise_executes third store inside (wasm_nat claim_slot) (wasm_nat left_slot)
      (wasm_nat right_slot) claim code kind left right Functions Typed Capacity Claim3 Left3 RightLive
      Claim Decoded SameLeft3 SameRight3 ltac:(lia))
      as [fourth [updated [PremiseRun [Scope4 [Obs4 [Functions4 [Typed4 Capacity4]]]]]]].
    unfold Witnesses.Support.word, Witnesses.Support.descriptor in PremiseRun.
    assert (Scoped : scope state fourth 10).
    { replace 10 with (7+3) by lia; eapply scope_trans; eassumption. }
    exists fourth, updated; split.
    2: { split; [exact Scoped|]; split; [rewrite Obs4, Obs3, Obs2, Obs1; lia|auto]. }
    rewrite native_tree_loop_body.
    do 2 native_pure.
    pose proof (counter_equal_run state store initial index count IndexBound Count) as Counter.
    assert (Different : Nat.eqb index count = false) by (apply Nat.eqb_neq; lia).
    rewrite Different in Counter; execute Counter.
    execute (branch_if_run state store (Build_frame initial G.E.collector_instance) 1%N false).
    do 3 native_pure; execute_under [VAL_num (VAL_int64 (wasm_nat64 kind))] ClaimRun.
    do 2 native_pure; execute_under [VAL_num (VAL_int64 (wasm_nat64 kind)); word claim_slot] LeftRun.
    do 2 native_pure; execute_under [VAL_num (VAL_int64 (wasm_nat64 kind)); word claim_slot; word left_slot] RightRun.
    execute PremiseRun.
    native_pure; execute (counter_increment_run fourth updated inside index IndexBound).
    native_pure; apply rt_refl.
  Qed.

  Lemma tree_loop_executes remaining state store lhs_index rhs_index witnesses_index lhs rhs witnesses kind count index
 :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state lhs_index = Some (treeh lhs) -> G.E.H.resolve state rhs_index = Some (treeh rhs) ->
    G.E.H.resolve state witnesses_index = Some (treeh witnesses) ->
    index + remaining = count -> (Z.of_nat count < 2 ^ 32)%Z ->
    (forall current offset, retains state current -> offset < count -> iteration_ready current lhs rhs witnesses kind offset) ->
    G.E.H.D.live_count (G.E.H.slots state) + 10 * remaining <= G.E.H.capacity ->
    exists next updated,
      reduce_trans
        (state, store, Build_frame (loop_locals kind lhs_index rhs_index witnesses_index count index) G.E.collector_instance,
          G.E.collector_loop_state native_tree_loop (to_e_list native_tree_loop))
        (next, updated, Build_frame (loop_locals kind lhs_index rhs_index witnesses_index count count) G.E.collector_instance, []) /\
      scope state next (10 * remaining) /\ G.E.H.observations next = G.E.H.observations state + 6 * remaining /\
      s_funcs updated = s_funcs store /\ store_typing updated /\ Resources.memory_capacity updated.
  Proof.
    revert state store index.
    induction remaining as [|remaining IH]; intros state store index
      Functions Typed Capacity LhsLive RhsLive WitnessesLive Remaining Count Ready Room.
    - assert (Equal : index = count) by lia; subst index.
      set (inside := loop_locals kind lhs_index rhs_index witnesses_index count count).
      exists state, store; split.
      2: { split; [apply scope_refl|]; split; [lia|auto]. }
      eapply G.E.collector_loop_exit with (accepted := true) (suffix := to_e_list (skipn 4 native_tree_loop)).
      rewrite native_tree_loop_body.
      do 2 native_pure.
      pose proof (counter_equal_run state store inside count count Count Count) as Counter.
      rewrite Nat.eqb_refl in Counter; execute Counter.
      execute (branch_if_run state store (Build_frame inside G.E.collector_instance) 1%N true).
      apply rt_refl.
    - assert (Index : index < count) by lia.
      destruct (tree_iteration_executes state store lhs_index rhs_index witnesses_index lhs rhs witnesses kind count index
        Functions Typed Capacity LhsLive RhsLive WitnessesLive Index Count
        (Ready state index (retains_refl state) Index) ltac:(lia))
        as [first [intermediate [Step [Scoped [Obs [Functions1 [Typed1 Capacity1]]]]]]].
      assert (FunctionsNext : s_funcs intermediate = s_funcs G.E.allocated_store) by (rewrite Functions1; exact Functions).
      assert (Lhs1 : G.E.H.resolve first lhs_index = Some (treeh lhs)) by (apply (proj1 Scoped); exact LhsLive).
      assert (Rhs1 : G.E.H.resolve first rhs_index = Some (treeh rhs)) by (apply (proj1 Scoped); exact RhsLive).
      assert (Witnesses1 : G.E.H.resolve first witnesses_index = Some (treeh witnesses))
        by (apply (proj1 Scoped); exact WitnessesLive).
      destruct (IH first intermediate (Datatypes.S index)
        FunctionsNext Typed1 Capacity1 Lhs1 Rhs1 Witnesses1 ltac:(lia) Count)
        as [next [updated [Rest [Scoped1 [Obs1 [Functions2 [Typed2 Capacity2]]]]]]].
      + intros current offset Scope1 Offset; apply Ready; [|exact Offset].
        eapply retains_trans; [exact (proj1 Scoped)|exact Scope1].
      + destruct Scoped; lia.
      + exists next, updated; split.
        * eapply rt_trans; [apply G.E.collector_loop_continue; exact Step|exact Rest].
        * split.
          -- replace (10 * Datatypes.S remaining) with (10 + 10 * remaining) by lia; eapply scope_trans; eassumption.
          -- split; [rewrite Obs1, Obs; lia|split; [rewrite Functions2, Functions1; reflexivity|auto]].
  Qed.

  Lemma native_tree_rule_executes state store locals lhs_index rhs_index witnesses_index lhs rhs witnesses kind :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state lhs_index = Some (treeh lhs) -> G.E.H.resolve state rhs_index = Some (treeh rhs) ->
    G.E.H.resolve state witnesses_index = Some (treeh witnesses) ->
    List.length (get_tree_raw rhs) = List.length (get_tree_raw lhs) ->
    List.length (get_tree_raw witnesses) = List.length (get_tree_raw lhs) -> tree_tag lhs = tree_tag rhs ->
    native_size_bound (List.length (get_tree_raw lhs)) ->
    (forall current index, retains state current -> index < List.length (get_tree_raw lhs) ->
      iteration_ready current lhs rhs witnesses kind index) ->
    G.E.H.D.live_count (G.E.H.slots state) + 10 * List.length (get_tree_raw lhs) <= G.E.H.capacity ->
    exists next updated,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 kind)); descriptor lhs_index; descriptor rhs_index;
            descriptor witnesses_index] ++ [AI_basic (BI_call 37%N)])
        (next, updated, Build_frame locals G.E.collector_instance, []) /\
      scope state next (10 * List.length (get_tree_raw lhs)) /\ G.E.H.observations next = G.E.H.observations state + 6 * List.length (get_tree_raw lhs) /\
      s_funcs updated = s_funcs store /\ store_typing updated /\ Resources.memory_capacity updated.
  Proof.
    intros Functions Typed Capacity LhsLive RhsLive WitnessesLive RightCount WitnessCount Markers Bound Ready Room.
    set (count := List.length (get_tree_raw lhs)).
    set (arguments := [VAL_num (VAL_int64 (wasm_nat64 kind)); descriptor lhs_index; descriptor rhs_index; descriptor witnesses_index]).
    set (initial := arguments ++ [word 0; word 0]).
    set (inside := loop_locals kind lhs_index rhs_index witnesses_index count 0).
    destruct (tree_loop_executes count state store lhs_index rhs_index witnesses_index lhs rhs witnesses kind count 0
      Functions Typed Capacity LhsLive RhsLive WitnessesLive eq_refl
      (native_size_bound_word _ Bound) Ready Room)
      as [next [updated [LoopRun Result]]].
    exists next, updated; split; [|exact Result].
    eapply G.E.collector_call_void with (code := native_tree_rule)
      (parameters := [T_num T_i64; T_num T_i32; T_num T_i32; T_num T_i32])
      (defaults := [word 0; word 0]) (address := 37%N) (arguments := arguments)
      (accepted := true) (final_frame := Build_frame
        (loop_locals kind lhs_index rhs_index witnesses_index count count)
        G.E.collector_instance); try reflexivity.
    - rewrite Functions; reflexivity.
    - change (reduce_trans (state, store, Build_frame initial G.E.collector_instance,
        to_e_list (modfunc_body native_tree_rule))
        (next, updated, Build_frame
          (loop_locals kind lhs_index rhs_index witnesses_index count count)
          G.E.collector_instance, [])).
      rewrite native_tree_rule_body.
      native_pure.
      execute (G.collector_size_executes combination handle_bytes (address := lhs_index)
        state store initial lhs Functions Bound LhsLive).
      native_pure.
      do 2 native_pure.
      execute (arity_run state store inside rhs_index rhs count Functions RhsLive RightCount Bound).
      do 2 native_pure.
      execute (arity_run state store inside witnesses_index witnesses count Functions WitnessesLive WitnessCount Bound).
      native_pure.
      execute (tag_run state store inside lhs_index (treeh lhs) (tree_tag lhs) Functions LhsLive eq_refl).
      native_pure.
      execute_under [VAL_num (VAL_int32 (wasm_bool (tree_tag lhs)))]
        (tag_run state store inside rhs_index (treeh rhs) (tree_tag lhs) Functions RhsLive ltac:(cbn; symmetry; exact Markers)).
      destruct (tree_tag lhs); do 2 native_pure.
      all: eapply rt_trans; [apply G.E.collector_loop_enter|exact LoopRun].
  Qed.
End WasmTreeLoop.
