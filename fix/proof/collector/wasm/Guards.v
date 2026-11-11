(** Proves execution of the collector guards for Tree shape, length,
    reference authentication, and exact handle comparison. *)

From Wasm Require Import datatypes operations opsem.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Collector Require Import Matching.
From FixProof.Shell Require Import ABI Host.
From FixProof.Collector.wasm Require Import Binary Layout Execution.
From FixProof.Support Require Import WasmNatural.
From Stdlib Require Import List NArith ZArith Lia Relation_Operators.
Import ListNotations.
Local Open Scope list_scope.

Definition evaluation_tree :=
  nth 2 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

(** The parsed Tree guard has the expected type-checking body. *)
Lemma evaluation_tree_body : modfunc_body evaluation_tree =
  [BI_local_get 0%N; BI_call 0%N; BI_testop T_i32 TO_eqz; BI_call 22%N;
   BI_local_get 0%N; BI_call 1%N; BI_const_num (VAL_int32 (Wasm_int.int_of_Z i32m 1%Z));
   BI_relop T_i32 (Relop_i ROI_eq); BI_call 22%N].
Proof. reflexivity. Qed.

Definition evaluation_size :=
  nth 3 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).
Definition evaluation_arity :=
  nth 4 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

(** The parsed size helper has the expected Tree and index-bound checks. *)
Lemma evaluation_size_body : modfunc_body evaluation_size =
  [BI_local_get 0%N; BI_call 24%N; BI_local_get 0%N; BI_call 6%N;
   BI_const_num (VAL_int64 (Wasm_int.int_of_Z i64m 134217729%Z));
   BI_relop T_i64 (Relop_i (ROI_lt SX_U)); BI_call 22%N;
   BI_local_get 0%N; BI_call 6%N; BI_cvtop T_i32 CVO_wrap T_i64 None].
Proof. reflexivity. Qed.
(** The parsed arity helper has the expected size-equality guard. *)
Lemma evaluation_arity_body : modfunc_body evaluation_arity =
  [BI_local_get 0%N; BI_call 25%N; BI_local_get 1%N;
   BI_relop T_i32 (Relop_i ROI_eq); BI_call 22%N].
Proof. reflexivity. Qed.

Definition evaluation_same :=
  nth 1 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

(** The parsed identity helper has the expected privileged-byte comparison body. *)
Lemma evaluation_same_body : modfunc_body evaluation_same =
  [BI_local_get 0%N; BI_local_get 1%N; BI_call 4%N; BI_call 22%N].
Proof. reflexivity. Qed.

