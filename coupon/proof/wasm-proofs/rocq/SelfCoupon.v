From Stdlib Require Import List NArith.
From Wasm Require Import datatypes operations opsem.
From FixProof Require Import Handle ApplyTree CouponApi CouponConstructors Host ModuleLayout ExecutionUtil.
Import ListNotations.

Module SelfCoupon (S : STORAGE) (P : PROGRAM S) (C : COUPON_STORAGE S) (R : EXTERNREFS S).
  Module U := ExecutionUtil S C R.
  Module B := CouponConstructors S P C.
  Import U U.W.

  Lemma ms_some coupons l r res : B.make_self_coupon coupons l r = Some res ->
    Api.is_equal l r = true /\ res = C.create_coupon C.Eq l r.
  Proof.
    unfold B.make_self_coupon; destruct (B.API.is_equal l r) eqn:Check; [|discriminate].
    intro Result; inversion Result; subst; split; [exact Check|reflexivity].
  Qed.
  Lemma ms_none coupons l r : B.make_self_coupon coupons l r = None -> Api.is_equal l r = false.
  Proof.
    unfold B.make_self_coupon; destruct (B.API.is_equal l r) eqn:Check;
      [discriminate|intro; exact Check].
  Qed.

  Section Execution.
    Context `{mem : BlockUpdateMemory}.
    Variable s : store_record.
    Variable inst : moduleinst.
    Variables equal_addr create_addr : funcaddr.
    Hypothesis equal_index : lookup_N (inst_funcs inst) 0%N = Some equal_addr.
    Hypothesis create_index : lookup_N (inst_funcs inst) 12%N = Some create_addr.
    Hypothesis equal_closure : lookup_N (s_funcs s) equal_addr =
      Some (FC_func_host type_rr_i32 fixpoint_is_equal).
    Hypothesis create_closure : lookup_N (s_funcs s) create_addr =
      Some (FC_func_host type_rr_r fixpoint_create_eq_coupon).

    Definition self_frame l r := Build_frame [extern_value l; extern_value r] inst.
    Definition self_cond := AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) self_success_body self_failure_body).

    Lemma self_prologue l r : runs s (self_frame l r) (to_e_list self_body)
      [v_to_e (VAL_num (VAL_int32 (wasm_bool (Api.is_equal l r)))); self_cond].
    Proof.
      eapply runs_trans with (mid := v_to_e_list [extern_value l; extern_value r] ++
        [AI_basic (BI_call 0%N); self_cond]).
      - change (runs s (self_frame l r)
          ([] ++ [AI_basic (BI_local_get 0%N); AI_basic (BI_local_get 1%N)] ++ [AI_basic (BI_call 0%N); self_cond])
          ([] ++ v_to_e_list [extern_value l; extern_value r] ++ [AI_basic (BI_call 0%N); self_cond])).
        apply (runs_context s (self_frame l r) _ _ [] _), local_pair; reflexivity.
      - change (runs s (self_frame l r)
          ([] ++ (v_to_e_list [extern_value l; extern_value r] ++ [AI_basic (BI_call 0%N)]) ++ [self_cond])
          ([] ++ v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_equal l r)))] ++ [self_cond])).
        apply (runs_context s (self_frame l r) _ _ [] _).
        eapply call_host with (hf := fixpoint_is_equal); try eassumption; try reflexivity.
        unfold host_values, extern_value; rewrite !R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma self_create l r : runs s (self_frame l r) (to_e_list self_success_body)
      (v_to_e_list [extern_value (C.create_coupon C.Eq l r)]).
    Proof.
      eapply runs_trans with (mid := v_to_e_list [extern_value l; extern_value r] ++ [AI_basic (BI_call 12%N)]).
      - change (runs s (self_frame l r)
          ([] ++ [AI_basic (BI_local_get 0%N); AI_basic (BI_local_get 1%N)] ++ [AI_basic (BI_call 12%N)])
          ([] ++ v_to_e_list [extern_value l; extern_value r] ++ [AI_basic (BI_call 12%N)])).
        apply (runs_context s (self_frame l r) _ _ [] _), local_pair; reflexivity.
      - eapply call_host with (hf := fixpoint_create_eq_coupon); try eassumption; try reflexivity.
        unfold host_values, extern_value; rewrite !R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma self_body_success l r : Api.is_equal l r = true ->
      runs s (self_frame l r) (to_e_list self_body) (v_to_e_list [extern_value (C.create_coupon C.Eq l r)]).
    Proof.
      intro Equal; eapply runs_trans; [apply self_prologue|].
      rewrite Equal.
      eapply runs_trans with (mid := [AI_basic (BI_block (BT_valtype (Some (T_ref T_externref))) self_success_body)]).
      - apply runs_step, r_simple, rs_if_true. vm_compute; discriminate.
      - eapply runs_trans with (mid := [AI_label 1 [] (to_e_list self_success_body)]).
        + apply runs_step; eapply r_block with (vs := []) (n := 0%nat) (t1s := []) (t2s := [T_ref T_externref]); reflexivity.
        + eapply runs_trans with (mid := [AI_label 1 [] (v_to_e_list [extern_value (C.create_coupon C.Eq l r)])]).
          * apply runs_label, self_create.
          * apply runs_step, r_simple, rs_label_const; reflexivity.
    Qed.

    Lemma self_body_trap l r : Api.is_equal l r = false ->
      runs s (self_frame l r) (to_e_list self_body) [AI_trap].
    Proof.
      intro Different; eapply runs_trans; [apply self_prologue|].
      rewrite Different.
      eapply runs_trans with (mid := [AI_basic (BI_block (BT_valtype (Some (T_ref T_externref))) self_failure_body)]).
      - apply runs_step, r_simple, rs_if_false; reflexivity.
      - eapply runs_trans with (mid := [AI_label 1 [] (to_e_list self_failure_body)]).
        + apply runs_step; eapply r_block with (vs := []) (n := 0%nat) (t1s := []) (t2s := [T_ref T_externref]); reflexivity.
        + eapply runs_trans with (mid := [AI_label 1 [] [AI_trap]]).
          * apply runs_label, runs_step, r_simple, rs_unreachable.
          * apply runs_step, r_simple, rs_label_trap.
    Qed.

    Variables self_addr : funcaddr.
    Hypothesis self_closure : lookup_N (s_funcs s) self_addr = Some (FC_func_native type_rr_r inst self_code).

    Lemma self_invoke l r caller : runs s caller
      (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke self_addr])
      [AI_frame 1 (self_frame l r) [AI_label 1 [] (to_e_list self_body)]].
    Proof.
      apply runs_step; eapply r_invoke_native with (vs := [extern_value l; extern_value r])
        (ts1 := [T_ref T_externref; T_ref T_externref]) (ts2 := [T_ref T_externref])
        (ts := []) (defaults := []) (n := 2%nat) (k := 0%nat); try eassumption; reflexivity.
    Qed.

    Lemma make_self_coupon_raw_run_invoke_some coupons l r res caller :
      B.make_self_coupon coupons l r = Some res ->
      runs s caller (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke self_addr])
        (v_to_e_list [extern_value res]).
    Proof.
      intro Make; destruct (ms_some _ _ _ _ Make) as [Equal ->].
      eapply runs_trans; [apply self_invoke|].
      eapply runs_trans with (mid := [AI_frame 1 (self_frame l r)
        [AI_label 1 [] (v_to_e_list [extern_value (C.create_coupon C.Eq l r)])]]).
      - apply runs_frame, runs_label, self_body_success; exact Equal.
      - eapply runs_trans with (mid := [AI_frame 1 (self_frame l r) (v_to_e_list [extern_value (C.create_coupon C.Eq l r)])]).
        + apply runs_frame, runs_step, r_simple, rs_label_const; reflexivity.
        + apply runs_step, r_simple, rs_local_const; reflexivity.
    Qed.

    Lemma make_self_coupon_raw_run_invoke_none coupons l r caller :
      B.make_self_coupon coupons l r = None ->
      runs s caller (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke self_addr]) [AI_trap].
    Proof.
      intro Make; eapply runs_trans; [apply self_invoke|].
      eapply runs_trans with (mid := [AI_frame 1 (self_frame l r) [AI_label 1 [] [AI_trap]]]).
      - apply runs_frame, runs_label, self_body_trap; eapply ms_none; exact Make.
      - eapply runs_trans with (mid := [AI_frame 1 (self_frame l r) [AI_trap]]).
        + apply runs_frame, runs_step, r_simple, rs_label_trap.
        + apply runs_step, r_simple, rs_local_trap.
    Qed.
  End Execution.

  Section InitializedExecution.
    Context `{mem : BlockUpdateMemory}.

    Theorem make_self_coupon_success coupons l r res caller :
      B.make_self_coupon coupons l r = Some res ->
      reduce_trans
        (tt, ready_store, caller, v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_self_coupon_idx])
        (tt, ready_store, caller, v_to_e_list [extern_value res]) /\ B.coupon_good res.
    Proof.
      intro Make; split; [|eapply B.make_self_coupon_good; exact Make].
      apply runs_sound.
      eapply make_self_coupon_raw_run_invoke_some with (inst := coupon_instance)
        (equal_addr := 0%N) (create_addr := 12%N); try eassumption.
      - apply coupon_equal_index.
      - apply coupon_create_eq_index.
      - rewrite ready_functions; apply coupon_equal_closure.
      - rewrite ready_functions; apply coupon_create_eq_closure.
      - rewrite ready_functions; apply coupon_self_closure.
    Qed.

    Theorem make_self_coupon_trap coupons l r caller :
      B.make_self_coupon coupons l r = None ->
      reduce_trans
        (tt, ready_store, caller, v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_self_coupon_idx])
        (tt, ready_store, caller, [AI_trap]).
    Proof.
      intro Make; apply runs_sound.
      eapply make_self_coupon_raw_run_invoke_none with (inst := coupon_instance)
        (equal_addr := 0%N); try eassumption.
      - apply coupon_equal_index.
      - rewrite ready_functions; apply coupon_equal_closure.
      - rewrite ready_functions; apply coupon_self_closure.
    Qed.
  End InitializedExecution.
End SelfCoupon.
