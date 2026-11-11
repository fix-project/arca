Module WasmAuthority.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Support := WasmSupport.
  Import Support.
  Module Witnesses := WasmWitnesses.
  Module Guard := WasmAuthorityGuard.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Definition authority_locals combination_index collector_index witnesses_index count index witness author :=
    [descriptor combination_index; descriptor collector_index; descriptor witnesses_index;
      witness; author; word count; word index].

  Definition iteration_ready state collector witnesses index :=
    exists bundle tag claim author,
      nth_error (get_tree_raw witnesses) index = Some (treeh bundle) /\
      get_tree_raw bundle = [treeh tag; treeh claim] /\
      get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] /\ tree_tag tag = true /\
      List.length (get_tree_raw claim) = 3 /\ Guard.author_bounded author /\
      observed_same state (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim) /\
      (author = X.kernel_author \/ observed_same (compared state) collector author).

  Lemma authority_loop_body : Guard.authority_loop =
    ltac:(let code := eval cbv [Guard.authority_loop native_authority modfunc_body mod_funcs evaluation_module nth] in
      Guard.authority_loop in exact code).
  Proof. reflexivity. Qed.

  Lemma authority_iteration_executes state store combination_index collector_index witnesses_index collector witnesses
      count index scratch_witness scratch_author :
    s_funcs store = s_funcs G.E.allocated_store ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.resolve state collector_index = Some collector -> G.E.H.resolve state witnesses_index = Some (treeh witnesses) ->
    index < count -> (Z.of_nat count < 2 ^ 32)%Z -> iteration_ready state collector witnesses index ->
    G.E.H.D.live_count (G.E.H.slots state) + 7 <= G.E.H.capacity ->
    exists next witness_slot author_slot,
      reduce_trans
        (state, store, Build_frame (authority_locals combination_index collector_index witnesses_index count index
          scratch_witness scratch_author) G.E.collector_instance, to_e_list Guard.authority_loop)
        (next, store, Build_frame (authority_locals combination_index collector_index witnesses_index count (Datatypes.S index)
          (word witness_slot) (word author_slot)) G.E.collector_instance,
          [AI_basic (BI_br 0%N)]) /\ scope state next 7.
  Proof.
    intros Functions Bytes CollectorLive WitnessesLive Index Count Ready Room.
    destruct Ready as [bundle [tag [claim [author Ready]]]].
    destruct Ready as [Field [Bundle [Tag [Tagged [Arity [Bound [Observed Allowed]]]]]]].
    set (initial := authority_locals combination_index collector_index witnesses_index count index
      scratch_witness scratch_author).
    assert (IndexBound : (Z.of_nat index < 2 ^ 32)%Z) by lia.
    destruct (child_run state store initial witnesses_index witnesses index (treeh bundle)
      Functions WitnessesLive Field IndexBound ltac:(lia))
      as [first [witness_slot [WitnessRun [WitnessLive [WitnessFresh [Scope1 Obs1]]]]]].
    pose proof (proj2 Scope1) as Count1.
    set (locals1 := authority_locals combination_index collector_index witnesses_index count index
      (word witness_slot) scratch_author).
    assert (Observed1 : observed_same first (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim)).
    { eapply observed_comparison_observations; eassumption. }
    destruct (Witnesses.collector_claim_executes first store locals1 (wasm_nat witness_slot) bundle tag claim author
      Functions WitnessLive Bundle Tag Tagged Arity Observed1 ltac:(lia))
      as [second [claim_slot [ClaimRun [ClaimLive [ClaimFresh [Scope2 Obs2]]]]]].
    unfold Witnesses.Support.word, Witnesses.Support.descriptor in ClaimRun.
    pose proof (proj2 Scope2) as Count2.
    set (locals2 := authority_locals combination_index collector_index witnesses_index count index
      (word witness_slot) scratch_author).
    assert (Witness2 : G.E.H.resolve second (wasm_nat witness_slot) = Some (treeh bundle))
      by (apply (proj1 Scope2); exact WitnessLive).
    destruct (child_run second store locals2 (wasm_nat witness_slot) bundle 0 (treeh tag)
      Functions Witness2 ltac:(rewrite Bundle; reflexivity) ltac:(lia) ltac:(lia))
      as [third [coupon_slot [CouponRun [CouponLive [CouponFresh [Scope3 Obs3]]]]]].
    pose proof (proj2 Scope3) as Count3.
    set (locals3 := authority_locals combination_index collector_index witnesses_index count index
      (word witness_slot) scratch_author).
    destruct (child_run third store locals3 (wasm_nat coupon_slot) tag 0 author
      Functions CouponLive ltac:(rewrite Tag; reflexivity) ltac:(lia) ltac:(lia))
      as [fourth [author_slot [AuthorRun [AuthorLive [AuthorFresh [Scope4 Obs4]]]]]].
    set (inside := authority_locals combination_index collector_index witnesses_index count index
      (word witness_slot) (word author_slot)).
    assert (Collector4 : G.E.H.resolve fourth collector_index = Some collector).
    { apply (proj1 Scope4), (proj1 Scope3), (proj1 Scope2), (proj1 Scope1); exact CollectorLive. }
    assert (Allowed4 : author = X.kernel_author \/ observed_same fourth collector author).
    { destruct Allowed as [Kernel|Same]; [left; exact Kernel|right].
      eapply observed_comparison_observations; [|exact Same].
      change (G.E.H.observations fourth = G.E.H.observations state + 2).
      rewrite Obs4, Obs3, Obs2, Obs1; reflexivity. }
    destruct (Guard.authority_author_accepts fourth store (descriptor combination_index) (descriptor witnesses_index)
      (word witness_slot) collector_index (wasm_nat author_slot)
      collector author count index Functions Bound Bytes Collector4 AuthorLive Allowed4)
      as [fifth [GuardRun Scope5]].
    unfold Guard.Support.word, Guard.Support.descriptor in GuardRun.
    assert (Scoped : scope state fifth 7).
    { replace 7 with (1+(4+(1+(1+0)))) by lia.
      eapply scope_trans; [exact Scope1|]; eapply scope_trans; [exact Scope2|].
      eapply scope_trans; [exact Scope3|]; eapply scope_trans; eassumption. }
    exists fifth, witness_slot, author_slot; split; [|exact Scoped].
    rewrite authority_loop_body.
    do 2 native_pure.
    pose proof (counter_equal_run state store initial index count IndexBound Count) as Counter.
    assert (Different : Nat.eqb index count = false) by (apply Nat.eqb_neq; lia).
    rewrite Different in Counter; execute Counter.
    execute (branch_if_run state store (Build_frame initial G.E.collector_instance) 1%N false).
    do 2 native_pure; execute WitnessRun; native_pure.
    native_pure; execute ClaimRun; native_pure.
    native_pure; execute CouponRun; execute AuthorRun; native_pure.
    execute GuardRun.
    native_pure; execute (counter_increment_run fifth store inside index IndexBound).
    native_pure; apply rt_refl.
  Qed.

  Lemma authority_loop_executes remaining state store combination_index collector_index witnesses_index collector witnesses count index
      scratch_witness scratch_author :
    s_funcs store = s_funcs G.E.allocated_store ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.resolve state collector_index = Some collector -> G.E.H.resolve state witnesses_index = Some (treeh witnesses) ->
    index + remaining = count -> (Z.of_nat count < 2 ^ 32)%Z ->
    (forall current offset, retains state current -> offset < count -> iteration_ready current collector witnesses offset) ->
    G.E.H.D.live_count (G.E.H.slots state) + 7 * remaining <= G.E.H.capacity ->
    exists next final_witness final_author,
      reduce_trans
        (state, store, Build_frame (authority_locals combination_index collector_index witnesses_index count index
          scratch_witness scratch_author) G.E.collector_instance,
          G.E.collector_loop_state Guard.authority_loop (to_e_list Guard.authority_loop))
        (next, store, Build_frame (authority_locals combination_index collector_index witnesses_index count count
          final_witness final_author) G.E.collector_instance, []) /\ scope state next (7 * remaining).
  Proof.
    revert state index scratch_witness scratch_author.
    induction remaining as [|remaining IH]; intros state index scratch_witness scratch_author
      Functions Bytes CollectorLive WitnessesLive Remaining Count Ready Room.
    - assert (Equal : index = count) by lia; subst index.
      set (inside := authority_locals combination_index collector_index witnesses_index count count
        scratch_witness scratch_author).
      exists state, scratch_witness, scratch_author; split; [|apply scope_refl].
      eapply G.E.collector_loop_exit with (accepted := true) (suffix := to_e_list (skipn 4 Guard.authority_loop)).
      rewrite authority_loop_body; do 2 native_pure.
      pose proof (counter_equal_run state store inside count count Count Count) as Counter.
      rewrite Nat.eqb_refl in Counter; execute Counter.
      execute (branch_if_run state store (Build_frame inside G.E.collector_instance) 1%N true); apply rt_refl.
    - assert (Index : index < count) by lia.
      destruct (authority_iteration_executes state store combination_index collector_index witnesses_index collector witnesses count index
        scratch_witness scratch_author Functions Bytes CollectorLive WitnessesLive Index Count
        (Ready state index (retains_refl state) Index) ltac:(lia))
        as [first [witness_slot [author_slot [Step Scoped]]]].
      assert (Collector1 : G.E.H.resolve first collector_index = Some collector) by (apply (proj1 Scoped); exact CollectorLive).
      assert (Witnesses1 : G.E.H.resolve first witnesses_index = Some (treeh witnesses))
        by (apply (proj1 Scoped); exact WitnessesLive).
      destruct (IH first (Datatypes.S index) (word witness_slot) (word author_slot)
        Functions Bytes Collector1 Witnesses1 ltac:(lia) Count)
        as [next [final_witness [final_author [Rest Scoped1]]]].
      + intros current offset Scope1 Offset; apply Ready; [|exact Offset].
        eapply retains_trans; [exact (proj1 Scoped)|exact Scope1].
      + destruct Scoped; lia.
      + exists next, final_witness, final_author; split.
        * eapply rt_trans; [apply G.E.collector_loop_continue; exact Step|exact Rest].
        * replace (7 * Datatypes.S remaining) with (7+7*remaining) by lia; eapply scope_trans; eassumption.
  Qed.

  Lemma collector_authority_execution state store locals combination_index collector witnesses :
    s_funcs store = s_funcs G.E.allocated_store ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.resolve state combination_index = Some (treeh combination) ->
    nth_error (get_tree_raw combination) 1 = Some collector ->
    nth_error (get_tree_raw combination) 5 = Some (treeh witnesses) ->
    native_size_bound (List.length (get_tree_raw witnesses)) ->
    Guard.witnesses_bounded witnesses ->
    (forall current index, retains state current -> index < List.length (get_tree_raw witnesses) ->
      iteration_ready current collector witnesses index) ->
    G.E.H.D.live_count (G.E.H.slots state) + (2 + 7 * List.length (get_tree_raw witnesses)) <= G.E.H.capacity ->
    exists next,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [descriptor combination_index] ++ [AI_basic (BI_call 40%N)])
        (next, store, Build_frame locals G.E.collector_instance, []) /\ scope state next (2 + 7 * List.length (get_tree_raw witnesses)).
  Proof.
    intros Functions Bytes CombinationLive Collector Witnesses Bound Limit Ready Room.
    set (count := List.length (get_tree_raw witnesses)).
    set (initial := descriptor combination_index :: repeat (word 0) 6).
    destruct (child_run state store initial combination_index combination 1 collector Functions CombinationLive Collector ltac:(lia) ltac:(lia))
      as [first [collector_index [CollectorRun [CollectorLive [CollectorFresh [Scope1 Obs1]]]]]].
    pose proof (proj2 Scope1) as Count1.
    set (locals1 := [descriptor combination_index; word collector_index; word 0; word 0; word 0; word 0; word 0]).
    assert (Combination1 : G.E.H.resolve first combination_index = Some (treeh combination))
      by (apply (proj1 Scope1); exact CombinationLive).
    destruct (child_run first store locals1 combination_index combination 5 (treeh witnesses) Functions Combination1 Witnesses ltac:(lia) ltac:(lia))
      as [second [witnesses_index [WitnessesRun [WitnessesLive [WitnessesFresh [Scope2 Obs2]]]]]].
    pose proof (proj2 Scope2) as Count2.
    set (locals2 := [descriptor combination_index; word collector_index; word witnesses_index; word 0; word 0; word 0; word 0]).
    set (inside := authority_locals combination_index (wasm_nat collector_index) (wasm_nat witnesses_index) count 0
      (word 0) (word 0)).
    assert (Collector2 : G.E.H.resolve second (wasm_nat collector_index) = Some collector)
      by (apply (proj1 Scope2); exact CollectorLive).
    assert (Scope02 : scope state second 2).
    { replace 2 with (1+1) by lia; eapply scope_trans; eassumption. }
    destruct (authority_loop_executes count second store combination_index (wasm_nat collector_index) (wasm_nat witnesses_index)
      collector witnesses count 0 (word 0) (word 0) Functions Bytes Collector2 WitnessesLive eq_refl
      (native_size_bound_word _ Bound))
      as [third [final_witness [final_author [LoopRun Scope3]]]].
    - intros current index Scoped Index; apply Ready; [|exact Index].
      eapply retains_trans; [exact (proj1 Scope02)|exact Scoped].
    - lia.
    - set (final_locals := authority_locals combination_index (wasm_nat collector_index) (wasm_nat witnesses_index) count count
        final_witness final_author).
      assert (Scoped : scope state third (2+7*count)).
      { eapply scope_trans; eassumption. }
      exists third; split; [|exact Scoped].
      eapply G.E.collector_call_void with (code := native_authority)
        (parameters := [T_num T_i32]) (defaults := repeat (word 0) 6) (address := 40%N)
        (arguments := [descriptor combination_index]) (accepted := true)
        (final_frame := Build_frame final_locals G.E.collector_instance); try reflexivity.
      + rewrite Functions; reflexivity.
      + change (reduce_trans (state, store, Build_frame initial G.E.collector_instance,
          to_e_list (modfunc_body native_authority)) (third, store, Build_frame final_locals G.E.collector_instance, [])).
        rewrite native_authority_body.
        native_pure; execute CollectorRun; native_pure.
        native_pure; execute WitnessesRun; native_pure.
        native_pure.
        execute (G.collector_size_executes combination handle_bytes (address := wasm_nat witnesses_index)
          second store locals2 witnesses Functions Bound WitnessesLive).
        native_pure.
        native_pure.
        pose proof (Guard.witness_limit_run second store inside count (native_size_bound_word _ Bound)) as LimitRun.
        assert (Fits : Z.leb (Z.of_nat count) Guard.witness_limit = true) by (apply Z.leb_le; exact Limit).
        rewrite Fits in LimitRun; execute LimitRun.
        execute (native_require second store inside true Functions).
        assert (Loop : reduce_trans
          (second, store, Build_frame inside G.E.collector_instance,
            [AI_basic (BI_block (BT_valtype None) [BI_loop (BT_valtype None) Guard.authority_loop])])
          (third, store, Build_frame final_locals G.E.collector_instance, [])).
        { eapply rt_trans; [apply G.E.collector_loop_enter|exact LoopRun]. }
        exact Loop.
  Qed.
End WasmAuthority.
