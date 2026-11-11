  Lemma checked_authority_iteration state store combination_index collector_index witnesses_index collector witnesses
      count index witness scratch_witness scratch_author :
    s_funcs store = s_funcs G.E.allocated_store ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.resolve state collector_index = Some collector -> G.E.H.resolve state witnesses_index = Some (treeh witnesses) ->
    index < count -> (Z.of_nat count < 2 ^ 32)%Z ->
    nth_error (get_tree_raw witnesses) index = Some witness -> lengths_bounded 3 witness ->
    G.E.H.D.live_count (G.E.H.slots state) + 7 <= G.E.H.capacity ->
    checked
      (state, store, Build_frame (authority_locals combination_index collector_index witnesses_index count index
        scratch_witness scratch_author) G.E.collector_instance, to_e_list Guard.authority_loop)
      (fun final => exists next witness_slot author_slot,
        final = (next, store, Build_frame (authority_locals combination_index collector_index witnesses_index count (Datatypes.S index)
          (word witness_slot) (word author_slot)) G.E.collector_instance,
          [AI_basic (BI_br 0%N)]) /\ scope state next 7 /\ authorized collector witness).
  Proof.
    intros Functions Bytes CollectorLive WitnessesLive Index Count Field Bounds Room.
    set (initial := authority_locals combination_index collector_index witnesses_index count index
      scratch_witness scratch_author).
    assert (IndexBound : (Z.of_nat index < 2 ^ 32)%Z) by lia.
    destruct (child_run state store initial witnesses_index witnesses index witness
      Functions WitnessesLive Field IndexBound ltac:(lia))
      as [first [witness_slot [WitnessRun [WitnessLive [WitnessFresh [Scope1 Obs1]]]]]].
    pose proof (proj2 Scope1) as Count1.
    set (locals1 := authority_locals combination_index collector_index witnesses_index count index
      (word witness_slot) scratch_author).
    rewrite authority_loop_body.
    do 2 authority_pure.
    pose proof (counter_equal_run state store initial index count IndexBound Count) as Counter.
    assert (Different : Nat.eqb index count = false) by (apply Nat.eqb_neq; lia).
    rewrite Different in Counter; unfold Authority.Support.word, Authority.Support.descriptor in Counter; checked_advance Counter.
    checked_advance (branch_if_run state store (Build_frame initial G.E.collector_instance) 1%N false).
    do 2 authority_pure; checked_advance WitnessRun; authority_pure.
    authority_pure.
    checked_use (checked_claim first store locals1 (wasm_nat witness_slot) witness
      Functions WitnessLive (lengths_bounded_weaken 2 witness Bounds) Bytes ltac:(lia)).
    intros final [second [claim_slot [claim [Final [Authenticated [ClaimLive [ClaimFresh [Scope2 Obs2]]]]]]]];
      subst final; normalize_checked.
    destruct Authenticated as [bundle [tag [author [Witness [Bundle [Tag [Tagged Arity]]]]]]]; subst witness.
    pose proof (proj2 Scope2) as Count2.
    authority_pure.
    set (locals2 := authority_locals combination_index collector_index witnesses_index count index
      (word witness_slot) scratch_author).
    assert (Witness2 : G.E.H.resolve second (wasm_nat witness_slot) = Some (treeh bundle))
      by (apply (proj1 Scope2); exact WitnessLive).
    destruct (child_run second store locals2 (wasm_nat witness_slot) bundle 0 (treeh tag)
      Functions Witness2 ltac:(rewrite Bundle; reflexivity) ltac:(lia) ltac:(lia))
      as [third [coupon_slot [CouponRun [CouponLive [CouponFresh [Scope3 Obs3]]]]]].
    pose proof (proj2 Scope3) as Count3.
    authority_pure; checked_advance CouponRun.
    set (locals3 := authority_locals combination_index collector_index witnesses_index count index
      (word witness_slot) scratch_author).
    destruct (child_run third store locals3 (wasm_nat coupon_slot) tag 0 author
      Functions CouponLive ltac:(rewrite Tag; reflexivity) ltac:(lia) ltac:(lia))
      as [fourth [author_slot [AuthorRun [AuthorLive [AuthorFresh [Scope4 Obs4]]]]]].
    checked_advance AuthorRun; authority_pure.
    set (inside := authority_locals combination_index collector_index witnesses_index count index
      (word witness_slot) (word author_slot)).
    assert (Collector4 : G.E.H.resolve fourth collector_index = Some collector).
    { apply (proj1 Scope4), (proj1 Scope3), (proj1 Scope2), (proj1 Scope1); exact CollectorLive. }
    assert (Bound : Guard.author_bounded author).
    { assert (AuthorBound : lengths_bounded 1 author).
      { eapply lengths_bounded_child; [|rewrite Tag; cbn; auto].
        eapply lengths_bounded_child; [exact Bounds|rewrite Bundle; cbn; auto]. }
      destruct author as [[[blob|node]|reference]|thunk|encoded]; cbn [Guard.author_bounded]; try exact I.
      exact (proj1 AuthorBound). }
    destruct (Guard.authority_author fourth store (descriptor combination_index) (descriptor witnesses_index)
      (word witness_slot) collector_index (wasm_nat author_slot)
      collector author count index Functions Bound Bytes Collector4 AuthorLive)
      as [fifth [accepted [GuardRun [Scope5 [Allowed Complete]]]]].
    unfold Guard.Support.word, Guard.Support.descriptor in GuardRun.
    destruct accepted.
    2: { eapply checked_trap; eapply G.E.collector_trap_suffix with
        (instructions := to_e_list [Guard.authority_accept]); exact GuardRun. }
    checked_advance GuardRun.
    assert (Scoped : scope state fifth 7).
    { replace 7 with (1+(4+(1+(1+0)))) by lia.
      eapply scope_trans; [exact Scope1|]; eapply scope_trans; [exact Scope2|].
      eapply scope_trans; [exact Scope3|]; eapply scope_trans; eassumption. }
    authority_pure; checked_advance (counter_increment_run fifth store inside index IndexBound).
    authority_pure.
    eapply checked_result; [apply rt_refl|].
    exists fifth, witness_slot, author_slot; split; [reflexivity|]; split; [exact Scoped|].
    exists bundle, tag, claim, author; repeat split; try reflexivity; try assumption.
    apply Allowed; reflexivity.
  Qed.

  Lemma checked_authority_loop remaining state store combination_index collector_index witnesses_index collector witnesses
      count index scratch_witness scratch_author :
    s_funcs store = s_funcs G.E.allocated_store ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.resolve state collector_index = Some collector -> G.E.H.resolve state witnesses_index = Some (treeh witnesses) ->
    index + remaining = count -> count = List.length (get_tree_raw witnesses) -> (Z.of_nat count < 2 ^ 32)%Z ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    G.E.H.D.live_count (G.E.H.slots state) + 7 * remaining <= G.E.H.capacity ->
    checked
      (state, store, Build_frame (authority_locals combination_index collector_index witnesses_index count index
        scratch_witness scratch_author) G.E.collector_instance,
        G.E.collector_loop_state Guard.authority_loop (to_e_list Guard.authority_loop))
      (fun final => exists next final_witness final_author,
        final = (next, store, Build_frame (authority_locals combination_index collector_index witnesses_index count count
          final_witness final_author) G.E.collector_instance, []) /\ scope state next (7 * remaining) /\
        forall offset witness, index <= offset < count -> nth_error (get_tree_raw witnesses) offset = Some witness ->
          authorized collector witness).
  Proof.
    revert state index scratch_witness scratch_author.
    induction remaining as [|remaining IH]; intros state index scratch_witness scratch_author
      Functions Bytes CollectorLive WitnessesLive Remaining Length Count Bounds Room.
    - assert (Equal : index = count) by lia; subst index.
      set (inside := authority_locals combination_index collector_index witnesses_index count count
        scratch_witness scratch_author).
      eapply checked_result.
      + eapply G.E.collector_loop_exit with (accepted := true) (suffix := to_e_list (skipn 4 Guard.authority_loop)).
        rewrite authority_loop_body; do 2 (cbn [authority_locals]; unfold Authority.Support.word, Authority.Support.descriptor; native_pure).
        pose proof (counter_equal_run state store inside count count Count Count) as Counter.
        rewrite Nat.eqb_refl in Counter; unfold Authority.Support.word, Authority.Support.descriptor in Counter; execute Counter.
        execute (branch_if_run state store (Build_frame inside G.E.collector_instance) 1%N true); apply rt_refl.
      + exists state, scratch_witness, scratch_author.
        split; [reflexivity|]; split; [apply scope_refl|]; intros; lia.
    - assert (Index : index < count) by lia.
      destruct (nth_error (get_tree_raw witnesses) index) as [witness|] eqn:Field.
      2: { apply nth_error_None in Field; lia. }
      assert (WitnessBound : lengths_bounded 3 witness).
      { rewrite Forall_forall in Bounds; apply Bounds; eapply nth_error_In; exact Field. }
      destruct (checked_authority_iteration state store combination_index collector_index witnesses_index collector witnesses
        count index witness scratch_witness scratch_author Functions Bytes
        CollectorLive WitnessesLive Index Count Field WitnessBound ltac:(lia)) as [final [Step [Trap|Success]]].
      + destruct final as [[[next updated] following] instructions]; subst instructions.
        eapply checked_trap; eapply G.E.collector_loop_exit with (accepted := false) (suffix := []); exact Step.
      + destruct Success as [first [witness_slot [author_slot [Final [Scoped Authorized]]]]]; subst final.
        assert (Collector1 : G.E.H.resolve first collector_index = Some collector)
          by (apply (proj1 Scoped); exact CollectorLive).
        assert (Witnesses1 : G.E.H.resolve first witnesses_index = Some (treeh witnesses))
          by (apply (proj1 Scoped); exact WitnessesLive).
        eapply checked_progress; [apply G.E.collector_loop_continue; exact Step|].
        eapply checked_weaken.
        * exact (IH first (Datatypes.S index) (word witness_slot) (word author_slot)
            Functions Bytes Collector1 Witnesses1 ltac:(lia) Length Count Bounds ltac:(destruct Scoped; lia)).
        * intros final [next [final_witness [final_author [Final [Scoped1 Rest]]]]].
          exists next, final_witness, final_author; split; [exact Final|]; split.
          -- replace (7 * Datatypes.S remaining) with (7+7*remaining) by lia; eapply scope_trans; eassumption.
          -- intros offset value Range Entry.
             destruct (Nat.eq_dec offset index) as [Equal|Different].
             ++ subst offset; rewrite Field in Entry; inversion Entry; subst value; exact Authorized.
             ++ apply (Rest offset value); [lia|exact Entry].
  Qed.

  Lemma collector_authority_validation state store locals combination_index collector witnesses :
    s_funcs store = s_funcs G.E.allocated_store ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.resolve state combination_index = Some (treeh combination) ->
    nth_error (get_tree_raw combination) 1 = Some collector ->
    nth_error (get_tree_raw combination) 5 = Some (treeh witnesses) ->
    lengths_bounded 1 (treeh witnesses) -> Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    G.E.H.D.live_count (G.E.H.slots state) + (2 + 7 * List.length (get_tree_raw witnesses)) <= G.E.H.capacity ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor combination_index] ++ [AI_basic (BI_call 40%N)])
      (fun final => exists next,
        final = (next, store, Build_frame locals G.E.collector_instance, []) /\ scope state next (2 + 7 * List.length (get_tree_raw witnesses)) /\
        Forall (authorized collector) (get_tree_raw witnesses)).
  Proof.
    intros Functions Bytes CombinationLive Collector Witnesses Bounds WitnessBounds Room.
    set (count := List.length (get_tree_raw witnesses)).
    set (initial := descriptor combination_index :: repeat (word 0) 6).
    eapply checked_call with (code := native_authority)
      (parameters := [T_num T_i32]) (results := []) (defaults := repeat (word 0) 6) (address := 40%N)
      (arguments := [descriptor combination_index]) (dummy := []); try reflexivity.
    - rewrite Functions; reflexivity.
    - lazymatch goal with |- checked _ ?post =>
        change (checked (state, store, Build_frame initial G.E.collector_instance,
          to_e_list (modfunc_body native_authority)) post) end.
      rewrite native_authority_body.
      destruct (child_run state store initial combination_index combination 1 collector Functions CombinationLive Collector ltac:(lia) ltac:(lia))
        as [first [collector_index [CollectorRun [CollectorLive [CollectorFresh [Scope1 Obs1]]]]]].
      authority_pure; checked_advance CollectorRun; authority_pure.
      pose proof (proj2 Scope1) as Count1.
      set (locals1 := [descriptor combination_index; word collector_index; word 0; word 0; word 0; word 0; word 0]).
      assert (Combination1 : G.E.H.resolve first combination_index = Some (treeh combination))
        by (apply (proj1 Scope1); exact CombinationLive).
      destruct (child_run first store locals1 combination_index combination 5 (treeh witnesses) Functions Combination1 Witnesses ltac:(lia) ltac:(lia))
        as [second [witnesses_index [WitnessesRun [WitnessesLive [WitnessesFresh [Scope2 Obs2]]]]]].
      authority_pure; checked_advance WitnessesRun; authority_pure.
      pose proof (proj2 Scope2) as Count2.
      set (locals2 := [descriptor combination_index; word collector_index; word witnesses_index; word 0; word 0; word 0; word 0]).
      set (inside := authority_locals combination_index (wasm_nat collector_index) (wasm_nat witnesses_index) count 0
        (word 0) (word 0)).
      authority_pure.
      checked_use (checked_size second store locals2 (wasm_nat witnesses_index) (treeh witnesses) Functions WitnessesLive Bounds).
      intros final [node [Node [Bound Final]]]; inversion Node; subst node; subst final; normalize_checked.
      authority_pure.
      assert (Collector2 : G.E.H.resolve second (wasm_nat collector_index) = Some collector)
        by (apply (proj1 Scope2); exact CollectorLive).
      assert (Scope02 : scope state second 2).
      { replace 2 with (1+1) by lia; eapply scope_trans; eassumption. }
      authority_pure.
      pose proof (Guard.witness_limit_run second store inside count (native_size_bound_word _ Bound)) as LimitRun.
      checked_advance LimitRun.
      destruct (Z.leb (Z.of_nat count) Guard.witness_limit) eqn:Fits.
      2: { checked_advance (native_require second store inside false Functions).
        eapply checked_trap; eapply G.E.collector_trap_suffix with (instructions := [AI_trap]); apply rt_refl. }
      checked_advance (native_require second store inside true Functions).
      assert (Enter : reduce_trans
        (second, store, Build_frame inside G.E.collector_instance,
          [AI_basic (BI_block (BT_valtype None) [BI_loop (BT_valtype None) Guard.authority_loop])])
        (second, store, Build_frame inside G.E.collector_instance,
          G.E.collector_loop_state Guard.authority_loop (to_e_list Guard.authority_loop)))
        by apply G.E.collector_loop_enter.
      unfold G.E.collector_loop_state in Enter; checked_advance Enter.
      pose proof (checked_authority_loop count second store combination_index (wasm_nat collector_index) (wasm_nat witnesses_index)
        collector witnesses count 0 (word 0) (word 0) Functions Bytes Collector2 WitnessesLive eq_refl eq_refl
        (native_size_bound_word _ Bound) WitnessBounds ltac:(lia)) as Loop.
      unfold G.E.collector_loop_state in Loop; checked_use Loop.
      intros final [third [final_witness [final_author [Final [Scope3 Authorized]]]]]; subst final;
        normalize_checked.
        set (final_locals := authority_locals combination_index (wasm_nat collector_index) (wasm_nat witnesses_index) count count
          final_witness final_author).
      assert (Scoped : scope state third (2+7*count)) by (eapply scope_trans; eassumption).
        eapply checked_result; [apply rt_refl|].
        exists []; split; [left; reflexivity|]; split; [reflexivity|].
        exists third; split; [reflexivity|]; split; [exact Scoped|].
        apply Forall_forall; intros witness Member.
        apply In_nth_error in Member as [index Field].
        apply (Authorized index witness); [|exact Field].
        split; [lia|]; unfold count; apply nth_error_Some; rewrite Field; discriminate.
  Qed.
