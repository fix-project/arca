(** Proves execution of the collector guards for Tree shape, length,
    reference authentication, and exact handle comparison. *)

From Wasm Require Import datatypes operations opsem.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Collector Require Import Matching.
From FixProof.Shell Require Import ABI Externref Host.
From FixProof.Collector.wasm Require Import Binary Layout Execution.
From FixProof.Support Require Import WasmNatural.
From Stdlib Require Import List NArith ZArith Lia Relation_Operators.
Import ListNotations.
Local Open Scope list_scope.

Definition evaluation_tree :=
  nth 2 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

(** The parsed Tree guard has the expected type-checking body. *)
Lemma evaluation_tree_body : modfunc_body evaluation_tree =
  [BI_local_get 0%N; BI_call 0%N; BI_testop T_i32 TO_eqz; BI_call 18%N;
   BI_local_get 0%N; BI_call 1%N; BI_const_num (VAL_int32 (Wasm_int.int_of_Z i32m 1%Z));
   BI_relop T_i32 (Relop_i ROI_eq); BI_call 18%N].
Proof. reflexivity. Qed.

Definition evaluation_size :=
  nth 3 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).
Definition evaluation_arity :=
  nth 4 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

(** The parsed size helper has the expected Tree and index-bound checks. *)
Lemma evaluation_size_body : modfunc_body evaluation_size =
  [BI_local_get 0%N; BI_call 20%N; BI_local_get 0%N; BI_call 6%N;
   BI_const_num (VAL_int64 (Wasm_int.int_of_Z i64m 4294967295%Z));
   BI_relop T_i64 (Relop_i (ROI_le SX_U)); BI_call 18%N;
   BI_local_get 0%N; BI_call 6%N; BI_cvtop T_i32 CVO_wrap T_i64 None].
Proof. reflexivity. Qed.
(** The parsed arity helper has the expected size-equality guard. *)
Lemma evaluation_arity_body : modfunc_body evaluation_arity =
  [BI_local_get 0%N; BI_call 21%N; BI_local_get 1%N;
   BI_relop T_i32 (Relop_i ROI_eq); BI_call 18%N].
Proof. reflexivity. Qed.

Definition evaluation_object :=
  nth 7 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

(** The parsed Object witness helper has the expected reference-authentication body. *)
Lemma evaluation_object_body : modfunc_body evaluation_object =
  [BI_local_get 1%N; BI_call 0%N; BI_testop T_i32 TO_eqz; BI_call 18%N;
   BI_local_get 0%N; BI_local_get 1%N; BI_call 11%N; BI_call 19%N].
Proof. reflexivity. Qed.

Definition evaluation_same :=
  nth 1 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

(** The parsed identity helper has the expected privileged-byte comparison body. *)
Lemma evaluation_same_body : modfunc_body evaluation_same =
  [BI_local_get 0%N; BI_call 4%N; BI_local_get 1%N; BI_call 4%N; BI_call 5%N; BI_call 18%N].
Proof. reflexivity. Qed.

