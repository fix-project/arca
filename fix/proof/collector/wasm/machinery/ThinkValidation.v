Module WasmThinkValidation.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Dispatcher := WasmDispatch.
  Import Dispatcher Dispatcher.D Dispatcher.D.V Dispatcher.D.V.V Dispatcher.D.V.V.Support.
  Module Prefix := WasmBinaryPrefixValidation.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Lemma think_map_validation state store locals (application : bool) lhs_index rhs_index witnesses_index lhs rhs witnesses :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.within_capacity state -> G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state witnesses_index = Some (treeh witnesses) -> lengths_bounded 1 (treeh witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 32 <= G.E.H.capacity ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list (Base.rule_arguments ((if application then 2 else 25)) lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
      (fun final => exists next updated result claim_kind,
        final = (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [Base.result_word result]) /\
        scope state next 32 /\ s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Claims.Code.statement_kind result = Some claim_kind /\
        X.check ((if application then X.ThinkApplication else X.ThinkSelection)) (get_tree_raw witnesses) lhs rhs = Some claim_kind).
  Proof.
    intros Functions Typed Capacity Within LhsLive RhsLive WitnessesLive Bounds WitnessBounds Bytes Room.
    apply checked_rule_call; [exact Functions|].
    set (code := (if application then 2 else 25)).
    assert (Binary : In code Base.binary_codes) by (destruct application; vm_compute; tauto).
    pose proof (Prefix.binary_endpoints_validation state store code lhs_index rhs_index witnesses_index witnesses
      Binary Functions WitnessesLive Bounds WitnessBounds Bytes ltac:(lia)) as Endpoints.
    lazymatch type of Endpoints with Prefix.Dispatcher.D.V.V.checked ?initial ?post => change (checked initial post) in Endpoints end.
    lazymatch type of Endpoints with checked ?initial ?post =>
      pose proof (checked_capacity initial post Within Endpoints) as SafePrefix end.
    eapply checked_rule_prefix with (cutoff := 106); [exact SafePrefix|].
    intros final [[first [p [a [b [witness [witness2 [claim [code_value [left [right
      [Final [Witnesses [Authenticated [Fields [ClaimBound [PLive [ALive [BLive [Owned1 Obs1]]]]]]]]]]]]]]]]]]] Within1];
      subst final; normalize_checked.
    set (inside := Base.endpoint_locals code lhs_index rhs_index witnesses_index p a b).
    lazymatch goal with |- checked (_, _, _, ?instructions) ?post =>
      change (checked (first, store, Build_frame inside G.E.collector_instance, instructions) post) end.
    assert (Scope1 : scope state first 7) by
        (eapply scope_weaken; [solve [eauto 8 using scope_trans, scope_refl, compared_scope]|lia]).
    pose proof (proj2 Scope1) as Count1.
    set (constructor := if application then X.Application else X.Selection).
    pose (first_code := 1).
    pose (first_kind := Eval).
    pose (first_left := left).
    pose (first_right := right).
    pose (second_code := if application then 2 else 6).
    pose (second_kind := if application then Apply else Select).
    pose (pivot_slot := wasm_nat b).
    pose (last_slot := rhs_index).
    pose (pivot := right).
    pose (last := rhs).
    assert (Kind1 : Claims.Code.statement_kind first_code = Some first_kind) by reflexivity.
    assert (CodeBound1 : (Z.of_nat first_code < 2 ^ 64)%Z) by (vm_compute; congruence).
    assert (Kind2 : Claims.Code.statement_kind second_code = Some second_kind) by (destruct application; reflexivity).
    assert (CodeBound2 : (Z.of_nat second_code < 2 ^ 64)%Z) by (destruct application; cbn; lia).
    assert (ClaimLength : List.length (get_tree_raw claim) = 3) by (rewrite Fields; reflexivity).
    assert (WitnessBound2 : lengths_bounded 3 witness2).
    { rewrite Witnesses in WitnessBounds; inversion WitnessBounds as [|? ? _ Rest]; subst; inversion Rest; assumption. }
    pose proof (Constructors.checked_map first store inside (wasm_nat a) constructor left Functions ALive ltac:(lia)) as Map.
    lazymatch type of Map with Constructors.V.V.checked ?initial ?post => change (checked initial post) in Map end.
    unfold Constructors.V.V.Support.word, Constructors.V.V.Support.descriptor in Map.
    lazymatch type of Map with checked ?initial ?post => pose proof (checked_capacity initial post Within1 Map) as SafeMap end.
    assert (Lhs1 : G.E.H.resolve first lhs_index = Some lhs) by (apply (proj1 Scope1); exact LhsLive).
    unfold Base.endpoint_locals, Base.scratch_locals, Base.rule_arguments, Base.result_word,
      Base.optional_descriptor, Base.Support.descriptor, Base.Support.word in inside.
    cbn [List.map List.app] in inside.
    unfold Base.rule_body at 1; rewrite native_exchange_body; cbn [nth skipn].
    let finish := ltac:(
      checked_use_under [descriptor lhs_index] SafeMap;
      let final := fresh "final" in let allocated := fresh "allocated" in let mapped_slot := fresh "mapped_slot" in let value := fresh "value" in
      let Final := fresh "Final" in let Construct := fresh "Construct" in let MapLive := fresh "MapLive" in let MapFresh := fresh "MapFresh" in
      let MapScope := fresh "MapScope" in let MapObs := fresh "MapObs" in let WithinMap := fresh "WithinMap" in
      intros final [[allocated [mapped_slot [value [Final [Construct [MapLive [MapFresh [MapScope MapObs]]]]]]]] WithinMap];
      subst final; normalize_checked;
      let slots_map := fresh "slots_map" in let map_locals := fresh "map_locals" in
      set (slots_map := [Some (wasm_nat p);None;Some (wasm_nat a);Some (wasm_nat b);None;Some (wasm_nat mapped_slot);None]);
      set (map_locals := Base.scratch_locals code lhs_index rhs_index witnesses_index slots_map 0);
      unfold Base.scratch_locals, Base.rule_arguments, Base.result_word, slots_map,
        Base.optional_descriptor, Base.Support.descriptor, Base.Support.word in map_locals;
      cbn [List.map List.app] in map_locals;
      let LhsMap := fresh "LhsMap" in
      assert (LhsMap : G.E.H.resolve allocated lhs_index = Some lhs) by (apply (proj1 MapScope); exact Lhs1);
      checked_use (checked_same allocated store map_locals lhs_index (wasm_nat mapped_slot) lhs value Functions LhsMap MapLive Bytes);
      let Same := fresh "Same" in intros final [Final Same]; subst final value; normalize_checked;
      let world := fresh "world" in set (world := compared allocated);
      let WithinWorld := fresh "WithinWorld" in assert (WithinWorld : G.E.H.within_capacity world) by exact WithinMap;
      let ScopeMap := fresh "ScopeMap" in assert (ScopeMap : scope state world 8) by
        (eapply scope_weaken; [solve [eauto 8 using scope_trans, scope_refl, compared_scope]|lia]);
      let CountMap := fresh "CountMap" in pose proof (proj2 ScopeMap) as CountMap;
      let PWorld := fresh "PWorld" in let AWorld := fresh "AWorld" in let BWorld := fresh "BWorld" in
      assert (PWorld : G.E.H.resolve world (wasm_nat p) = Some (treeh claim)) by (apply (proj1 MapScope); exact PLive);
      assert (AWorld : G.E.H.resolve world (wasm_nat a) = Some left) by (apply (proj1 MapScope); exact ALive);
      assert (BWorld : G.E.H.resolve world (wasm_nat b) = Some right) by (apply (proj1 MapScope); exact BLive);
      let Premise1 := fresh "Premise1" in
      pose proof (Premises.checked_premise world store map_locals (wasm_nat p) (wasm_nat a) (wasm_nat b) claim 1 left right
        Functions Typed Capacity PWorld AWorld BWorld ClaimLength ClaimBound CodeBound1 Bytes ltac:(lia)) as Premise1;
      lazymatch type of Premise1 with Premises.Integers.V.V.checked ?initial ?post => change (checked initial post) in Premise1 end;
      unfold Premises.Integers.V.V.Support.word, Premises.Integers.V.V.Support.descriptor in Premise1;
      let Safe1 := fresh "Safe1" in
      lazymatch type of Premise1 with checked ?initial ?post => pose proof (checked_capacity initial post WithinWorld Premise1) as Safe1 end;
      do 3 dispatch_pure;
      checked_use Safe1;
      let final := fresh "final" in let second := fresh "second" in let intermediate := fresh "intermediate" in let blob1 := fresh "blob1" in
      let Final := fresh "Final" in let Fields1 := fresh "Fields1" in let Decoded1 := fresh "Decoded1" in let Scoped2 := fresh "Scoped2" in
      let Obs2 := fresh "Obs2" in let SameFunctions2 := fresh "SameFunctions2" in let Typed2 := fresh "Typed2" in let Capacity2 := fresh "Capacity2" in
      let Within2 := fresh "Within2" in
      intros final [[second [intermediate [blob1 [Final [Fields1 [Decoded1 [Scoped2 [Obs2 [SameFunctions2 [Typed2 Capacity2]]]]]]]]]] Within2];
      subst final; normalize_checked;
      let Read1 := fresh "Read1" in
      pose proof (Premises.authenticated_read witness claim blob1 first_code first_kind first_left first_right Authenticated Fields1 Decoded1 Kind1) as Read1;
      let Scope02 := fresh "Scope02" in assert (Scope02 : scope state second 11) by
        (eapply scope_weaken; [solve [eauto 8 using scope_trans, scope_refl, compared_scope]|lia]);
      let Count2 := fresh "Count2" in pose proof (proj2 Scope02) as Count2;
      let Functions2 := fresh "Functions2" in
      assert (Functions2 : s_funcs intermediate = s_funcs G.E.allocated_store) by (rewrite SameFunctions2; exact Functions);
      let Root2 := fresh "Root2" in
      assert (Root2 : G.E.H.resolve second witnesses_index = Some (treeh witnesses)) by (apply (proj1 Scope02); exact WitnessesLive);
      dispatch_pure;
      let Reopen := fresh "Reopen" in
      pose proof (Authentication.checked_witness_claim second intermediate map_locals witnesses_index witnesses 1 witness2 Functions2 Root2
        ltac:(rewrite Witnesses; reflexivity) ltac:(lia) (lengths_bounded_weaken 2 witness2 WitnessBound2) Bytes ltac:(lia)) as Reopen;
      lazymatch type of Reopen with Authentication.V.V.checked ?initial ?post => change (checked initial post) in Reopen end;
      unfold Authentication.V.V.Support.word, Authentication.V.V.Support.descriptor in Reopen;
      let SafeReopen := fresh "SafeReopen" in
      lazymatch type of Reopen with checked ?initial ?post => pose proof (checked_capacity initial post Within2 Reopen) as SafeReopen end;
      checked_use_under [VAL_num (VAL_int64 (wasm_nat64 second_code))] SafeReopen;
      let third := fresh "third" in let q := fresh "q" in let claim2 := fresh "claim2" in let Auth2 := fresh "Auth2" in
      let QLive := fresh "QLive" in let QFresh := fresh "QFresh" in let Scope3 := fresh "Scope3" in let Obs3 := fresh "Obs3" in let Within3 := fresh "Within3" in
      intros final [[third [q [claim2 [Final [Auth2 [QLive [QFresh [Scope3 Obs3]]]]]]]] Within3];
      subst final; normalize_checked;
      dispatch_pure;
      let slots := fresh "slots" in let following := fresh "following" in
      set (slots := [Some (wasm_nat p);Some (wasm_nat q);Some (wasm_nat a);Some (wasm_nat b);None;Some (wasm_nat mapped_slot);None]);
      set (following := Base.scratch_locals code lhs_index rhs_index witnesses_index slots 0);
      unfold Base.scratch_locals, Base.rule_arguments, Base.result_word, slots,
        Base.optional_descriptor, Base.Support.descriptor, Base.Support.word in following;
      cbn [List.map List.app] in following;
      let Scope03 := fresh "Scope03" in assert (Scope03 : scope state third 16) by
        (eapply scope_weaken; [solve [eauto 8 using scope_trans, scope_refl, compared_scope]|lia]);
      let Count3 := fresh "Count3" in pose proof (proj2 Scope03) as Count3;
      let PivotLive := fresh "PivotLive" in let LastLive := fresh "LastLive" in
      assert (PivotLive : G.E.H.resolve third pivot_slot = Some pivot) by (apply (proj1 Scope3), (proj1 Scoped2); assumption);
      assert (LastLive : G.E.H.resolve third last_slot = Some last) by (apply (proj1 Scope03); assumption);
      let ClaimLength2 := fresh "ClaimLength2" in
      assert (ClaimLength2 : List.length (get_tree_raw claim2) = 3) by
        (destruct Auth2 as [bundle [tag [author [_ [_ [_ [_ Length]]]]]]]; exact Length);
      let ClaimBound2 := fresh "ClaimBound2" in
      assert (ClaimBound2 : lengths_bounded 2 (treeh claim2)) by
        (destruct Auth2 as [bundle [tag [author [Witness [Bundle _]]]]]; subst witness2;
          eapply lengths_bounded_child; [exact WitnessBound2|rewrite Bundle; cbn; auto]);
      do 2 dispatch_pure;
      let Premise2 := fresh "Premise2" in
      pose proof (Premises.checked_premise third intermediate following (wasm_nat q) pivot_slot last_slot claim2 second_code pivot last
        Functions2 Typed2 Capacity2 QLive PivotLive LastLive ClaimLength2 ClaimBound2 CodeBound2 Bytes ltac:(lia)) as Premise2;
      lazymatch type of Premise2 with Premises.Integers.V.V.checked ?initial ?post => change (checked initial post) in Premise2 end;
      unfold Premises.Integers.V.V.Support.word, Premises.Integers.V.V.Support.descriptor in Premise2;
      let Safe2 := fresh "Safe2" in
      lazymatch type of Premise2 with checked ?initial ?post => pose proof (checked_capacity initial post Within3 Premise2) as Safe2 end;
      checked_use Safe2;
      let fourth := fresh "fourth" in let updated := fresh "updated" in let blob2 := fresh "blob2" in
      let Fields2 := fresh "Fields2" in let Decoded2 := fresh "Decoded2" in let Scope4 := fresh "Scope4" in let Obs4 := fresh "Obs4" in
      let SameFunctions4 := fresh "SameFunctions4" in let Typed4 := fresh "Typed4" in let Capacity4 := fresh "Capacity4" in let Within4 := fresh "Within4" in
      intros final [[fourth [updated [blob2 [Final [Fields2 [Decoded2 [Scope4 [Obs4 [SameFunctions4 [Typed4 Capacity4]]]]]]]]]] Within4];
      subst final; normalize_checked;
      let Read2 := fresh "Read2" in
      pose proof (Premises.authenticated_read witness2 claim2 blob2 second_code second_kind pivot last Auth2 Fields2 Decoded2 Kind2) as Read2;
      let Functions4 := fresh "Functions4" in
      assert (Functions4 : s_funcs updated = s_funcs G.E.allocated_store) by (rewrite SameFunctions4; exact Functions2);
      unfold first_kind, first_left, first_right in Read1; unfold second_kind, pivot, last in Read2;
      cbn in Read1, Read2;
      destruct left as [[[blob|node]|reference]|thunk|encoded]; unfold constructor in Construct; cbn [X.construct] in Construct; try discriminate;
      inversion Construct; subst lhs; clear Construct;
      finish_branch) in
    destruct application; cbn [code] in *; repeat dispatch_pure; apply branch_label; do 2 dispatch_pure; finish.
  Qed.
End WasmThinkValidation.
