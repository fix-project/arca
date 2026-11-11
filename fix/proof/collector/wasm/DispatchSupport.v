Module WasmDispatchSupport.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module V := WasmValidation.
  Import V V.V V.V.Support.
  Module Base := WasmRules.
  Module Resources := WasmResources.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.

  Lemma checked_capacity initial post :
    G.E.configuration_capacity initial -> checked initial post ->
    checked initial (fun final => post final /\ G.E.configuration_capacity final).
  Proof.
    intros Within [final [Run [Trap|Post]]]; exists final; split; [exact Run|left; exact Trap|exact Run|].
    right; split; [exact Post|].
    exact (G.E.collector_runs_capacity combination true true handle_bytes initial final Run Within).
  Qed.

  Lemma checked_rule_prefix cutoff state store frame middle post :
    checked (state, store, frame, to_e_list (firstn cutoff Base.rule_body)) middle ->
    (forall final, middle final ->
      checked (in_context 0 (LH_base [] (to_e_list (skipn cutoff Base.rule_body))) final) post) ->
    checked (state, store, frame, to_e_list Base.rule_body) post.
  Proof.
    intros Prefix Continue.
    assert (Shape : in_context 0 (LH_base [] (to_e_list (skipn cutoff Base.rule_body)))
        (state, store, frame, to_e_list (firstn cutoff Base.rule_body)) =
      (state, store, frame, to_e_list Base.rule_body)).
    { cbn [in_context lfill v_to_e_list seq.map seq.cat].
      rewrite <- to_e_list_cat, firstn_skipn; reflexivity. }
    rewrite <- Shape; eapply checked_context; eassumption.
  Qed.

  Definition branch_validated before (code : nat) (lhs_index rhs_index witnesses_index : Wasm_int.Int32.int) rule witnesses lhs rhs
      (_ : nat) budget final :=
    exists next updated following result kind depth (context : lholed depth),
      final = (next, updated, Build_frame following G.E.collector_instance,
        lfill context (v_to_e_list [Base.result_word result] ++ [AI_basic BI_return])) /\
      scope before next budget /\ G.E.H.within_capacity next /\
      s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
      Claims.Code.statement_kind result = Some kind /\ X.check rule witnesses lhs rhs = Some kind.

  Lemma branch_label before code lhs_index rhs_index witnesses_index rule witnesses lhs rhs offset budget
      state store frame body suffix :
    checked (state, store, frame, body)
      (branch_validated before code lhs_index rhs_index witnesses_index rule witnesses lhs rhs (Datatypes.S offset) budget) ->
    checked (state, store, frame, AI_label 0 [] body :: suffix)
      (branch_validated before code lhs_index rhs_index witnesses_index rule witnesses lhs rhs offset budget).
  Proof.
    intro Body.
    assert (Shape : in_context 1 (LH_rec [] 0 [] (LH_base [] []) suffix) (state, store, frame, body) =
      (state, store, frame, AI_label 0 [] body :: suffix)).
    { cbn [in_context lfill]; rewrite !seq.cat0s, !seq.cats0; reflexivity. }
    rewrite <- Shape; eapply checked_context; [exact Body|].
    intros final [next [updated [following [result [kind [depth [context [Final Post]]]]]]]]; subst final.
    eapply checked_result; [apply rt_refl|].
    exists next, updated, following, result, kind, (Datatypes.S depth), (LH_rec [] 0 [] context suffix).
    split; [|exact Post].
    cbn [in_context lfill List.app seq.cat v_to_e_list seq.map]; rewrite !seq.cats0; reflexivity.
  Qed.

  Lemma checked_rule_call state store locals code lhs_index rhs_index witnesses_index rule witnesses lhs rhs budget :
    s_funcs store = s_funcs G.E.allocated_store ->
    checked
      (state, store, Build_frame (Base.initial_locals code lhs_index rhs_index witnesses_index) G.E.collector_instance,
        to_e_list Base.rule_body)
      (branch_validated state code lhs_index rhs_index witnesses_index rule witnesses lhs rhs 0 budget) ->
    checked
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list (Base.rule_arguments code lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
      (fun final => exists next updated result kind,
        final = (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [Base.result_word result]) /\
        scope state next budget /\ s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Claims.Code.statement_kind result = Some kind /\ X.check rule witnesses lhs rhs = Some kind).
  Proof.
    intros Functions [final [Body [Trap|Success]]].
    - destruct final as [[[next updated] following] instructions]; subst instructions.
      eapply checked_trap; eapply G.E.collector_call with (code := native_exchange)
        (parameters := [T_num T_i64; T_num T_i32; T_num T_i32; T_num T_i32]) (results := [T_num T_i64])
        (defaults := repeat (word 0) 4) (address := 38%N)
        (arguments := Base.rule_arguments code lhs_index rhs_index witnesses_index) (values := [Base.result_word 0])
        (accepted := false) (final_frame := following); try reflexivity.
      + rewrite Functions; reflexivity.
      + exact Body.
    - destruct Success as [next [updated [following [result [kind [depth [context
        [Final [Scoped [Within [FunctionsNext [Typed [Capacity [Kind Checked]]]]]]]]]]]]]]; subst final.
      eapply checked_result.
      + eapply G.E.collector_call_return with (code := native_exchange)
          (parameters := [T_num T_i64; T_num T_i32; T_num T_i32; T_num T_i32]) (results := [T_num T_i64])
          (defaults := repeat (word 0) 4) (address := 38%N)
          (arguments := Base.rule_arguments code lhs_index rhs_index witnesses_index) (values := [Base.result_word result])
          (accepted := true) (final_frame := Build_frame following G.E.collector_instance) (context := context); try reflexivity.
        * rewrite Functions; reflexivity.
        * exact Body.
      + exists next, updated, result, kind; repeat first [assumption|split|reflexivity].
  Qed.
End WasmDispatchSupport.