Module WasmGuards (S : STORAGE) (P : PROGRAM S) .
  Module Matching := CouponMatching S.
  Module E := WasmExecution S P.
  Import S.
  Local Notation HBlobObj := E.H.H.HBlobObj.

  Section Execution.
    Context `{mem : BlockUpdateMemory}.
    Variable combination : S.TreeName.
    Variable handle_bytes : nat -> S.handle -> option S.BlobName.
    Context {address : Wasm_int.Int32.int}.
    Definition descriptor_value := VAL_num (VAL_int32 address).
    Local Instance collector_host : host :=
      E.H.shell_host combination E.collector_instance true true handle_bytes.

    (** The modeled query import returns the requested property of a handle stored in a
        local. *)
    Lemma collector_query_local state store locals local index operation values :
      lookup_N (s_funcs store) index =
        Some (FC_func_host (shell_signature operation) operation) ->
      lookup_N (inst_funcs E.collector_instance) index = Some index ->
      lookup_N locals local = Some (descriptor_value) ->
      (match shell_signature operation with Tf parameters _ =>
        parameters = [T_num T_i32] end) ->
      E.H.host_values combination E.collector_instance true true handle_bytes state store
        operation [descriptor_value] = Some (state, values) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          [AI_basic (BI_local_get local); AI_basic (BI_call index)])
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list values).
    Proof.
      intros Closure Index Local Signature Values.
      eapply rt_trans with (y := (state, store, Build_frame locals E.collector_instance,
        [v_to_e (descriptor_value); AI_basic (BI_call index)])).
      - apply rt_step; eapply r_label with (lh := LH_base [] [AI_basic (BI_call index)]).
        + eapply r_local_get; exact Local.
        + reflexivity.
        + reflexivity.
      - eapply E.collector_import_executes with (address := index) (operation := operation)
          (arguments := [descriptor_value]) (result := result_values values);
          try exact Closure; try exact Index.
        + destruct (shell_signature operation); cbn in Signature; rewrite Signature; reflexivity.
        + apply E.H.host_values_result; [destruct operation; cbn in Signature; congruence|exact Values].
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
      lookup_N locals local = Some (descriptor_value) ->
      E.H.resolve state address = Some (value) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          to_e_list [BI_local_get local; BI_call 0%N; BI_testop T_i32 TO_eqz; BI_call 22%N])
        (state, store, Build_frame locals E.collector_instance,
          match value with Data (Object _) => [] | _ => [AI_trap] end).
    Proof.
      intros Functions Local Live.
      eapply rt_trans with (y := (state, store,
        Build_frame locals E.collector_instance,
        [v_to_e (VAL_num (VAL_int32 (E.H.i32_of_nat (E.H.H.value_type value))));
         AI_basic (BI_testop T_i32 TO_eqz); AI_basic (BI_call 22%N)])).
      - eapply E.collector_runs_context with (prefix := [])
          (suffix := [AI_basic (BI_testop T_i32 TO_eqz); AI_basic (BI_call 22%N)])
          (instructions := [AI_basic (BI_local_get local); AI_basic (BI_call 0%N)])
          (instructions' := [v_to_e (VAL_num (VAL_int32
            (E.H.i32_of_nat (E.H.H.value_type value))))]).
        eapply collector_query_local with (operation := shell_get_type)
          (values := [VAL_num (VAL_int32 (E.H.i32_of_nat (E.H.H.value_type value)))]);
          try reflexivity; try exact Local.
        + rewrite Functions; reflexivity.
        + unfold E.H.host_values, descriptor_value; rewrite Live; reflexivity.
      - eapply rt_trans with (y := (state, store,
          Build_frame locals E.collector_instance,
          [v_to_e (VAL_num (VAL_int32 (wasm_bool (app_testop_i i32m TO_eqz
            (E.H.i32_of_nat (E.H.H.value_type value)))))); AI_basic (BI_call 22%N)])).
        + apply rt_step; eapply r_label with (lh := LH_base [] [AI_basic (BI_call 22%N)]);
            [apply r_simple, rs_testop_i32|reflexivity|reflexivity].
        + destruct value as [[object|reference]|thunk|encode];
            first [apply E.collector_require_executes with (condition := true)
                  |apply E.collector_require_executes with (condition := false)];
            rewrite Functions; reflexivity.
      all: try assumption.
    Qed.

    (** The native Tree guard accepts Tree Objects. *)
    Lemma collector_tree_data_guard_executes state store object :
      s_funcs store = s_funcs E.allocated_store ->
      E.H.resolve state address = Some (Data (Object object)) ->
      reduce_trans
        (state, store, Build_frame [descriptor_value] E.collector_instance,
          to_e_list [BI_local_get 0%N; BI_call 1%N;
            BI_const_num (VAL_int32 (Wasm_int.int_of_Z i32m 1%Z));
            BI_relop T_i32 (Relop_i ROI_eq); BI_call 22%N])
        (state, store, Build_frame [descriptor_value] E.collector_instance,
          match object with TreeObj _ => [] | BlobObj _ => [AI_trap] end).
    Proof.
      intros Functions Live.
      set (kind := match object with BlobObj _ => 0 | TreeObj _ => 1 end).
      set (word := VAL_int32 (E.H.i32_of_nat kind)).
      set (one := VAL_int32 (Wasm_int.int_of_Z i32m 1%Z)).
      eapply rt_trans with (y := (state, store,
        Build_frame [descriptor_value] E.collector_instance,
        [v_to_e (VAL_num word); v_to_e (VAL_num one);
         AI_basic (BI_relop T_i32 (Relop_i ROI_eq)); AI_basic (BI_call 22%N)])).
      - eapply E.collector_runs_context with (prefix := [])
          (suffix := [v_to_e (VAL_num one); AI_basic (BI_relop T_i32 (Relop_i ROI_eq));
            AI_basic (BI_call 22%N)])
          (instructions := [AI_basic (BI_local_get 0%N); AI_basic (BI_call 1%N)])
          (instructions' := [v_to_e (VAL_num word)]).
        eapply collector_query_local with (operation := shell_get_data_type)
           (values := [VAL_num word]); try reflexivity.
        + rewrite Functions; reflexivity.
        + unfold E.H.host_values, descriptor_value; rewrite Live.
          destruct object; reflexivity.
      - eapply rt_trans with (y := (state, store,
          Build_frame [descriptor_value] E.collector_instance,
          [v_to_e (VAL_num (VAL_int32 (wasm_bool
            (app_relop (Relop_i ROI_eq) word one)))); AI_basic (BI_call 22%N)])).
        + apply rt_step; eapply r_label with (lh := LH_base [] [AI_basic (BI_call 22%N)]).
          * apply r_simple; apply rs_relop with (v1 := word) (v2 := one)
              (t := T_i32) (op := Relop_i ROI_eq); reflexivity.
          * reflexivity.
          * reflexivity.
        + destruct object;
            first [apply E.collector_require_executes with (condition := true)
                  |apply E.collector_require_executes with (condition := false)];
            rewrite Functions; reflexivity.
      all: try assumption.
    Qed.

    (** The body of the native Tree guard accepts a Tree Object. *)
    Lemma collector_tree_body_executes state store value :
      s_funcs store = s_funcs E.allocated_store ->
      E.H.resolve state address = Some (value) ->
      reduce_trans
        (state, store, Build_frame [descriptor_value] E.collector_instance,
          to_e_list (modfunc_body evaluation_tree))
        (state, store, Build_frame [descriptor_value] E.collector_instance,
          match value with Data (Object (TreeObj _)) => [] | _ => [AI_trap] end).
    Proof.
      intros Functions Live; rewrite evaluation_tree_body.
      set (suffix := to_e_list [BI_local_get 0%N; BI_call 1%N;
        BI_const_num (VAL_int32 (Wasm_int.int_of_Z i32m 1%Z));
        BI_relop T_i32 (Relop_i ROI_eq); BI_call 22%N]).
      change (reduce_trans
        (state, store, Build_frame [descriptor_value] E.collector_instance,
          to_e_list [BI_local_get 0%N; BI_call 0%N; BI_testop T_i32 TO_eqz; BI_call 22%N] ++ suffix)
        (state, store, Build_frame [descriptor_value] E.collector_instance,
          match value with Data (Object (TreeObj _)) => [] | _ => [AI_trap] end)).
      destruct value as [[object|reference]|thunk|encode].
      - eapply collector_guard_prefix with (accepted := true).
        + eapply collector_type_guard_executes with (value := Data (Object object)); [exact Functions|reflexivity|exact Live].
        + eapply collector_tree_data_guard_executes; assumption.
      - eapply collector_guard_prefix with (accepted := false) (result := suffix);
          [eapply collector_type_guard_executes with (value := Data (Ref reference)); [exact Functions|reflexivity|exact Live]|apply rt_refl].
      - eapply collector_guard_prefix with (accepted := false) (result := suffix);
          [eapply collector_type_guard_executes with (value := Thunk thunk); [exact Functions|reflexivity|exact Live]|apply rt_refl].
      - eapply collector_guard_prefix with (accepted := false) (result := suffix);
          [eapply collector_type_guard_executes with (value := Encode encode); [exact Functions|reflexivity|exact Live]|apply rt_refl].
      all: try assumption.
    Qed.

    (** Calling the native Tree guard accepts a Tree Object. *)
    Lemma collector_tree_executes state store locals value :
      s_funcs store = s_funcs E.allocated_store ->
      E.H.resolve state address = Some (value) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [descriptor_value] ++ [AI_basic (BI_call 24%N)])
        (state, store, Build_frame locals E.collector_instance,
          match value with Data (Object (TreeObj _)) => [] | _ => [AI_trap] end).
    Proof.
      intros Functions Live.
      replace (match value with Data (Object (TreeObj _)) => [] | _ => [AI_trap] end)
        with (if match value with Data (Object (TreeObj _)) => true | _ => false end
              then [] else [AI_trap])
        by (destruct value as [[[]|reference]|thunk|encode]; reflexivity).
      eapply E.collector_call_void with (code := evaluation_tree)
        (parameters := [T_num T_i32]) (defaults := [])
        (index := 24%N) (address := 24%N) (arguments := [descriptor_value])
        (final_frame := Build_frame [descriptor_value] E.collector_instance);
        try reflexivity.
      - rewrite Functions; reflexivity.
      - pose proof (collector_tree_body_executes state store value Functions Live) as Body.
        destruct value as [[[]|reference]|thunk|encode]; exact Body.
      all: try assumption.
    Qed.

    (** The size helper's type guard rejects a non-Tree Object. *)
    Lemma collector_size_non_tree state store value :
      s_funcs store = s_funcs E.allocated_store ->
      (forall tree, value <> Data (Object (TreeObj tree))) ->
      E.H.resolve state address = Some (value) ->
      reduce_trans
        (state, store, Build_frame [descriptor_value] E.collector_instance,
          to_e_list (modfunc_body evaluation_size))
        (state, store, Build_frame [descriptor_value] E.collector_instance, [AI_trap]).
    Proof.
      intros Functions NotTree Live; rewrite evaluation_size_body.
      collector_execute_under E.collector_runs_context (@nil datatypes.value)
        (E.collector_local_get combination true true handle_bytes state store [descriptor_value] 0%N
          (descriptor_value) eq_refl).
      eapply E.collector_trap_suffix with
        (instructions := v_to_e_list [descriptor_value] ++ [AI_basic (BI_call 24%N)]).
      pose proof (collector_tree_executes state store [descriptor_value] value Functions Live) as Guard.
      destruct value as [[[]|reference]|thunk|encode]; try exact Guard.
      exfalso; eapply NotTree; reflexivity.
      all: try assumption.
    Qed.

    (** The native size call traps on a non-Tree Object. *)
    Lemma collector_size_rejects_non_tree state store locals value :
      s_funcs store = s_funcs E.allocated_store ->
      (forall tree, value <> Data (Object (TreeObj tree))) ->
      E.H.resolve state address = Some (value) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [descriptor_value] ++ [AI_basic (BI_call 25%N)])
        (state, store, Build_frame locals E.collector_instance, [AI_trap]).
    Proof.
      intros Functions NotTree Live.
      eapply E.collector_call with (code := evaluation_size)
        (parameters := [T_num T_i32]) (results := [T_num T_i32]) (defaults := [])
        (index := 25%N) (address := 25%N) (arguments := [descriptor_value])
        (values := [VAL_num (VAL_int32 (wasm_nat 0))]) (accepted := false)
        (final_frame := Build_frame [descriptor_value] E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - eapply collector_size_non_tree with (value := value); assumption.
      all: try assumption.
    Qed.

    (** The native length import reads a handle's modeled length from a local. *)
    Lemma collector_length_local state store locals local tree :
      s_funcs store = s_funcs E.allocated_store ->
      lookup_N locals local = Some (descriptor_value) ->
      (Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 64)%Z ->
      E.H.resolve state address = Some (E.H.H.HTreeObj tree) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          [AI_basic (BI_local_get local); AI_basic (BI_call 6%N)])
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 (List.length (get_tree_raw tree))))]).
    Proof.
      intros Functions Local Bound Live.
      eapply collector_query_local with (operation := shell_len)
        (values := [VAL_num (VAL_int64 (wasm_nat64 (List.length (get_tree_raw tree))))]);
        try reflexivity; try exact Local.
      - rewrite Functions; reflexivity.
      - unfold E.H.host_values, descriptor_value; rewrite Live.
        change (E.H.length_result state (List.length (get_tree_raw tree)) =
          Some (state, [VAL_num (VAL_int64 (wasm_nat64 (List.length (get_tree_raw tree))))])).
        unfold E.H.length_result.
        assert (Fits : Z.ltb (Z.of_nat (List.length (get_tree_raw tree))) (2 ^ 64) = true)
          by (apply Z.ltb_lt; exact Bound).
        rewrite Fits; reflexivity.
      all: try assumption.
    Qed.

    (** The size helper body returns the Tree length when it fits the native index bound. *)
    Lemma collector_size_body_executes state store tree :
      s_funcs store = s_funcs E.allocated_store ->
      (Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 64)%Z ->
      E.H.resolve state address = Some (E.H.H.HTreeObj tree) ->
      reduce_trans
        (state, store, Build_frame [descriptor_value] E.collector_instance,
          to_e_list (modfunc_body evaluation_size))
        (state, store, Build_frame [descriptor_value] E.collector_instance,
          if Z.ltb (Z.of_nat (List.length (get_tree_raw tree))) (Z.of_N native_table_limit + 1)%Z
          then v_to_e_list [VAL_num (VAL_int32 (wasm_nat (List.length (get_tree_raw tree))))] else [AI_trap]).
    Proof.
      intros Functions Wide Live.
      set (locals := [descriptor_value]).
      set (length := List.length (get_tree_raw tree)).
      rewrite evaluation_size_body.
      collector_execute_under E.collector_runs_context (@nil value)
        (E.collector_local_get combination true true handle_bytes state store locals 0%N
          (descriptor_value) eq_refl).
      collector_execute_under E.collector_runs_context (@nil value)
        (collector_tree_executes state store locals (Data (Object (TreeObj tree))) Functions Live).
      collector_execute_under E.collector_runs_context (@nil value)
        (collector_length_local state store locals 0%N tree Functions eq_refl Wide Live).
      eapply rt_trans with (y := (state, store, Build_frame locals E.collector_instance,
        [v_to_e (VAL_num (VAL_int32 (wasm_bool (Z.ltb (Z.of_nat length) (Z.of_N native_table_limit + 1)%Z)))); AI_basic (BI_call 22%N);
         AI_basic (BI_local_get 0%N); AI_basic (BI_call 6%N); AI_basic (BI_cvtop T_i32 CVO_wrap T_i64 None)])).
      - pose proof (native_size_comparison length Wide) as Fits.
        rewrite <- Fits.
        apply rt_step; eapply r_label with (lh := LH_base []
          [AI_basic (BI_call 22%N); AI_basic (BI_local_get 0%N); AI_basic (BI_call 6%N);
           AI_basic (BI_cvtop T_i32 CVO_wrap T_i64 None)]).
        + apply r_simple; apply rs_relop with (v1 := VAL_int64 (wasm_nat64 length))
            (v2 := VAL_int64 (Wasm_int.int_of_Z i64m 134217729%Z))
            (t := T_i64) (op := Relop_i (ROI_lt SX_U)); reflexivity.
        + reflexivity.
        + reflexivity.
      - destruct (Z.ltb (Z.of_nat length) (Z.of_N native_table_limit + 1)%Z) eqn:Fits.
        + collector_execute_under E.collector_runs_context (@nil value)
            (E.collector_require_executes combination true true handle_bytes state store locals true
              ltac:(rewrite Functions; reflexivity)).
          collector_execute_under E.collector_runs_context (@nil value)
            (collector_length_local state store locals 0%N tree Functions eq_refl Wide Live).
          apply rt_step, r_simple; eapply rs_convert_success with
            (t1 := T_i64) (t2 := T_i32) (op := CVO_wrap) (sx := None)
            (v := VAL_int64 (wasm_nat64 length)) (v' := VAL_int32 (wasm_nat length));
            try reflexivity.
          apply wasm_nat64_wrap; exact Wide.
        + eapply E.collector_trap_suffix with
            (instructions := [v_to_e (VAL_num (VAL_int32 (wasm_bool false))); AI_basic (BI_call 22%N)]).
          apply E.collector_require_executes with (condition := false); rewrite Functions; reflexivity.
      all: try assumption.
    Qed.

    (** The native size call returns a Tree's length when it fits the native index bound. *)
    Lemma collector_size_executes state store locals tree :
      s_funcs store = s_funcs E.allocated_store ->
      native_size_bound (List.length (get_tree_raw tree)) ->
      E.H.resolve state address = Some (E.H.H.HTreeObj tree) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [descriptor_value] ++ [AI_basic (BI_call 25%N)])
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [VAL_num (VAL_int32
            (E.H.i32_of_nat (E.H.T.O.data_length (Object (TreeObj tree)))))]).
    Proof.
      intros Functions Bound Live.
      eapply E.collector_call with (code := evaluation_size)
        (parameters := [T_num T_i32]) (results := [T_num T_i32]) (defaults := [])
        (index := 25%N) (address := 25%N)
        (arguments := [descriptor_value])
        (values := [VAL_num (VAL_int32
          (E.H.i32_of_nat (E.H.T.O.data_length (Object (TreeObj tree)))))]) (accepted := true)
        (final_frame := Build_frame [descriptor_value] E.collector_instance);
        try reflexivity.
      - rewrite Functions; reflexivity.
      - pose proof (collector_size_body_executes state store tree Functions ltac:(pose proof (native_size_bound_word _ Bound); lia) Live) as Run.
        assert (Fits : Z.ltb (Z.of_nat (List.length (get_tree_raw tree))) (Z.of_N native_table_limit + 1)%Z = true)
          by (apply Z.ltb_lt; exact Bound).
        rewrite Fits in Run; exact Run.
      all: try assumption.
    Qed.

    (** The native size call traps on a Tree too large for the native index range. *)
    Lemma collector_size_rejects_large state store locals tree :
      s_funcs store = s_funcs E.allocated_store ->
      (Z.of_N native_table_limit + 1 <= Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 64)%Z ->
      E.H.resolve state address = Some (E.H.H.HTreeObj tree) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [descriptor_value] ++ [AI_basic (BI_call 25%N)])
        (state, store, Build_frame locals E.collector_instance, [AI_trap]).
    Proof.
      intros Functions [Large Wide] Live.
      eapply E.collector_call with (code := evaluation_size)
        (parameters := [T_num T_i32]) (results := [T_num T_i32]) (defaults := [])
        (index := 25%N) (address := 25%N) (arguments := [descriptor_value])
        (values := [VAL_num (VAL_int32 (wasm_nat 0))]) (accepted := false)
        (final_frame := Build_frame [descriptor_value] E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - pose proof (collector_size_body_executes state store tree Functions Wide Live) as Run.
        assert (Fits : Z.ltb (Z.of_nat (List.length (get_tree_raw tree))) (Z.of_N native_table_limit + 1)%Z = false)
          by (apply Z.ltb_ge; exact Large).
        rewrite Fits in Run; exact Run.
      all: try assumption.
    Qed.

    (** The native size call traps when its input fails the required Tree shape or size
        bounds. *)
    Lemma collector_size_rejects_shape state store locals h :
      s_funcs store = s_funcs E.allocated_store ->
      (forall tree, h = Data (Object (TreeObj tree)) ->
        (Z.of_N native_table_limit + 1 <= Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 64)%Z) ->
      E.H.resolve state address = Some (h) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [descriptor_value] ++ [AI_basic (BI_call 25%N)])
        (state, store, Build_frame locals E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Invalid Live; destruct h as [[[blob|tree]|reference]|thunk|encode].
      - eapply collector_size_rejects_non_tree with (value := Data (Object (BlobObj blob))); [exact Functions|intros; discriminate|exact Live].
      - eapply collector_size_rejects_large with (tree := tree); [exact Functions|apply Invalid; reflexivity|exact Live].
      - eapply collector_size_rejects_non_tree with (value := Data (Ref reference)); [exact Functions|intros; discriminate|exact Live].
      - eapply collector_size_rejects_non_tree with (value := Thunk thunk); [exact Functions|intros; discriminate|exact Live].
      - eapply collector_size_rejects_non_tree with (value := Encode encode); [exact Functions|intros; discriminate|exact Live].
      all: try assumption.
    Qed.

    (** The arity helper body accepts a Tree with the requested bounded length. *)
    Lemma collector_arity_body_executes state store tree length :
      s_funcs store = s_funcs E.allocated_store ->
      native_size_bound (List.length (get_tree_raw tree)) ->
      E.H.resolve state address = Some (E.H.H.HTreeObj tree) ->
      reduce_trans
        (state, store, Build_frame
          [descriptor_value; VAL_num (VAL_int32 length)] E.collector_instance,
          to_e_list (modfunc_body evaluation_arity))
        (state, store, Build_frame
          [descriptor_value; VAL_num (VAL_int32 length)] E.collector_instance,
          if app_relop (Relop_i ROI_eq)
            (VAL_int32 (E.H.i32_of_nat (E.H.T.O.data_length (Object (TreeObj tree)))))
            (VAL_int32 length) then [] else [AI_trap]).
    Proof.
      intros Functions Bound Live; rewrite evaluation_arity_body.
      set (frame := Build_frame
        [descriptor_value; VAL_num (VAL_int32 length)] E.collector_instance).
      set (value := descriptor_value).
      set (word := VAL_int32 (E.H.i32_of_nat (E.H.T.O.data_length (Object (TreeObj tree))))).
      eapply rt_trans with (y := (state, store, frame,
        [v_to_e value; AI_basic (BI_call 25%N); AI_basic (BI_local_get 1%N);
         AI_basic (BI_relop T_i32 (Relop_i ROI_eq)); AI_basic (BI_call 22%N)])).
      - apply rt_step; eapply r_label with (lh := LH_base []
          [AI_basic (BI_call 25%N); AI_basic (BI_local_get 1%N);
           AI_basic (BI_relop T_i32 (Relop_i ROI_eq)); AI_basic (BI_call 22%N)]).
        + eapply r_local_get with (j := 0%N) (v := value); reflexivity.
        + reflexivity.
        + reflexivity.
      - eapply rt_trans with (y := (state, store, frame,
          [v_to_e (VAL_num word); AI_basic (BI_local_get 1%N);
           AI_basic (BI_relop T_i32 (Relop_i ROI_eq)); AI_basic (BI_call 22%N)])).
        + eapply E.collector_runs_context with (prefix := [])
            (suffix := [AI_basic (BI_local_get 1%N);
              AI_basic (BI_relop T_i32 (Relop_i ROI_eq)); AI_basic (BI_call 22%N)])
            (instructions := v_to_e_list [value] ++ [AI_basic (BI_call 25%N)])
            (instructions' := [v_to_e (VAL_num word)]).
          eapply collector_size_executes; assumption.
        + eapply rt_trans with (y := (state, store, frame,
            [v_to_e (VAL_num word); v_to_e (VAL_num (VAL_int32 length));
             AI_basic (BI_relop T_i32 (Relop_i ROI_eq)); AI_basic (BI_call 22%N)])).
          * apply rt_step; eapply r_label with (lh := LH_base [VAL_num word]
              [AI_basic (BI_relop T_i32 (Relop_i ROI_eq)); AI_basic (BI_call 22%N)]).
            -- eapply r_local_get with (j := 1%N) (v := VAL_num (VAL_int32 length)); reflexivity.
            -- reflexivity.
            -- reflexivity.
          * eapply rt_trans with (y := (state, store, frame,
              [v_to_e (VAL_num (VAL_int32 (wasm_bool
                (app_relop (Relop_i ROI_eq) word (VAL_int32 length))))); AI_basic (BI_call 22%N)])).
            -- apply rt_step; eapply r_label with (lh := LH_base [] [AI_basic (BI_call 22%N)]).
               ++ apply r_simple; apply rs_relop with (v1 := word) (v2 := VAL_int32 length)
                    (t := T_i32) (op := Relop_i ROI_eq); reflexivity.
               ++ reflexivity.
               ++ reflexivity.
            -- apply E.collector_require_executes; rewrite Functions; reflexivity.
      all: try assumption.
    Qed.

    (** The native arity call accepts a Tree with the requested bounded length. *)
    Lemma collector_arity_executes state store locals tree length :
      s_funcs store = s_funcs E.allocated_store ->
      native_size_bound (List.length (get_tree_raw tree)) ->
      E.H.resolve state address = Some (E.H.H.HTreeObj tree) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [descriptor_value; VAL_num (VAL_int32 length)] ++
            [AI_basic (BI_call 26%N)])
        (state, store, Build_frame locals E.collector_instance,
          if app_relop (Relop_i ROI_eq)
            (VAL_int32 (E.H.i32_of_nat (E.H.T.O.data_length (Object (TreeObj tree)))))
            (VAL_int32 length) then [] else [AI_trap]).
    Proof.
      intros Functions Bound Live.
      eapply E.collector_call_void with (code := evaluation_arity)
        (parameters := [T_num T_i32; T_num T_i32]) (defaults := [])
        (index := 26%N) (address := 26%N)
        (arguments := [descriptor_value; VAL_num (VAL_int32 length)])
        (final_frame := Build_frame
          [descriptor_value; VAL_num (VAL_int32 length)] E.collector_instance);
        try reflexivity.
      - rewrite Functions; reflexivity.
      - eapply collector_arity_body_executes; assumption.
      all: try assumption.
    Qed.

    (** The native arity call traps on a non-Tree Object. *)
    Lemma collector_arity_rejects_non_tree state store locals value length :
      s_funcs store = s_funcs E.allocated_store ->
      (forall tree, value <> Data (Object (TreeObj tree))) ->
      E.H.resolve state address = Some (value) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [descriptor_value; VAL_num (VAL_int32 length)] ++ [AI_basic (BI_call 26%N)])
        (state, store, Build_frame locals E.collector_instance, [AI_trap]).
    Proof.
      intros Functions NotTree Live.
      set (arguments := [descriptor_value; VAL_num (VAL_int32 length)]).
      eapply E.collector_call_void with (code := evaluation_arity)
        (parameters := [T_num T_i32; T_num T_i32]) (defaults := [])
        (index := 26%N) (address := 26%N) (arguments := arguments) (accepted := false)
        (final_frame := Build_frame arguments E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - rewrite evaluation_arity_body.
        collector_execute_under E.collector_runs_context (@nil datatypes.value)
          (E.collector_local_get combination true true handle_bytes state store arguments 0%N
            (descriptor_value) eq_refl).
        eapply E.collector_trap_suffix with
          (instructions := v_to_e_list [descriptor_value] ++ [AI_basic (BI_call 25%N)]).
        eapply collector_size_rejects_non_tree with (value := value); assumption.
      all: try assumption.
    Qed.

    (** The native arity call traps when the Tree length differs from the requested arity. *)
    Lemma collector_arity_rejects_length state store locals tree expected :
      s_funcs store = s_funcs E.allocated_store ->
      native_size_bound (List.length (get_tree_raw tree)) -> (Z.of_nat expected < 2 ^ 32)%Z ->
      List.length (get_tree_raw tree) <> expected ->
      E.H.resolve state address = Some (E.H.H.HTreeObj tree) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [descriptor_value; VAL_num (VAL_int32 (wasm_nat expected))] ++
            [AI_basic (BI_call 26%N)])
        (state, store, Build_frame locals E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Bound Expected Different Live.
      pose proof (collector_arity_executes state store locals tree (wasm_nat expected) Functions Bound Live) as Run.
      change (reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [descriptor_value; VAL_num (VAL_int32 (wasm_nat expected))] ++
            [AI_basic (BI_call 26%N)])
        (state, store, Build_frame locals E.collector_instance,
          if Wasm_int.int_eq i32m (wasm_nat (List.length (get_tree_raw tree))) (wasm_nat expected)
          then [] else [AI_trap])) in Run.
      rewrite (wasm_nat_equal _ _ (native_size_bound_word _ Bound) Expected) in Run.
      assert (Mismatch : Nat.eqb (List.length (get_tree_raw tree)) expected = false)
        by (apply Nat.eqb_neq; exact Different).
      rewrite Mismatch in Run; exact Run.
      all: try assumption.
    Qed.

    (** The native arity call traps when the Tree exceeds the supported index bound. *)
    Lemma collector_arity_rejects_large state store locals tree expected :
      s_funcs store = s_funcs E.allocated_store ->
      (Z.of_N native_table_limit + 1 <= Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 64)%Z ->
      E.H.resolve state address = Some (E.H.H.HTreeObj tree) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [descriptor_value; VAL_num (VAL_int32 expected)] ++
            [AI_basic (BI_call 26%N)])
        (state, store, Build_frame locals E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Large Live.
      set (arguments := [descriptor_value; VAL_num (VAL_int32 expected)]).
      eapply E.collector_call_void with (code := evaluation_arity)
        (parameters := [T_num T_i32; T_num T_i32]) (defaults := [])
        (index := 26%N) (address := 26%N) (arguments := arguments) (accepted := false)
        (final_frame := Build_frame arguments E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - rewrite evaluation_arity_body.
        collector_execute_under E.collector_runs_context (@nil value)
          (E.collector_local_get combination true true handle_bytes state store arguments 0%N
            (descriptor_value) eq_refl).
        eapply E.collector_trap_suffix with
          (instructions := v_to_e_list [descriptor_value] ++ [AI_basic (BI_call 25%N)]).
        eapply collector_size_rejects_large with (tree := tree); assumption.
      all: try assumption.
    Qed.

    (** The native arity call traps on an invalid Tree shape or unsupported size. *)
    Lemma collector_arity_rejects_shape state store locals h expected :
      s_funcs store = s_funcs E.allocated_store -> (Z.of_nat expected < 2 ^ 32)%Z ->
      (forall tree, h = Data (Object (TreeObj tree)) ->
        (Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 64)%Z /\ List.length (get_tree_raw tree) <> expected) ->
      E.H.resolve state address = Some (h) ->
      reduce_trans
        (state, store, Build_frame locals E.collector_instance,
          v_to_e_list [descriptor_value; VAL_num (VAL_int32 (wasm_nat expected))] ++ [AI_basic (BI_call 26%N)])
        (state, store, Build_frame locals E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Expected Invalid Live; destruct h as [[[blob|tree]|reference]|thunk|encode].
      - eapply collector_arity_rejects_non_tree with (value := Data (Object (BlobObj blob))); [exact Functions|intros; discriminate|exact Live].
      - destruct (Invalid _ eq_refl) as [Wide Different].
        destruct (Z.ltb (Z.of_nat (List.length (get_tree_raw tree))) (Z.of_N native_table_limit + 1)%Z) eqn:Fits.
        + eapply collector_arity_rejects_length with (tree := tree); try assumption; apply Z.ltb_lt; exact Fits.
        + eapply collector_arity_rejects_large with (tree := tree); [exact Functions|split; [apply Z.ltb_ge; exact Fits|exact Wide]|exact Live].
      - eapply collector_arity_rejects_non_tree with (value := Data (Ref reference)); [exact Functions|intros; discriminate|exact Live].
      - eapply collector_arity_rejects_non_tree with (value := Thunk thunk); [exact Functions|intros; discriminate|exact Live].
      - eapply collector_arity_rejects_non_tree with (value := Encode encode); [exact Functions|intros; discriminate|exact Live].
      all: try assumption.
    Qed.


  End Execution.
End WasmGuards.
