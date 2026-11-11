Module WasmBinaryPrefixValidation.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Dispatcher := WasmDispatch.
  Import Dispatcher Dispatcher.D Dispatcher.D.V Dispatcher.D.V.V Dispatcher.D.V.V.Support.
  Module Prefixes := WasmPrefixes.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Lemma binary_endpoints_validation state store code lhs rhs root witnesses :
    In code Base.binary_codes -> s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state root = Some (treeh witnesses) -> lengths_bounded 1 (treeh witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 7 <= G.E.H.capacity ->
    checked
      (state, store, Build_frame (Base.initial_locals code lhs rhs root) G.E.collector_instance,
        to_e_list (firstn 106 Base.rule_body))
      (fun final => exists next p a b witness second_witness claim code_value left right,
        final = (next, store, Build_frame (Base.endpoint_locals code lhs rhs root p a b) G.E.collector_instance, []) /\
        get_tree_raw witnesses = [witness; second_witness] /\
        Authentication.Witnesses.authenticated_claim witness claim /\
        get_tree_raw claim = [code_value; left; right] /\ lengths_bounded 2 (treeh claim) /\
        G.E.H.resolve next (wasm_nat p) = Some (treeh claim) /\
        G.E.H.resolve next (wasm_nat a) = Some left /\ G.E.H.resolve next (wasm_nat b) = Some right /\
        scope state next 7 /\
        G.E.H.observations next = G.E.H.observations state + 2).
  Proof.
    intros Code Functions Live Bounds WitnessBounds Bytes Room.
    pose proof (Prefixes.binary_prefix_validation state store code lhs rhs root witnesses
      Code Functions Live Bounds WitnessBounds Bytes ltac:(lia)) as Prefix.
    lazymatch type of Prefix with Prefixes.D.V.V.checked ?initial ?post => change (checked initial post) in Prefix end.
    assert (Shape : to_e_list (firstn 106 Base.rule_body) =
      to_e_list (firstn 48 Base.rule_body) ++ to_e_list (firstn 58 (skipn 48 Base.rule_body))) by reflexivity.
    rewrite Shape.
    lazymatch type of Prefix with checked ?source ?post =>
      eapply checked_context with (initial := source) (context := LH_base [] (to_e_list (firstn 58 (skipn 48 Base.rule_body))))
        (before := post); [exact Prefix|] end.
    intros final [first [p [witness [second_witness [claim [Final [Witnesses [Authenticated [ClaimLive [ClaimFresh [Scope1 Obs1]]]]]]]]]]];
      subst final; normalize_checked.
    set (locals1 := Base.single_locals code lhs rhs root p 0).
    lazymatch goal with |- checked (_, _, _, ?instructions) ?post =>
      change (checked (first, store, Build_frame locals1 G.E.collector_instance, instructions) post) end.
    assert (Length : List.length (get_tree_raw claim) = 3).
    { destruct Authenticated as [bundle [tag [author [_ [_ [_ [_ Length]]]]]]]; exact Length. }
    destruct (Premises.triple_fields _ Length) as [code_value [left [right Fields]]].
    assert (ClaimBound : lengths_bounded 2 (treeh claim)).
    { assert (WitnessBound : lengths_bounded 3 witness).
      { rewrite Witnesses in WitnessBounds; inversion WitnessBounds; assumption. }
      destruct Authenticated as [bundle [tag [author [Witness [Bundle _]]]]]; subst witness.
      eapply lengths_bounded_child; [exact WitnessBound|rewrite Bundle; cbn; auto]. }
    destruct (Constructors.A.exchange_endpoint_executes first store locals1 (wasm_nat p) claim left true
      Functions ClaimLive ltac:(rewrite Fields; reflexivity) ltac:(pose proof (proj2 Scope1); lia))
      as [second [a [LeftRun [LeftLive [LeftFresh [Scope2 Obs2]]]]]].
    unfold Constructors.A.Support.word, Constructors.A.Support.descriptor in LeftRun.
    set (locals2 := Base.scratch_locals code lhs rhs root [Some (wasm_nat p);None;Some (wasm_nat a);None;None;None;None] 0).
    assert (Claim2 : G.E.H.resolve second (wasm_nat p) = Some (treeh claim)) by (apply (proj1 Scope2); exact ClaimLive).
    destruct (Constructors.A.exchange_endpoint_executes second store locals2 (wasm_nat p) claim right false
      Functions Claim2 ltac:(rewrite Fields; reflexivity) ltac:(pose proof (proj2 Scope1); pose proof (proj2 Scope2); lia))
      as [third [b [RightRun [RightLive [RightFresh [Scope3 Obs3]]]]]].
    unfold Constructors.A.Support.word, Constructors.A.Support.descriptor in RightRun.
    assert (Claim3 : G.E.H.resolve third (wasm_nat p) = Some (treeh claim)) by (apply (proj1 Scope3); exact Claim2).
    assert (Left3 : G.E.H.resolve third (wasm_nat a) = Some left) by (apply (proj1 Scope3); exact LeftLive).



    unfold Base.rule_body at 1; rewrite native_exchange_body; cbn [nth skipn firstn].
    let finish := ltac:(
      repeat dispatch_pure;
      checked_advance LeftRun; dispatch_pure;
      dispatch_pure; checked_advance RightRun; dispatch_pure;
      eapply checked_result; [apply rt_refl|];
      exists third, p, a, b, witness, second_witness, claim, code_value, left, right;
      split; [reflexivity|]; split; [exact Witnesses|]; split; [exact Authenticated|]; split; [exact Fields|];
      split; [exact ClaimBound|]; split; [exact Claim3|]; split; [exact Left3|]; split; [exact RightLive|];
      split; [replace 7 with (5+(1+1)) by lia; eapply scope_trans; [exact Scope1|]; eapply scope_trans; eassumption|rewrite Obs3, Obs2, Obs1; reflexivity]) in
    unfold Base.binary_codes in Code; cbn in Code;
    destruct Code as [Code|[Code|[Code|[Code|[Code|Impossible]]]]]; try contradiction; subst code; finish.
  Qed.

  Lemma unary_endpoints_validation state store code lhs rhs root witnesses :
    In code [7;16] -> s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state root = Some (treeh witnesses) -> lengths_bounded 1 (treeh witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 7 <= G.E.H.capacity ->
    checked
      (state, store, Build_frame (Base.initial_locals code lhs rhs root) G.E.collector_instance,
        to_e_list (firstn 106 Base.rule_body))
      (fun final => exists next p a b witness claim code_value left right,
        final = (next, store, Build_frame (Base.endpoint_locals code lhs rhs root p a b) G.E.collector_instance, []) /\
        get_tree_raw witnesses = [witness] /\
        Authentication.Witnesses.authenticated_claim witness claim /\
        get_tree_raw claim = [code_value; left; right] /\ lengths_bounded 2 (treeh claim) /\
        G.E.H.resolve next (wasm_nat p) = Some (treeh claim) /\
        G.E.H.resolve next (wasm_nat a) = Some left /\ G.E.H.resolve next (wasm_nat b) = Some right /\
        scope state next 7 /\
        G.E.H.observations next = G.E.H.observations state + 2).
  Proof.
    intros Code Functions Live Bounds WitnessBounds Bytes Room.
    pose proof (Prefixes.unary_prefix_validation state store code lhs rhs root witnesses
      ltac:(destruct Code as [Code|[Code|Impossible]]; try contradiction; subst code; vm_compute; tauto)
      Functions Live Bounds WitnessBounds Bytes ltac:(lia)) as Prefix.
    lazymatch type of Prefix with Prefixes.D.V.V.checked ?initial ?post => change (checked initial post) in Prefix end.
    assert (Shape : to_e_list (firstn 106 Base.rule_body) =
      to_e_list (firstn 48 Base.rule_body) ++ to_e_list (firstn 58 (skipn 48 Base.rule_body))) by reflexivity.
    rewrite Shape.
    lazymatch type of Prefix with checked ?source ?post =>
      eapply checked_context with (initial := source) (context := LH_base [] (to_e_list (firstn 58 (skipn 48 Base.rule_body))))
        (before := post); [exact Prefix|] end.
    intros final [first [p [witness [claim [Final [Witnesses [Authenticated [ClaimLive [ClaimFresh [Scope1 Obs1]]]]]]]]]];
      subst final; normalize_checked.
    set (locals1 := Base.single_locals code lhs rhs root p 0).
    lazymatch goal with |- checked (_, _, _, ?instructions) ?post =>
      change (checked (first, store, Build_frame locals1 G.E.collector_instance, instructions) post) end.
    assert (Length : List.length (get_tree_raw claim) = 3).
    { destruct Authenticated as [bundle [tag [author [_ [_ [_ [_ Length]]]]]]]; exact Length. }
    destruct (Premises.triple_fields _ Length) as [code_value [left [right Fields]]].
    assert (ClaimBound : lengths_bounded 2 (treeh claim)).
    { assert (WitnessBound : lengths_bounded 3 witness).
      { rewrite Witnesses in WitnessBounds; inversion WitnessBounds; assumption. }
      destruct Authenticated as [bundle [tag [author [Witness [Bundle _]]]]]; subst witness.
      eapply lengths_bounded_child; [exact WitnessBound|rewrite Bundle; cbn; auto]. }
    destruct (Constructors.A.exchange_endpoint_executes first store locals1 (wasm_nat p) claim left true
      Functions ClaimLive ltac:(rewrite Fields; reflexivity) ltac:(pose proof (proj2 Scope1); lia))
      as [second [a [LeftRun [LeftLive [LeftFresh [Scope2 Obs2]]]]]].
    unfold Constructors.A.Support.word, Constructors.A.Support.descriptor in LeftRun.
    set (locals2 := Base.scratch_locals code lhs rhs root [Some (wasm_nat p);None;Some (wasm_nat a);None;None;None;None] 0).
    assert (Claim2 : G.E.H.resolve second (wasm_nat p) = Some (treeh claim)) by (apply (proj1 Scope2); exact ClaimLive).
    destruct (Constructors.A.exchange_endpoint_executes second store locals2 (wasm_nat p) claim right false
      Functions Claim2 ltac:(rewrite Fields; reflexivity) ltac:(pose proof (proj2 Scope1); pose proof (proj2 Scope2); lia))
      as [third [b [RightRun [RightLive [RightFresh [Scope3 Obs3]]]]]].
    unfold Constructors.A.Support.word, Constructors.A.Support.descriptor in RightRun.
    assert (Claim3 : G.E.H.resolve third (wasm_nat p) = Some (treeh claim)) by (apply (proj1 Scope3); exact Claim2).
    assert (Left3 : G.E.H.resolve third (wasm_nat a) = Some left) by (apply (proj1 Scope3); exact LeftLive).



    unfold Base.rule_body at 1; rewrite native_exchange_body; cbn [nth skipn firstn].
    let finish := ltac:(
      repeat dispatch_pure;
      checked_advance LeftRun; dispatch_pure;
      dispatch_pure; checked_advance RightRun; dispatch_pure;
      eapply checked_result; [apply rt_refl|];
      exists third, p, a, b, witness, claim, code_value, left, right;
      split; [reflexivity|]; split; [exact Witnesses|]; split; [exact Authenticated|]; split; [exact Fields|];
      split; [exact ClaimBound|]; split; [exact Claim3|]; split; [exact Left3|]; split; [exact RightLive|];
      split; [replace 7 with (5+(1+1)) by lia; eapply scope_trans; [exact Scope1|]; eapply scope_trans; eassumption|rewrite Obs3, Obs2, Obs1; reflexivity]) in
    cbn in Code;
    destruct Code as [Code|[Code|Impossible]]; try contradiction; subst code; finish.
  Qed.
End WasmBinaryPrefixValidation.
