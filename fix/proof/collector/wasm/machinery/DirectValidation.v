Module WasmDirectValidation.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Dispatcher := WasmDispatch.
  Import Dispatcher Dispatcher.D Dispatcher.D.V Dispatcher.D.V.V Dispatcher.D.V.V.Support.
  Module Prefixes := WasmPrefixes.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Definition direct_exchange kind := match kind with
    | Base.Symmetry => X.Sym | Base.EvalResult => X.EvalResultEq
    | Base.ThinkData => X.ThinkToForce | Base.ThunkStep => X.ThunkEq end.

  Lemma direct_validation state store locals kind lhs_index rhs_index witnesses_index lhs rhs witnesses :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.within_capacity state -> G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state witnesses_index = Some (treeh witnesses) -> lengths_bounded 1 (treeh witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 32 <= G.E.H.capacity ->
    checked
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list (Base.rule_arguments (Base.direct_code kind) lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
      (fun final => exists next updated result claim_kind,
        final = (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [Base.result_word result]) /\
        scope state next 32 /\ s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Claims.Code.statement_kind result = Some claim_kind /\
        X.check (direct_exchange kind) (get_tree_raw witnesses) lhs rhs = Some claim_kind).
  Proof.
    intros Functions Typed Capacity Within LhsLive RhsLive WitnessesLive Bounds WitnessBounds Bytes Room.
    apply checked_rule_call; [exact Functions|].
    set (code := Base.direct_code kind).
    set (initial := Base.initial_locals code lhs_index rhs_index witnesses_index).
    assert (Unary : In code Base.unary_codes) by (destruct kind; vm_compute; tauto).
    pose proof (Prefixes.unary_prefix_validation state store code lhs_index rhs_index witnesses_index witnesses
      Unary Functions WitnessesLive Bounds WitnessBounds Bytes ltac:(lia)) as Prefix.
    lazymatch type of Prefix with Prefixes.D.V.V.checked ?initial ?post => change (checked initial post) in Prefix end.
    lazymatch type of Prefix with checked ?initial ?post =>
      pose proof (checked_capacity initial post Within Prefix) as SafePrefix end.
    eapply checked_rule_prefix with (cutoff := 48); [exact SafePrefix|].
    intros final [[first [claim_slot [witness [claim [Final [Witnesses [Authenticated [ClaimLive [ClaimFresh [Scope1 Obs1]]]]]]]]]] Within1];
      subst final; normalize_checked.
    set (inside := Base.single_locals code lhs_index rhs_index witnesses_index claim_slot 0).
    lazymatch goal with |- checked (_, _, _, ?instructions) ?post =>
      change (checked (first, store, Build_frame inside G.E.collector_instance, instructions) post) end.

    assert (ClaimLength : List.length (get_tree_raw claim) = 3).
    { destruct Authenticated as [bundle [tag [author [_ [_ [_ [_ Length]]]]]]]; exact Length. }
    assert (ClaimBounds : lengths_bounded 2 (treeh claim)).
    { assert (WitnessBound : lengths_bounded 3 witness).
      { rewrite Witnesses in WitnessBounds; inversion WitnessBounds; assumption. }
      destruct Authenticated as [bundle [tag [author [Witness [Bundle _]]]]]; subst witness.
      eapply lengths_bounded_child; [exact WitnessBound|rewrite Bundle; cbn; auto]. }
    set (left_index := match kind with Base.Symmetry => rhs_index | _ => lhs_index end).
    set (right_index := match kind with Base.Symmetry => lhs_index | _ => rhs_index end).
    set (expected_left := Base.direct_left kind lhs rhs).
    set (expected_right := Base.direct_right kind lhs rhs).
    set (premise_code := Base.direct_claim kind).
    set (premise_kind := match kind with Base.Symmetry => Equivalent | Base.EvalResult => Eval | _ => Think end).
    assert (Left1 : G.E.H.resolve first left_index = Some expected_left)
      by (destruct kind; apply (proj1 Scope1); assumption).
    assert (Right1 : G.E.H.resolve first right_index = Some expected_right)
      by (destruct kind; apply (proj1 Scope1); assumption).
    assert (PremiseKind : Claims.Code.statement_kind premise_code = Some premise_kind) by (destruct kind; reflexivity).
    assert (PremiseBound : (Z.of_nat premise_code < 2 ^ 64)%Z) by (destruct kind; cbn; lia).
    pose proof (proj2 Scope1) as Count1.
    unfold Base.rule_body at 1; rewrite native_exchange_body; cbn [nth skipn].
    let finish := ltac:(
      do 3 dispatch_pure;
      let Premise := fresh "Premise" in
      pose proof (Premises.checked_premise first store inside (wasm_nat claim_slot) left_index right_index
        claim premise_code expected_left expected_right Functions Typed Capacity ClaimLive Left1 Right1 ClaimLength ClaimBounds
        PremiseBound Bytes ltac:(lia)) as Premise;
      lazymatch type of Premise with Premises.Integers.V.V.checked ?initial ?post => change (checked initial post) in Premise end;
      unfold Premises.Integers.V.V.Support.word, Premises.Integers.V.V.Support.descriptor in Premise;
      let Safe := fresh "Safe" in
      lazymatch type of Premise with checked ?initial ?post => pose proof (checked_capacity initial post Within1 Premise) as Safe end;
      checked_use Safe;
      let final := fresh "final" in let second := fresh "second" in let updated := fresh "updated" in let blob := fresh "blob" in
      let Final := fresh "Final" in let Fields := fresh "Fields" in let Decoded := fresh "Decoded" in
      let Scoped := fresh "Scoped" in let Obs := fresh "Obs" in let SameFunctions := fresh "SameFunctions" in
      let NextTyped := fresh "NextTyped" in let NextCapacity := fresh "NextCapacity" in let Within2 := fresh "Within2" in
      intros final [[second [updated [blob [Final [Fields [Decoded [Scoped [Obs [SameFunctions [NextTyped NextCapacity]]]]]]]]]] Within2];
      subst final; normalize_checked;
      let Read := fresh "Read" in
      pose proof (Premises.authenticated_read witness claim blob premise_code premise_kind expected_left expected_right
        Authenticated Fields Decoded PremiseKind) as Read;
      let NextFunctions := fresh "NextFunctions" in
      assert (NextFunctions : s_funcs updated = s_funcs G.E.allocated_store) by (rewrite SameFunctions; exact Functions);
      unfold premise_kind, expected_left, expected_right in Read;
      cbn [direct_exchange Base.direct_code Base.direct_claim Base.direct_left Base.direct_right] in Read |- *;
      finish_branch) in
    destruct kind; cbn [code Base.direct_code] in *; repeat dispatch_pure; apply branch_label;
    [finish|finish|
      dispatch_pure;
      assert (Rhs1 : G.E.H.resolve first rhs_index = Some rhs) by (apply (proj1 Scope1); exact RhsLive);
      checked_use (checked_data first store inside rhs_index rhs Functions Rhs1);
      intros final [Final [datum Rhs]]; subst final rhs; normalize_checked; finish|
      dispatch_pure;
      assert (Rhs1 : G.E.H.resolve first rhs_index = Some rhs) by (apply (proj1 Scope1); exact RhsLive);
      pose proof (Guards.checked_type_guard first store inside rhs_index rhs 2 Functions Rhs1 ltac:(lia)) as Guard;
      lazymatch type of Guard with Guards.V.V.checked ?initial ?post => change (checked initial post) in Guard end;
      unfold Guards.V.V.Support.word, Guards.V.V.Support.descriptor in Guard;
      checked_use Guard;
      intros final [Final Classification]; subst final; normalize_checked;
      destruct rhs as [[[blob|node]|reference]|thunk|encoded]; cbn [G.E.H.H.value_type] in Classification; try discriminate;
      finish].
  Qed.
End WasmDirectValidation.