Module WasmGuards (S : STORAGE) (P : PROGRAM S) (R : EXTERNREFS S).
  Module Matching := CouponMatching S.
  Module E := WasmExecution S P R.
  Import S R.
  Local Notation extern_value := E.H.W.extern_value.
  Local Notation HBlobObj := E.H.W.H.HBlobObj.

  Section Execution.
    Context `{mem : BlockUpdateMemory}.
    Variable combination : S.TreeName.
    Variable handle_bytes : nat -> S.handle -> option S.BlobName.
    Local Instance collector_host : host :=
      E.H.shell_host combination E.collector_instance true handle_bytes.

    (** The modeled query import returns the requested property of a handle stored in a
        local. *)
    Lemma collector_query_local state store locals local index operation value values :
      lookup_N (s_funcs store) index =
        Some (FC_func_host (shell_signature operation) operation) ->
      lookup_N (inst_funcs E.collector_instance) index = Some index ->
      lookup_N locals local = Some (extern_value value) ->
      (match shell_signature operation with Tf parameters _ =>
        parameters = [T_ref T_externref] end) ->
      E.H.host_values combination E.collector_instance true handle_bytes state store
        operation [extern_value value] = Some values ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          [AI_basic (BI_local_get local); AI_basic (BI_call index)])
        (E.H.next_state state operation, store, Build_frame locals E.collector_instance,
          v_to_e_list values).
    Proof.
      intros Closure Index Local Signature Values.
      eapply rt_trans with (y := (state, store, Build_frame locals E.collector_instance,
        [v_to_e (extern_value value); AI_basic (BI_call index)])).
      - apply rt_step; eapply r_label with (lh := LH_base [] [AI_basic (BI_call index)]).
        + eapply r_local_get; exact Local.
        + reflexivity.
        + reflexivity.
      - eapply E.collector_import_executes with (address := index) (operation := operation)
          (arguments := [extern_value value]) (result := result_values values);
          try exact Closure; try exact Index.
        + destruct (shell_signature operation); cbn in Signature; rewrite Signature; reflexivity.
        + apply E.H.host_values_result; exact Values.
    Qed.

    (** A successful handle-property guard executes its instruction prefix without changing
        the remaining computation. *)
    Lemma collector_guard_prefix state store frame instructions (accepted : bool) suffix result :
      reduce_trans (state, store, frame, instructions)
        (state, store, frame, if accepted then [] else [AI_trap]) ->
      reduce_trans (state, store, frame, suffix) (state, store, frame, result) ->
      reduce_trans (state, store, frame, instructions ++ suffix)
        (state, store, frame, if accepted then result else [AI_trap]).
    Proof.
      intros Guard Rest.
      eapply rt_trans with (y := (state, store, frame,
        (if accepted then [] else [AI_trap]) ++ suffix)).
      - apply E.collector_runs_context with (prefix := []); exact Guard.
      - destruct accepted; [exact Rest|].
        destruct suffix as [|first suffix]; [apply rt_refl|].
        apply rt_step, r_simple; eapply rs_trap with
          (lh := LH_base [] (first :: suffix)); [discriminate|reflexivity].
    Qed.

    (** The native type guard accepts a handle with the requested type. *)
    Lemma collector_type_guard_executes state store locals local value :
      s_funcs store = s_funcs E.allocated_store ->
      lookup_N locals local = Some (extern_value value) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          to_e_list [BI_local_get local; BI_call 0%N; BI_testop T_i32 TO_eqz; BI_call 18%N])
        (state, store, Build_frame locals E.collector_instance,
          match value with Data (Object _) => [] | _ => [AI_trap] end).
    Proof.
      intros Functions Local.
      eapply rt_trans with (y := (state, store,
        Build_frame locals E.collector_instance,
        [v_to_e (VAL_num (VAL_int32 (E.H.W.i32_of_nat (E.H.W.H.value_type value))));
         AI_basic (BI_testop T_i32 TO_eqz); AI_basic (BI_call 18%N)])).
      - eapply E.collector_runs_context with (prefix := [])
          (suffix := [AI_basic (BI_testop T_i32 TO_eqz); AI_basic (BI_call 18%N)])
          (instructions := [AI_basic (BI_local_get local); AI_basic (BI_call 0%N)])
          (instructions' := [v_to_e (VAL_num (VAL_int32
            (E.H.W.i32_of_nat (E.H.W.H.value_type value))))]).
        eapply collector_query_local with (operation := shell_get_type) (value := value)
          (values := [VAL_num (VAL_int32 (E.H.W.i32_of_nat (E.H.W.H.value_type value)))]);
          try reflexivity; try exact Local.
        + rewrite Functions; reflexivity.
        + unfold E.H.host_values, extern_value; rewrite to_handle_to_externref; reflexivity.
      - eapply rt_trans with (y := (state, store,
          Build_frame locals E.collector_instance,
          [v_to_e (VAL_num (VAL_int32 (wasm_bool (app_testop_i i32m TO_eqz
            (E.H.W.i32_of_nat (E.H.W.H.value_type value)))))); AI_basic (BI_call 18%N)])).
        + apply rt_step; eapply r_label with (lh := LH_base [] [AI_basic (BI_call 18%N)]);
            [apply r_simple, rs_testop_i32|reflexivity|reflexivity].
        + destruct value as [[object|reference]|thunk|encode];
            first [apply E.collector_require_executes with (condition := true)
                  |apply E.collector_require_executes with (condition := false)];
            rewrite Functions; reflexivity.
    Qed.

    (** The native Tree guard accepts Tree Objects. *)
    Lemma collector_tree_data_guard_executes state store object :
      s_funcs store = s_funcs E.allocated_store ->
      reduce_trans
        (state, store, Build_frame [extern_value (Data (Object object))] E.collector_instance,
          to_e_list [BI_local_get 0%N; BI_call 1%N;
            BI_const_num (VAL_int32 (Wasm_int.int_of_Z i32m 1%Z));
            BI_relop T_i32 (Relop_i ROI_eq); BI_call 18%N])
        (state, store, Build_frame [extern_value (Data (Object object))] E.collector_instance,
          match object with TreeObj _ => [] | BlobObj _ => [AI_trap] end).
    Proof.
      intro Functions.
      set (kind := match object with BlobObj _ => 0 | TreeObj _ => 1 end).
      set (word := VAL_int32 (E.H.W.i32_of_nat kind)).
      set (one := VAL_int32 (Wasm_int.int_of_Z i32m 1%Z)).
      eapply rt_trans with (y := (state, store,
        Build_frame [extern_value (Data (Object object))] E.collector_instance,
        [v_to_e (VAL_num word); v_to_e (VAL_num one);
         AI_basic (BI_relop T_i32 (Relop_i ROI_eq)); AI_basic (BI_call 18%N)])).
      - eapply E.collector_runs_context with (prefix := [])
          (suffix := [v_to_e (VAL_num one); AI_basic (BI_relop T_i32 (Relop_i ROI_eq));
            AI_basic (BI_call 18%N)])
          (instructions := [AI_basic (BI_local_get 0%N); AI_basic (BI_call 1%N)])
          (instructions' := [v_to_e (VAL_num word)]).
        eapply collector_query_local with (operation := shell_get_data_type)
          (value := Data (Object object)) (values := [VAL_num word]); try reflexivity.
        + rewrite Functions; reflexivity.
        + unfold E.H.host_values, extern_value; rewrite to_handle_to_externref.
          destruct object; reflexivity.
      - eapply rt_trans with (y := (state, store,
          Build_frame [extern_value (Data (Object object))] E.collector_instance,
          [v_to_e (VAL_num (VAL_int32 (wasm_bool
            (app_relop (Relop_i ROI_eq) word one)))); AI_basic (BI_call 18%N)])).
        + apply rt_step; eapply r_label with (lh := LH_base [] [AI_basic (BI_call 18%N)]).
          * apply r_simple; apply rs_relop with (v1 := word) (v2 := one)
              (t := T_i32) (op := Relop_i ROI_eq); reflexivity.
          * reflexivity.
          * reflexivity.
        + destruct object;
            first [apply E.collector_require_executes with (condition := true)
                  |apply E.collector_require_executes with (condition := false)];
            rewrite Functions; reflexivity.
    Qed.

    (** The body of the native Tree guard accepts a Tree Object. *)
    Lemma collector_tree_body_executes state store value :
      s_funcs store = s_funcs E.allocated_store ->
      reduce_trans
        (state, store, Build_frame [extern_value value] E.collector_instance,
          to_e_list (modfunc_body evaluation_tree))
        (state, store, Build_frame [extern_value value] E.collector_instance,
          match value with Data (Object (TreeObj _)) => [] | _ => [AI_trap] end).
    Proof.
      intro Functions; rewrite evaluation_tree_body.
      set (suffix := to_e_list [BI_local_get 0%N; BI_call 1%N;
        BI_const_num (VAL_int32 (Wasm_int.int_of_Z i32m 1%Z));
        BI_relop T_i32 (Relop_i ROI_eq); BI_call 18%N]).
      change (reduce_trans
        (state, store, Build_frame [extern_value value] E.collector_instance,
          to_e_list [BI_local_get 0%N; BI_call 0%N; BI_testop T_i32 TO_eqz; BI_call 18%N] ++ suffix)
        (state, store, Build_frame [extern_value value] E.collector_instance,
          match value with Data (Object (TreeObj _)) => [] | _ => [AI_trap] end)).
      destruct value as [[object|reference]|thunk|encode].
      - eapply collector_guard_prefix with (accepted := true).
        + apply collector_type_guard_executes with (value := Data (Object object)); [exact Functions|reflexivity].
        + apply collector_tree_data_guard_executes; exact Functions.
      - eapply collector_guard_prefix with (accepted := false) (result := suffix);
          [apply collector_type_guard_executes with (value := Data (Ref reference)); [exact Functions|reflexivity]|apply rt_refl].
      - eapply collector_guard_prefix with (accepted := false) (result := suffix);
          [apply collector_type_guard_executes with (value := Thunk thunk); [exact Functions|reflexivity]|apply rt_refl].
      - eapply collector_guard_prefix with (accepted := false) (result := suffix);
          [apply collector_type_guard_executes with (value := Encode encode); [exact Functions|reflexivity]|apply rt_refl].
    Qed.

    (** Calling the native Tree guard accepts a Tree Object. *)
    Lemma collector_tree_executes state store locals value :
      s_funcs store = s_funcs E.allocated_store ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [extern_value value] ++ [AI_basic (BI_call 20%N)])
        (state, store, Build_frame locals E.collector_instance,
          match value with Data (Object (TreeObj _)) => [] | _ => [AI_trap] end).
    Proof.
      intro Functions.
      replace (match value with Data (Object (TreeObj _)) => [] | _ => [AI_trap] end)
        with (if match value with Data (Object (TreeObj _)) => true | _ => false end
              then [] else [AI_trap])
        by (destruct value as [[[]|reference]|thunk|encode]; reflexivity).
      eapply E.collector_call_void with (code := evaluation_tree)
        (parameters := [T_ref T_externref]) (defaults := [])
        (index := 20%N) (address := 20%N) (arguments := [extern_value value])
        (final_frame := Build_frame [extern_value value] E.collector_instance);
        try reflexivity.
      - rewrite Functions; reflexivity.
      - destruct value as [[[]|reference]|thunk|encode];
          apply collector_tree_body_executes; exact Functions.
    Qed.

    (** The size helper's type guard rejects a non-Tree Object. *)
    Lemma collector_size_non_tree state store value :
      s_funcs store = s_funcs E.allocated_store ->
      (forall tree, value <> Data (Object (TreeObj tree))) ->
      reduce_trans
        (state, store, Build_frame [extern_value value] E.collector_instance,
          to_e_list (modfunc_body evaluation_size))
        (state, store, Build_frame [extern_value value] E.collector_instance, [AI_trap]).
    Proof.
      intros Functions NotTree; rewrite evaluation_size_body.
      collector_execute_under E.collector_runs_context (@nil datatypes.value)
        (E.collector_local_get combination true handle_bytes state store [extern_value value] 0%N
          (extern_value value) eq_refl).
      eapply E.collector_trap_suffix with
        (instructions := v_to_e_list [extern_value value] ++ [AI_basic (BI_call 20%N)]).
      pose proof (collector_tree_executes state store [extern_value value] value Functions) as Guard.
      destruct value as [[[]|reference]|thunk|encode]; try exact Guard.
      exfalso; eapply NotTree; reflexivity.
    Qed.

    (** The native size call traps on a non-Tree Object. *)
    Lemma collector_size_rejects_non_tree state store locals value :
      s_funcs store = s_funcs E.allocated_store ->
      (forall tree, value <> Data (Object (TreeObj tree))) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [extern_value value] ++ [AI_basic (BI_call 21%N)])
        (state, store, Build_frame locals E.collector_instance, [AI_trap]).
    Proof.
      intros Functions NotTree.
      eapply E.collector_call with (code := evaluation_size)
        (parameters := [T_ref T_externref]) (results := [T_num T_i32]) (defaults := [])
        (index := 21%N) (address := 21%N) (arguments := [extern_value value])
        (values := [VAL_num (VAL_int32 (wasm_nat 0))]) (accepted := false)
        (final_frame := Build_frame [extern_value value] E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - apply collector_size_non_tree; assumption.
    Qed.

    (** The native length import reads a handle's modeled length from a local. *)
    Lemma collector_length_local state store locals local tree :
      s_funcs store = s_funcs E.allocated_store ->
      lookup_N locals local = Some (extern_value (Data (Object (TreeObj tree)))) ->
      (Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 64)%Z ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          [AI_basic (BI_local_get local); AI_basic (BI_call 6%N)])
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 (List.length (get_tree_raw tree))))]).
    Proof.
      intros Functions Local Bound.
      eapply collector_query_local with (operation := shell_len)
        (value := Data (Object (TreeObj tree)))
        (values := [VAL_num (VAL_int64 (wasm_nat64 (List.length (get_tree_raw tree))))]);
        try reflexivity; try exact Local.
      - rewrite Functions; reflexivity.
      - unfold E.H.host_values, extern_value; rewrite to_handle_to_externref.
        apply E.H.length_result_small; exact Bound.
    Qed.

    (** The size helper body returns the Tree length when it fits the native index bound. *)
    Lemma collector_size_body_executes state store tree :
      s_funcs store = s_funcs E.allocated_store ->
      (Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 64)%Z ->
      reduce_trans
        (state, store, Build_frame [extern_value (Data (Object (TreeObj tree)))] E.collector_instance,
          to_e_list (modfunc_body evaluation_size))
        (state, store, Build_frame [extern_value (Data (Object (TreeObj tree)))] E.collector_instance,
          if Z.ltb (Z.of_nat (List.length (get_tree_raw tree))) (2 ^ 32)%Z
          then v_to_e_list [VAL_num (VAL_int32 (wasm_nat (List.length (get_tree_raw tree))))] else [AI_trap]).
    Proof.
      intros Functions Wide.
      set (locals := [extern_value (Data (Object (TreeObj tree)))]).
      set (length := List.length (get_tree_raw tree)).
      rewrite evaluation_size_body.
      collector_execute_under E.collector_runs_context (@nil value)
        (E.collector_local_get combination true handle_bytes state store locals 0%N
          (extern_value (Data (Object (TreeObj tree)))) eq_refl).
      collector_execute_under E.collector_runs_context (@nil value)
        (collector_tree_executes state store locals (Data (Object (TreeObj tree))) Functions).
      collector_execute_under E.collector_runs_context (@nil value)
        (collector_length_local state store locals 0%N tree Functions eq_refl Wide).
      eapply rt_trans with (y := (state, store, Build_frame locals E.collector_instance,
        [v_to_e (VAL_num (VAL_int32 (wasm_bool (Z.ltb (Z.of_nat length) (2 ^ 32)%Z)))); AI_basic (BI_call 18%N);
         AI_basic (BI_local_get 0%N); AI_basic (BI_call 6%N); AI_basic (BI_cvtop T_i32 CVO_wrap T_i64 None)])).
      - pose proof (wasm_nat64_fits32 length Wide) as Fits.
        rewrite <- Fits.
        apply rt_step; eapply r_label with (lh := LH_base []
          [AI_basic (BI_call 18%N); AI_basic (BI_local_get 0%N); AI_basic (BI_call 6%N);
           AI_basic (BI_cvtop T_i32 CVO_wrap T_i64 None)]).
        + apply r_simple; apply rs_relop with (v1 := VAL_int64 (wasm_nat64 length))
            (v2 := VAL_int64 (Wasm_int.int_of_Z i64m 4294967295%Z))
            (t := T_i64) (op := Relop_i (ROI_le SX_U)); reflexivity.
        + reflexivity.
        + reflexivity.
      - destruct (Z.ltb (Z.of_nat length) (2 ^ 32)%Z) eqn:Fits.
        + collector_execute_under E.collector_runs_context (@nil value)
            (E.collector_require_executes combination true handle_bytes state store locals true
              ltac:(rewrite Functions; reflexivity)).
          collector_execute_under E.collector_runs_context (@nil value)
            (collector_length_local state store locals 0%N tree Functions eq_refl Wide).
          apply rt_step, r_simple; eapply rs_convert_success with
            (t1 := T_i64) (t2 := T_i32) (op := CVO_wrap) (sx := None)
            (v := VAL_int64 (wasm_nat64 length)) (v' := VAL_int32 (wasm_nat length));
            try reflexivity.
          apply wasm_nat64_wrap; exact Wide.
        + eapply E.collector_trap_suffix with
            (instructions := [v_to_e (VAL_num (VAL_int32 (wasm_bool false))); AI_basic (BI_call 18%N)]).
          apply E.collector_require_executes with (condition := false); rewrite Functions; reflexivity.
    Qed.

    (** The native size call returns a Tree's length when it fits the native index bound. *)
    Lemma collector_size_executes state store locals tree :
      s_funcs store = s_funcs E.allocated_store ->
      (Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 32)%Z ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [extern_value (Data (Object (TreeObj tree)))] ++ [AI_basic (BI_call 21%N)])
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [VAL_num (VAL_int32
            (E.H.W.i32_of_nat (E.H.Tables.T.O.data_length (Object (TreeObj tree)))))]).
    Proof.
      intros Functions Bound.
      eapply E.collector_call with (code := evaluation_size)
        (parameters := [T_ref T_externref]) (results := [T_num T_i32]) (defaults := [])
        (index := 21%N) (address := 21%N)
        (arguments := [extern_value (Data (Object (TreeObj tree)))])
        (values := [VAL_num (VAL_int32
          (E.H.W.i32_of_nat (E.H.Tables.T.O.data_length (Object (TreeObj tree)))))]) (accepted := true)
        (final_frame := Build_frame [extern_value (Data (Object (TreeObj tree)))] E.collector_instance);
        try reflexivity.
      - rewrite Functions; reflexivity.
      - pose proof (collector_size_body_executes state store tree Functions ltac:(cbn in Bound |- *; lia)) as Run.
        assert (Fits : Z.ltb (Z.of_nat (List.length (get_tree_raw tree))) (2 ^ 32)%Z = true)
          by (apply Z.ltb_lt; exact Bound).
        rewrite Fits in Run; exact Run.
    Qed.

    (** The native size call traps on a Tree too large for the native index range. *)
    Lemma collector_size_rejects_large state store locals tree :
      s_funcs store = s_funcs E.allocated_store ->
      (2 ^ 32 <= Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 64)%Z ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [extern_value (Data (Object (TreeObj tree)))] ++ [AI_basic (BI_call 21%N)])
        (state, store, Build_frame locals E.collector_instance, [AI_trap]).
    Proof.
      intros Functions [Large Wide].
      eapply E.collector_call with (code := evaluation_size)
        (parameters := [T_ref T_externref]) (results := [T_num T_i32]) (defaults := [])
        (index := 21%N) (address := 21%N) (arguments := [extern_value (Data (Object (TreeObj tree)))])
        (values := [VAL_num (VAL_int32 (wasm_nat 0))]) (accepted := false)
        (final_frame := Build_frame [extern_value (Data (Object (TreeObj tree)))] E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - pose proof (collector_size_body_executes state store tree Functions Wide) as Run.
        assert (Fits : Z.ltb (Z.of_nat (List.length (get_tree_raw tree))) (2 ^ 32)%Z = false)
          by (apply Z.ltb_ge; exact Large).
        rewrite Fits in Run; exact Run.
    Qed.

    (** The native size call traps when its input fails the required Tree shape or size
        bounds. *)
    Lemma collector_size_rejects_shape state store locals h :
      s_funcs store = s_funcs E.allocated_store ->
      (forall tree, h = Data (Object (TreeObj tree)) ->
        (2 ^ 32 <= Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 64)%Z) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [extern_value h] ++ [AI_basic (BI_call 21%N)])
        (state, store, Build_frame locals E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Invalid; destruct h as [[[]|reference]|thunk|encode].
      - apply collector_size_rejects_non_tree; [exact Functions|intros; discriminate].
      - apply collector_size_rejects_large; [exact Functions|apply Invalid; reflexivity].
      - apply collector_size_rejects_non_tree; [exact Functions|intros; discriminate].
      - apply collector_size_rejects_non_tree; [exact Functions|intros; discriminate].
      - apply collector_size_rejects_non_tree; [exact Functions|intros; discriminate].
    Qed.

    (** The arity helper body accepts a Tree with the requested bounded length. *)
    Lemma collector_arity_body_executes state store tree length :
      s_funcs store = s_funcs E.allocated_store ->
      (Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 32)%Z ->
      reduce_trans
        (state, store, Build_frame
          [extern_value (Data (Object (TreeObj tree))); VAL_num (VAL_int32 length)] E.collector_instance,
          to_e_list (modfunc_body evaluation_arity))
        (state, store, Build_frame
          [extern_value (Data (Object (TreeObj tree))); VAL_num (VAL_int32 length)] E.collector_instance,
          if app_relop (Relop_i ROI_eq)
            (VAL_int32 (E.H.W.i32_of_nat (E.H.Tables.T.O.data_length (Object (TreeObj tree)))))
            (VAL_int32 length) then [] else [AI_trap]).
    Proof.
      intros Functions Bound; rewrite evaluation_arity_body.
      set (frame := Build_frame
        [extern_value (Data (Object (TreeObj tree))); VAL_num (VAL_int32 length)] E.collector_instance).
      set (value := extern_value (Data (Object (TreeObj tree)))).
      set (word := VAL_int32 (E.H.W.i32_of_nat (E.H.Tables.T.O.data_length (Object (TreeObj tree))))).
      eapply rt_trans with (y := (state, store, frame,
        [v_to_e value; AI_basic (BI_call 21%N); AI_basic (BI_local_get 1%N);
         AI_basic (BI_relop T_i32 (Relop_i ROI_eq)); AI_basic (BI_call 18%N)])).
      - apply rt_step; eapply r_label with (lh := LH_base []
          [AI_basic (BI_call 21%N); AI_basic (BI_local_get 1%N);
           AI_basic (BI_relop T_i32 (Relop_i ROI_eq)); AI_basic (BI_call 18%N)]).
        + eapply r_local_get with (j := 0%N) (v := value); reflexivity.
        + reflexivity.
        + reflexivity.
      - eapply rt_trans with (y := (state, store, frame,
          [v_to_e (VAL_num word); AI_basic (BI_local_get 1%N);
           AI_basic (BI_relop T_i32 (Relop_i ROI_eq)); AI_basic (BI_call 18%N)])).
        + eapply E.collector_runs_context with (prefix := [])
            (suffix := [AI_basic (BI_local_get 1%N);
              AI_basic (BI_relop T_i32 (Relop_i ROI_eq)); AI_basic (BI_call 18%N)])
            (instructions := v_to_e_list [value] ++ [AI_basic (BI_call 21%N)])
            (instructions' := [v_to_e (VAL_num word)]).
          apply collector_size_executes; assumption.
        + eapply rt_trans with (y := (state, store, frame,
            [v_to_e (VAL_num word); v_to_e (VAL_num (VAL_int32 length));
             AI_basic (BI_relop T_i32 (Relop_i ROI_eq)); AI_basic (BI_call 18%N)])).
          * apply rt_step; eapply r_label with (lh := LH_base [VAL_num word]
              [AI_basic (BI_relop T_i32 (Relop_i ROI_eq)); AI_basic (BI_call 18%N)]).
            -- eapply r_local_get with (j := 1%N) (v := VAL_num (VAL_int32 length)); reflexivity.
            -- reflexivity.
            -- reflexivity.
          * eapply rt_trans with (y := (state, store, frame,
              [v_to_e (VAL_num (VAL_int32 (wasm_bool
                (app_relop (Relop_i ROI_eq) word (VAL_int32 length))))); AI_basic (BI_call 18%N)])).
            -- apply rt_step; eapply r_label with (lh := LH_base [] [AI_basic (BI_call 18%N)]).
               ++ apply r_simple; apply rs_relop with (v1 := word) (v2 := VAL_int32 length)
                    (t := T_i32) (op := Relop_i ROI_eq); reflexivity.
               ++ reflexivity.
               ++ reflexivity.
            -- apply E.collector_require_executes; rewrite Functions; reflexivity.
    Qed.

    (** The native arity call accepts a Tree with the requested bounded length. *)
    Lemma collector_arity_executes state store locals tree length :
      s_funcs store = s_funcs E.allocated_store ->
      (Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 32)%Z ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [extern_value (Data (Object (TreeObj tree))); VAL_num (VAL_int32 length)] ++
            [AI_basic (BI_call 22%N)])
        (state, store, Build_frame locals E.collector_instance,
          if app_relop (Relop_i ROI_eq)
            (VAL_int32 (E.H.W.i32_of_nat (E.H.Tables.T.O.data_length (Object (TreeObj tree)))))
            (VAL_int32 length) then [] else [AI_trap]).
    Proof.
      intros Functions Bound.
      eapply E.collector_call_void with (code := evaluation_arity)
        (parameters := [T_ref T_externref; T_num T_i32]) (defaults := [])
        (index := 22%N) (address := 22%N)
        (arguments := [extern_value (Data (Object (TreeObj tree))); VAL_num (VAL_int32 length)])
        (final_frame := Build_frame
          [extern_value (Data (Object (TreeObj tree))); VAL_num (VAL_int32 length)] E.collector_instance);
        try reflexivity.
      - rewrite Functions; reflexivity.
      - apply collector_arity_body_executes; assumption.
    Qed.

    (** The native arity call traps on a non-Tree Object. *)
    Lemma collector_arity_rejects_non_tree state store locals value length :
      s_funcs store = s_funcs E.allocated_store ->
      (forall tree, value <> Data (Object (TreeObj tree))) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [extern_value value; VAL_num (VAL_int32 length)] ++ [AI_basic (BI_call 22%N)])
        (state, store, Build_frame locals E.collector_instance, [AI_trap]).
    Proof.
      intros Functions NotTree.
      set (arguments := [extern_value value; VAL_num (VAL_int32 length)]).
      eapply E.collector_call_void with (code := evaluation_arity)
        (parameters := [T_ref T_externref; T_num T_i32]) (defaults := [])
        (index := 22%N) (address := 22%N) (arguments := arguments) (accepted := false)
        (final_frame := Build_frame arguments E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - rewrite evaluation_arity_body.
        collector_execute_under E.collector_runs_context (@nil datatypes.value)
          (E.collector_local_get combination true handle_bytes state store arguments 0%N
            (extern_value value) eq_refl).
        eapply E.collector_trap_suffix with
          (instructions := v_to_e_list [extern_value value] ++ [AI_basic (BI_call 21%N)]).
        apply collector_size_rejects_non_tree; assumption.
    Qed.

    (** The native arity call traps when the Tree length differs from the requested arity. *)
    Lemma collector_arity_rejects_length state store locals tree expected :
      s_funcs store = s_funcs E.allocated_store ->
      (Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 32)%Z -> (Z.of_nat expected < 2 ^ 32)%Z ->
      List.length (get_tree_raw tree) <> expected ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [extern_value (Data (Object (TreeObj tree))); VAL_num (VAL_int32 (wasm_nat expected))] ++
            [AI_basic (BI_call 22%N)])
        (state, store, Build_frame locals E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Bound Expected Different.
      pose proof (collector_arity_executes state store locals tree (wasm_nat expected) Functions Bound) as Run.
      change (reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [extern_value (Data (Object (TreeObj tree))); VAL_num (VAL_int32 (wasm_nat expected))] ++
            [AI_basic (BI_call 22%N)])
        (state, store, Build_frame locals E.collector_instance,
          if Wasm_int.int_eq i32m (wasm_nat (List.length (get_tree_raw tree))) (wasm_nat expected)
          then [] else [AI_trap])) in Run.
      rewrite (wasm_nat_equal _ _ Bound Expected) in Run.
      assert (Mismatch : Nat.eqb (List.length (get_tree_raw tree)) expected = false)
        by (apply Nat.eqb_neq; exact Different).
      rewrite Mismatch in Run; exact Run.
    Qed.

    (** The native arity call traps when the Tree exceeds the supported index bound. *)
    Lemma collector_arity_rejects_large state store locals tree expected :
      s_funcs store = s_funcs E.allocated_store ->
      (2 ^ 32 <= Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 64)%Z ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [extern_value (Data (Object (TreeObj tree))); VAL_num (VAL_int32 expected)] ++
            [AI_basic (BI_call 22%N)])
        (state, store, Build_frame locals E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Large.
      set (arguments := [extern_value (Data (Object (TreeObj tree))); VAL_num (VAL_int32 expected)]).
      eapply E.collector_call_void with (code := evaluation_arity)
        (parameters := [T_ref T_externref; T_num T_i32]) (defaults := [])
        (index := 22%N) (address := 22%N) (arguments := arguments) (accepted := false)
        (final_frame := Build_frame arguments E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - rewrite evaluation_arity_body.
        collector_execute_under E.collector_runs_context (@nil value)
          (E.collector_local_get combination true handle_bytes state store arguments 0%N
            (extern_value (Data (Object (TreeObj tree)))) eq_refl).
        eapply E.collector_trap_suffix with
          (instructions := v_to_e_list [extern_value (Data (Object (TreeObj tree)))] ++ [AI_basic (BI_call 21%N)]).
        apply collector_size_rejects_large; assumption.
    Qed.

    (** The native arity call traps on an invalid Tree shape or unsupported size. *)
    Lemma collector_arity_rejects_shape state store locals h expected :
      s_funcs store = s_funcs E.allocated_store -> (Z.of_nat expected < 2 ^ 32)%Z ->
      (forall tree, h = Data (Object (TreeObj tree)) ->
        (Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 64)%Z /\ List.length (get_tree_raw tree) <> expected) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [extern_value h; VAL_num (VAL_int32 (wasm_nat expected))] ++ [AI_basic (BI_call 22%N)])
        (state, store, Build_frame locals E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Expected Invalid; destruct h as [[[blob|tree]|reference]|thunk|encode].
      - apply collector_arity_rejects_non_tree; [exact Functions|intros; discriminate].
      - destruct (Invalid _ eq_refl) as [Wide Different].
        destruct (Z.ltb (Z.of_nat (List.length (get_tree_raw tree))) (2 ^ 32)%Z) eqn:Fits.
        + apply collector_arity_rejects_length; try assumption; apply Z.ltb_lt; exact Fits.
        + apply collector_arity_rejects_large; [exact Functions|split; [apply Z.ltb_ge; exact Fits|exact Wide]].
      - apply collector_arity_rejects_non_tree; [exact Functions|intros; discriminate].
      - apply collector_arity_rejects_non_tree; [exact Functions|intros; discriminate].
      - apply collector_arity_rejects_non_tree; [exact Functions|intros; discriminate].
    Qed.

    (** The identity-check helper body succeeds when privileged byte observations compare
        equal. *)
    Lemma collector_same_body_executes state store lhs rhs lhs_bytes rhs_bytes equal :
      s_funcs store = s_funcs E.allocated_store ->
      handle_bytes state lhs = Some lhs_bytes ->
      handle_bytes (Datatypes.S state) rhs = Some rhs_bytes ->
      E.H.Tables.T.O.compare_handles (HBlobObj lhs_bytes) (HBlobObj rhs_bytes) = Some equal ->
      reduce_trans
        (state, store, Build_frame [extern_value lhs; extern_value rhs] E.collector_instance,
          to_e_list (modfunc_body evaluation_same))
        (Datatypes.S (Datatypes.S state), store,
          Build_frame [extern_value lhs; extern_value rhs] E.collector_instance,
          if equal then [] else [AI_trap]).
    Proof.
      intros Functions Left Right Compared; rewrite evaluation_same_body.
      set (frame := Build_frame [extern_value lhs; extern_value rhs] E.collector_instance).
      set (a := extern_value lhs); set (b := extern_value rhs).
      set (bytes_a := extern_value (HBlobObj lhs_bytes)).
      set (bytes_b := extern_value (HBlobObj rhs_bytes)).
      eapply rt_trans with (y := (state, store, frame,
        [v_to_e a; AI_basic (BI_call 4%N); AI_basic (BI_local_get 1%N);
         AI_basic (BI_call 4%N); AI_basic (BI_call 5%N); AI_basic (BI_call 18%N)])).
      - apply rt_step; eapply r_label with
          (lh := LH_base [] [AI_basic (BI_call 4%N); AI_basic (BI_local_get 1%N);
            AI_basic (BI_call 4%N); AI_basic (BI_call 5%N); AI_basic (BI_call 18%N)]).
        + eapply r_local_get with (j := 0%N) (v := a); reflexivity.
        + reflexivity.
        + reflexivity.
      - eapply rt_trans with (y := (Datatypes.S state, store, frame,
          [v_to_e bytes_a; AI_basic (BI_local_get 1%N); AI_basic (BI_call 4%N);
           AI_basic (BI_call 5%N); AI_basic (BI_call 18%N)])).
        + eapply E.collector_runs_context with (prefix := [])
            (suffix := [AI_basic (BI_local_get 1%N); AI_basic (BI_call 4%N);
              AI_basic (BI_call 5%N); AI_basic (BI_call 18%N)])
            (instructions := v_to_e_list [a] ++ [AI_basic (BI_call 4%N)])
            (instructions' := v_to_e_list [bytes_a]).
          eapply E.collector_import_executes with (operation := shell_handle_bytes)
            (index := 4%N) (address := 4%N) (arguments := [a])
            (result := result_values [bytes_a]); try reflexivity.
          * rewrite Functions; reflexivity.
          * eapply E.H.handle_bytes_result; [reflexivity|exact Left].
        + eapply rt_trans with (y := (Datatypes.S state, store, frame,
            [v_to_e bytes_a; v_to_e b; AI_basic (BI_call 4%N);
             AI_basic (BI_call 5%N); AI_basic (BI_call 18%N)])).
          * apply rt_step; eapply r_label with
              (lh := LH_base [bytes_a] [AI_basic (BI_call 4%N);
                AI_basic (BI_call 5%N); AI_basic (BI_call 18%N)]).
            -- eapply r_local_get with (j := 1%N) (v := b); reflexivity.
            -- reflexivity.
            -- reflexivity.
          * eapply rt_trans with (y := (Datatypes.S (Datatypes.S state), store, frame,
              [v_to_e bytes_a; v_to_e bytes_b; AI_basic (BI_call 5%N); AI_basic (BI_call 18%N)])).
            -- eapply E.collector_runs_context with (prefix := [bytes_a])
                (suffix := [AI_basic (BI_call 5%N); AI_basic (BI_call 18%N)])
                (instructions := v_to_e_list [b] ++ [AI_basic (BI_call 4%N)])
                (instructions' := v_to_e_list [bytes_b]).
               eapply E.collector_import_executes with (operation := shell_handle_bytes)
                 (index := 4%N) (address := 4%N) (arguments := [b])
                 (result := result_values [bytes_b]); try reflexivity.
               ++ rewrite Functions; reflexivity.
               ++ eapply E.H.handle_bytes_result; [reflexivity|exact Right].
            -- eapply rt_trans with (y := (Datatypes.S (Datatypes.S state), store, frame,
                 [v_to_e (VAL_num (VAL_int32 (wasm_bool equal))); AI_basic (BI_call 18%N)])).
               ++ eapply E.collector_runs_context with (prefix := []) (suffix := [AI_basic (BI_call 18%N)])
                    (instructions := v_to_e_list [bytes_a; bytes_b] ++ [AI_basic (BI_call 5%N)])
                    (instructions' := [v_to_e (VAL_num (VAL_int32 (wasm_bool equal)))]).
                  eapply E.collector_import_executes with (operation := shell_equals)
                    (index := 5%N) (address := 5%N) (arguments := [bytes_a; bytes_b])
                    (result := result_values [VAL_num (VAL_int32 (wasm_bool equal))]); try reflexivity.
                  ** rewrite Functions; reflexivity.
                  ** eapply E.H.compare_blobs_result; exact Compared.
               ++ eapply E.collector_require_executes with (condition := equal).
                  rewrite Functions; reflexivity.
    Qed.

    (** The native identity-check call succeeds when privileged byte observations compare
        equal. *)
    Lemma collector_same_executes state store locals lhs rhs lhs_bytes rhs_bytes equal :
      s_funcs store = s_funcs E.allocated_store ->
      handle_bytes state lhs = Some lhs_bytes ->
      handle_bytes (Datatypes.S state) rhs = Some rhs_bytes ->
      E.H.Tables.T.O.compare_handles (HBlobObj lhs_bytes) (HBlobObj rhs_bytes) = Some equal ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [extern_value lhs; extern_value rhs] ++ [AI_basic (BI_call 19%N)])
        (Datatypes.S (Datatypes.S state), store, Build_frame locals E.collector_instance,
          if equal then [] else [AI_trap]).
    Proof.
      intros Functions Left Right Compared.
      eapply E.collector_call_void with (code := evaluation_same)
        (parameters := [T_ref T_externref; T_ref T_externref]) (defaults := [])
        (index := 19%N) (address := 19%N) (arguments := [extern_value lhs; extern_value rhs])
        (accepted := equal) (final_frame := Build_frame [extern_value lhs; extern_value rhs] E.collector_instance);
        try reflexivity.
      - rewrite Functions; reflexivity.
      - eapply collector_same_body_executes; eassumption.
    Qed.

    (** The native Tree lookup returns the requested in-bounds child. *)
    Lemma collector_tree_element_executes state store locals value index :
      s_funcs store = s_funcs E.allocated_store ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [extern_value value; VAL_num (VAL_int32 index)] ++ [AI_basic (BI_call 7%N)])
        (state, store, Build_frame locals E.collector_instance,
          match E.H.W.Api.get_tree_data_api value (E.H.W.nat_of_i32 index) with
          | Some child => v_to_e_list [extern_value child]
          | None => [AI_trap] end).
    Proof.
      intro Functions.
      replace (match E.H.W.Api.get_tree_data_api value (E.H.W.nat_of_i32 index) with
        | Some child => v_to_e_list [extern_value child] | None => [AI_trap] end)
        with (result_to_stack
          (match E.H.W.Api.get_tree_data_api value (E.H.W.nat_of_i32 index) with
           | Some child => result_values [extern_value child] | None => result_trap end))
        by (destruct (E.H.W.Api.get_tree_data_api value (E.H.W.nat_of_i32 index)); reflexivity).
      eapply E.collector_import_executes with (index := 7%N) (address := 7%N)
        (operation := shell_get_tree_element)
        (arguments := [extern_value value; VAL_num (VAL_int32 index)])
        (result := match E.H.W.Api.get_tree_data_api value (E.H.W.nat_of_i32 index) with
          | Some child => result_values [extern_value child] | None => result_trap end);
        try reflexivity.
      - rewrite Functions; reflexivity.
      - unfold E.H.host_result.
        destruct (function_type_eq_dec _ _); [|contradiction].
        destruct (shell_function_eq_dec _ _); [discriminate|].
        unfold E.H.host_values, extern_value; rewrite to_handle_to_externref.
        destruct (E.H.W.Api.get_tree_data_api value (E.H.W.nat_of_i32 index)); reflexivity.
    Qed.

    (** The Object-authentication helper body accepts a supplied Object whose Ref matches
        the expected handle. *)
    Lemma collector_object_body_executes state store reference object reference_bytes object_bytes equal :
      s_funcs store = s_funcs E.allocated_store ->
      handle_bytes state reference = Some reference_bytes ->
      handle_bytes (Datatypes.S state) (Data (Ref (E.H.W.H.data_reference (Object object)))) =
        Some object_bytes ->
      E.H.Tables.T.O.compare_handles (HBlobObj reference_bytes) (HBlobObj object_bytes) = Some equal ->
      reduce_trans
        (state, store, Build_frame [extern_value reference; extern_value (Data (Object object))]
          E.collector_instance, to_e_list (modfunc_body evaluation_object))
        (Datatypes.S (Datatypes.S state), store,
          Build_frame [extern_value reference; extern_value (Data (Object object))] E.collector_instance,
          if equal then [] else [AI_trap]).
    Proof.
      intros Functions Reference ObjectBytes Compared; rewrite evaluation_object_body.
      set (frame := Build_frame [extern_value reference; extern_value (Data (Object object))]
        E.collector_instance).
      set (result := Data (Ref (E.H.W.H.data_reference (Object object)))).
      eapply rt_trans with (y := (state, store, frame,
        to_e_list [BI_local_get 0%N; BI_local_get 1%N; BI_call 11%N; BI_call 19%N])).
      - eapply E.collector_runs_context with (prefix := [])
          (suffix := to_e_list [BI_local_get 0%N; BI_local_get 1%N; BI_call 11%N; BI_call 19%N])
          (instructions := to_e_list
            [BI_local_get 1%N; BI_call 0%N; BI_testop T_i32 TO_eqz; BI_call 18%N])
          (instructions' := []).
        apply collector_type_guard_executes with (value := Data (Object object));
          [exact Functions|reflexivity].
      - eapply rt_trans with (y := (state, store, frame,
          [v_to_e (extern_value reference); AI_basic (BI_local_get 1%N);
           AI_basic (BI_call 11%N); AI_basic (BI_call 19%N)])).
        + apply rt_step; eapply r_label with (lh := LH_base []
            [AI_basic (BI_local_get 1%N); AI_basic (BI_call 11%N); AI_basic (BI_call 19%N)]).
          * eapply r_local_get with (j := 0%N) (v := extern_value reference); reflexivity.
          * reflexivity.
          * reflexivity.
        + eapply rt_trans with (y := (state, store, frame,
            [v_to_e (extern_value reference); v_to_e (extern_value result); AI_basic (BI_call 19%N)])).
          * eapply E.collector_runs_context with (prefix := [extern_value reference])
              (suffix := [AI_basic (BI_call 19%N)])
              (instructions := [AI_basic (BI_local_get 1%N); AI_basic (BI_call 11%N)])
              (instructions' := [v_to_e (extern_value result)]).
            eapply collector_query_local with (operation := shell_create_ref)
              (value := Data (Object object)) (values := [extern_value result]); try reflexivity.
            -- rewrite Functions; reflexivity.
            -- unfold E.H.host_values, extern_value; rewrite to_handle_to_externref; reflexivity.
          * eapply collector_same_executes; eassumption.
    Qed.

    (** The native Object-authentication call accepts the supplied Object when its Ref
        matches. *)
    Lemma collector_object_executes state store locals reference object reference_bytes object_bytes equal :
      s_funcs store = s_funcs E.allocated_store ->
      handle_bytes state reference = Some reference_bytes ->
      handle_bytes (Datatypes.S state) (Data (Ref (E.H.W.H.data_reference (Object object)))) =
        Some object_bytes ->
      E.H.Tables.T.O.compare_handles (HBlobObj reference_bytes) (HBlobObj object_bytes) = Some equal ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [extern_value reference; extern_value (Data (Object object))] ++ [AI_basic (BI_call 25%N)])
        (Datatypes.S (Datatypes.S state), store, Build_frame locals E.collector_instance,
          if equal then [] else [AI_trap]).
    Proof.
      intros Functions Reference ObjectBytes Compared.
      eapply E.collector_call_void with (code := evaluation_object)
        (parameters := [T_ref T_externref; T_ref T_externref]) (defaults := [])
        (index := 25%N) (address := 25%N)
        (arguments := [extern_value reference; extern_value (Data (Object object))]) (accepted := equal)
        (final_frame := Build_frame [extern_value reference; extern_value (Data (Object object))]
          E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - eapply collector_object_body_executes; eassumption.
    Qed.

    (** The native Object-authentication call traps when its check fails. *)
    Lemma collector_object_rejects state store locals reference value :
      s_funcs store = s_funcs E.allocated_store ->
      (forall object, value <> Data (Object object)) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [extern_value reference; extern_value value] ++ [AI_basic (BI_call 25%N)])
        (state, store, Build_frame locals E.collector_instance, [AI_trap]).
    Proof.
      intros Functions NotObject.
      eapply E.collector_call_void with (code := evaluation_object)
        (parameters := [T_ref T_externref; T_ref T_externref]) (defaults := [])
        (index := 25%N) (address := 25%N)
        (arguments := [extern_value reference; extern_value value]) (accepted := false)
        (final_frame := Build_frame [extern_value reference; extern_value value] E.collector_instance);
        try reflexivity.
      - rewrite Functions; reflexivity.
      - rewrite evaluation_object_body.
        set (suffix := to_e_list [BI_local_get 0%N; BI_local_get 1%N; BI_call 11%N; BI_call 19%N]).
        eapply collector_guard_prefix with (accepted := false) (result := suffix)
          (instructions := to_e_list
            [BI_local_get 1%N; BI_call 0%N; BI_testop T_i32 TO_eqz; BI_call 18%N])
          (suffix := suffix); [|apply rt_refl].
        pose proof (collector_type_guard_executes state store
          [extern_value reference; extern_value value] 1%N value Functions eq_refl) as Guard.
        destruct value as [[object|ref]|thunk|encode]; try exact Guard.
        exfalso; apply (NotObject object); reflexivity.
    Qed.

    (** Equal bytes returned by privileged handle inspection identify exactly the same
        abstract handle. *)
    Hypothesis bytes_identify : forall i j lhs rhs lhs_bytes rhs_bytes,
      handle_bytes i lhs = Some lhs_bytes -> handle_bytes j rhs = Some rhs_bytes ->
      get_blob_data lhs_bytes = get_blob_data rhs_bytes -> lhs = rhs.

    (** A successful privileged-byte comparison proves exact identity of the compared
        handles. *)
    Lemma collector_same_comparison_sound state lhs rhs lhs_bytes rhs_bytes :
      handle_bytes state lhs = Some lhs_bytes ->
      handle_bytes (Datatypes.S state) rhs = Some rhs_bytes ->
      E.H.Tables.T.O.compare_handles (HBlobObj lhs_bytes) (HBlobObj rhs_bytes) = Some true ->
      Matching.handle_eqb lhs rhs = true.
    Proof.
      intros Left Right Compared; apply Matching.handle_eqb_spec.
      eapply bytes_identify; [exact Left|exact Right|].
      apply E.H.Tables.T.O.compare_handles_equal in Compared as [_ [_ Related]].
      apply E.H.Tables.T.O.indistinguishable_blobs in Related; exact Related.
    Qed.
    (** A successful Object witness comparison proves that the supplied Ref names that
        Object. *)
    Lemma collector_object_comparison_sound state reference object reference_bytes object_bytes :
      handle_bytes state reference = Some reference_bytes ->
      handle_bytes (Datatypes.S state) (Data (Ref (E.H.W.H.data_reference (Object object)))) =
        Some object_bytes ->
      E.H.Tables.T.O.compare_handles (HBlobObj reference_bytes) (HBlobObj object_bytes) = Some true ->
      reference = Data (Ref (E.H.W.H.data_reference (Object object))).
    Proof.
      intros Reference ObjectBytes Compared.
      apply Matching.handle_eqb_spec.
      eapply collector_same_comparison_sound; eassumption.
    Qed.
  End Execution.
End WasmGuards.
