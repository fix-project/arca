From Stdlib Require Import List Bool Arith NArith.
From Wasm Require Import datatypes operations opsem.
From FixProof Require Import Handle ApplyTree CouponApi CouponConstructors Host Init ModuleLayout CouponTable.
Import ListNotations.
Open Scope list_scope.

Definition dispatch_success_body := [BI_local_get 1%N; BI_local_get 2%N; BI_local_get 0%N; BI_call_indirect 1%N 0%N].
Definition dispatch_guard := [BI_local_get 0%N; BI_table_size 1%N; BI_relop T_i32 (Relop_i (ROI_lt SX_U))].
Definition dispatch_body := dispatch_guard ++
  [BI_if (BT_valtype (Some (T_ref T_externref))) dispatch_success_body self_failure_body].
Definition dispatch_code := Build_module_func 5%N [] dispatch_body.
Definition dispatch_type := Tf [T_num T_i32; T_ref T_externref; T_ref T_externref] [T_ref T_externref].
Lemma coupon_module_dispatch_code : lookup_N (mod_funcs coupon_module) 13%N = Some dispatch_code.
Proof. reflexivity. Qed.
Lemma coupon_module_dispatch_signature : lookup_N (mod_types coupon_module) 5%N = Some dispatch_type.
Proof. reflexivity. Qed.

