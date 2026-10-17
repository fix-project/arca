From Stdlib Require Import List Bool Arith NArith ZArith Lia.
From Wasm Require Import numerics datatypes operations opsem.
From FixProof Require Import Handle CouponApi Host CouponTable TreeLayout WasmNatural.
Import ListNotations.
Open Scope list_scope.

Module LoopUtil (S : STORAGE) (C : COUPON_STORAGE S) (R : EXTERNREFS S).
  Module T := CouponTable S C R.
  Import T T.U T.U.W.
  Section Execution.
    Context `{mem : BlockUpdateMemory}.
    Definition loop_frame l r c n i := Build_frame
      [extern_value l; extern_value r; c; VAL_num (VAL_int32 (wasm_nat n)); VAL_num (VAL_int32 (wasm_nat i))] coupon_instance.
    Definition loop_cont body := [AI_basic (BI_loop (BT_valtype None) (tree_loop_code body))].
    Definition loop_state body code := [AI_label 0 [] [AI_label 0 (loop_cont body) code]].
    Lemma bool_word b : numerics.wasm_bool b = T.U.W.wasm_bool b.
    Proof. destruct b; reflexivity. Qed.

    Lemma if_void s f (b : bool) yes no out : terminal_form out ->
      runs s f (to_e_list (if b then yes else no)) out ->
      runs s f [v_to_e (VAL_num (VAL_int32 (T.U.W.wasm_bool b)));
        AI_basic (BI_if (BT_valtype None) yes no)] out.
    Proof.
      intros Terminal Branch.
      eapply runs_trans with (mid := [AI_basic (BI_block (BT_valtype None) (if b then yes else no))]).
      - apply runs_step, r_simple; destruct b.
        + apply rs_if_true; vm_compute; discriminate.
        + apply rs_if_false; reflexivity.
      - eapply runs_trans with (mid := [AI_label 0 [] (to_e_list (if b then yes else no))]).
        + apply runs_step; eapply r_block with (vs := []) (n := 0%nat) (t1s := []) (t2s := []); reflexivity.
        + eapply runs_trans with (mid := [AI_label 0 [] out]); [apply runs_label; exact Branch|].
          apply runs_step, r_simple; destruct Terminal as [Const| ->];
            [apply rs_label_const; exact Const|apply rs_label_trap].
    Qed.

    Lemma local_indices s f j k x y : lookup_N (f_locs f) j = Some x ->
      lookup_N (f_locs f) k = Some y ->
      runs s f (to_e_list [BI_local_get j; BI_local_get k]) (v_to_e_list [x; y]).
    Proof.
      intros X Y; eapply runs_trans with (mid := [v_to_e x; AI_basic (BI_local_get k)]).
      - apply runs_step, (step_context s f [AI_basic (BI_local_get j)] [v_to_e x] []
          [AI_basic (BI_local_get k)]), r_local_get; exact X.
      - apply runs_step, (step_context s f [AI_basic (BI_local_get k)] [v_to_e y] [x] []), r_local_get; exact Y.
    Qed.

    Lemma frame_if_externref s f f' (b : bool) yes no out : terminal_form out ->
      frame_runs s (f, to_e_list (if b then yes else no)) (f', out) ->
      frame_runs s (f, [v_to_e (VAL_num (VAL_int32 (T.U.W.wasm_bool b)));
        AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) yes no)]) (f', out).
    Proof.
      intros Terminal Branch.
      eapply frame_runs_trans with (middle := (f,
        [AI_basic (BI_block (BT_valtype (Some (T_ref T_externref))) (if b then yes else no))])).
      - apply runs_to_frame_runs, runs_step, r_simple; destruct b.
        + apply rs_if_true; vm_compute; discriminate.
        + apply rs_if_false; reflexivity.
      - eapply frame_runs_trans with (middle := (f, [AI_label 1 [] (to_e_list (if b then yes else no))])).
        + apply frame_runs_step; eapply r_block with (vs := []) (n := 0%nat) (t1s := [])
            (t2s := [T_ref T_externref]); reflexivity.
        + eapply frame_runs_trans with (middle := (f', [AI_label 1 [] out])).
          * exact (frame_runs_label s _ _ 1 [] Branch).
          * apply frame_runs_step, r_simple; destruct Terminal as [Const| ->];
              [apply rs_label_const; exact Const|apply rs_label_trap].
    Qed.

    Lemma loop_context s f f' es es' body : frame_runs s (f, es) (f', es') ->
      frame_runs s (f, loop_state body es) (f', loop_state body es').
    Proof.
      intro Run; unfold loop_state.
      exact (frame_runs_label s _ _ 0 []
        (frame_runs_label s _ _ 0 (loop_cont body) Run)).
    Qed.

    Lemma loop_enter s f body : frame_runs s (f, to_e_list (tree_loop body))
      (f, loop_state body (to_e_list (tree_loop_code body))).
    Proof.
      eapply frame_runs_trans with (middle := (f, [AI_label 0 [] (loop_cont body)])).
      - apply frame_runs_step; eapply r_block with (vs := []) (n := 0%nat) (t1s := []) (t2s := []); reflexivity.
      - unfold loop_state; apply (frame_runs_label s (f, loop_cont body)
          (f, [AI_label 0 (loop_cont body) (to_e_list (tree_loop_code body))]) 0 []).
        apply frame_runs_step; eapply r_loop with (vs := []) (n := 0%nat) (t1s := []) (t2s := []); reflexivity.
    Qed.

    Lemma counter_compare coupons l r c n i : (Z.of_nat i < 2 ^ 31)%Z -> (Z.of_nat n < 2 ^ 31)%Z ->
      runs (coupon_store coupons) (loop_frame l r c n i)
        (to_e_list [BI_local_get 4%N; BI_local_get 3%N; BI_relop T_i32 (Relop_i (ROI_ge SX_S))])
        (v_to_e_list [VAL_num (VAL_int32 (T.U.W.wasm_bool (negb (Nat.ltb i n))))]).
    Proof.
      intros IBound NBound; eapply runs_trans with (mid :=
        [v_to_e (VAL_num (VAL_int32 (wasm_nat i))); AI_basic (BI_local_get 3%N); AI_basic (BI_relop T_i32 (Relop_i (ROI_ge SX_S))) ]).
      - apply runs_step, (step_context _ _ [AI_basic (BI_local_get 4%N)]
          [v_to_e (VAL_num (VAL_int32 (wasm_nat i)))] []
          [AI_basic (BI_local_get 3%N); AI_basic (BI_relop T_i32 (Relop_i (ROI_ge SX_S)))]), r_local_get; reflexivity.
      - eapply runs_trans with (mid := v_to_e_list [VAL_num (VAL_int32 (wasm_nat i)); VAL_num (VAL_int32 (wasm_nat n))] ++
          [AI_basic (BI_relop T_i32 (Relop_i (ROI_ge SX_S))) ]).
        + apply runs_step, (step_context _ _ [AI_basic (BI_local_get 3%N)]
            [v_to_e (VAL_num (VAL_int32 (wasm_nat n)))] [VAL_num (VAL_int32 (wasm_nat i))]
            [AI_basic (BI_relop T_i32 (Relop_i (ROI_ge SX_S)))]), r_local_get; reflexivity.
        + rewrite <- bool_word; apply runs_step, r_simple, wasm_nat_ge_signed_step; assumption.
    Qed.

    Lemma loop_check_run coupons l r c n i body : (Z.of_nat i < 2 ^ 31)%Z -> (Z.of_nat n < 2 ^ 31)%Z ->
      frame_runs (coupon_store coupons)
        (loop_frame l r c n i, loop_state body (to_e_list (tree_loop_code body)))
        (loop_frame l r c n i, loop_state body
          ([v_to_e (VAL_num (VAL_int32 (T.U.W.wasm_bool (negb (Nat.ltb i n))))); AI_basic (BI_br_if 1%N)] ++
           to_e_list (body ++ tree_increment))).
    Proof.
      intros IBound NBound; apply loop_context, runs_to_frame_runs.
      apply (runs_context _ _ (to_e_list [BI_local_get 4%N; BI_local_get 3%N; BI_relop T_i32 (Relop_i (ROI_ge SX_S))])
        (v_to_e_list [VAL_num (VAL_int32 (T.U.W.wasm_bool (negb (Nat.ltb i n))))]) []
        (AI_basic (BI_br_if 1%N) :: to_e_list (body ++ tree_increment))).
      apply counter_compare; assumption.
    Qed.

    Lemma loop_continue coupons l r c n i body : (i < n)%nat -> (Z.of_nat n < 2 ^ 31)%Z ->
      frame_runs (coupon_store coupons)
        (loop_frame l r c n i, loop_state body (to_e_list (tree_loop_code body)))
        (loop_frame l r c n i, loop_state body (to_e_list (body ++ tree_increment))).
    Proof.
      intros Less Bound; eapply frame_runs_trans; [apply loop_check_run; lia|].
      assert (Nat.ltb i n = true) as Test by (apply Nat.ltb_lt; exact Less); rewrite Test.
      apply loop_context, runs_to_frame_runs, runs_step.
      apply (step_context _ _ [v_to_e (VAL_num (VAL_int32 (T.U.W.wasm_bool false))); AI_basic (BI_br_if 1%N)]
        [] [] (to_e_list (body ++ tree_increment))), r_simple, rs_br_if_false; reflexivity.
    Qed.

    Lemma loop_stop coupons l r c n body : (Z.of_nat n < 2 ^ 31)%Z ->
      frame_runs (coupon_store coupons)
        (loop_frame l r c n n, loop_state body (to_e_list (tree_loop_code body))) (loop_frame l r c n n, []).
    Proof.
      intro Bound; eapply frame_runs_trans; [apply loop_check_run; assumption|].
      rewrite Nat.ltb_irrefl; cbn [negb].
      eapply frame_runs_trans with (middle := (loop_frame l r c n n,
        loop_state body (AI_basic (BI_br 1%N) :: to_e_list (body ++ tree_increment)))).
      - apply loop_context, runs_to_frame_runs, runs_step.
        apply (step_context _ _ [v_to_e (VAL_num (VAL_int32 (T.U.W.wasm_bool true))); AI_basic (BI_br_if 1%N)]
          [AI_basic (BI_br 1%N)] [] (to_e_list (body ++ tree_increment))), r_simple, rs_br_if_true; vm_compute; discriminate.
      - apply frame_runs_step, r_simple; eapply rs_br with (vs := []) (i := 1%nat)
          (lh := LH_rec [] 0 (loop_cont body) (LH_base [] (to_e_list (body ++ tree_increment))) []); reflexivity.
    Qed.

    Lemma counter_add coupons l r c n i : (Z.of_nat i < 2 ^ 32)%Z ->
      runs (coupon_store coupons) (loop_frame l r c n i)
        (to_e_list [BI_local_get 4%N; BI_const_num (VAL_int32 (Wasm_int.int_of_Z i32m 1%Z)); BI_binop T_i32 (Binop_i BOI_add)])
        (v_to_e_list [VAL_num (VAL_int32 (wasm_nat (S i)))]).
    Proof.
      intro Bound; eapply runs_trans with (mid := v_to_e_list [VAL_num (VAL_int32 (wasm_nat i)); VAL_num (VAL_int32 (wasm_nat 1))] ++
        [AI_basic (BI_binop T_i32 (Binop_i BOI_add))]).
      - apply runs_step, (step_context _ _ [AI_basic (BI_local_get 4%N)]
          [v_to_e (VAL_num (VAL_int32 (wasm_nat i)))] []
          [v_to_e (VAL_num (VAL_int32 (wasm_nat 1))); AI_basic (BI_binop T_i32 (Binop_i BOI_add))]), r_local_get; reflexivity.
      - apply runs_step, r_simple, wasm_nat_increment_step; exact Bound.
    Qed.

    Lemma loop_increment coupons l r c n i body : (Z.of_nat i < 2 ^ 32)%Z ->
      frame_runs (coupon_store coupons)
        (loop_frame l r c n i, loop_state body (to_e_list tree_increment))
        (loop_frame l r c n (S i), loop_state body (to_e_list (tree_loop_code body))).
    Proof.
      intro Bound; eapply frame_runs_trans with (middle := (loop_frame l r c n i,
        loop_state body [v_to_e (VAL_num (VAL_int32 (wasm_nat (S i)))); AI_basic (BI_local_set 4%N); AI_basic (BI_br 0%N)])).
      - apply loop_context, runs_to_frame_runs.
        apply (runs_context _ _ (to_e_list [BI_local_get 4%N; BI_const_num (VAL_int32 (Wasm_int.int_of_Z i32m 1%Z)); BI_binop T_i32 (Binop_i BOI_add)])
          (v_to_e_list [VAL_num (VAL_int32 (wasm_nat (S i)))]) [] [AI_basic (BI_local_set 4%N); AI_basic (BI_br 0%N)]).
        apply counter_add; exact Bound.
      - eapply frame_runs_trans with (middle := (loop_frame l r c n (S i), loop_state body [AI_basic (BI_br 0%N)])).
        + apply loop_context, frame_runs_step; eapply r_label with (lh := LH_base [] [AI_basic (BI_br 0%N)]).
          * eapply r_local_set with (f := loop_frame l r c n i) (f' := loop_frame l r c n (S i))
              (i := 4%N) (v := VAL_num (VAL_int32 (wasm_nat (S i)))) (vd := null_extern); reflexivity.
          * reflexivity.
          * reflexivity.
        + eapply frame_runs_trans with (middle := (loop_frame l r c n (S i), [AI_label 0 [] (loop_cont body)])).
          * unfold loop_state; apply (frame_runs_label _
              (loop_frame l r c n (S i), [AI_label 0 (loop_cont body) [AI_basic (BI_br 0%N)]])
              (loop_frame l r c n (S i), loop_cont body) 0 []).
            apply frame_runs_step, r_simple; eapply rs_br with (vs := []) (i := 0%nat) (lh := LH_base [] []); reflexivity.
          * unfold loop_state; apply (frame_runs_label _
              (loop_frame l r c n (S i), loop_cont body)
              (loop_frame l r c n (S i), [AI_label 0 (loop_cont body) (to_e_list (tree_loop_code body))]) 0 []).
            apply frame_runs_step; eapply r_loop with (vs := []) (n := 0%nat) (t1s := []) (t2s := []); reflexivity.
    Qed.

    Lemma loop_iteration coupons l r c c' n i body : (i < n)%nat -> (Z.of_nat n < 2 ^ 31)%Z ->
      frame_runs (coupon_store coupons) (loop_frame l r c n i, to_e_list body) (loop_frame l r c' n i, []) ->
      frame_runs (coupon_store coupons)
        (loop_frame l r c n i, loop_state body (to_e_list (tree_loop_code body)))
        (loop_frame l r c' n (S i), loop_state body (to_e_list (tree_loop_code body))).
    Proof.
      intros Less Bound Body; eapply frame_runs_trans; [apply loop_continue; assumption|].
      eapply frame_runs_trans with (middle := (loop_frame l r c' n i, loop_state body (to_e_list tree_increment))).
      - apply loop_context; unfold to_e_list at 1; rewrite map_app.
        apply (frame_runs_context _ (loop_frame l r c n i, to_e_list body) (loop_frame l r c' n i, []) [] (to_e_list tree_increment)); exact Body.
      - apply loop_increment; cbn in Bound; cbn; lia.
    Qed.

    Lemma loop_body_trap coupons l r c n i body f' : (i < n)%nat -> (Z.of_nat n < 2 ^ 31)%Z ->
      frame_runs (coupon_store coupons) (loop_frame l r c n i, to_e_list body) (f', [AI_trap]) ->
      frame_runs (coupon_store coupons)
        (loop_frame l r c n i, loop_state body (to_e_list (tree_loop_code body))) (f', [AI_trap]).
    Proof.
      intros Less Bound Body; eapply frame_runs_trans; [apply loop_continue; assumption|].
      eapply frame_runs_trans with (middle := (f', loop_state body (AI_trap :: to_e_list tree_increment))).
      - apply loop_context; unfold to_e_list at 1; rewrite map_app.
        apply (frame_runs_context _ (loop_frame l r c n i, to_e_list body) (f', [AI_trap]) [] (to_e_list tree_increment)); exact Body.
      - eapply frame_runs_trans with (middle := (f', loop_state body [AI_trap])).
        + apply loop_context, runs_to_frame_runs, (trap_context _ _ [] (to_e_list tree_increment)); discriminate.
        + eapply frame_runs_trans with (middle := (f', [AI_label 0 [] [AI_trap]])).
          * unfold loop_state; apply (frame_runs_label _
              (f', [AI_label 0 (loop_cont body) [AI_trap]]) (f', [AI_trap]) 0 []).
            apply frame_runs_step, r_simple, rs_label_trap.
          * apply frame_runs_step, r_simple, rs_label_trap.
    Qed.
  End Execution.
End LoopUtil.
