Module WasmPremise.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Integers := WasmInteger.
  Import Integers Integers.V Integers.V.V Integers.V.V.Support.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Module Witnesses := WasmWitnesses.

  Lemma authenticated_read witness claim code number kind lhs rhs :
    Witnesses.authenticated_claim witness claim ->
    get_tree_raw claim = [G.E.H.H.HBlobObj code; lhs; rhs] ->
    decode_u64 (get_blob_data code) = Some number -> Claims.Code.statement_kind number = Some kind ->
    X.read kind witness = Some (lhs, rhs).
  Proof.
    intros [bundle [tag [author [Witness [Bundle [Tag [Tagged Length]]]]]]] Fields Number Kind; subst witness.
    unfold X.read, X.Wire.decode; cbn [G.E.H.H.HTreeObj].
    rewrite Bundle; cbn [G.E.H.H.HTreeObj]; rewrite Tagged, Tag.
    rewrite X.Wire.API.handle_eqb_refl, Fields.
    cbn [G.E.H.H.HBlobObj].
    unfold X.Wire.H.to_nat.
    change (X.Wire.Code.statement_kind number = Some kind) in Kind.
    rewrite Number; cbn [X.Wire.H.obind X.Wire.H.omap]; rewrite Kind.
    cbn [X.Wire.H.omap].
    destruct (X.API.statement_kind_eq_dec kind kind); [reflexivity|contradiction].
  Qed.

  Lemma triple_fields (values : list handle) : List.length values = 3 ->
    exists first second third, values = [first; second; third].
  Proof.
    destruct values as [|first [|second [|third [|fourth rest]]]]; cbn; intro Length; try discriminate.
    exists first, second, third; reflexivity.
  Qed.

  Lemma checked_premise state store locals claim_index lhs_index rhs_index claim kind lhs rhs :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state claim_index = Some (treeh claim) -> G.E.H.resolve state lhs_index = Some lhs ->
    G.E.H.resolve state rhs_index = Some rhs -> List.length (get_tree_raw claim) = 3 ->
    lengths_bounded 2 (treeh claim) -> (Z.of_nat kind < 2 ^ 64)%Z ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 3 <= G.E.H.capacity ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 kind)); descriptor claim_index;
        descriptor lhs_index; descriptor rhs_index] ++ [AI_basic (BI_call 31%N)])
      (fun final => exists next updated code,
        final = (next, updated, Build_frame locals G.E.collector_instance, []) /\
        get_tree_raw claim = [G.E.H.H.HBlobObj code; lhs; rhs] /\ decode_u64 (get_blob_data code) = Some kind /\
        scope state next 3 /\ G.E.H.observations next = G.E.H.observations state + 4 /\
        s_funcs updated = s_funcs store /\ store_typing updated /\ Resources.memory_capacity updated).
  Proof.
    intros Functions Typed Capacity ClaimLive LhsLive RhsLive Length Bounds KindBound Bytes Room.
    destruct (triple_fields _ Length) as [code_value [left [right Fields]]].
    set (arguments := [VAL_num (VAL_int64 (wasm_nat64 kind)); descriptor claim_index;
      descriptor lhs_index; descriptor rhs_index]).
    set (initial := arguments).
    eapply checked_call with (code := evaluation_premise)
      (parameters := [T_num T_i64; T_num T_i32; T_num T_i32; T_num T_i32])
      (results := []) (defaults := []) (address := 31%N) (arguments := arguments) (dummy := []); try reflexivity.
    - rewrite Functions; reflexivity.
    - lazymatch goal with |- checked _ ?post =>
        change (checked (state, store, Build_frame initial G.E.collector_instance,
          to_e_list (modfunc_body evaluation_premise)) post) end.
      rewrite evaluation_premise_header.
      destruct (child_run state store initial claim_index claim 0 code_value Functions ClaimLive
        ltac:(rewrite Fields; reflexivity) ltac:(lia) ltac:(lia))
        as [first [code_slot [CodeRun [CodeLive [CodeFresh [Scope1 Obs1]]]]]].
      checked_pure; checked_advance CodeRun.
      set (locals1 := arguments).
      assert (CodeBound : lengths_bounded 1 code_value).
      { eapply lengths_bounded_child; [exact Bounds|rewrite Fields; cbn; auto]. }
      checked_use (checked_integer first store locals1 (wasm_nat code_slot) code_value
        Functions Typed Capacity CodeLive CodeBound).
      intros final [updated [code [number [Final [Code [Decoded [SameFunctions [TypedNext CapacityNext]]]]]]]];
        subst final code_value; normalize_checked.
      assert (FunctionsNext : s_funcs updated = s_funcs G.E.allocated_store) by (rewrite SameFunctions; exact Functions).
      checked_pure.
      pose proof (wasm_nat64_equal number kind (decode_u64_bound _ _ Decoded) KindBound) as Compared.
      pose proof (native_relop first updated locals1 (VAL_int64 (wasm_nat64 number))
        (VAL_int64 (wasm_nat64 kind)) T_i64 (Relop_i ROI_eq) eq_refl) as Compare.
      rewrite Compared in Compare; checked_advance Compare.
      destruct (Nat.eqb number kind) eqn:Equal.
      2: { checked_advance (native_require first updated locals1 false FunctionsNext).
        eapply checked_trap; eapply G.E.collector_trap_suffix with (instructions := [AI_trap]); apply rt_refl. }
      apply Nat.eqb_eq in Equal; subst number.
      checked_advance (native_require first updated locals1 true FunctionsNext).
      pose (second := first).
      assert (Scope2 : scope state second 1) by exact Scope1.
      assert (Obs2 : G.E.H.observations second = G.E.H.observations first) by reflexivity.
      assert (Claim2 : G.E.H.resolve second claim_index = Some (treeh claim))
        by (apply (proj1 Scope2); exact ClaimLive).
      destruct (child_run second updated locals1 claim_index claim 1 left FunctionsNext Claim2
        ltac:(rewrite Fields; reflexivity) ltac:(lia) ltac:(destruct Scope2; lia))
        as [third [left_slot [LeftRun [LeftLive [LeftFresh [Scope3 Obs3]]]]]].
      checked_pure; checked_advance LeftRun.
      set (locals2 := arguments).
      assert (Scope03 : scope state third 2).
      { replace 2 with (1+1) by lia; eapply scope_trans; eassumption. }
      assert (Lhs3 : G.E.H.resolve third lhs_index = Some lhs) by (apply (proj1 Scope03); exact LhsLive).
      checked_pure.
      checked_use (checked_same third updated locals2 (wasm_nat left_slot) lhs_index left lhs
        FunctionsNext LeftLive Lhs3 Bytes).
      intros final [Final Left]; subst final left; normalize_checked.
      assert (Scope34 : scope second (compared third) 1) by exact Scope3.
      pose (fourth := compared third).
      assert (Scope4 : scope second fourth 1) by exact Scope3.
      assert (Obs4 : G.E.H.observations fourth = G.E.H.observations third + 2) by reflexivity.
      assert (Scope04 : scope state fourth 2).
      { replace 2 with (1+1) by lia; eapply scope_trans; eassumption. }
      assert (Claim4 : G.E.H.resolve fourth claim_index = Some (treeh claim))
        by (apply (proj1 Scope04); exact ClaimLive).
      destruct (child_run fourth updated locals2 claim_index claim 2 right FunctionsNext Claim4
        ltac:(rewrite Fields; reflexivity) ltac:(lia) ltac:(destruct Scope04; lia))
        as [fifth [right_slot [RightRun [RightLive [RightFresh [Scope5 Obs5]]]]]].
      checked_pure; checked_advance RightRun.
      set (inside := arguments).
      assert (Scope05 : scope state fifth 3).
      { replace 3 with (2+1) by lia; eapply scope_trans; eassumption. }
      assert (Rhs5 : G.E.H.resolve fifth rhs_index = Some rhs) by (apply (proj1 Scope05); exact RhsLive).
      checked_pure.
      checked_use (checked_same fifth updated inside (wasm_nat right_slot) rhs_index right rhs
        FunctionsNext RightLive Rhs5 Bytes).
      intros final [Final Right]; subst final right; normalize_checked.
      assert (Scope56 : scope fourth (compared fifth) 1) by exact Scope5.
      pose (next := compared fifth).
      assert (Obs6 : G.E.H.observations next = G.E.H.observations fifth + 2) by reflexivity.
      eapply checked_result; [apply rt_refl|].
      exists []; split; [left; reflexivity|]; split; [reflexivity|].
      exists next, updated, code; split; [reflexivity|]; split; [exact Fields|]; split; [exact Decoded|]; split.
      + exact Scope05.
      + split.
        * rewrite Obs6; change (G.E.H.observations fifth + 2 = G.E.H.observations state + 4).
          rewrite Obs5, Obs4; change (G.E.H.observations third + 2 + 2 = G.E.H.observations state + 4).
          rewrite Obs3, Obs2, Obs1; lia.
        * auto.
  Qed.
End WasmPremise.
