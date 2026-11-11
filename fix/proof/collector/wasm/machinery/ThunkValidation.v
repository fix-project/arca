Module WasmThunkValidation.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Dispatcher := WasmDispatch.
  Import Dispatcher Dispatcher.D Dispatcher.D.V Dispatcher.D.V.V Dispatcher.D.V.V.Support.
  Module Prefixes := WasmPrefixes.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Lemma thunk_encode_validation state store locals (strict : bool) lhs_index rhs_index witnesses_index lhs rhs witnesses :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.within_capacity state -> G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state witnesses_index = Some (treeh witnesses) -> lengths_bounded 1 (treeh witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 32 <= G.E.H.capacity ->
    checked
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list (Base.rule_arguments (if strict then 14 else 15) lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
      (fun final => exists next updated result claim_kind,
        final = (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [Base.result_word result]) /\
        scope state next 32 /\ s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Claims.Code.statement_kind result = Some claim_kind /\
        X.check (if strict then X.ThunkStrictEq else X.ThunkShallowEq) (get_tree_raw witnesses) lhs rhs = Some claim_kind).
  Proof.
    intros Functions Typed Capacity Within LhsLive RhsLive WitnessesLive Bounds WitnessBounds Bytes Room.
    apply checked_rule_call; [exact Functions|].
    set (code := (if strict then 14 else 15)).
    set (initial := Base.initial_locals code lhs_index rhs_index witnesses_index).
    assert (Unary : In code Base.unary_codes) by (destruct strict; vm_compute; tauto).
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
    set (constructor := if strict then X.Strict else X.Shallow).
    set (premise_kind := VAL_num (VAL_int64 (wasm_nat64 4))).
    assert (Rhs1 : G.E.H.resolve first rhs_index = Some rhs) by (apply (proj1 Scope1); exact RhsLive).
    pose proof (proj2 Scope1) as Count1.
    pose proof (Constructors.checked_map first store inside rhs_index constructor rhs Functions Rhs1 ltac:(lia)) as Map.
    lazymatch type of Map with Constructors.V.V.checked ?initial ?post => change (checked initial post) in Map end.
    unfold Constructors.V.V.Support.word, Constructors.V.V.Support.descriptor in Map.
    lazymatch type of Map with checked ?initial ?post =>
      pose proof (checked_capacity initial post Within1 Map) as SafeMap end.
    unfold Base.rule_body at 1; rewrite native_exchange_body; cbn [nth skipn].
    let finish := ltac:(
      checked_use_under [premise_kind; word claim_slot; descriptor lhs_index] SafeMap;
      let final := fresh "final" in let allocated := fresh "allocated" in let copy := fresh "copy" in let value := fresh "value" in
      let Final := fresh "Final" in let Construct := fresh "Construct" in let Present := fresh "Present" in let Fresh := fresh "Fresh" in
      let Scope2 := fresh "Scope2" in let Obs2 := fresh "Obs2" in let Within2 := fresh "Within2" in
      intros final [[allocated [copy [value [Final [Construct [Present [Fresh [Scope2 Obs2]]]]]]]] Within2];
      subst final; normalize_checked; unfold premise_kind;
      let slots := fresh "slots" in let following := fresh "following" in
      set (slots := [Some (wasm_nat claim_slot);None;None;None;None;Some (wasm_nat copy);None]);
      set (following := Base.scratch_locals code lhs_index rhs_index witnesses_index slots 0);
      let Claim2 := fresh "Claim2" in let Left2 := fresh "Left2" in
      assert (Claim2 : G.E.H.resolve allocated (wasm_nat claim_slot) = Some (treeh claim)) by (apply (proj1 Scope2); exact ClaimLive);
      assert (Left2 : G.E.H.resolve allocated lhs_index = Some lhs) by (apply (proj1 Scope2), (proj1 Scope1); exact LhsLive);
      let Count2 := fresh "Count2" in pose proof (proj2 Scope2) as Count2;
      let Premise := fresh "Premise" in
      pose proof (Premises.checked_premise allocated store following (wasm_nat claim_slot) lhs_index (wasm_nat copy) claim 4 lhs value
        Functions Typed Capacity Claim2 Left2 Present ClaimLength ClaimBounds ltac:(vm_compute; congruence) Bytes ltac:(lia)) as Premise;
      lazymatch type of Premise with Premises.Integers.V.V.checked ?initial ?post => change (checked initial post) in Premise end;
      unfold Premises.Integers.V.V.Support.word, Premises.Integers.V.V.Support.descriptor in Premise;
      let Safe := fresh "Safe" in
      lazymatch type of Premise with checked ?initial ?post => pose proof (checked_capacity initial post Within2 Premise) as Safe end;
      checked_use Safe;
      let second := fresh "second" in let updated := fresh "updated" in let blob := fresh "blob" in
      let Fields := fresh "Fields" in let Decoded := fresh "Decoded" in let Scoped := fresh "Scoped" in let Obs := fresh "Obs" in
      let SameFunctions := fresh "SameFunctions" in let NextTyped := fresh "NextTyped" in let NextCapacity := fresh "NextCapacity" in
      let Within3 := fresh "Within3" in
      intros final [[second [updated [blob [Final [Fields [Decoded [Scoped [Obs [SameFunctions [NextTyped NextCapacity]]]]]]]]]] Within3];
      subst final; normalize_checked;
      let Read := fresh "Read" in
      pose proof (Premises.authenticated_read witness claim blob 4 Think lhs value Authenticated Fields Decoded eq_refl) as Read;

      let NextFunctions := fresh "NextFunctions" in
      assert (NextFunctions : s_funcs updated = s_funcs G.E.allocated_store) by (rewrite SameFunctions; exact Functions);
      destruct rhs as [data|thunk|encoded]; cbn [constructor X.construct] in Construct; try discriminate;
      inversion Construct; subst value; clear Construct;
      finish_branch) in
    destruct strict; cbn [code] in *; repeat dispatch_pure; apply branch_label; do 3 dispatch_pure; finish.
  Qed.
End WasmThunkValidation.
