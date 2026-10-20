(** Supplies execution lemmas and tactics for local variables, arithmetic,
    host calls, integer decoding, and observed handle comparisons. *)

From FixProof.Collector.wasm Require Import Backend.
Import ListNotations.
Local Open Scope list_scope.

Module WasmSupport (C : COLLECTOR_CONTEXT).
  Import C S R.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.

  Local Notation ext := G.E.H.W.extern_value.
  Local Notation treeh := G.E.H.W.H.HTreeObj.
  Local Notation blobh := G.E.H.W.H.HBlobObj.
  Local Notation lower := X.D.EC.E.EP.E.lower.
  Local Notation lift := X.D.EC.E.EP.E.lift.
  Local Notation local_get_run := (G.E.collector_local_get combination true handle_bytes).

  Definition word n := VAL_num (VAL_int32 (wasm_nat n)).


  (** Writing a valid local executes as the corresponding local-list update. *)
  Lemma local_set_run state store locals index value :
    (N.to_nat index < List.length locals)%nat ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        [v_to_e value; AI_basic (BI_local_set index)])
      (state, store, Build_frame (seq.set_nth value locals (N.to_nat index) value)
        G.E.collector_instance, []).
  Proof.
    intro Bound; apply rt_step; eapply r_local_set with (vd := value);
      [reflexivity|apply (ssrbool.introT ssrnat.ltP); exact Bound|reflexivity].
  Qed.

  (** The arity helper executes successfully for a Tree of the required bounded length. *)
  Lemma arity_run state store locals tree length :
    s_funcs store = s_funcs G.E.allocated_store ->
    List.length (get_tree_raw tree) = length -> (Z.of_nat length < 2 ^ 32)%Z ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [ext (treeh tree); word length] ++ [AI_basic (BI_call 22%N)])
      (state, store, Build_frame locals G.E.collector_instance, []).
  Proof.
    intros Functions Length Bound.
    assert (TreeBound : (Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 32)%Z)
      by now rewrite Length.
    pose proof (G.collector_arity_executes combination handle_bytes state store locals tree
      (wasm_nat length) Functions TreeBound) as Run.
    change (reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [ext (treeh tree); word length] ++ [AI_basic (BI_call 22%N)])
      (state, store, Build_frame locals G.E.collector_instance,
        if Wasm_int.int_eq i32m (wasm_nat (List.length (get_tree_raw tree))) (wasm_nat length)
        then [] else [AI_trap])) in Run.
    rewrite Length, (wasm_nat_equal length length Bound Bound), Nat.eqb_refl in Run; exact Run.
  Qed.

  (** The Tree-child import executes an in-bounds lookup without changing the store. *)
  Lemma child_run state store locals tree index child :
    s_funcs store = s_funcs G.E.allocated_store ->
    (Z.of_nat index < 2 ^ 32)%Z -> nth_error (get_tree_raw tree) index = Some child ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [ext (treeh tree); word index] ++ [AI_basic (BI_call 7%N)])
      (state, store, Build_frame locals G.E.collector_instance, v_to_e_list [ext child]).
  Proof.
    intros Functions Bound Child.
    pose proof (G.collector_tree_element_executes combination handle_bytes state store locals
      (treeh tree) (wasm_nat index) Functions) as Run.
    change (reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [ext (treeh tree); word index] ++ [AI_basic (BI_call 7%N)])
      (state, store, Build_frame locals G.E.collector_instance,
        match nth_error (get_tree_raw tree) (Wasm_int.nat_of_uint i32m (wasm_nat index)) with
        | Some value => v_to_e_list [ext value] | None => [AI_trap] end)) in Run.
    rewrite (wasm_nat_to_nat index Bound), Child in Run; exact Run.
  Qed.
  (** The Tag query import returns the Tree's Tag marker. *)
  Lemma tag_run state store locals value tagged :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.W.Api.is_tag value = tagged ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [ext value] ++ [AI_basic (BI_call 3%N); AI_basic (BI_call 18%N)])
      (state, store, Build_frame locals G.E.collector_instance, if tagged then [] else [AI_trap]).
  Proof.
    intros Functions Tagged.
    set (result := VAL_num (VAL_int32 (wasm_bool tagged))).
    eapply rt_trans with (y := (state, store, Build_frame locals G.E.collector_instance,
      [v_to_e result; AI_basic (BI_call 18%N)])).
    - eapply G.E.collector_runs_context with (prefix := []) (suffix := [AI_basic (BI_call 18%N)])
        (instructions := v_to_e_list [ext value] ++ [AI_basic (BI_call 3%N)])
        (instructions' := v_to_e_list [result]).
      eapply G.E.collector_import_executes with (operation := shell_is_tag)
        (index := 3%N) (address := 3%N) (arguments := [ext value])
        (result := result_values [result]); try reflexivity.
      + rewrite Functions; reflexivity.
      + apply G.E.H.host_values_result.
        unfold G.E.H.host_values, ext; rewrite to_handle_to_externref.
        rewrite Tagged; destruct tagged; reflexivity.
    - apply G.E.collector_require_executes with (condition := tagged); rewrite Functions; reflexivity.
  Qed.

  Ltac execute_under stack run := collector_execute_under G.E.collector_runs_context stack run.
  Ltac execute run := execute_under (@nil value) run.

  (** A native integer relational instruction computes the modeled comparison result. *)
  Lemma native_relop state store locals lhs rhs ty op :
    relop_typecheck lhs rhs ty op = true ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num lhs; VAL_num rhs] ++ [AI_basic (BI_relop ty op)])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool (app_relop op lhs rhs)))]).
  Proof. intro Typed; apply rt_step, r_simple, rs_relop; exact Typed. Qed.

  (** A native integer binary instruction computes the modeled arithmetic result. *)
  Lemma native_binop state store locals lhs rhs ty op result :
    binop_typecheck lhs rhs ty op = true -> app_binop op lhs rhs = Some result ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num lhs; VAL_num rhs] ++ [AI_basic (BI_binop ty op)])
      (state, store, Build_frame locals G.E.collector_instance, v_to_e_list [VAL_num result]).
  Proof. intros Typed Result; apply rt_step, r_simple; eapply rs_binop_success; eassumption. Qed.

  (** A native i32 test computes the modeled Boolean result. *)
  Lemma native_test32 state store locals operand op :
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 operand)] ++ [AI_basic (BI_testop T_i32 op)])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool (app_testop_i i32m op operand)))]).
  Proof. apply rt_step, r_simple, rs_testop_i32. Qed.

  (** A successful value-returning Shell import executes without changing the store. *)
  Lemma native_value_import state store locals index operation arguments values :
    s_funcs store = s_funcs G.E.allocated_store ->
    lookup_N (s_funcs G.E.allocated_store) index =
      Some (FC_func_host (shell_signature operation) operation) ->
    lookup_N (inst_funcs G.E.collector_instance) index = Some index ->
    (match shell_signature operation with Tf parameters _ =>
      List.length arguments = List.length parameters end) ->
    G.E.H.host_values combination G.E.collector_instance true handle_bytes state store
      operation arguments = Some values ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list arguments ++ [AI_basic (BI_call index)])
      (G.E.H.next_state state operation, store, Build_frame locals G.E.collector_instance,
        v_to_e_list values).
  Proof.
    intros Functions Closure Index Arity Values.
    eapply G.E.collector_import_executes with (operation := operation) (address := index)
      (result := result_values values); try eassumption.
    - rewrite Functions; exact Closure.
    - apply G.E.H.host_values_result; exact Values.
  Qed.

  (** The native type-query import returns the handle's modeled type code. *)
  Lemma native_type_query state store locals index input :
    s_funcs store = s_funcs G.E.allocated_store ->
    lookup_N locals index = Some (ext input) ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        to_e_list [BI_local_get index; BI_call 0%N])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [word (G.E.H.W.H.value_type input)]).
  Proof.
    intros Functions Local.
    eapply G.collector_query_local with (operation := shell_get_type) (value := input);
      try exact Local; try reflexivity.
    - rewrite Functions; reflexivity.
    - unfold G.E.H.host_values, ext; rewrite to_handle_to_externref; reflexivity.
  Qed.

  (** The native Object-type query returns the modeled type code for Data. *)
  Lemma native_data_type_query state store locals index input :
    s_funcs store = s_funcs G.E.allocated_store ->
    lookup_N locals index = Some (ext (Data input)) ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        to_e_list [BI_local_get index; BI_call 1%N])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [word (match input with Object (TreeObj _) | Ref (TreeRef _) => 1 | _ => 0 end)]).
  Proof.
    intros Functions Local.
    eapply G.collector_query_local with (operation := shell_get_data_type) (value := Data input);
      try exact Local; try reflexivity.
    - rewrite Functions; reflexivity.
    - unfold G.E.H.host_values, ext; rewrite to_handle_to_externref; destruct input as [[b|t]|[b|t]]; reflexivity.
  Qed.

  (** The require helper executes its condition check with the modeled success or trap
      result. *)
  Lemma native_require state store locals (condition : bool) :
    s_funcs store = s_funcs G.E.allocated_store ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool condition))] ++ [AI_basic (BI_call 18%N)])
      (state, store, Build_frame locals G.E.collector_instance, if condition then [] else [AI_trap]).
  Proof. intro Functions; apply G.E.collector_require_executes; rewrite Functions; reflexivity. Qed.

  Ltac local_value locals index :=
    let found := eval hnf in (lookup_N locals index) in
    lazymatch found with
    | Some ?entry => let entry := eval hnf in entry in constr:(Some entry)
    | None => constr:(@None value)
    end.

  Ltac native_pure_at state store locals stack instruction :=
    lazymatch instruction with
    | BI_local_get ?index =>
        let found := eval hnf in (lookup_N locals index) in
        lazymatch found with
        | Some ?value => execute_under stack (local_get_run state store locals index value eq_refl)
        | _ => fail "cannot resolve local" index found
        end
    | BI_local_set ?index =>
        let prefix := eval cbv beta iota zeta delta [firstn List.length List.app Nat.sub] in (firstn (List.length stack - 1) stack) in
        let found := eval cbv beta iota zeta delta [nth_error List.length List.app Nat.sub] in (nth_error stack (List.length stack - 1)) in
        lazymatch found with
        | Some ?value => execute_under prefix
            (local_set_run state store locals index value ltac:(lazy; lia))
        end
    | BI_relop ?ty ?op =>
        let prefix := eval cbv beta iota zeta delta [firstn List.length List.app Nat.sub] in (firstn (List.length stack - 2) stack) in
        let pair := eval cbv beta iota zeta delta [skipn List.length List.app Nat.sub] in (skipn (List.length stack - 2) stack) in
        lazymatch pair with
        | [VAL_num ?lhs; VAL_num ?rhs] => execute_under prefix
            (native_relop state store locals lhs rhs ty op ltac:(reflexivity))
        | _ => fail "relop stack does not have two numeric operands" pair
        end
    | BI_binop ?ty ?op =>
        let prefix := eval cbv beta iota zeta delta [firstn List.length List.app Nat.sub] in (firstn (List.length stack - 2) stack) in
        let pair := eval cbv beta iota zeta delta [skipn List.length List.app Nat.sub] in (skipn (List.length stack - 2) stack) in
        lazymatch pair with
        | [VAL_num ?lhs; VAL_num ?rhs] =>
            let computed := eval vm_compute in (app_binop op lhs rhs) in
            lazymatch computed with
            | Some ?result => execute_under prefix
                (native_binop state store locals lhs rhs ty op result ltac:(reflexivity) eq_refl)
            end
        end
    | BI_testop T_i32 ?op =>
        let prefix := eval cbv beta iota zeta delta [firstn List.length List.app Nat.sub] in (firstn (List.length stack - 1) stack) in
        let last := eval cbv beta iota zeta delta [nth_error List.length List.app Nat.sub] in (nth_error stack (List.length stack - 1)) in
        lazymatch last with
        | Some (VAL_num (VAL_int32 ?operand)) => execute_under prefix
            (native_test32 state store locals operand op)
        end
    | BI_call 18%N =>
        let prefix := eval cbv beta iota zeta delta [firstn List.length List.app Nat.sub] in (firstn (List.length stack - 1) stack) in
        let last := eval cbv beta iota zeta delta [nth_error List.length List.app Nat.sub] in (nth_error stack (List.length stack - 1)) in
        lazymatch last with
        | Some (VAL_num (VAL_int32 ?operand)) =>
            let zero := eval vm_compute in (Wasm_int.int_eq i32m operand (wasm_nat 0)) in
            let condition := eval cbn in (negb zero) in
            lazymatch goal with
            | Functions : s_funcs store = s_funcs G.E.allocated_store |- _ =>
                execute_under prefix (native_require state store locals condition Functions)
            end
        end
    | BI_if (BT_valtype None) ?yes ?no =>
        let prefix := eval cbv beta iota zeta delta [firstn List.length List.app Nat.sub] in (firstn (List.length stack - 1) stack) in
        let last := eval cbv beta iota zeta delta [nth_error List.length List.app Nat.sub] in (nth_error stack (List.length stack - 1)) in
        lazymatch last with
        | Some (VAL_num (VAL_int32 ?operand)) =>
            let zero := eval vm_compute in (Wasm_int.int_eq i32m operand (wasm_nat 0)) in
            lazymatch zero with
            | true => lazymatch no with
                | [] => execute_under prefix
                    (G.E.collector_if_skip combination true handle_bytes state store
                      (Build_frame locals G.E.collector_instance) yes)
                | _ => execute_under prefix
                    (G.E.collector_if_enter combination true handle_bytes state store
                      (Build_frame locals G.E.collector_instance) false yes no)
                end
            | false => execute_under prefix
                (G.E.collector_if_enter combination true handle_bytes state store
                  (Build_frame locals G.E.collector_instance) true yes no)
            end
        end
    | _ => fail "not a pure instruction" instruction
    end.

  Ltac native_pure_scan state store locals stack instructions :=
    lazymatch instructions with
    | ?head :: ?tail =>
        let converted := eval cbn [e_to_v_opt e_to_vref_opt option_map v_to_e vref_to_e word G.E.H.W.H.value_type G.E.H.W.extern_value] in (e_to_v_opt head) in
        lazymatch converted with
        | Some ?value =>
            let stack := eval cbn [List.app] in (stack ++ [value]) in
            native_pure_scan state store locals stack tail
        | None => lazymatch head with
            | AI_basic ?instruction => native_pure_at state store locals stack instruction
            | _ => fail "not a basic instruction" head
            end
        | _ => fail "value conversion did not simplify" converted
        end
    | _ => fail "no next instruction" instructions
    end.

  Ltac native_pure :=
    try fold reduce_trans;
    lazymatch goal with
    | |- ?relation (?state, ?store, Build_frame ?locals G.E.collector_instance, ?instructions) _ =>
        let instructions := eval cbn [to_e_list v_to_e_list List.app seq.map v_to_e vref_to_e G.E.H.W.H.HTreeObj G.E.H.W.H.HBlobObj] in instructions in
        native_pure_scan state store locals (@nil value) instructions
    | _ => fail "not a reduction goal with an explicit frame"
    end.

  (** The integer helper's guards accept an eight-byte Blob Object. *)
  Lemma collector_integer_guard_executes state store h :
    s_funcs store = s_funcs G.E.allocated_store ->
    reduce_trans
      (state, store, Build_frame [ext h] G.E.collector_instance,
        to_e_list (firstn 8 (modfunc_body evaluation_integer)))
      (state, store, Build_frame [ext h] G.E.collector_instance,
        match h with Data (Object (BlobObj _)) => [] | _ => [AI_trap] end).
  Proof.
    intro Functions; rewrite evaluation_integer_body; cbn [firstn].
    pose proof (G.collector_type_guard_executes combination handle_bytes state store [ext h] 0%N h
      Functions eq_refl) as ObjectGuard.
    destruct h as [[object|reference]|thunk|encode].
    - execute ObjectGuard.
      set (kind := match object with BlobObj _ => 0 | TreeObj _ => 1 end).
      set (accepted := match object with BlobObj _ => true | TreeObj _ => false end).
      assert (Kind : reduce_trans
        (state, store, Build_frame [ext (Data (Object object))] G.E.collector_instance,
          [AI_basic (BI_local_get 0%N); AI_basic (BI_call 1%N)])
        (state, store, Build_frame [ext (Data (Object object))] G.E.collector_instance,
          v_to_e_list [word kind])).
      { eapply G.collector_query_local with (operation := shell_get_data_type)
          (value := Data (Object object)); try reflexivity.
        - rewrite Functions; reflexivity.
        - unfold G.E.H.host_values, ext; rewrite to_handle_to_externref; destruct object; reflexivity. }
      execute Kind.
      assert (Test : reduce_trans
        (state, store, Build_frame [ext (Data (Object object))] G.E.collector_instance,
          [v_to_e (word kind); AI_basic (BI_testop T_i32 TO_eqz)])
        (state, store, Build_frame [ext (Data (Object object))] G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int32 (wasm_bool accepted))])).
      { destruct object; apply rt_step, r_simple, rs_testop_i32. }
      execute Test.
      destruct object;
        first [apply G.E.collector_require_executes with (condition := true)
              |apply G.E.collector_require_executes with (condition := false)]; rewrite Functions; reflexivity.
    - eapply G.E.collector_trap_suffix with (instructions := to_e_list
        [BI_local_get 0%N; BI_call 0%N; BI_testop T_i32 TO_eqz; BI_call 18%N]); exact ObjectGuard.
    - eapply G.E.collector_trap_suffix with (instructions := to_e_list
        [BI_local_get 0%N; BI_call 0%N; BI_testop T_i32 TO_eqz; BI_call 18%N]); exact ObjectGuard.
    - eapply G.E.collector_trap_suffix with (instructions := to_e_list
        [BI_local_get 0%N; BI_call 0%N; BI_testop T_i32 TO_eqz; BI_call 18%N]); exact ObjectGuard.
  Qed.

  (** The integer helper body loads a valid eight-byte Blob and returns its unsigned i64
      value. *)
  Lemma collector_integer_body_executes state store blob n before page :
    s_funcs store = s_funcs G.E.allocated_store ->
    s_mems store = [before] ->
    G.E.H.attachment_allowed before (get_blob_data blob) = true ->
    attachment_page (get_blob_data blob) = Some page ->
    decode_u64 (get_blob_data blob) = Some n ->
    reduce_trans
      (state, store, Build_frame [ext (blobh blob)] G.E.collector_instance,
        to_e_list (modfunc_body evaluation_integer))
      (state, upd_s_mem store [page], Build_frame [ext (blobh blob)] G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 n))]).
  Proof.
    intros Functions Memory Allowed Page Decoded.
    pose proof (decode_u64_width _ _ Decoded) as Width.
    set (locals := [ext (blobh blob)]).
    pose proof (collector_integer_guard_executes state store (blobh blob) Functions) as Guard.
    rewrite evaluation_integer_body in Guard; cbn [firstn] in Guard.
    rewrite evaluation_integer_body; execute Guard.
    assert (Length : reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        [AI_basic (BI_local_get 0%N); AI_basic (BI_call 6%N)])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 8))])).
    { eapply G.collector_query_local with (operation := shell_len)
        (value := blobh blob); try reflexivity.
      - rewrite Functions; reflexivity.
      - unfold G.E.H.host_values, ext; rewrite to_handle_to_externref.
        change (G.E.H.length_result (List.length (get_blob_data blob)) =
          Some [VAL_num (VAL_int64 (wasm_nat64 8))]).
        replace (List.length (get_blob_data blob)) with 8 by (symmetry; exact Width).
        reflexivity. }
    execute Length.
    assert (Equal : reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        [v_to_e (VAL_num (VAL_int64 (wasm_nat64 8)));
         v_to_e (VAL_num (VAL_int64 (wasm_nat64 8)));
         AI_basic (BI_relop T_i64 (Relop_i ROI_eq))])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool true))])).
    { apply rt_step, r_simple; eapply rs_relop with
        (v1 := VAL_int64 (wasm_nat64 8)) (v2 := VAL_int64 (wasm_nat64 8))
        (t := T_i64) (op := Relop_i ROI_eq); reflexivity. }
    execute Equal.
    execute (G.E.collector_require_executes combination true handle_bytes state store locals true
      ltac:(rewrite Functions; reflexivity)).
    execute (local_get_run state store locals 0%N (ext (blobh blob)) eq_refl).
    assert (Attach : reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [ext (blobh blob); word 0] ++ [AI_basic (BI_call 8%N)])
      (state, upd_s_mem store [page], Build_frame locals G.E.collector_instance, [])).
    { eapply G.E.collector_import_executes with (operation := shell_attach_blob)
        (address := 8%N) (arguments := [ext (blobh blob); word 0])
        (result := result_values []); try reflexivity.
      - rewrite Functions; reflexivity.
      - unfold G.E.H.host_result.
        destruct (function_type_eq_dec _ _); [|contradiction].
        destruct (shell_function_eq_dec _ _); [|contradiction].
        unfold G.E.H.attachment_result, ext; rewrite to_handle_to_externref.
        rewrite Memory.
        cbn -[G.E.H.attachment_allowed attachment_page upd_s_mem].
        rewrite Allowed, Page; reflexivity. }
    execute Attach.
    destruct (attachment_page_u64 _ _ _ Page Decoded) as [Loaded Number].
    change (wasm_deserialise (get_blob_data blob) T_i64 = VAL_int64 (wasm_nat64 n)) in Number.
    rewrite <- Number.
    apply rt_step; eapply r_load_success with (m := page) (bs := get_blob_data blob).
    - reflexivity.
    - exact Loaded.
  Qed.

  (** The native integer call decodes an attached eight-byte Blob as an unsigned i64. *)
  Lemma collector_integer_executes state store locals blob n before page :
    s_funcs store = s_funcs G.E.allocated_store ->
    s_mems store = [before] ->
    G.E.H.attachment_allowed before (get_blob_data blob) = true ->
    attachment_page (get_blob_data blob) = Some page ->
    decode_u64 (get_blob_data blob) = Some n ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [ext (blobh blob)] ++ [AI_basic (BI_call 23%N)])
      (state, upd_s_mem store [page], Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 n))]).
  Proof.
    intros Functions Memory Allowed Page Decoded.
    eapply G.E.collector_call with (code := evaluation_integer)
      (parameters := [T_ref T_externref]) (results := [T_num T_i64])
      (defaults := []) (index := 23%N) (address := 23%N)
      (arguments := [ext (blobh blob)]) (accepted := true)
      (values := [VAL_num (VAL_int64 (wasm_nat64 n))])
      (final_frame := Build_frame [ext (blobh blob)] G.E.collector_instance);
      try reflexivity.
    - rewrite Functions; reflexivity.
    - eapply collector_integer_body_executes; eassumption.
  Qed.

  (** The native integer call traps when its input is not a Blob Object. *)
  Lemma collector_integer_rejects_type state store locals h :
    s_funcs store = s_funcs G.E.allocated_store ->
    (forall blob, h <> blobh blob) ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [ext h] ++ [AI_basic (BI_call 23%N)])
      (state, store, Build_frame locals G.E.collector_instance, [AI_trap]).
  Proof.
    intros Functions NotBlob.
    eapply G.E.collector_call with (code := evaluation_integer)
      (parameters := [T_ref T_externref]) (results := [T_num T_i64]) (defaults := [])
      (index := 23%N) (address := 23%N) (arguments := [ext h])
      (values := [VAL_num (VAL_int64 (wasm_nat64 0))]) (accepted := false)
      (final_frame := Build_frame [ext h] G.E.collector_instance); try reflexivity.
    - rewrite Functions; reflexivity.
    - eapply G.E.collector_trap_suffix with
        (instructions := to_e_list (firstn 8 (modfunc_body evaluation_integer)))
        (suffix := to_e_list (skipn 8 (modfunc_body evaluation_integer))).
      pose proof (collector_integer_guard_executes state store h Functions) as Guard.
      destruct h as [[[]|reference]|thunk|encode]; try exact Guard.
      exfalso; eapply NotBlob; reflexivity.
  Qed.

  (** The native integer call traps when the Blob does not contain exactly eight bytes. *)
  Lemma collector_integer_rejects_width state store locals blob :
    s_funcs store = s_funcs G.E.allocated_store ->
    (Z.of_nat (List.length (get_blob_data blob)) < 2 ^ 64)%Z ->
    List.length (get_blob_data blob) <> 8 ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [ext (blobh blob)] ++ [AI_basic (BI_call 23%N)])
      (state, store, Build_frame locals G.E.collector_instance, [AI_trap]).
  Proof.
    intros Functions Bound Different.
    set (length := List.length (get_blob_data blob)).
    set (inside := [ext (blobh blob)]).
    eapply G.E.collector_call with (code := evaluation_integer)
      (parameters := [T_ref T_externref]) (results := [T_num T_i64]) (defaults := [])
      (index := 23%N) (address := 23%N) (arguments := [ext (blobh blob)])
      (values := [VAL_num (VAL_int64 (wasm_nat64 0))]) (accepted := false)
      (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
    - rewrite Functions; reflexivity.
    - pose proof (collector_integer_guard_executes state store (blobh blob) Functions) as Guard.
      rewrite evaluation_integer_body in Guard; cbn [firstn] in Guard.
      rewrite evaluation_integer_body; execute Guard.
      assert (Length : reduce_trans
        (state, store, Build_frame inside G.E.collector_instance,
          [AI_basic (BI_local_get 0%N); AI_basic (BI_call 6%N)])
        (state, store, Build_frame inside G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 length))])).
      { eapply G.collector_query_local with (operation := shell_len)
          (value := blobh blob); try reflexivity.
        - rewrite Functions; reflexivity.
        - unfold G.E.H.host_values, ext; rewrite to_handle_to_externref.
          apply G.E.H.length_result_small; exact Bound. }
      execute Length.
      assert (Mismatch : app_relop (Relop_i ROI_eq) (VAL_int64 (wasm_nat64 length))
        (VAL_int64 (wasm_nat64 8)) = false).
      { change (Wasm_int.Int64.eq (wasm_nat64 length) (wasm_nat64 8) = false).
        unfold Wasm_int.Int64.eq.
        rewrite (wasm_nat64_unsigned length Bound), (wasm_nat64_unsigned 8 ltac:(vm_compute; congruence)).
        change ((if Coqlib.zeq (Z.of_nat length) 8 then true else false) = false).
        destruct (Coqlib.zeq (Z.of_nat length) 8); [exfalso; apply Different; lia|reflexivity]. }
      assert (Compared : reduce_trans
        (state, store, Build_frame inside G.E.collector_instance,
          [v_to_e (VAL_num (VAL_int64 (wasm_nat64 length)));
           v_to_e (VAL_num (VAL_int64 (wasm_nat64 8))); AI_basic (BI_relop T_i64 (Relop_i ROI_eq))])
        (state, store, Build_frame inside G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int32 (wasm_bool false))])).
      { rewrite <- Mismatch; apply rt_step, r_simple; eapply rs_relop with
          (v1 := VAL_int64 (wasm_nat64 length)) (v2 := VAL_int64 (wasm_nat64 8))
          (t := T_i64) (op := Relop_i ROI_eq); reflexivity. }
      execute Compared.
      eapply G.E.collector_trap_suffix with
        (instructions := [v_to_e (VAL_num (VAL_int32 (wasm_bool false))); AI_basic (BI_call 18%N)]).
      apply G.E.collector_require_executes with (condition := false); rewrite Functions; reflexivity.
  Qed.

  Definition observed_comparison state lhs rhs equal := exists left right,
    handle_bytes state lhs = Some left /\
    handle_bytes (Datatypes.S state) rhs = Some right /\
    G.E.H.Tables.T.O.compare_handles (blobh left) (blobh right) = Some equal.

  Definition observed_same state lhs rhs := observed_comparison state lhs rhs true.

  (** Available privileged handle bytes make the modeled comparison observation defined. *)
  Lemma observed_comparison_total state lhs rhs :
    (forall index value, exists bytes, handle_bytes index value = Some bytes) ->
    exists equal, observed_comparison state lhs rhs equal.
  Proof.
    intro Bytes.
    destruct (Bytes state lhs) as [left Left].
    destruct (Bytes (Datatypes.S state) rhs) as [right Right].
    destruct (proj2 (G.E.H.Tables.T.O.compare_handles_defined (blobh left) (blobh right))
      (conj eq_refl eq_refl)) as [equal Compared].
    exists equal, left, right; auto.
  Qed.

  (** A successful identity observation implies exact equality of the abstract handles. *)
  Lemma observed_same_identifies state lhs rhs : observed_same state lhs rhs -> lhs = rhs.
  Proof.
    intros [left [right [Left [Right Compared]]]].
    apply X.API.handle_eqb_spec.
    eapply (G.collector_same_comparison_sound handle_bytes C.bytes_identify); eassumption.
  Qed.

  (** A ready identity observation executes the native comparison helper. *)
  Lemma same_run state store locals lhs rhs equal :
    s_funcs store = s_funcs G.E.allocated_store -> observed_comparison state lhs rhs equal ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [ext lhs; ext rhs] ++ [AI_basic (BI_call 19%N)])
      (Datatypes.S (Datatypes.S state), store, Build_frame locals G.E.collector_instance,
        if equal then [] else [AI_trap]).
  Proof.
    intros Functions [left [right [Left [Right Compared]]]].
    eapply G.collector_same_executes with (equal := equal); eassumption.
  Qed.

  (** The native loop counter test agrees with the bounded natural-number test. *)
  Lemma counter_test_run state store locals left_local right_local left right :
    (Z.of_nat left < 2 ^ 32)%Z -> (Z.of_nat right < 2 ^ 32)%Z ->
    lookup_N locals left_local = Some (word left) -> lookup_N locals right_local = Some (word right) ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        to_e_list [BI_local_get left_local; BI_local_get right_local;
          BI_relop T_i32 (Relop_i ROI_eq); BI_br_if 1%N])
      (state, store, Build_frame locals G.E.collector_instance,
        if Nat.eqb left right then [AI_basic (BI_br 1%N)] else []).
  Proof.
    intros LeftBound RightBound Left Right.
    execute (local_get_run state store locals left_local (word left) Left).
    execute_under [word left] (local_get_run state store locals right_local (word right) Right).
    eapply rt_trans with (y := (state, store, Build_frame locals G.E.collector_instance,
      [v_to_e (VAL_num (VAL_int32 (wasm_bool (Nat.eqb left right)))); AI_basic (BI_br_if 1%N)])).
    - pose proof (wasm_nat_equal left right LeftBound RightBound) as Compared.
      change (app_relop (Relop_i ROI_eq) (VAL_int32 (wasm_nat left)) (VAL_int32 (wasm_nat right)) =
        Nat.eqb left right) in Compared.
      rewrite <- Compared.
      apply rt_step; eapply r_label with (lh := LH_base [] [AI_basic (BI_br_if 1%N)]).
      + apply r_simple; eapply rs_relop with (v1 := VAL_int32 (wasm_nat left))
          (v2 := VAL_int32 (wasm_nat right)) (t := T_i32) (op := Relop_i ROI_eq); reflexivity.
      + reflexivity.
      + reflexivity.
    - destruct (Nat.eqb left right); apply rt_step, r_simple;
        [apply rs_br_if_true; vm_compute; discriminate|apply rs_br_if_false; reflexivity].
  Qed.

  (** The Tree-creation import constructs the Tree represented by the supplied
      externref-table prefix. *)
  Lemma collector_create_tree_executes state store locals index table values unused :
    s_funcs store = s_funcs G.E.allocated_store ->
    stab store G.E.collector_instance (Wasm_int.N_of_uint i32m index) = Some table ->
    tt_elem_type (tableinst_type table) = T_externref ->
    tableinst_elem table = map G.E.H.Tables.externref values ++ unused ->
    (Z.of_nat (List.length values) < 2 ^ 32)%Z ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 index); word (List.length values)] ++ [AI_basic (BI_call 9%N)])
      (state, store, Build_frame locals G.E.collector_instance, v_to_e_list [ext (treeh (create_tree values))]).
  Proof.
    intros Functions Table Element Contents Bound.
    pose proof (G.E.H.Tables.create_tree_from_table_correct store G.E.collector_instance
      (Wasm_int.N_of_uint i32m index) table values unused Table Element Contents) as Construct.
    eapply G.E.collector_import_executes with (operation := shell_create_tree)
      (arguments := [VAL_num (VAL_int32 index); word (List.length values)])
      (result := result_values [ext (treeh (create_tree values))]); try reflexivity.
    - rewrite Functions; reflexivity.
    - apply G.E.H.host_values_result.
      unfold G.E.H.host_values, word, G.E.H.W.nat_of_i32.
      rewrite (wasm_nat_to_nat _ Bound), Construct; reflexivity.
  Qed.

  (** The Tag-creation import constructs the Tag described by the modeled fields and
      selected issuer. *)
  Lemma collector_create_tag_executes state store locals index depth table authors body issuing_author :
    s_funcs store = s_funcs G.E.allocated_store ->
    stab store G.E.collector_instance (Wasm_int.N_of_uint i32m index) = Some table ->
    tt_elem_type (tableinst_type table) = T_externref ->
    tableinst_elem table = map G.E.H.Tables.externref (treeh authors :: body) ->
    (Z.of_nat depth < 2 ^ 32)%Z ->
    G.E.H.Tables.T.A.author_at depth (treeh combination) = Some issuing_author ->
    let tag := create_tree_tagged true (treeh (G.E.H.Tables.T.add_author authors issuing_author) :: body) in
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 index); word depth] ++ [AI_basic (BI_call 12%N)])
      (state, store, Build_frame locals G.E.collector_instance, v_to_e_list [ext (treeh tag)]).
  Proof.
    intros Functions Table Element Contents Bound Selected tag.
    pose proof (G.E.H.Tables.create_tag_from_table_correct combination store G.E.collector_instance
      (Wasm_int.N_of_uint i32m index) depth table (treeh authors :: body) Table Element Contents) as Construct.
    change (G.E.H.Tables.T.A.author_at depth (G.E.H.Tables.T.O.EC.E.EP.E.H.HTreeObj combination) = Some issuing_author)
      in Selected.
    unfold G.E.H.Tables.T.create_tag in Construct; rewrite Selected in Construct.
    eapply G.E.collector_import_executes with (operation := shell_create_tag)
      (arguments := [VAL_num (VAL_int32 index); word depth]) (result := result_values [ext (treeh tag)]);
      try reflexivity.
    - rewrite Functions; reflexivity.
    - apply G.E.H.host_values_result.
      unfold G.E.H.host_values, word, G.E.H.W.nat_of_i32.
      rewrite (wasm_nat_to_nat _ Bound), Construct; reflexivity.
  Qed.

  (** The integer-creation import constructs the Blob encoding the supplied unsigned
      integer. *)
  Lemma collector_create_integer_executes state store locals number :
    s_funcs store = s_funcs G.E.allocated_store -> (Z.of_nat number < 2 ^ 64)%Z ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 number))] ++ [AI_basic (BI_call 10%N)])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [ext (blobh (create_blob (from_nat number)))]).
  Proof.
    intros Functions Bound.
    eapply G.E.collector_import_executes with (operation := shell_create_blob_i64)
      (arguments := [VAL_num (VAL_int64 (wasm_nat64 number))])
      (result := result_values [ext (blobh (create_blob (from_nat number)))]); try reflexivity.
    - rewrite Functions; reflexivity.
    - apply G.E.H.host_values_result; unfold G.E.H.host_values.
      rewrite serialise_i64_encode_u64, (wasm_nat64_unsigned _ Bound), Nat2Z.id; reflexivity.
  Qed.

End WasmSupport.
