From Stdlib Require Import List Bool Arith NArith.
From Wasm Require Import datatypes operations opsem.
From FixProof Require Import Handle ApplyTree CouponApi CouponConstructors Host ModuleLayout ExecutionUtil.
Import ListNotations.
Open Scope nat_scope.

Module EvalBlobCoupon (S : STORAGE) (P : PROGRAM S) (C : COUPON_STORAGE S) (R : EXTERNREFS S).
  Module U := ExecutionUtil S C R.
  Module B := CouponConstructors S P C.
  Import U U.W.

  Lemma make_some coupons l r res : B.make_eval_blob_coupon coupons l r = Some res ->
    W.H.get_type l = 0%nat /\ Api.is_equal l r = true /\ res = C.create_coupon C.Eval l r.
  Proof.
    unfold B.make_eval_blob_coupon; destruct ((B.D.EC.E.EP.E.H.get_type l =? 0) && B.API.is_equal l r)%bool eqn:Check;
      [|discriminate].
    intro Result; inversion Result; subst; apply andb_true_iff in Check as [Tag Equal].
    apply Nat.eqb_eq in Tag; auto.
  Qed.

  Lemma make_none coupons l r : B.make_eval_blob_coupon coupons l r = None ->
    W.H.get_type l <> 0%nat \/ Api.is_equal l r = false.
  Proof.
    unfold B.make_eval_blob_coupon; destruct ((B.D.EC.E.EP.E.H.get_type l =? 0) && B.API.is_equal l r)%bool eqn:Check;
      [discriminate|intro].
    apply andb_false_iff in Check as [Tag|Equal]; [left; apply Nat.eqb_neq; exact Tag|right; exact Equal].
  Qed.

  Section Execution.
    Context `{mem : BlockUpdateMemory}.
    Definition argument_frame l r := Build_frame [extern_value l; extern_value r] coupon_instance.
    Definition eval_blob_result l r := v_to_e_list [extern_value (C.create_coupon C.Eval l r)].

    Lemma create_eval l r : runs ready_store (argument_frame l r) (to_e_list eval_blob_create_body) (eval_blob_result l r).
    Proof.
      eapply local_pair_host with (hf := fixpoint_create_eval_coupon) (addr := 13%N); try reflexivity.
      unfold host_values, extern_value; rewrite !R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma equal_prologue l r : runs ready_store (argument_frame l r) (to_e_list eval_blob_equal_body)
      [v_to_e (VAL_num (VAL_int32 (wasm_bool (Api.is_equal l r))));
       AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) eval_blob_create_body [BI_unreachable])].
    Proof.
      change (runs ready_store (argument_frame l r)
        ([] ++ [AI_basic (BI_local_get 0%N); AI_basic (BI_local_get 1%N); AI_basic (BI_call 0%N)] ++
          [AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) eval_blob_create_body [BI_unreachable])])
        ([] ++ v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_equal l r)))] ++
          [AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) eval_blob_create_body [BI_unreachable])])).
      apply (runs_context _ _ _ _ [] _).
      eapply local_pair_host with (hf := fixpoint_is_equal) (addr := 0%N); try reflexivity.
      unfold host_values, extern_value; rewrite !R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma blob_prologue l r : runs ready_store (argument_frame l r) (to_e_list eval_blob_body)
      [v_to_e (VAL_num (VAL_int32 (wasm_bool (W.H.get_type l =? 0))));
       AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) eval_blob_equal_body [BI_unreachable])].
    Proof.
      change (runs ready_store (argument_frame l r)
        ([] ++ [AI_basic (BI_local_get 0%N); AI_basic (BI_call 18%N)] ++
          [AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) eval_blob_equal_body [BI_unreachable])])
        ([] ++ v_to_e_list [VAL_num (VAL_int32 (wasm_bool (W.H.get_type l =? 0)))] ++
          [AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) eval_blob_equal_body [BI_unreachable])])).
      apply (runs_context _ _ _ _ [] _).
      eapply local_first_host with (hf := fixpoint_is_blob_obj) (addr := 18%N); try reflexivity.
      unfold host_values, extern_value; rewrite R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma eval_blob_body_success l r : W.H.get_type l = 0%nat -> Api.is_equal l r = true ->
      runs ready_store (argument_frame l r) (to_e_list eval_blob_body) (eval_blob_result l r).
    Proof.
      intros Tag Equal; eapply runs_trans; [apply blob_prologue|].
      rewrite Tag; apply if_externref; [left; reflexivity|].
      eapply runs_trans; [apply equal_prologue|].
      rewrite Equal; apply if_externref; [left; reflexivity|apply create_eval].
    Qed.

    Lemma eval_blob_body_trap l r : W.H.get_type l <> 0%nat \/ Api.is_equal l r = false ->
      runs ready_store (argument_frame l r) (to_e_list eval_blob_body) [AI_trap].
    Proof.
      intro Fail; eapply runs_trans; [apply blob_prologue|].
      apply if_externref; [right; reflexivity|].
      destruct (W.H.get_type l =? 0) eqn:Tag.
      - apply Nat.eqb_eq in Tag; destruct Fail as [Different|Equal]; [contradiction|].
        eapply runs_trans; [apply equal_prologue|].
        rewrite Equal; apply if_externref; [right; reflexivity|].
        apply runs_step, r_simple, rs_unreachable.
      - apply runs_step, r_simple, rs_unreachable.
    Qed.

    Lemma make_eval_blob_coupon_run_invoke_some coupons l r res caller :
      B.make_eval_blob_coupon coupons l r = Some res ->
      runs ready_store caller
        (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_eval_blobobj_coupon_idx])
        (v_to_e_list [extern_value res]).
    Proof.
      intro Make; destruct (make_some _ _ _ _ Make) as [Tag [Equal ->]].
      eapply invoke_pair with (inst := coupon_instance) (body := eval_blob_body) (typeidx := 0%N).
      - rewrite ready_functions; reflexivity.
      - apply eval_blob_body_success; assumption.
      - left; split; reflexivity.
    Qed.

    Lemma make_eval_blobobj_coupon_run_invoke_none coupons l r caller :
      B.make_eval_blob_coupon coupons l r = None ->
      runs ready_store caller
        (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_eval_blobobj_coupon_idx]) [AI_trap].
    Proof.
      intro Make; eapply invoke_pair with (inst := coupon_instance) (body := eval_blob_body) (typeidx := 0%N).
      - rewrite ready_functions; reflexivity.
      - apply eval_blob_body_trap; eapply make_none; exact Make.
      - right; reflexivity.
    Qed.

    Theorem make_eval_blob_coupon_success coupons l r res caller :
      B.make_eval_blob_coupon coupons l r = Some res ->
      reduce_trans
        (tt, ready_store, caller, v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_eval_blobobj_coupon_idx])
        (tt, ready_store, caller, v_to_e_list [extern_value res]) /\ B.coupon_good res.
    Proof.
      intro Make; split; [apply runs_sound; eapply make_eval_blob_coupon_run_invoke_some|eapply B.make_eval_blob_coupon_good]; exact Make.
    Qed.

    Theorem make_eval_blob_coupon_trap coupons l r caller :
      B.make_eval_blob_coupon coupons l r = None ->
      reduce_trans
        (tt, ready_store, caller, v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_eval_blobobj_coupon_idx])
        (tt, ready_store, caller, [AI_trap]).
    Proof. intro Make; apply runs_sound; eapply make_eval_blobobj_coupon_run_invoke_none; exact Make. Qed.
  End Execution.
End EvalBlobCoupon.
