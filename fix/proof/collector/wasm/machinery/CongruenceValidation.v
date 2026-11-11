Module WasmCongruenceValidation.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Dispatcher := WasmDispatch.
  Import Dispatcher Dispatcher.D Dispatcher.D.V Dispatcher.D.V.V Dispatcher.D.V.V.Support.
  Module Prefixes := WasmPrefixes.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Definition congruence_exchange kind := match kind with
    | X.Application => X.EqApplication | X.Selection => X.EqSelection
    | X.Identification => X.EqIdentification | X.Strict => X.EqEncodeStrict | X.Shallow => X.EqEncodeShallow
    | X.TreeReference => X.TreeRefEq | X.BlobReference => X.BlobRefEq end.

  Lemma congruence_validation state store locals kind lhs_index rhs_index witnesses_index lhs rhs witnesses :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.within_capacity state -> G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state witnesses_index = Some (treeh witnesses) -> lengths_bounded 1 (treeh witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 32 <= G.E.H.capacity ->
    checked
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list (Base.rule_arguments (Base.mapped_rule kind) lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
      (fun final => exists next updated result claim_kind,
        final = (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [Base.result_word result]) /\
        scope state next 32 /\ s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Claims.Code.statement_kind result = Some claim_kind /\
        X.check (congruence_exchange kind) (get_tree_raw witnesses) lhs rhs = Some claim_kind).
  Proof.
    intros Functions Typed Capacity Within LhsLive RhsLive WitnessesLive Bounds WitnessBounds Bytes Room.
    apply checked_rule_call; [exact Functions|].
    set (code := Base.mapped_rule kind).
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
    destruct (Premises.triple_fields _ ClaimLength) as [code_value [a [b Fields]]].
    assert (Lhs1 : G.E.H.resolve first lhs_index = Some lhs) by (apply (proj1 Scope1); exact LhsLive).
    assert (Rhs1 : G.E.H.resolve first rhs_index = Some rhs) by (apply (proj1 Scope1); exact RhsLive).
    pose proof (proj2 Scope1) as Count1.
    pose proof (Mapped.checked_mapped first store inside kind lhs_index rhs_index (wasm_nat claim_slot)
      lhs rhs claim code_value a b Functions Typed Capacity Lhs1 Rhs1 ClaimLive Fields ClaimBounds Bytes ltac:(lia)) as Mapped.
    lazymatch type of Mapped with Mapped.Constructors.V.V.checked ?initial ?post => change (checked initial post) in Mapped end.
    unfold Mapped.Constructors.V.V.Support.word, Mapped.Constructors.V.V.Support.descriptor in Mapped.
    lazymatch type of Mapped with checked ?initial ?post =>
      pose proof (checked_capacity initial post Within1 Mapped) as SafeMapped end.
    unfold Base.rule_body at 1; rewrite native_exchange_body; cbn [nth skipn].
    let finish := ltac:(
      checked_use SafeMapped;
      let final := fresh "final" in let second := fresh "second" in let updated := fresh "updated" in let blob := fresh "blob" in
      let Final := fresh "Final" in let ClaimFields := fresh "ClaimFields" in let Decoded := fresh "Decoded" in
      let ConstructA := fresh "ConstructA" in let ConstructB := fresh "ConstructB" in let Scoped := fresh "Scoped" in
      let SameFunctions := fresh "SameFunctions" in let NextTyped := fresh "NextTyped" in let NextCapacity := fresh "NextCapacity" in
      let Within2 := fresh "Within2" in
      intros final [[second [updated [blob [Final [ClaimFields [Decoded [ConstructA [ConstructB [Scoped [SameFunctions [NextTyped NextCapacity]]]]]]]]]]] Within2];
      subst final; normalize_checked;
      let Read := fresh "Read" in
      pose proof (Premises.authenticated_read witness claim blob 0 Equivalent a b Authenticated ClaimFields Decoded eq_refl) as Read;
      let NextFunctions := fresh "NextFunctions" in
      assert (NextFunctions : s_funcs updated = s_funcs G.E.allocated_store) by (rewrite SameFunctions; exact Functions);
      cbn [congruence_exchange] in *;
      finish_branch) in
    destruct kind; cbn [code Base.mapped_rule] in *; repeat dispatch_pure; apply branch_label;
      do 3 dispatch_pure; finish.
  Qed.
End WasmCongruenceValidation.
