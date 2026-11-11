Module WasmTreeValidation.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module A := WasmAuthentication.
  Import A A.V A.V.V A.V.V.Support.
  Module Premises := WasmPremise.
  Module Trees := WasmTreeLoop.
  Module Resources := WasmResources.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.
  Local Notation loop_locals := Trees.loop_locals.
  Local Notation native_tree_loop := Trees.native_tree_loop.

  Ltac tree_pure := cbn [Trees.loop_locals]; unfold Trees.Support.word, Trees.Support.descriptor; checked_pure.

  Lemma checked_tree_iteration state store lhs_index rhs_index witnesses_index lhs rhs witnesses kind claim_kind count index
      witness left right :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state lhs_index = Some (treeh lhs) -> G.E.H.resolve state rhs_index = Some (treeh rhs) ->
    G.E.H.resolve state witnesses_index = Some (treeh witnesses) ->
    index < count -> (Z.of_nat count < 2 ^ 32)%Z -> Claims.Code.statement_kind kind = Some claim_kind ->
    nth_error (get_tree_raw witnesses) index = Some witness ->
    nth_error (get_tree_raw lhs) index = Some left -> nth_error (get_tree_raw rhs) index = Some right ->
    lengths_bounded 3 witness ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 10 <= G.E.H.capacity ->
    checked
      (state, store, Build_frame (loop_locals kind lhs_index rhs_index witnesses_index count index
       ) G.E.collector_instance, to_e_list native_tree_loop)
      (fun final => exists next updated,
        final = (next, updated, Build_frame (loop_locals kind lhs_index rhs_index witnesses_index count (Datatypes.S index)
         ) G.E.collector_instance, [AI_basic (BI_br 0%N)]) /\
        scope state next 10 /\ X.read claim_kind witness = Some (left, right) /\
        s_funcs updated = s_funcs store /\ store_typing updated /\ Resources.memory_capacity updated).
  Proof.
    intros Functions Typed Capacity LhsLive RhsLive WitnessesLive Index Count Kind Field Left Right Bounds Bytes Room.
    set (initial := loop_locals kind lhs_index rhs_index witnesses_index count index).
    assert (IndexBound : (Z.of_nat index < 2 ^ 32)%Z) by lia.
    rewrite Trees.native_tree_loop_body.
    do 2 tree_pure.
    pose proof (counter_equal_run state store initial index count IndexBound Count) as Counter.
    assert (Different : Nat.eqb index count = false) by (apply Nat.eqb_neq; lia).
    rewrite Different in Counter; checked_advance Counter.
    checked_advance (branch_if_run state store (Build_frame initial G.E.collector_instance) 1%N false).
    do 3 tree_pure.
    checked_use_under [VAL_num (VAL_int64 (wasm_nat64 kind))] (checked_witness_claim state store initial witnesses_index witnesses index witness
      Functions WitnessesLive Field IndexBound (lengths_bounded_weaken 2 witness Bounds) Bytes ltac:(lia)).
    intros final [first [claim_slot [claim [Final [Authenticated [ClaimLive [ClaimFresh [Scope1 Obs1]]]]]]]];
      subst final; normalize_checked.
    pose proof (proj2 Scope1) as Count1.
    set (locals1 := loop_locals kind lhs_index rhs_index witnesses_index count index).
    assert (Lhs1 : G.E.H.resolve first lhs_index = Some (treeh lhs)) by (apply (proj1 Scope1); exact LhsLive).
    destruct (child_run first store locals1 lhs_index lhs index left Functions Lhs1 Left IndexBound ltac:(lia))
      as [second [left_slot [LeftRun [LeftLive [LeftFresh [Scope2 Obs2]]]]]].
    pose proof (proj2 Scope2) as Count2.
    do 2 tree_pure; checked_advance_under [VAL_num (VAL_int64 (wasm_nat64 kind)); word claim_slot] LeftRun.
    set (locals2 := loop_locals kind lhs_index rhs_index witnesses_index count index).
    assert (Scope02 : scope state second 6).
    { replace 6 with (5+1) by lia; eapply scope_trans; eassumption. }
    assert (Rhs2 : G.E.H.resolve second rhs_index = Some (treeh rhs)) by (apply (proj1 Scope02); exact RhsLive).
    destruct (child_run second store locals2 rhs_index rhs index right Functions Rhs2 Right IndexBound ltac:(lia))
      as [third [right_slot [RightRun [RightLive [RightFresh [Scope3 Obs3]]]]]].
    pose proof (proj2 Scope3) as Count3.
    do 2 tree_pure; checked_advance_under [VAL_num (VAL_int64 (wasm_nat64 kind)); word claim_slot; word left_slot] RightRun.
    set (inside := loop_locals kind lhs_index rhs_index witnesses_index count index).
    assert (Scope03 : scope state third 7).
    { replace 7 with (6+1) by lia; eapply scope_trans; eassumption. }
    assert (Claim3 : G.E.H.resolve third (wasm_nat claim_slot) = Some (treeh claim))
      by (apply (proj1 Scope3), (proj1 Scope2); exact ClaimLive).
    assert (Left3 : G.E.H.resolve third (wasm_nat left_slot) = Some left) by (apply (proj1 Scope3); exact LeftLive).
    assert (ClaimLength : List.length (get_tree_raw claim) = 3).
    { destruct Authenticated as [bundle [tag [author [_ [_ [_ [_ Length]]]]]]]; exact Length. }
    assert (ClaimBound : lengths_bounded 2 (treeh claim)).
    { destruct Authenticated as [bundle [tag [author [Witness [Bundle _]]]]]; subst witness.
      eapply lengths_bounded_child; [exact Bounds|rewrite Bundle; cbn; auto]. }
    assert (KindBound : (Z.of_nat kind < 2 ^ 64)%Z).
    { pose proof (Claims.Code.statement_kind_roundtrip _ _ Kind) as Code.
      pose proof (Claims.Code.statement_code_bound _ _ Code); cbn; lia. }
    pose proof (Premises.checked_premise third store inside (wasm_nat claim_slot) (wasm_nat left_slot)
      (wasm_nat right_slot) claim kind left right Functions Typed Capacity Claim3 Left3 RightLive
      ClaimLength ClaimBound KindBound Bytes ltac:(lia)) as Premise.
    lazymatch type of Premise with Premises.Integers.V.V.checked ?initial ?post =>
      change (checked initial post) in Premise end.
    unfold Premises.Integers.V.V.Support.word, Premises.Integers.V.V.Support.descriptor in Premise.
    checked_use Premise.
    intros final [fourth [updated [code [Final [Claim [Decoded [Scope4 [Obs4 [Functions4 [Typed4 Capacity4]]]]]]]]]];
      subst final; normalize_checked.
    assert (Scoped : scope state fourth 10).
    { replace 10 with (7+3) by lia; eapply scope_trans; eassumption. }
    tree_pure; checked_advance (counter_increment_run fourth updated inside index IndexBound).
    tree_pure.
    eapply checked_result; [apply rt_refl|].
    exists fourth, updated; split; [reflexivity|]; split; [exact Scoped|]; split.
    - eapply Premises.authenticated_read; eassumption.
    - auto.
  Qed.

  Lemma checked_tree_loop remaining state store lhs_index rhs_index witnesses_index lhs rhs witnesses kind claim_kind count index
      :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state lhs_index = Some (treeh lhs) -> G.E.H.resolve state rhs_index = Some (treeh rhs) ->
    G.E.H.resolve state witnesses_index = Some (treeh witnesses) ->
    index + remaining = count -> count = List.length (get_tree_raw lhs) ->
    count = List.length (get_tree_raw rhs) -> count = List.length (get_tree_raw witnesses) ->
    (Z.of_nat count < 2 ^ 32)%Z -> Claims.Code.statement_kind kind = Some claim_kind ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 10 * remaining <= G.E.H.capacity ->
    checked
      (state, store, Build_frame (loop_locals kind lhs_index rhs_index witnesses_index count index
       ) G.E.collector_instance,
        G.E.collector_loop_state native_tree_loop (to_e_list native_tree_loop))
      (fun final => exists next updated,
        final = (next, updated, Build_frame (loop_locals kind lhs_index rhs_index witnesses_index count count) G.E.collector_instance, []) /\ scope state next (10 * remaining) /\
        s_funcs updated = s_funcs store /\ store_typing updated /\ Resources.memory_capacity updated /\
        forall offset witness left right, index <= offset < count ->
          nth_error (get_tree_raw witnesses) offset = Some witness ->
          nth_error (get_tree_raw lhs) offset = Some left -> nth_error (get_tree_raw rhs) offset = Some right ->
          X.read claim_kind witness = Some (left, right)).
  Proof.
    revert state store index.
    induction remaining as [|remaining IH]; intros state store index
      Functions Typed Capacity LhsLive RhsLive WitnessesLive Remaining LeftCount RightCount WitnessCount Count Kind Bounds Bytes Room.
    - assert (Equal : index = count) by lia; subst index.
      set (inside := loop_locals kind lhs_index rhs_index witnesses_index count count).
      eapply checked_result.
      + eapply G.E.collector_loop_exit with (accepted := true) (suffix := to_e_list (skipn 4 native_tree_loop)).
        rewrite Trees.native_tree_loop_body.
        do 2 (cbn [Trees.loop_locals]; unfold Trees.Support.word, Trees.Support.descriptor; native_pure).
        pose proof (counter_equal_run state store inside count count Count Count) as Counter.
        rewrite Nat.eqb_refl in Counter; execute Counter.
        execute (branch_if_run state store (Build_frame inside G.E.collector_instance) 1%N true); apply rt_refl.
      + exists state, store; split; [reflexivity|];
          split; [apply scope_refl|]; split; [reflexivity|]; split; [exact Typed|]; split; [exact Capacity|]; intros; lia.
    - assert (Index : index < count) by lia.
      destruct (nth_error (get_tree_raw witnesses) index) as [witness|] eqn:Field;
        [|apply nth_error_None in Field; lia].
      destruct (nth_error (get_tree_raw lhs) index) as [left|] eqn:Left;
        [|apply nth_error_None in Left; lia].
      destruct (nth_error (get_tree_raw rhs) index) as [right|] eqn:Right;
        [|apply nth_error_None in Right; lia].
      assert (WitnessBound : lengths_bounded 3 witness).
      { rewrite Forall_forall in Bounds; apply Bounds; eapply nth_error_In; exact Field. }
      destruct (checked_tree_iteration state store lhs_index rhs_index witnesses_index lhs rhs witnesses kind claim_kind count index
        witness left right Functions Typed Capacity LhsLive RhsLive WitnessesLive
        Index Count Kind Field Left Right WitnessBound Bytes ltac:(lia)) as [final [Step [Trap|Success]]].
      + destruct final as [[[next updated] following] instructions]; subst instructions.
        eapply checked_trap; eapply G.E.collector_loop_exit with (accepted := false) (suffix := []); exact Step.
      + destruct Success as [first [updated [Final [Scoped [Read [Functions1 [Typed1 Capacity1]]]]]]];
          subst final.
        assert (Lhs1 : G.E.H.resolve first lhs_index = Some (treeh lhs)) by (apply (proj1 Scoped); exact LhsLive).
        assert (Rhs1 : G.E.H.resolve first rhs_index = Some (treeh rhs)) by (apply (proj1 Scoped); exact RhsLive).
        assert (Witnesses1 : G.E.H.resolve first witnesses_index = Some (treeh witnesses)) by (apply (proj1 Scoped); exact WitnessesLive).
        assert (NextFunctions : s_funcs updated = s_funcs G.E.allocated_store) by (rewrite Functions1; exact Functions).
        eapply checked_progress; [apply G.E.collector_loop_continue; exact Step|].
        eapply checked_weaken.
        * exact (IH first updated (Datatypes.S index)
            NextFunctions Typed1 Capacity1 Lhs1 Rhs1 Witnesses1 ltac:(lia) LeftCount RightCount WitnessCount Count Kind
            Bounds Bytes ltac:(destruct Scoped; lia)).
        * intros final [next [last_store [Final [Scoped1 [Functions2 [Typed2 [Capacity2 Rest]]]]]]].
          exists next, last_store; split; [exact Final|]; split.
          -- replace (10 * Datatypes.S remaining) with (10+10*remaining) by lia; eapply scope_trans; eassumption.
          -- split; [rewrite Functions2; exact Functions1|]; split; [exact Typed2|]; split; [exact Capacity2|].
             intros offset value a b Range Entry Lhs Rhs.
             destruct (Nat.eq_dec offset index) as [Equal|Different].
             ++ subst offset; rewrite Field in Entry; rewrite Left in Lhs; rewrite Right in Rhs;
                  inversion Entry; inversion Lhs; inversion Rhs; subst; exact Read.
             ++ apply (Rest offset value a b); try assumption; lia.
  Qed.

  Lemma children_ready kind left right witnesses :
    List.length right = List.length left -> List.length witnesses = List.length left ->
    (forall offset witness a b, offset < List.length left -> nth_error witnesses offset = Some witness ->
      nth_error left offset = Some a -> nth_error right offset = Some b -> X.read kind witness = Some (a, b)) ->
    X.children kind left right witnesses = true.
  Proof.
    revert right witnesses; induction left as [|a left IH]; intros [|b right] [|witness witnesses] RightCount WitnessCount Ready;
      cbn [List.length] in *; try discriminate; [reflexivity|].
    cbn [X.children]; rewrite (Ready 0 witness a b ltac:(lia) eq_refl eq_refl eq_refl).
    rewrite !X.API.handle_eqb_refl; cbn.
    apply IH; [lia|lia|].
    intros offset value x y Bound Entry Left Right.
    exact (Ready (Datatypes.S offset) value x y ltac:(cbn; lia) Entry Left Right).
  Qed.

  Lemma checked_tree_rule state store locals kind claim_kind lhs_index rhs_index witnesses_index lhs rhs witnesses :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state witnesses_index = Some (treeh witnesses) ->
    lengths_bounded 1 lhs -> lengths_bounded 1 rhs -> lengths_bounded 1 (treeh witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) -> Claims.Code.statement_kind kind = Some claim_kind ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 10 * List.length (get_tree_raw witnesses) <= G.E.H.capacity ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 kind)); descriptor lhs_index; descriptor rhs_index; descriptor witnesses_index] ++
        [AI_basic (BI_call 37%N)])
      (fun final => exists next updated,
        final = (next, updated, Build_frame locals G.E.collector_instance, []) /\ scope state next (10 * List.length (get_tree_raw witnesses)) /\
        s_funcs updated = s_funcs store /\ store_typing updated /\ Resources.memory_capacity updated /\
        X.tree claim_kind (get_tree_raw witnesses) lhs rhs = Some claim_kind).
  Proof.
    intros Functions Typed Capacity LhsLive RhsLive WitnessesLive LeftBound RightBound WitnessBound Bounds Kind Bytes Room.
    set (arguments := [VAL_num (VAL_int64 (wasm_nat64 kind)); descriptor lhs_index; descriptor rhs_index; descriptor witnesses_index]).
    set (initial := arguments ++ repeat (word 0) 2).
    eapply checked_call with (code := native_tree_rule)
      (parameters := [T_num T_i64; T_num T_i32; T_num T_i32; T_num T_i32]) (results := [])
      (defaults := repeat (word 0) 2) (address := 37%N) (arguments := arguments) (dummy := []); try reflexivity.
    - rewrite Functions; reflexivity.
    - lazymatch goal with |- checked _ ?post =>
        change (checked (state, store, Build_frame initial G.E.collector_instance,
          to_e_list (modfunc_body native_tree_rule)) post) end.
      rewrite native_tree_rule_body.
      tree_pure.
      checked_use (checked_size state store initial lhs_index lhs Functions LhsLive LeftBound).
      intros final [left [Left [Bound Final]]]; subst lhs final; normalize_checked.
      set (count := List.length (get_tree_raw left)).
      set (inside := loop_locals kind lhs_index rhs_index witnesses_index count 0 ).
      tree_pure; do 2 tree_pure.
      checked_use (checked_arity state store inside rhs_index rhs count Functions RhsLive RightBound
        (native_size_bound_word _ Bound)).
      intros final [Final [right [Right RightCount]]]; subst rhs final; normalize_checked.
      do 2 tree_pure.
      checked_use (checked_arity state store inside witnesses_index (treeh witnesses) count Functions WitnessesLive WitnessBound
        (native_size_bound_word _ Bound)).
      intros final [Final [actual [Witness WitnessCount]]]; inversion Witness; subst actual; subst final; normalize_checked.
      tree_pure.
      checked_advance (tag_run state store inside lhs_index (treeh left) (tree_tag left) Functions LhsLive eq_refl).
      tree_pure.
      checked_advance_under [VAL_num (VAL_int32 (wasm_bool (tree_tag left)))]
        (tag_run state store inside rhs_index (treeh right) (tree_tag right) Functions RhsLive eq_refl).
      pose proof (native_relop state store inside (VAL_int32 (wasm_bool (tree_tag left)))
        (VAL_int32 (wasm_bool (tree_tag right))) T_i32 (Relop_i ROI_eq) eq_refl) as Compared.
      assert (TagCompare : app_relop (Relop_i ROI_eq) (VAL_int32 (wasm_bool (tree_tag left)))
        (VAL_int32 (wasm_bool (tree_tag right))) = Bool.eqb (tree_tag left) (tree_tag right))
        by (destruct (tree_tag left), (tree_tag right); reflexivity).
      rewrite TagCompare in Compared; checked_advance Compared.
      destruct (Bool.eqb (tree_tag left) (tree_tag right)) eqn:Markers.
      2: { checked_advance (native_require state store inside false Functions).
        eapply checked_trap; eapply G.E.collector_trap_suffix with (instructions := [AI_trap]); apply rt_refl. }
      checked_advance (native_require state store inside true Functions).
      eapply checked_progress; [apply G.E.collector_loop_enter|].
      eapply checked_weaken.
      + exact (checked_tree_loop count state store lhs_index rhs_index witnesses_index left right witnesses kind claim_kind count 0
           Functions Typed Capacity LhsLive RhsLive WitnessesLive eq_refl eq_refl
          ltac:(symmetry; exact RightCount) ltac:(symmetry; exact WitnessCount)
          (native_size_bound_word _ Bound) Kind Bounds Bytes ltac:(unfold count; rewrite WitnessCount in Room; exact Room)).
      + intros final [next [updated [Final [Scoped [NextFunctions [NextTyped [NextCapacity Reads]]]]]]];
          subst final.
        exists []; split; [left; reflexivity|]; split; [reflexivity|].
        exists next, updated; split; [reflexivity|]; split; [rewrite WitnessCount; exact Scoped|]; split; [exact NextFunctions|];
          split; [exact NextTyped|]; split; [exact NextCapacity|].
        cbn [X.tree G.E.H.H.HTreeObj]; rewrite Markers; cbn.
        assert (Children : X.children claim_kind (get_tree_raw left) (get_tree_raw right) (get_tree_raw witnesses) = true).
        { apply children_ready; [exact RightCount|exact WitnessCount|].
          intros offset witness a b Offset Entry Lhs Rhs; apply (Reads offset witness a b); try assumption; unfold count; lia. }
        rewrite Children; reflexivity.
  Qed.
End WasmTreeValidation.
