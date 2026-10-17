From Stdlib Require Import List Bool Arith NArith Lia.
From Wasm Require Import datatypes operations opsem.
From FixProof Require Import Handle ApplyTree CouponApi CouponConstructors Host ModuleLayout CouponTable.
Import ListNotations.
Open Scope list_scope.

Module EvalEqCoupon (S : STORAGE) (P : PROGRAM S) (C : COUPON_STORAGE S) (R : EXTERNREFS S).
  Module T := CouponTable S C R.
  Module B := CouponConstructors S P C.
  Import T T.U T.U.W.
  Definition endpoint_guard cl1 cr1 cl2 cr2 l r :=
    (Api.is_equal cl1 cl2 && Api.is_equal cr2 l && Api.is_equal cr1 r)%bool.

  Lemma make_some coupons l r res : B.make_eval_eq_coupon coupons l r = Some res ->
    exists c1 c2 rest cl, coupons = c1 :: c2 :: rest /\ Api.is_eval_coupon c1 = true /\ Api.is_eq_coupon c2 = true /\
      C.get_coupon_lhs c1 = Some cl /\ C.get_coupon_rhs c1 = Some r /\
      C.get_coupon_lhs c2 = Some cl /\ C.get_coupon_rhs c2 = Some l /\ res = C.create_coupon C.Eval l r.
  Proof.
    destruct coupons as [|c1 [|c2 rest]]; cbn [B.make_eval_eq_coupon]; try discriminate.
    destruct (B.read_coupon C.Eval c1) as [[cl1 cr1]|] eqn:Read1,
      (B.read_coupon C.Eq c2) as [[cl2 cr2]|] eqn:Read2; try discriminate.
    intro Make; change ((if endpoint_guard cl1 cr1 cl2 cr2 l r then Some (C.create_coupon C.Eval l r) else None) = Some res) in Make.
    destruct (endpoint_guard cl1 cr1 cl2 cr2 l r) eqn:Check; [|discriminate].
    inversion Make; subst res; unfold endpoint_guard in Check; apply andb_true_iff in Check as [Ends Right].
    apply andb_true_iff in Ends as [Middle Left]; apply Api.is_equal_match in Middle;
      apply Api.is_equal_match in Left; apply Api.is_equal_match in Right; subst cl2 cr2 cr1.
    destruct (B.read_coupon_type _ _ _ _ Read1) as [Tag1 [Lhs1 Rhs1]],
      (B.read_coupon_type _ _ _ _ Read2) as [Tag2 [Lhs2 Rhs2]].
    exists c1, c2, rest, cl1; repeat split; try assumption; try reflexivity; apply Api.is_type_match; assumption.
  Qed.

  Lemma make_none coupons l r : B.make_eval_eq_coupon coupons l r = None ->
    (List.length coupons < 2)%nat \/ exists c1 c2 rest, coupons = c1 :: c2 :: rest /\
      (Api.is_eval_coupon c1 = false \/ Api.is_eq_coupon c2 = false \/
       exists cl1 cr1 cl2 cr2, C.get_coupon_lhs c1 = Some cl1 /\ C.get_coupon_rhs c1 = Some cr1 /\
         C.get_coupon_lhs c2 = Some cl2 /\ C.get_coupon_rhs c2 = Some cr2 /\
         (Api.is_equal cl1 cl2 = false \/ Api.is_equal cr2 l = false \/ Api.is_equal cr1 r = false)).
  Proof.
    destruct coupons as [|c1 [|c2 rest]]; intro Make; try (left; cbn; lia).
    right; exists c1, c2, rest; split; [reflexivity|].
    destruct (B.API.is_type C.Eval c1) eqn:Tag1; [|left; exact Tag1].
    destruct (B.API.is_type C.Eq c2) eqn:Tag2; [|right; left; exact Tag2].
    destruct (B.type_lhs_exist _ _ Tag1) as [cl1 Lhs1], (B.type_rhs_exist _ _ Tag1) as [cr1 Rhs1],
      (B.type_lhs_exist _ _ Tag2) as [cl2 Lhs2], (B.type_rhs_exist _ _ Tag2) as [cr2 Rhs2].
    cbn [B.make_eval_eq_coupon] in Make; unfold B.read_coupon in Make; rewrite Tag1, Tag2, Lhs1, Rhs1, Lhs2, Rhs2 in Make.
    destruct (B.API.is_equal cl1 cl2 && B.API.is_equal cr2 l && B.API.is_equal cr1 r) eqn:Check; [discriminate|].
    apply andb_false_iff in Check as [Ends|Right]; [apply andb_false_iff in Ends as [Middle|Left]|];
      right; right; exists cl1, cr1, cl2, cr2; repeat split; try assumption; tauto.
  Qed.

  Section Execution.
    Context `{mem : BlockUpdateMemory}.
    Definition result l r := v_to_e_list [extern_value (C.create_coupon C.Eval l r)].

    Lemma eval_tag coupons l r c1 c2 : runs (coupon_store coupons) (two_working_frame l r c1 c2)
      (to_e_list [BI_local_get 2%N; BI_call 4%N])
      (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_eval_coupon c1))) ]).
    Proof.
      eapply local_index_host with (hf := fixpoint_is_eval_coupon) (addr := 4%N); try reflexivity.
      unfold host_values, extern_value; rewrite R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma eq_tag coupons l r c1 c2 : runs (coupon_store coupons) (two_working_frame l r c1 c2)
      (to_e_list [BI_local_get 3%N; BI_call 3%N])
      (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_eq_coupon c2))) ]).
    Proof.
      eapply local_index_host with (hf := fixpoint_is_eq_coupon) (addr := 3%N); try reflexivity.
      unfold host_values, extern_value; rewrite R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma guards_run coupons l r c1 c2 cl1 cr1 cl2 cr2 :
      C.get_coupon_lhs c1 = Some cl1 -> C.get_coupon_rhs c1 = Some cr1 ->
      C.get_coupon_lhs c2 = Some cl2 -> C.get_coupon_rhs c2 = Some cr2 ->
      Forall2 (fun guard bit => runs (coupon_store coupons) (two_working_frame l r c1 c2)
        (to_e_list guard) (v_to_e_list [VAL_num (VAL_int32 (wasm_bool bit))])) eval_eq_guards
        [Api.is_eval_coupon c1; Api.is_eq_coupon c2; Api.is_equal cl1 cl2; Api.is_equal cr2 l; Api.is_equal cr1 r].
    Proof.
      intros Lhs1 Rhs1 Lhs2 Rhs2; constructor; [apply eval_tag|].
      constructor; [apply eq_tag|]; constructor.
      - apply (equal_runs _ _ [AI_basic (BI_local_get 2%N); AI_basic (BI_call 10%N)]
          [AI_basic (BI_local_get 3%N); AI_basic (BI_call 10%N)] cl1 cl2); [reflexivity| |].
        + eapply (coupon_get_endpoint true); [reflexivity|reflexivity|exact Lhs1].
        + eapply (coupon_get_endpoint true); [reflexivity|reflexivity|exact Lhs2].
      - constructor.
        + eapply getter_compare with (hf := fixpoint_get_coupon_rhs) (getter_addr := 11%N); try reflexivity.
          unfold host_values, extern_value; rewrite R.to_handle_to_externref, Rhs2; reflexivity.
        + constructor; [|constructor].
          eapply getter_compare with (hf := fixpoint_get_coupon_rhs) (getter_addr := 11%N); try reflexivity.
          unfold host_values, extern_value; rewrite R.to_handle_to_externref, Rhs1; reflexivity.
    Qed.

    Lemma create_run coupons l r c1 c2 : runs (coupon_store coupons) (two_working_frame l r c1 c2)
      (to_e_list eval_create_body) (result l r).
    Proof.
      eapply local_pair_host with (hf := fixpoint_create_eval_coupon) (addr := 13%N); try reflexivity.
      unfold host_values, extern_value; rewrite !R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma tests_bad_first coupons l r c1 c2 : Api.is_eval_coupon c1 = false ->
      runs (coupon_store coupons) (two_working_frame l r c1 c2) (to_e_list eval_eq_tests) [AI_trap].
    Proof. intro Tag; apply guard_chain_bad_head; pose proof (eval_tag coupons l r c1 c2) as Run; rewrite Tag in Run; exact Run. Qed.

    Lemma tests_bad_second coupons l r c1 c2 : Api.is_eval_coupon c1 = true -> Api.is_eq_coupon c2 = false ->
      runs (coupon_store coupons) (two_working_frame l r c1 c2) (to_e_list eval_eq_tests) [AI_trap].
    Proof.
      intros Tag1 Tag2; apply guard_chain_good_head.
      - pose proof (eval_tag coupons l r c1 c2) as Run; rewrite Tag1 in Run; exact Run.
      - apply guard_chain_bad_head; pose proof (eq_tag coupons l r c1 c2) as Run; rewrite Tag2 in Run; exact Run.
      - right; reflexivity.
    Qed.

    Lemma tests_result coupons l r c1 c2 cl1 cr1 cl2 cr2 : Api.is_eval_coupon c1 = true -> Api.is_eq_coupon c2 = true ->
      C.get_coupon_lhs c1 = Some cl1 -> C.get_coupon_rhs c1 = Some cr1 ->
      C.get_coupon_lhs c2 = Some cl2 -> C.get_coupon_rhs c2 = Some cr2 ->
      runs (coupon_store coupons) (two_working_frame l r c1 c2) (to_e_list eval_eq_tests)
        (if endpoint_guard cl1 cr1 cl2 cr2 l r then result l r else [AI_trap]).
    Proof.
      intros Tag1 Tag2 Lhs1 Rhs1 Lhs2 Rhs2.
      pose proof (guards_run coupons l r c1 c2 cl1 cr1 cl2 cr2 Lhs1 Rhs1 Lhs2 Rhs2) as Guards.
      assert (Terminal : terminal_form (result l r)) by (left; reflexivity).
      pose proof (guard_chain_runs _ _ _ _ eval_create_body _ Guards (create_run coupons l r c1 c2) Terminal) as Run.
      rewrite Tag1, Tag2 in Run; unfold endpoint_guard.
      destruct (Api.is_equal cl1 cl2), (Api.is_equal cr2 l), (Api.is_equal cr1 r); exact Run.
    Qed.

    Lemma tests_execution c1 c2 rest l r : runs (coupon_store (c1 :: c2 :: rest)) (two_working_frame l r c1 c2)
      (to_e_list eval_eq_tests) (match B.make_eval_eq_coupon (c1 :: c2 :: rest) l r with
        | Some res => v_to_e_list [extern_value res] | None => [AI_trap] end).
    Proof.
      cbn [B.make_eval_eq_coupon]; unfold B.read_coupon.
      destruct (B.API.is_type C.Eval c1) eqn:Tag1; [|apply tests_bad_first; exact Tag1].
      destruct (B.type_lhs_exist _ _ Tag1) as [cl1 Lhs1], (B.type_rhs_exist _ _ Tag1) as [cr1 Rhs1].
      rewrite Lhs1, Rhs1; destruct (B.API.is_type C.Eq c2) eqn:Tag2; [|apply tests_bad_second; assumption].
      destruct (B.type_lhs_exist _ _ Tag2) as [cl2 Lhs2], (B.type_rhs_exist _ _ Tag2) as [cr2 Rhs2].
      rewrite Lhs2, Rhs2.
      pose proof (tests_result (c1 :: c2 :: rest) l r c1 c2 cl1 cr1 cl2 cr2 Tag1 Tag2 Lhs1 Rhs1 Lhs2 Rhs2) as Run.
      change (runs (coupon_store (c1 :: c2 :: rest)) (two_working_frame l r c1 c2) (to_e_list eval_eq_tests)
        (if B.API.is_equal cl1 cl2 && B.API.is_equal cr2 l && B.API.is_equal cr1 r then result l r else [AI_trap])) in Run.
      destruct (B.API.is_equal cl1 cl2 && B.API.is_equal cr2 l && B.API.is_equal cr1 r); exact Run.
    Qed.

    Theorem make_eval_eq_coupon_execution coupons l r caller : runs (coupon_store coupons) caller
      (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_eval_eq_coupon_idx])
      (match B.make_eval_eq_coupon coupons l r with Some res => v_to_e_list [extern_value res] | None => [AI_trap] end).
    Proof.
      apply invoke_two_coupons with (tests := eval_eq_tests).
      - rewrite coupon_store_functions; reflexivity.
      - destruct (B.make_eval_eq_coupon coupons l r); [left; split; reflexivity|right; reflexivity].
      - destruct coupons as [|c1 [|c2 rest]]; [reflexivity|reflexivity|apply tests_execution].
    Qed.

    Theorem make_eval_eq_coupon_success coupons l r res caller : Forall B.coupon_good coupons ->
      B.make_eval_eq_coupon coupons l r = Some res ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_eval_eq_coupon_idx])
        (tt, coupon_store coupons, caller, v_to_e_list [extern_value res]) /\ B.coupon_good res.
    Proof.
      intros Good Make; split.
      - apply runs_sound; pose proof (make_eval_eq_coupon_execution coupons l r caller) as Run; rewrite Make in Run; exact Run.
      - eapply B.make_eval_eq_coupon_good; eassumption.
    Qed.

    Theorem make_eval_eq_coupon_trap coupons l r caller : B.make_eval_eq_coupon coupons l r = None ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_eval_eq_coupon_idx])
        (tt, coupon_store coupons, caller, [AI_trap]).
    Proof. intro Make; apply runs_sound; pose proof (make_eval_eq_coupon_execution coupons l r caller) as Run; rewrite Make in Run; exact Run. Qed.
  End Execution.
End EvalEqCoupon.
