Module WasmForceValidation.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Dispatcher := WasmDispatch.
  Import Dispatcher Dispatcher.D Dispatcher.D.V Dispatcher.D.V.V Dispatcher.D.V.V.Support.
  Module Prefix := WasmBinaryPrefixValidation.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Lemma force_strict_validation state store locals lhs_index rhs_index witnesses_index lhs rhs witnesses :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.within_capacity state -> G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state witnesses_index = Some (treeh witnesses) -> lengths_bounded 1 (treeh witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 32 <= G.E.H.capacity ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list (Base.rule_arguments 7 lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
      (fun final => exists next updated result claim_kind,
        final = (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [Base.result_word result]) /\
        scope state next 32 /\ s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Claims.Code.statement_kind result = Some claim_kind /\
        X.check X.ForceToEncodeStrict (get_tree_raw witnesses) lhs rhs = Some claim_kind).
  Proof.
    intros Functions Typed Capacity Within LhsLive RhsLive WitnessesLive Bounds WitnessBounds Bytes Room.
    apply checked_rule_call; [exact Functions|].
    set (code := 7).
    assert (Binary : In code [7;16]) by (vm_compute; tauto).
    pose proof (Prefix.unary_endpoints_validation state store code lhs_index rhs_index witnesses_index witnesses
      Binary Functions WitnessesLive Bounds WitnessBounds Bytes ltac:(lia)) as Endpoints.
    lazymatch type of Endpoints with Prefix.Dispatcher.D.V.V.checked ?initial ?post => change (checked initial post) in Endpoints end.
    lazymatch type of Endpoints with checked ?initial ?post =>
      pose proof (checked_capacity initial post Within Endpoints) as SafePrefix end.
    eapply checked_rule_prefix with (cutoff := 106); [exact SafePrefix|].
    intros final [[first [p [a [b [witness [claim [code_value [left [right
      [Final [Witnesses [Authenticated [Fields [ClaimBound [PLive [ALive [BLive [Owned1 Obs1]]]]]]]]]]]]]]]]]] Within1];
      subst final; normalize_checked.
    set (inside := Base.endpoint_locals code lhs_index rhs_index witnesses_index p a b).
    lazymatch goal with |- checked (_, _, _, ?instructions) ?post =>
      change (checked (first, store, Build_frame inside G.E.collector_instance, instructions) post) end.
    assert (Scope1 : scope state first 7) by
        (eapply scope_weaken; [solve [eauto 8 using scope_trans, scope_refl, compared_scope]|lia]).
    pose proof (proj2 Scope1) as Count1.
    pose proof (Constructors.checked_map first store inside (wasm_nat a) X.Strict left Functions ALive ltac:(lia)) as Map.
    lazymatch type of Map with Constructors.V.V.checked ?initial ?post => change (checked initial post) in Map end.
    unfold Constructors.V.V.Support.word, Constructors.V.V.Support.descriptor in Map.
    lazymatch type of Map with checked ?initial ?post => pose proof (checked_capacity initial post Within1 Map) as SafeMap end.
    assert (Lhs1 : G.E.H.resolve first lhs_index = Some lhs) by (apply (proj1 Scope1); exact LhsLive).
    unfold Base.endpoint_locals, Base.scratch_locals, Base.rule_arguments, Base.result_word,
      Base.optional_descriptor, Base.Support.descriptor, Base.Support.word in inside.
    cbn [List.map List.app] in inside.
    unfold Base.rule_body at 1; rewrite native_exchange_body; cbn [nth skipn].
    repeat dispatch_pure; apply branch_label; do 2 dispatch_pure.
    checked_use_under [descriptor lhs_index] SafeMap.
    intros final [[allocated [copy [value [Final [Construct [Present [Fresh [Scope2 Obs2]]]]]]]] Within2];
      subst final; normalize_checked.
    set (slots := [Some (wasm_nat p);None;Some (wasm_nat a);Some (wasm_nat b);None;Some (wasm_nat copy);None]).
    set (following := Base.scratch_locals code lhs_index rhs_index witnesses_index slots 0).
    unfold Base.scratch_locals, Base.rule_arguments, Base.result_word, slots,
      Base.optional_descriptor, Base.Support.descriptor, Base.Support.word in following.
    cbn [List.map List.app] in following.
    assert (Lhs2 : G.E.H.resolve allocated lhs_index = Some lhs) by (apply (proj1 Scope2); exact Lhs1).
    checked_use (checked_same allocated store following lhs_index (wasm_nat copy) lhs value Functions Lhs2 Present Bytes).
    intros final [Final Same]; subst final value; normalize_checked.
    set (second := compared allocated).

    assert (Scope02 : scope state second 8) by
        (eapply scope_weaken; [solve [eauto 8 using scope_trans, scope_refl, compared_scope]|lia]).
    pose proof (proj2 Scope02) as Count2.
    assert (WithinSecond : G.E.H.within_capacity second) by exact Within2.
    assert (Rhs2 : G.E.H.resolve second rhs_index = Some rhs) by (apply (proj1 Scope02); exact RhsLive).
    assert (B2 : G.E.H.resolve second (wasm_nat b) = Some right) by (apply (proj1 Scope2); exact BLive).
    pose proof (Strict.checked_strict_result second store following (wasm_nat b) rhs_index right rhs
      Functions eq_refl eq_refl B2 Rhs2 Bytes ltac:(lia)) as Result.
    lazymatch type of Result with Strict.V.V.checked ?initial ?post => change (checked initial post) in Result end.
    unfold Strict.Force.strict_rhs_code, Strict.V.V.Support.word, Strict.V.V.Support.descriptor in Result.
    lazymatch type of Result with checked ?initial ?post =>
      pose proof (checked_capacity initial post WithinSecond Result) as SafeResult end.
    checked_use SafeResult.
    intros final [[third [datum [Final [Scope3 [Obs3 [Data Rhs]]]]]] Within3]; subst final; normalize_checked.

    assert (Scope03 : scope state third 9) by
        (eapply scope_weaken; [solve [eauto 8 using scope_trans, scope_refl, compared_scope]|lia]).
    pose proof (proj2 Scope03) as Count3.
    assert (P3 : G.E.H.resolve third (wasm_nat p) = Some (treeh claim)) by (apply (proj1 Scope3), (proj1 Scope2); exact PLive).
    assert (A3 : G.E.H.resolve third (wasm_nat a) = Some left) by (apply (proj1 Scope3), (proj1 Scope2); exact ALive).
    assert (B3 : G.E.H.resolve third (wasm_nat b) = Some right) by (apply (proj1 Scope3); exact B2).
    do 3 dispatch_pure.
    pose proof (Premises.checked_premise third store following (wasm_nat p) (wasm_nat a) (wasm_nat b) claim 3 left right
      Functions Typed Capacity P3 A3 B3 ltac:(rewrite Fields; reflexivity) ClaimBound ltac:(vm_compute; congruence) Bytes ltac:(lia)) as Premise.
    lazymatch type of Premise with Premises.Integers.V.V.checked ?initial ?post => change (checked initial post) in Premise end.
    unfold Premises.Integers.V.V.Support.word, Premises.Integers.V.V.Support.descriptor in Premise.
    lazymatch type of Premise with checked ?initial ?post =>
      pose proof (checked_capacity initial post Within3 Premise) as SafePremise end.
    checked_use SafePremise.
    intros final [[fourth [updated [blob [Final [ClaimFields [Decoded [Scope4 [Obs4 [SameFunctions [NextTyped NextCapacity]]]]]]]]]] Within4];
      subst final; normalize_checked.
    pose proof (Premises.authenticated_read witness claim blob 3 Force left right Authenticated ClaimFields Decoded eq_refl) as Read.

    assert (NextFunctions : s_funcs updated = s_funcs G.E.allocated_store) by (rewrite SameFunctions; exact Functions).
    destruct left as [data|thunk|encoded]; cbn [X.construct] in Construct; try discriminate.
    inversion Construct; subst lhs; clear Construct.
    subst right rhs.
    finish_branch.
  Qed.

  Lemma force_shallow_validation state store locals lhs_index rhs_index witnesses_index lhs rhs witnesses :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.within_capacity state -> G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state witnesses_index = Some (treeh witnesses) -> lengths_bounded 1 (treeh witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 32 <= G.E.H.capacity ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list (Base.rule_arguments 16 lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
      (fun final => exists next updated result claim_kind,
        final = (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [Base.result_word result]) /\
        scope state next 32 /\ s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Claims.Code.statement_kind result = Some claim_kind /\
        X.check X.ForceToEncodeShallow (get_tree_raw witnesses) lhs rhs = Some claim_kind).
  Proof.
    intros Functions Typed Capacity Within LhsLive RhsLive WitnessesLive Bounds WitnessBounds Bytes Room.
    apply checked_rule_call; [exact Functions|].
    set (code := 16).
    assert (Binary : In code [7;16]) by (vm_compute; tauto).
    pose proof (Prefix.unary_endpoints_validation state store code lhs_index rhs_index witnesses_index witnesses
      Binary Functions WitnessesLive Bounds WitnessBounds Bytes ltac:(lia)) as Endpoints.
    lazymatch type of Endpoints with Prefix.Dispatcher.D.V.V.checked ?initial ?post => change (checked initial post) in Endpoints end.
    lazymatch type of Endpoints with checked ?initial ?post =>
      pose proof (checked_capacity initial post Within Endpoints) as SafePrefix end.
    eapply checked_rule_prefix with (cutoff := 106); [exact SafePrefix|].
    intros final [[first [p [a [b [witness [claim [code_value [left [right
      [Final [Witnesses [Authenticated [Fields [ClaimBound [PLive [ALive [BLive [Owned1 Obs1]]]]]]]]]]]]]]]]]] Within1];
      subst final; normalize_checked.
    set (inside := Base.endpoint_locals code lhs_index rhs_index witnesses_index p a b).
    lazymatch goal with |- checked (_, _, _, ?instructions) ?post =>
      change (checked (first, store, Build_frame inside G.E.collector_instance, instructions) post) end.
    assert (Scope1 : scope state first 7) by
        (eapply scope_weaken; [solve [eauto 8 using scope_trans, scope_refl, compared_scope]|lia]).
    pose proof (proj2 Scope1) as Count1.
    pose proof (Constructors.checked_map first store inside (wasm_nat a) X.Shallow left Functions ALive ltac:(lia)) as Map.
    lazymatch type of Map with Constructors.V.V.checked ?initial ?post => change (checked initial post) in Map end.
    unfold Constructors.V.V.Support.word, Constructors.V.V.Support.descriptor in Map.
    lazymatch type of Map with checked ?initial ?post => pose proof (checked_capacity initial post Within1 Map) as SafeMap end.
    assert (Lhs1 : G.E.H.resolve first lhs_index = Some lhs) by (apply (proj1 Scope1); exact LhsLive).
    unfold Base.endpoint_locals, Base.scratch_locals, Base.rule_arguments, Base.result_word,
      Base.optional_descriptor, Base.Support.descriptor, Base.Support.word in inside.
    cbn [List.map List.app] in inside.
    unfold Base.rule_body at 1; rewrite native_exchange_body; cbn [nth skipn].
    repeat dispatch_pure; apply branch_label; do 2 dispatch_pure.
    checked_use_under [descriptor lhs_index] SafeMap.
    intros final [[allocated [copy [value [Final [Construct [Present [Fresh [Scope2 Obs2]]]]]]]] Within2];
      subst final; normalize_checked.
    set (slots := [Some (wasm_nat p);None;Some (wasm_nat a);Some (wasm_nat b);None;Some (wasm_nat copy);None]).
    set (following := Base.scratch_locals code lhs_index rhs_index witnesses_index slots 0).
    unfold Base.scratch_locals, Base.rule_arguments, Base.result_word, slots,
      Base.optional_descriptor, Base.Support.descriptor, Base.Support.word in following.
    cbn [List.map List.app] in following.
    assert (Lhs2 : G.E.H.resolve allocated lhs_index = Some lhs) by (apply (proj1 Scope2); exact Lhs1).
    checked_use (checked_same allocated store following lhs_index (wasm_nat copy) lhs value Functions Lhs2 Present Bytes).
    intros final [Final Same]; subst final value; normalize_checked.
    set (second := compared allocated).

    assert (Scope02 : scope state second 8) by
        (eapply scope_weaken; [solve [eauto 8 using scope_trans, scope_refl, compared_scope]|lia]).
    pose proof (proj2 Scope02) as Count2.
    assert (WithinSecond : G.E.H.within_capacity second) by exact Within2.
    assert (Rhs2 : G.E.H.resolve second rhs_index = Some rhs) by (apply (proj1 Scope02); exact RhsLive).
    assert (B2 : G.E.H.resolve second (wasm_nat b) = Some right) by (apply (proj1 Scope2); exact BLive).
    do 2 dispatch_pure.
    pose proof (Constructors.checked_lower second store following (wasm_nat b) right Functions B2 ltac:(lia)) as Lower.
    lazymatch type of Lower with Constructors.V.V.checked ?initial ?post => change (checked initial post) in Lower end.
    unfold Constructors.V.V.Support.word, Constructors.V.V.Support.descriptor in Lower.
    lazymatch type of Lower with checked ?initial ?post =>
      pose proof (checked_capacity initial post WithinSecond Lower) as SafeLower end.
    checked_use_under [descriptor rhs_index] SafeLower.
    intros final [[lowered [lower_slot [lower_data [Final [LowerData [LowerLive [LowerScope LowerObs]]]]]]] WithinLower];
      subst final; normalize_checked.
    rename following into map_locals.
    set (slots_lower := [Some (wasm_nat p);None;Some (wasm_nat a);Some (wasm_nat b);None;Some (wasm_nat copy);Some lower_slot]).
    set (following := Base.scratch_locals code lhs_index rhs_index witnesses_index slots_lower 0).
    unfold Base.scratch_locals, Base.rule_arguments, Base.result_word, slots_lower,
      Base.optional_descriptor, Base.Support.descriptor, Base.Support.word in following.
    cbn [List.map List.app] in following.
    assert (RhsLower : G.E.H.resolve lowered rhs_index = Some rhs) by (apply (proj1 LowerScope); exact Rhs2).
    checked_use (checked_same lowered store following rhs_index lower_slot rhs (X.D.EC.E.EP.E.lower right)
      Functions RhsLower LowerLive Bytes).
    intros final [Final Rhs]; subst final; normalize_checked.
    set (third := compared lowered).
    assert (Scope3 : scope second third 1) by exact LowerScope.

    assert (Within3 : G.E.H.within_capacity third) by exact WithinLower.
    assert (Scope03 : scope state third 9) by
        (eapply scope_weaken; [solve [eauto 8 using scope_trans, scope_refl, compared_scope]|lia]).
    pose proof (proj2 Scope03) as Count3.
    assert (P3 : G.E.H.resolve third (wasm_nat p) = Some (treeh claim)) by (apply (proj1 Scope3), (proj1 Scope2); exact PLive).
    assert (A3 : G.E.H.resolve third (wasm_nat a) = Some left) by (apply (proj1 Scope3), (proj1 Scope2); exact ALive).
    assert (B3 : G.E.H.resolve third (wasm_nat b) = Some right) by (apply (proj1 Scope3); exact B2).
    do 3 dispatch_pure.
    pose proof (Premises.checked_premise third store following (wasm_nat p) (wasm_nat a) (wasm_nat b) claim 3 left right
      Functions Typed Capacity P3 A3 B3 ltac:(rewrite Fields; reflexivity) ClaimBound ltac:(vm_compute; congruence) Bytes ltac:(lia)) as Premise.
    lazymatch type of Premise with Premises.Integers.V.V.checked ?initial ?post => change (checked initial post) in Premise end.
    unfold Premises.Integers.V.V.Support.word, Premises.Integers.V.V.Support.descriptor in Premise.
    lazymatch type of Premise with checked ?initial ?post =>
      pose proof (checked_capacity initial post Within3 Premise) as SafePremise end.
    checked_use SafePremise.
    intros final [[fourth [updated [blob [Final [ClaimFields [Decoded [Scope4 [Obs4 [SameFunctions [NextTyped NextCapacity]]]]]]]]]] Within4];
      subst final; normalize_checked.
    pose proof (Premises.authenticated_read witness claim blob 3 Force left right Authenticated ClaimFields Decoded eq_refl) as Read.

    assert (NextFunctions : s_funcs updated = s_funcs G.E.allocated_store) by (rewrite SameFunctions; exact Functions).
    destruct left as [data|thunk|encoded]; cbn [X.construct] in Construct; try discriminate.
    inversion Construct; subst lhs; clear Construct.
    subst right rhs.
    finish_branch.
  Qed.
End WasmForceValidation.
