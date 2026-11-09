From FixProof.Collector.wasm Require Import Backend Support Refinement.
Import ListNotations.
Local Open Scope list_scope.

Module WasmResolution (C : COLLECTOR_CONTEXT).
  Import C S R.
  Module V := WasmRefinement C.
  Import V V.Support.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation ext := G.E.H.W.extern_value.

  Definition selected_return state store code (final : nat * store_record * frame * list administrative_instruction) :=
    let '(state', store', _, instructions) := final in
    state' = state /\ store' = store /\ exists depth (context : lholed depth),
      instructions = lfill context
        (v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 code))] ++ [AI_basic BI_return]).

  Lemma selected_label state store code at_state at_store frame body suffix :
    checked (at_state, at_store, frame, body) (selected_return state store code) ->
    checked (at_state, at_store, frame, [AI_label 0 [] body] ++ suffix) (selected_return state store code).
  Proof.
    intro Body.
    assert (Shape : in_context 1 (LH_rec [] 0 [] (LH_base [] []) suffix)
        (at_state, at_store, frame, body) = (at_state, at_store, frame, [AI_label 0 [] body] ++ suffix)).
    { cbn [in_context lfill]; rewrite List.app_nil_r; reflexivity. }
    rewrite <- Shape.
    eapply checked_context with
      (initial := (at_state, at_store, frame, body))
      (context := LH_rec [] 0 [] (LH_base [] []) suffix); [exact Body|].
    intros [[[state' store'] frame'] instructions] [State [Store [depth [context Return]]]]; subst instructions.
    eapply checked_result; [apply rt_refl|].
    split; [exact State|]; split; [exact Store|].
    exists (S depth), (LH_rec [] 0 [] context suffix).
    cbn [lfill]; rewrite List.app_nil_r; reflexivity.
  Qed.

  Ltac resolve_query functions checked_mode :=
    lazymatch goal with
    | |- ?relation (?state, ?store, Build_frame ?locals G.E.collector_instance,
        ?head :: AI_basic (BI_call ?index) :: ?tail) _ =>
      let parsed := eval cbn [e_to_v_opt e_to_vref_opt option_map v_to_e vref_to_e G.E.H.W.extern_value] in (e_to_v_opt head) in
      lazymatch parsed with
      | Some (VAL_ref (VAL_ref_extern (to_externref ?input))) =>
      let operation := lazymatch index with
        | 0%N => constr:(shell_get_type)
        | 1%N => constr:(shell_get_data_type)
        | 2%N => constr:(shell_get_encode_type)
        | 20%N => constr:(shell_get_thunk_type)
        end in
      let answer := lazymatch index with
        | 0%N => constr:(Some (G.E.H.W.H.value_type input))
        | 1%N => eval cbn [G.E.H.data_type] in (G.E.H.data_type input)
        | 2%N => eval cbn [G.E.H.encode_type] in (G.E.H.encode_type input)
        | 20%N => eval cbn [G.E.H.thunk_type] in (G.E.H.thunk_type input)
        end in
      lazymatch answer with
      | Some ?number =>
        let Query := fresh "Query" in
        assert (Query : reduce_trans
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [ext input] ++ [AI_basic (BI_call index)])
          (state, store, Build_frame locals G.E.collector_instance, v_to_e_list [word number]))
          by (exact (native_value_import state store locals index operation [ext input] [word number]
            functions eq_refl eq_refl eq_refl
            ltac:(unfold G.E.H.host_values, ext; rewrite to_handle_to_externref; reflexivity)));
        lazymatch checked_mode with
        | true => checked_advance Query
        | false => execute Query
        end
      end
      end
    end.

  Ltac drop_empty_label functions :=
    lazymatch goal with
    | |- checked (?state, ?store, ?frame, AI_label 0 [] ?body :: ?suffix) _ =>
      let Empty := fresh "Empty" in
      assert (Empty : reduce_trans (state, store, frame, body) (state, store, frame, []))
        by (cbn [to_e_list List.map seq.map];
          repeat first [native_pure | resolve_query functions false]; apply rt_refl);
      let Drop := fresh "Drop" in
      assert (Drop : reduce_trans (state, store, frame, [AI_label 0 [] body]) (state, store, frame, []))
        by (eapply rt_trans; [apply G.E.collector_runs_label; exact Empty|];
          apply rt_step, r_simple, rs_label_const; reflexivity);
      checked_advance Drop
    end.

  Ltac resolve_body functions :=
    normalize_checked; cbn [to_e_list List.map seq.map];
    cbn [Nat.eqb] in *;
    first [
      lazymatch goal with
      | |- checked (?state, ?store, ?frame, [AI_trap]) _ => apply checked_trap, rt_refl
      | |- checked (?state, ?store, ?frame, v_to_e_list ?values ++ [AI_basic BI_return]) _ =>
        eapply checked_result; [apply rt_refl|]; split; [reflexivity|]; split; [reflexivity|];
        exists 0, (LH_base [] []); reflexivity
      | |- checked (?state, ?store, ?frame, [?head; AI_basic BI_return]) _ =>
        eapply checked_result; [apply rt_refl|]; split; [reflexivity|]; split; [reflexivity|];
        exists 0, (LH_base [] []); reflexivity
      | |- checked (?state, ?store, ?frame, AI_label 0 [] ?body :: ?suffix) _ =>
        first [apply selected_label; resolve_body functions
          | drop_empty_label functions; resolve_body functions]
      end
    | checked_pure; resolve_body functions
    | resolve_query functions true; resolve_body functions
    ].

  Theorem resolve_validation code lhs result state store locals :
    (Z.of_nat code < 2 ^ 64)%Z -> X.select_exchange code lhs = Some result ->
    s_funcs store = s_funcs G.E.allocated_store ->
    checked
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 code)); ext lhs] ++ [AI_basic (BI_call 43%N)])
      (fun final => final = (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 result))])).
  Proof.
    intros Bound Selected Functions.
    eapply checked_call with (code := native_resolve)
      (parameters := [T_num T_i64; T_ref T_externref]) (results := [T_num T_i64])
      (address := 43%N) (defaults := [word 0])
      (arguments := [VAL_num (VAL_int64 (wasm_nat64 code)); ext lhs])
      (dummy := [VAL_num (VAL_int64 (wasm_nat64 result))]); try reflexivity.
    - rewrite Functions; reflexivity.
    - eapply checked_weaken with (before := selected_return state store result).
      + unfold native_resolve at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
        destruct code as [|[|[|[|[|[|[|[|[|[|[|[|[|[|[|[|[|[|[|[|[|[|[|[|[|code]]]]]]]]]]]]]]]]]]]]]]]]];
          destruct lhs as [[[blob|tree]|[blob|tree]]|[tree|reference|tree]|[[tree|reference|tree]|[tree|reference|tree]]];
          cbn [X.select_exchange] in Selected; try discriminate; inversion Selected; subst result;
          resolve_body Functions.
      + intros [[[state' store'] frame'] instructions] [State [Store [depth [context Return]]]];
          subst state' store' instructions.
        exists [VAL_num (VAL_int64 (wasm_nat64 result))]; split.
        * right; exists depth, context; reflexivity.
        * split; [reflexivity|]; reflexivity.
  Qed.
End WasmResolution.