Module Dispatcher (S : STORAGE) (P : PROGRAM S) (C : COUPON_STORAGE S) (R : EXTERNREFS S).
  Module T := CouponTable S C R.
  Module B := CouponConstructors S P C.
  Import T T.U T.U.W.

  Definition request_to_nat req := match req with
    | B.TreeEq => 0%nat | B.EqApplication => 1%nat | B.ForceResultEq => 2%nat
    | B.EqEncodeStrict => 3%nat | B.ThinkApplication => 4%nat | B.ThinkToForce => 5%nat
    | B.ForceToEncodeStrict => 6%nat | B.EvalEq => 7%nat | B.EvalBlobObj => 8%nat
    | B.EvalTreeObj => 9%nat | B.Sym => 10%nat | B.Trans => 11%nat | B.Self => 12%nat end.
  Definition request_to_func_idx req := match req with
    | B.TreeEq => func_make_eq_tree_coupon_idx | B.EqApplication => func_make_eq_application_coupon_idx
    | B.ForceResultEq => func_make_force_result_eq_coupon_idx | B.EqEncodeStrict => func_make_eq_encode_strict_coupon_idx
    | B.ThinkApplication => func_make_think_application_coupon_idx | B.ThinkToForce => func_make_think_to_force_coupon_idx
    | B.ForceToEncodeStrict => func_make_force_to_encode_strict_coupon_idx | B.EvalEq => func_make_eval_eq_coupon_idx
    | B.EvalBlobObj => func_make_eval_blobobj_coupon_idx | B.EvalTreeObj => func_make_eval_tree_coupon_idx
    | B.Sym => func_make_sym_coupon_idx | B.Trans => func_make_trans_coupon_idx | B.Self => func_make_self_coupon_idx end.
  Lemma request_to_nat_bound req : (request_to_nat req < 13)%nat.
  Proof. destruct req; repeat constructor. Qed.

  Section Execution.
    Context `{mem : BlockUpdateMemory}.
    Definition request_value req := VAL_num (VAL_int32 (i32_of_nat (request_to_nat req))).
    Definition dispatch_frame req l r := Build_frame [request_value req; extern_value l; extern_value r] coupon_instance.
    Definition child_call req l r := v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke (request_to_func_idx req)].
    Definition suspended_dispatch req l r :=
      [AI_frame 1 (dispatch_frame req l r) [AI_label 1 [] [AI_label 1 [] (child_call req l r)]]].

    Lemma guard_run coupons req l r : runs (coupon_store coupons) (dispatch_frame req l r) (to_e_list dispatch_guard)
      (v_to_e_list [VAL_num (VAL_int32 (wasm_bool true))]).
    Proof.
      eapply runs_trans with (mid := v_to_e (request_value req) ::
        [AI_basic (BI_table_size 1%N); AI_basic (BI_relop T_i32 (Relop_i (ROI_lt SX_U))) ]).
      - apply runs_step, (step_context _ _ [AI_basic (BI_local_get 0%N)] [v_to_e (request_value req)] []
          [AI_basic (BI_table_size 1%N); AI_basic (BI_relop T_i32 (Relop_i (ROI_lt SX_U)))]), r_local_get; reflexivity.
      - eapply runs_trans with (mid := v_to_e_list [request_value req; VAL_num (VAL_int32 (i32_of_nat 13))] ++
          [AI_basic (BI_relop T_i32 (Relop_i (ROI_lt SX_U))) ]).
        + apply runs_step, (step_context _ _ [AI_basic (BI_table_size 1%N)]
            [v_to_e (VAL_num (VAL_int32 (i32_of_nat 13)))] [request_value req]
            [AI_basic (BI_relop T_i32 (Relop_i (ROI_lt SX_U))) ]).
          eapply r_table_size with (sz := 13%nat); reflexivity.
        + destruct req; apply runs_step, r_simple, rs_relop; reflexivity.
    Qed.

    Lemma indirect_call_run coupons req l r : runs (coupon_store coupons) (dispatch_frame req l r)
      (to_e_list dispatch_success_body) (child_call req l r).
    Proof.
      eapply runs_trans with (mid := v_to_e (extern_value l) ::
        [AI_basic (BI_local_get 2%N); AI_basic (BI_local_get 0%N); AI_basic (BI_call_indirect 1%N 0%N)]).
      - apply runs_step, (step_context _ _ [AI_basic (BI_local_get 1%N)] [v_to_e (extern_value l)] []
          [AI_basic (BI_local_get 2%N); AI_basic (BI_local_get 0%N); AI_basic (BI_call_indirect 1%N 0%N)]), r_local_get; reflexivity.
      - eapply runs_trans with (mid := v_to_e_list [extern_value l; extern_value r] ++
          [AI_basic (BI_local_get 0%N); AI_basic (BI_call_indirect 1%N 0%N)]).
        + apply runs_step, (step_context _ _ [AI_basic (BI_local_get 2%N)] [v_to_e (extern_value r)] [extern_value l]
            [AI_basic (BI_local_get 0%N); AI_basic (BI_call_indirect 1%N 0%N)]), r_local_get; reflexivity.
        + eapply runs_trans with (mid := v_to_e_list [extern_value l; extern_value r; request_value req] ++
            [AI_basic (BI_call_indirect 1%N 0%N)]).
          * apply runs_step, (step_context _ _ [AI_basic (BI_local_get 0%N)] [v_to_e (request_value req)] [extern_value l; extern_value r]
              [AI_basic (BI_call_indirect 1%N 0%N)]), r_local_get; reflexivity.
          * apply runs_step; apply (step_context _ _
              [v_to_e (request_value req); AI_basic (BI_call_indirect 1%N 0%N)]
              [AI_invoke (request_to_func_idx req)] [extern_value l; extern_value r] []).
            destruct req; (eapply r_call_indirect_success;
              [reflexivity|rewrite coupon_store_functions; reflexivity|reflexivity]).
    Qed.

    Lemma body_prologue coupons req l r : runs (coupon_store coupons) (dispatch_frame req l r) (to_e_list dispatch_body)
      [AI_label 1 [] (child_call req l r)].
    Proof.
      eapply runs_trans.
      - apply guard_prologue; apply guard_run.
      - eapply runs_trans with (mid := [AI_basic (BI_block (BT_valtype (Some (T_ref T_externref))) dispatch_success_body)]).
        + apply runs_step, r_simple, rs_if_true; vm_compute; discriminate.
        + eapply runs_trans with (mid := [AI_label 1 [] (to_e_list dispatch_success_body)]).
          * apply runs_step; eapply r_block with (vs := []) (n := 0%nat) (t1s := []) (t2s := [T_ref T_externref]); reflexivity.
          * apply runs_label, indirect_call_run.
    Qed.

    Lemma invoke_dispatch coupons req l r caller : runs (coupon_store coupons) caller
      (v_to_e_list [request_value req; extern_value l; extern_value r] ++ [AI_invoke func_make_coupon_idx])
      [AI_frame 1 (dispatch_frame req l r) [AI_label 1 [] (to_e_list dispatch_body)]].
    Proof.
      apply runs_step; eapply r_invoke_native with (vs := [request_value req; extern_value l; extern_value r])
        (ts1 := [T_num T_i32; T_ref T_externref; T_ref T_externref]) (ts2 := [T_ref T_externref])
        (inst := coupon_instance) (x := 5%N) (ts := []) (defaults := []) (n := 3%nat) (k := 0%nat)
        (cl := FC_func_native dispatch_type coupon_instance dispatch_code) (code := dispatch_code).
      - rewrite coupon_store_functions; reflexivity.
      - reflexivity.
      - reflexivity.
      - reflexivity.
      - reflexivity.
      - reflexivity.
      - reflexivity.
      - reflexivity.
      - reflexivity.
    Qed.

    Theorem make_coupon_prologue coupons req l r caller : runs (coupon_store coupons) caller
      (v_to_e_list [request_value req; extern_value l; extern_value r] ++ [AI_invoke func_make_coupon_idx])
      (suspended_dispatch req l r).
    Proof.
      eapply runs_trans; [apply invoke_dispatch|].
      apply runs_frame, runs_label, body_prologue.
    Qed.

    Theorem make_coupon_prologue_reduction coupons req l r caller :
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [request_value req; extern_value l; extern_value r] ++ [AI_invoke func_make_coupon_idx])
        (tt, coupon_store coupons, caller, suspended_dispatch req l r).
    Proof. apply runs_sound, make_coupon_prologue. Qed.

    Theorem make_coupon_from_native coupons req l r caller out :
      runs (coupon_store coupons) (dispatch_frame req l r) (child_call req l r) out ->
      ((const_list out = true /\ List.length out = 1%nat) \/ out = [AI_trap]) ->
      runs (coupon_store coupons) caller
        (v_to_e_list [request_value req; extern_value l; extern_value r] ++ [AI_invoke func_make_coupon_idx]) out.
    Proof.
      intros Child Terminal; eapply runs_trans; [apply make_coupon_prologue|].
      eapply runs_trans with (mid := [AI_frame 1 (dispatch_frame req l r) [AI_label 1 [] [AI_label 1 [] out]]]).
      - apply runs_frame, runs_label, runs_label; exact Child.
      - destruct Terminal as [[Const Len]| ->].
        + eapply runs_trans with (mid := [AI_frame 1 (dispatch_frame req l r) [AI_label 1 [] out]]).
          * apply runs_frame, runs_label, runs_step, r_simple, rs_label_const; exact Const.
          * eapply runs_trans with (mid := [AI_frame 1 (dispatch_frame req l r) out]).
            -- apply runs_frame, runs_step, r_simple, rs_label_const; exact Const.
            -- apply runs_step, r_simple, rs_local_const; assumption.
        + eapply runs_trans with (mid := [AI_frame 1 (dispatch_frame req l r) [AI_label 1 [] [AI_trap]]]).
          * apply runs_frame, runs_label, runs_step, r_simple, rs_label_trap.
          * eapply runs_trans with (mid := [AI_frame 1 (dispatch_frame req l r) [AI_trap]]).
            -- apply runs_frame, runs_step, r_simple, rs_label_trap.
            -- apply runs_step, r_simple, rs_local_trap.
    Qed.
  End Execution.
End Dispatcher.
