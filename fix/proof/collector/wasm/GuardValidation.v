Module WasmGuardValidation.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module V := WasmValidation.
  Import V V.V V.V.Support.
  Module Rules := WasmRules.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.

  Lemma checked_type_guard state store locals index input expected :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some input -> expected <= 3 ->
    checked
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [descriptor index] ++ to_e_list
          [BI_call 0%N; BI_const_num (VAL_int32 (wasm_nat expected));
           BI_relop T_i32 (Relop_i ROI_eq); BI_call 22%N])
      (fun final => final = (state, store, Build_frame locals G.E.collector_instance, []) /\
        G.E.H.H.value_type input = expected).
  Proof.
    intros Functions Live Expected.
    assert (TypeBound : (Z.of_nat (G.E.H.H.value_type input) < 2 ^ 32)%Z).
    { destruct input as [[[]|reference]|thunk|encoded]; cbn [G.E.H.H.value_type]; lia. }
    pose proof (wasm_nat_equal (G.E.H.H.value_type input) expected TypeBound ltac:(cbn; lia)) as Compared.
    change (app_relop (Relop_i ROI_eq)
      (VAL_int32 (wasm_nat (G.E.H.H.value_type input))) (VAL_int32 (wasm_nat expected)) =
      Nat.eqb (G.E.H.H.value_type input) expected) in Compared.
    assert (Run : reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [descriptor index] ++ to_e_list
          [BI_call 0%N; BI_const_num (VAL_int32 (wasm_nat expected));
           BI_relop T_i32 (Relop_i ROI_eq); BI_call 22%N])
      (state, store, Build_frame locals G.E.collector_instance,
        if Nat.eqb (G.E.H.H.value_type input) expected then [] else [AI_trap])).
    { execute (native_value_import state store state locals 0%N shell_get_type [descriptor index]
        [word (G.E.H.H.value_type input)] Functions eq_refl eq_refl eq_refl ltac:(discriminate)
        ltac:(unfold G.E.H.host_values, descriptor; rewrite Live; reflexivity)).
      pose proof (native_relop state store locals (VAL_int32 (wasm_nat (G.E.H.H.value_type input)))
        (VAL_int32 (wasm_nat expected)) T_i32 (Relop_i ROI_eq) eq_refl) as CompareRun.
      rewrite Compared in CompareRun; execute CompareRun.
      exact (native_require state store locals (Nat.eqb (G.E.H.H.value_type input) expected) Functions). }
    destruct (Nat.eqb (G.E.H.H.value_type input) expected) eqn:Same.
    - eapply checked_result; [exact Run|]; split; [reflexivity|apply Nat.eqb_eq; exact Same].
    - eapply checked_trap; exact Run.
  Qed.

  Lemma checked_leaf_guard state store locals index input :
    s_funcs store = s_funcs G.E.allocated_store -> lookup_N locals 1%N = Some (descriptor index) ->
    G.E.H.resolve state index = Some input ->
    checked
      (state, store, Build_frame locals G.E.collector_instance,
        to_e_list [BI_local_get 1%N; BI_call 0%N; BI_const_num (VAL_int32 (wasm_nat 3));
          BI_relop T_i32 (Relop_i ROI_ne); BI_call 22%N;
          BI_local_get 1%N; BI_call 0%N; BI_testop T_i32 TO_eqz;
          BI_if (BT_valtype None)
            [BI_local_get 1%N; BI_call 1%N; BI_testop T_i32 TO_eqz; BI_call 22%N] []])
      (fun final => final = (state, store, Build_frame locals G.E.collector_instance, []) /\
        X.reflexive_eval input = true).
  Proof.
    intros Functions Local Live; destruct (X.reflexive_eval input) eqn:Leaf.
    - eapply checked_result; [eapply Rules.native_leaf_guard; eassumption|]; auto.
    - destruct input as [[[blob|node]|reference]|thunk|encoded]; cbn [X.reflexive_eval] in Leaf; try discriminate.
      + eapply checked_trap.
        execute (native_type_query state store locals 1%N index (G.E.H.H.HTreeObj node) Functions Local Live).
        do 2 native_pure.
        execute (native_type_query state store locals 1%N index (G.E.H.H.HTreeObj node) Functions Local Live).
        native_pure; apply G.E.collector_if_void with (condition := true) (accepted := false).
        execute (native_data_type_query state store locals 1%N index (Object (TreeObj node)) Functions Local Live).
        do 2 native_pure; apply rt_refl.
      + eapply checked_trap.
        execute (native_type_query state store locals 1%N index (Encode encoded) Functions Local Live).
        do 2 native_pure; eapply G.E.collector_trap_suffix with (instructions := [AI_trap]); apply rt_refl.
  Qed.
End WasmGuardValidation.
