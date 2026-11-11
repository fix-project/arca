Module WasmResultValidation.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Dispatcher := WasmDispatch.
  Import Dispatcher Dispatcher.D Dispatcher.D.V Dispatcher.D.V.V Dispatcher.D.V.V.Support.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Lemma claim_bounds witness claim :
    Authentication.Witnesses.authenticated_claim witness claim -> lengths_bounded 3 witness ->
    List.length (get_tree_raw claim) = 3 /\ lengths_bounded 2 (treeh claim).
  Proof.
    intros [bundle [tag [author [Witness [Bundle [Tag [Tagged Length]]]]]]] Bounds; subst witness.
    split; [exact Length|]; eapply lengths_bounded_child; [exact Bounds|rewrite Bundle; cbn; auto].
  Qed.

  Ltac scratch locals :=
    unfold Base.scratch_locals, Base.rule_arguments, Base.result_word,
      Base.optional_descriptor, Base.Support.descriptor, Base.Support.word in locals;
    cbn [List.map List.app] in locals.

  Lemma force_results_validation state store locals lhs_index rhs_index witnesses_index lhs rhs witnesses :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.within_capacity state -> G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state witnesses_index = Some (treeh witnesses) -> lengths_bounded 1 (treeh witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 32 <= G.E.H.capacity ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list (Base.rule_arguments 1 lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
      (fun final => exists next updated result claim_kind,
        final = (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [Base.result_word result]) /\
        scope state next 32 /\ s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Claims.Code.statement_kind result = Some claim_kind /\ X.check X.ForceResultEq (get_tree_raw witnesses) lhs rhs = Some claim_kind).
  Proof.
    intros Functions Typed Capacity Within LhsLive RhsLive WitnessesLive Bounds WitnessBounds Bytes Room.
    apply checked_rule_call; [exact Functions|].
    set (initial := Base.initial_locals 1 lhs_index rhs_index witnesses_index).
    unfold Base.rule_body at 1; rewrite native_exchange_body; cbn [nth].
    do 18 dispatch_pure; apply branch_label; dispatch_pure.
    checked_use (checked_arity state store initial witnesses_index (treeh witnesses) 3 Functions WitnessesLive Bounds
      ltac:(vm_compute; congruence)).
    intros final [Final [actual [Tree Length]]]; inversion Tree; subst actual final; normalize_checked.
    destruct (Premises.triple_fields _ Length) as [w0 [w1 [w2 Witnesses]]].
    assert (Bound0 : lengths_bounded 3 w0) by (rewrite Witnesses in WitnessBounds; inversion WitnessBounds; assumption).
    assert (Bound1 : lengths_bounded 3 w1).
    { rewrite Witnesses in WitnessBounds; inversion WitnessBounds as [|? ? _ Rest]; subst; inversion Rest; assumption. }
    assert (Bound2 : lengths_bounded 3 w2).
    { rewrite Witnesses in WitnessBounds; inversion WitnessBounds as [|? ? _ Rest]; subst;
        inversion Rest as [|? ? _ Last]; subst; inversion Last; assumption. }
    dispatch_pure.
    pose proof (Authentication.checked_witness_claim state store initial witnesses_index witnesses 0 w0 Functions WitnessesLive
      ltac:(rewrite Witnesses; reflexivity) ltac:(lia) (lengths_bounded_weaken 2 w0 Bound0) Bytes ltac:(lia)) as Open0.
    lazymatch type of Open0 with Authentication.V.V.checked ?source ?post => change (checked source post) in Open0 end.
    unfold Authentication.V.V.Support.word, Authentication.V.V.Support.descriptor in Open0.
    lazymatch type of Open0 with checked ?source ?post => pose proof (checked_capacity source post Within Open0) as Safe0 end.
    checked_use Safe0.
    intros final [[first [p [claim0 [Final [Auth0 [PLive [PFresh [Scope1 Obs1]]]]]]]] Within1]; subst final; normalize_checked.
    dispatch_pure.
    set (locals1 := Base.scratch_locals 1 lhs_index rhs_index witnesses_index [Some (wasm_nat p);None;None;None;None;None;None] 0).
    scratch locals1.

    pose proof (proj2 Scope1) as Count1.
    assert (Root1 : G.E.H.resolve first witnesses_index = Some (treeh witnesses)) by (apply (proj1 Scope1); exact WitnessesLive).
    dispatch_pure.
    pose proof (Authentication.checked_witness_claim first store locals1 witnesses_index witnesses 1 w1 Functions Root1
      ltac:(rewrite Witnesses; reflexivity) ltac:(lia) (lengths_bounded_weaken 2 w1 Bound1) Bytes ltac:(lia)) as Open1.
    lazymatch type of Open1 with Authentication.V.V.checked ?source ?post => change (checked source post) in Open1 end.
    unfold Authentication.V.V.Support.word, Authentication.V.V.Support.descriptor in Open1.
    lazymatch type of Open1 with checked ?source ?post => pose proof (checked_capacity source post Within1 Open1) as Safe1 end.
    checked_use Safe1.
    intros final [[second [q [claim1 [Final [Auth1 [QLive [QFresh [Scope2 Obs2]]]]]]]] Within2]; subst final; normalize_checked.
    dispatch_pure.
    set (locals2 := Base.scratch_locals 1 lhs_index rhs_index witnesses_index [Some (wasm_nat p);Some (wasm_nat q);None;None;None;None;None] 0).
    scratch locals2.

    pose proof (proj2 Scope2) as Count2.
    destruct (claim_bounds w0 claim0 Auth0 Bound0) as [Length0 ClaimBound0].
    destruct (claim_bounds w1 claim1 Auth1 Bound1) as [Length1 ClaimBound1].
    destruct (Premises.triple_fields _ Length0) as [c0 [l0 [right0 Fields0]]].
    destruct (Premises.triple_fields _ Length1) as [c1 [l1 [right1 Fields1]]].
    assert (P2 : G.E.H.resolve second (wasm_nat p) = Some (treeh claim0)) by (apply (proj1 Scope2); exact PLive).
    destruct (Constructors.A.exchange_endpoint_executes second store locals2 (wasm_nat p) claim0 right0 false Functions P2
      ltac:(rewrite Fields0; reflexivity) ltac:(lia)) as [third [a [ARun [ALive [AFresh [Scope3 Obs3]]]]]].
    unfold Constructors.A.Support.word, Constructors.A.Support.descriptor in ARun.
    assert (Within3 : G.E.H.within_capacity third) by (eapply Resources.execution_descriptor_capacity; eassumption).
    dispatch_pure; checked_advance ARun; dispatch_pure.
    set (locals3 := Base.scratch_locals 1 lhs_index rhs_index witnesses_index [Some (wasm_nat p);Some (wasm_nat q);Some (wasm_nat a);None;None;None;None] 0).
    scratch locals3.
    assert (Q3 : G.E.H.resolve third (wasm_nat q) = Some (treeh claim1)) by (apply (proj1 Scope3); exact QLive).
    pose proof (proj2 Scope3) as Count3.
    destruct (Constructors.A.exchange_endpoint_executes third store locals3 (wasm_nat q) claim1 right1 false Functions Q3
      ltac:(rewrite Fields1; reflexivity) ltac:(lia)) as [fourth [b [BRun [BLive [BFresh [Scope4 Obs4]]]]]].
    unfold Constructors.A.Support.word, Constructors.A.Support.descriptor in BRun.
    assert (Within4 : G.E.H.within_capacity fourth) by (eapply Resources.execution_descriptor_capacity; eassumption).
    dispatch_pure; checked_advance BRun; dispatch_pure.
    set (locals4 := Base.scratch_locals 1 lhs_index rhs_index witnesses_index [Some (wasm_nat p);Some (wasm_nat q);Some (wasm_nat a);Some (wasm_nat b);None;None;None] 0).
    scratch locals4.


    assert (Scope04 : scope state fourth 12) by
        (eapply scope_weaken; [solve [eauto 8 using scope_trans, scope_refl, compared_scope]|lia]).
    pose proof (proj2 Scope04) as Count4.
    assert (P4 : G.E.H.resolve fourth (wasm_nat p) = Some (treeh claim0)) by (apply (proj1 Scope4), (proj1 Scope3); exact P2).
    assert (A4 : G.E.H.resolve fourth (wasm_nat a) = Some right0) by (apply (proj1 Scope4); exact ALive).
    assert (Left4 : G.E.H.resolve fourth lhs_index = Some lhs) by (apply (proj1 Scope04); exact LhsLive).
    do 3 dispatch_pure.
    pose proof (Premises.checked_premise fourth store locals4 (wasm_nat p) lhs_index (wasm_nat a) claim0 3 lhs right0
      Functions Typed Capacity P4 Left4 A4 Length0 ClaimBound0 ltac:(vm_compute; congruence) Bytes ltac:(lia)) as Premise0.
    lazymatch type of Premise0 with Premises.Integers.V.V.checked ?source ?post => change (checked source post) in Premise0 end.
    unfold Premises.Integers.V.V.Support.word, Premises.Integers.V.V.Support.descriptor in Premise0.
    lazymatch type of Premise0 with checked ?source ?post => pose proof (checked_capacity source post Within4 Premise0) as SafePremise0 end.
    checked_use SafePremise0.
    intros final [[fifth [store1 [blob0 [Final [ClaimFields0 [Decoded0 [Scope5 [Obs5 [Functions1 [Typed1 Capacity1]]]]]]]]]] Within5];
      subst final; normalize_checked.
    pose proof (Premises.authenticated_read w0 claim0 blob0 3 Force lhs right0 Auth0 ClaimFields0 Decoded0 eq_refl) as Read0.

    assert (Scope05 : scope state fifth 15) by
        (eapply scope_weaken; [solve [eauto 8 using scope_trans, scope_refl, compared_scope]|lia]).
    pose proof (proj2 Scope05) as Count5.
    assert (FunctionsStore1 : s_funcs store1 = s_funcs G.E.allocated_store) by (rewrite Functions1; exact Functions).
    assert (Q5 : G.E.H.resolve fifth (wasm_nat q) = Some (treeh claim1)) by (apply (proj1 Scope5), (proj1 Scope4); exact Q3).
    assert (B5 : G.E.H.resolve fifth (wasm_nat b) = Some right1) by (apply (proj1 Scope5); exact BLive).
    assert (Right5 : G.E.H.resolve fifth rhs_index = Some rhs) by (apply (proj1 Scope05); exact RhsLive).
    do 3 dispatch_pure.
    pose proof (Premises.checked_premise fifth store1 locals4 (wasm_nat q) rhs_index (wasm_nat b) claim1 3 rhs right1
      FunctionsStore1 Typed1 Capacity1 Q5 Right5 B5 Length1 ClaimBound1 ltac:(vm_compute; congruence) Bytes ltac:(lia)) as Premise1.
    lazymatch type of Premise1 with Premises.Integers.V.V.checked ?source ?post => change (checked source post) in Premise1 end.
    unfold Premises.Integers.V.V.Support.word, Premises.Integers.V.V.Support.descriptor in Premise1.
    lazymatch type of Premise1 with checked ?source ?post => pose proof (checked_capacity source post Within5 Premise1) as SafePremise1 end.
    checked_use SafePremise1.
    intros final [[sixth [store2 [blob1 [Final [ClaimFields1 [Decoded1 [Scope6 [Obs6 [Functions2 [Typed2 Capacity2]]]]]]]]]] Within6];
      subst final; normalize_checked.
    pose proof (Premises.authenticated_read w1 claim1 blob1 3 Force rhs right1 Auth1 ClaimFields1 Decoded1 eq_refl) as Read1.

    assert (Scope06 : scope state sixth 18) by
        (eapply scope_weaken; [solve [eauto 8 using scope_trans, scope_refl, compared_scope]|lia]).
    pose proof (proj2 Scope06) as Count6.
    assert (FunctionsStore2 : s_funcs store2 = s_funcs G.E.allocated_store) by (rewrite Functions2; exact FunctionsStore1).
    assert (Root6 : G.E.H.resolve sixth witnesses_index = Some (treeh witnesses)) by (apply (proj1 Scope06); exact WitnessesLive).
    dispatch_pure.
    pose proof (Authentication.checked_witness_claim sixth store2 locals4 witnesses_index witnesses 2 w2 FunctionsStore2 Root6
      ltac:(rewrite Witnesses; reflexivity) ltac:(lia) (lengths_bounded_weaken 2 w2 Bound2) Bytes ltac:(lia)) as Open2.
    lazymatch type of Open2 with Authentication.V.V.checked ?source ?post => change (checked source post) in Open2 end.
    unfold Authentication.V.V.Support.word, Authentication.V.V.Support.descriptor in Open2.
    lazymatch type of Open2 with checked ?source ?post => pose proof (checked_capacity source post Within6 Open2) as Safe2 end.
    checked_use_under [VAL_num (VAL_int64 (wasm_nat64 0))] Safe2.
    intros final [[seventh [t [claim2 [Final [Auth2 [TLive [TFresh [Scope7 Obs7]]]]]]]] Within7]; subst final; normalize_checked.
    set (locals7 := Base.scratch_locals 1 lhs_index rhs_index witnesses_index [Some (wasm_nat p);Some (wasm_nat q);Some (wasm_nat a);Some (wasm_nat b);Some (wasm_nat t);None;None] 0).
    scratch locals7.

    assert (Scope07 : scope state seventh 23) by
        (eapply scope_weaken; [solve [eauto 8 using scope_trans, scope_refl, compared_scope]|lia]).
    pose proof (proj2 Scope07) as Count7.
    assert (A7 : G.E.H.resolve seventh (wasm_nat a) = Some right0) by (apply (proj1 Scope7), (proj1 Scope6), (proj1 Scope5); exact A4).
    assert (B7 : G.E.H.resolve seventh (wasm_nat b) = Some right1) by (apply (proj1 Scope7), (proj1 Scope6); exact B5).
    destruct (claim_bounds w2 claim2 Auth2 Bound2) as [Length2 ClaimBound2].
    do 2 dispatch_pure.
    pose proof (Premises.checked_premise seventh store2 locals7 (wasm_nat t) (wasm_nat a) (wasm_nat b) claim2 0 right0 right1
      FunctionsStore2 Typed2 Capacity2 TLive A7 B7 Length2 ClaimBound2 ltac:(vm_compute; congruence) Bytes ltac:(lia)) as Premise2.
    lazymatch type of Premise2 with Premises.Integers.V.V.checked ?source ?post => change (checked source post) in Premise2 end.
    unfold Premises.Integers.V.V.Support.word, Premises.Integers.V.V.Support.descriptor in Premise2.
    lazymatch type of Premise2 with checked ?source ?post => pose proof (checked_capacity source post Within7 Premise2) as SafePremise2 end.
    checked_use SafePremise2.
    intros final [[eighth [updated [blob2 [Final [ClaimFields2 [Decoded2 [Scope8 [Obs8 [Functions8 [Typed8 Capacity8]]]]]]]]]] Within8];
      subst final; normalize_checked.
    pose proof (Premises.authenticated_read w2 claim2 blob2 0 Equivalent right0 right1 Auth2 ClaimFields2 Decoded2 eq_refl) as Read2.

    assert (FunctionsNext : s_funcs updated = s_funcs G.E.allocated_store) by (rewrite Functions8; exact FunctionsStore2).
    finish_branch.
  Qed.
End WasmResultValidation.
