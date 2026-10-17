From Stdlib Require Import List Bool Arith NArith Lia.
From Wasm Require Import datatypes operations opsem.
From FixProof Require Import Handle ApplyTree CouponApi CouponConstructors Host ModuleLayout CouponTable.
Import ListNotations.
Open Scope list_scope.

Module ThinkApplicationCoupon (S : STORAGE) (P : PROGRAM S) (C : COUPON_STORAGE S) (R : EXTERNREFS S).
  Module T := CouponTable S C R.
  Module B := CouponConstructors S P C.
  Import T T.U T.U.W.

  Definition abstract_result cl1 cr1 cl2 cr2 l r := if Api.is_equal cr1 cl2 then
    match Api.create_application_thunk_api cl1 with
    | Some th => if Api.is_equal th l && Api.is_equal cr2 r then Some (C.create_coupon C.Think l r) else None
    | None => None end else None.

  Lemma make_some coupons l r res : B.make_think_application_coupon coupons l r = Some res ->
    exists c1 c2 rest cl1 cr1 cr2, coupons = c1 :: c2 :: rest /\
      Api.is_eval_coupon c1 = true /\ Api.is_apply_coupon c2 = true /\
      C.get_coupon_lhs c1 = Some cl1 /\ C.get_coupon_rhs c1 = Some cr1 /\
      C.get_coupon_lhs c2 = Some cr1 /\ C.get_coupon_rhs c2 = Some cr2 /\
      Api.create_application_thunk_api cl1 = Some l /\ Api.is_equal cr2 r = true /\ res = C.create_coupon C.Think l r.
  Proof.
    destruct coupons as [|c1 [|c2 rest]]; cbn [B.make_think_application_coupon]; try discriminate.
    destruct (B.read_coupon C.Eval c1) as [[cl1 cr1]|] eqn:Read1,
      (B.read_coupon C.Apply c2) as [[cl2 cr2]|] eqn:Read2; try discriminate.
    destruct (B.API.is_equal cr1 cl2) eqn:Middle; [|discriminate].
    destruct (B.API.create_application_thunk_api cl1) as [th|] eqn:Create; [|discriminate].
    destruct (B.API.is_equal th l && B.API.is_equal cr2 r) eqn:Check; [|discriminate].
    intro Make; inversion Make; subst res; apply andb_true_iff in Check as [Left Right].
    apply Api.is_equal_match in Middle; apply Api.is_equal_match in Left; subst cl2 th.
    destruct (B.read_coupon_type _ _ _ _ Read1) as [Tag1 [Lhs1 Rhs1]],
      (B.read_coupon_type _ _ _ _ Read2) as [Tag2 [Lhs2 Rhs2]].
    exists c1, c2, rest, cl1, cr1, cr2; repeat split; try assumption; try reflexivity; apply Api.is_type_match; assumption.
  Qed.

  Lemma make_none coupons l r : B.make_think_application_coupon coupons l r = None ->
    (List.length coupons < 2)%nat \/ exists c1 c2 rest, coupons = c1 :: c2 :: rest /\
      (Api.is_eval_coupon c1 = false \/ Api.is_apply_coupon c2 = false \/
       exists cl1 cr1 cl2 cr2, C.get_coupon_lhs c1 = Some cl1 /\ C.get_coupon_rhs c1 = Some cr1 /\
         C.get_coupon_lhs c2 = Some cl2 /\ C.get_coupon_rhs c2 = Some cr2 /\
         (Api.is_equal cr1 cl2 = false \/ Api.create_application_thunk_api cl1 = None \/ exists th,
           Api.create_application_thunk_api cl1 = Some th /\ (Api.is_equal th l = false \/ Api.is_equal cr2 r = false))).
  Proof.
    destruct coupons as [|c1 [|c2 rest]]; intro Make; try (left; cbn; lia).
    right; exists c1, c2, rest; split; [reflexivity|].
    destruct (B.API.is_type C.Eval c1) eqn:Tag1; [|left; exact Tag1].
    destruct (B.API.is_type C.Apply c2) eqn:Tag2; [|right; left; exact Tag2].
    destruct (B.type_lhs_exist _ _ Tag1) as [cl1 Lhs1], (B.type_rhs_exist _ _ Tag1) as [cr1 Rhs1],
      (B.type_lhs_exist _ _ Tag2) as [cl2 Lhs2], (B.type_rhs_exist _ _ Tag2) as [cr2 Rhs2].
    right; right; exists cl1, cr1, cl2, cr2; split; [exact Lhs1|]; split; [exact Rhs1|]; split; [exact Lhs2|]; split; [exact Rhs2|].
    cbn [B.make_think_application_coupon] in Make; unfold B.read_coupon in Make;
      rewrite Tag1, Tag2, Lhs1, Rhs1, Lhs2, Rhs2 in Make.
    destruct (B.API.is_equal cr1 cl2) eqn:Middle; [|left; exact Middle].
    destruct (B.API.create_application_thunk_api cl1) as [th|] eqn:Create; [|right; left; exact Create].
    destruct (B.API.is_equal th l && B.API.is_equal cr2 r) eqn:Check; [discriminate|].
    apply andb_false_iff in Check; right; right; exists th; auto.
  Qed.

  Section Execution.
    Context `{mem : BlockUpdateMemory}.
    Definition result l r := v_to_e_list [extern_value (C.create_coupon C.Think l r)].
    Definition option_result value := match value with Some h => v_to_e_list [extern_value h] | None => [AI_trap] end.

    Lemma eval_tag coupons l r c1 c2 : runs (coupon_store coupons) (two_working_frame l r c1 c2)
      (to_e_list [BI_local_get 2%N; BI_call 4%N]) (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_eval_coupon c1))) ]).
    Proof.
      eapply local_index_host with (hf := fixpoint_is_eval_coupon) (addr := 4%N); try reflexivity.
      unfold host_values, extern_value; rewrite R.to_handle_to_externref; reflexivity.
    Qed.
    Lemma apply_tag coupons l r c1 c2 : runs (coupon_store coupons) (two_working_frame l r c1 c2)
      (to_e_list [BI_local_get 3%N; BI_call 5%N]) (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_apply_coupon c2))) ]).
    Proof.
      eapply local_index_host with (hf := fixpoint_is_apply_coupon) (addr := 5%N); try reflexivity.
      unfold host_values, extern_value; rewrite R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma prefix_guards_run coupons l r c1 c2 cr1 cl2 : C.get_coupon_rhs c1 = Some cr1 -> C.get_coupon_lhs c2 = Some cl2 ->
      Forall2 (fun guard bit => runs (coupon_store coupons) (two_working_frame l r c1 c2) (to_e_list guard)
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool bit))])) think_application_prefix_guards
        [Api.is_eval_coupon c1; Api.is_apply_coupon c2; Api.is_equal cr1 cl2].
    Proof.
      intros Rhs1 Lhs2; constructor; [apply eval_tag|]; constructor; [apply apply_tag|]; constructor; [|constructor].
      apply (equal_runs _ _ [AI_basic (BI_local_get 2%N); AI_basic (BI_call 11%N)]
        [AI_basic (BI_local_get 3%N); AI_basic (BI_call 10%N)] cr1 cl2); [reflexivity| |].
      - eapply (coupon_get_endpoint false); [reflexivity|reflexivity|exact Rhs1].
      - eapply (coupon_get_endpoint true); [reflexivity|reflexivity|exact Lhs2].
    Qed.

    Lemma application_value coupons l r c1 c2 cl1 : C.get_coupon_lhs c1 = Some cl1 ->
      runs (coupon_store coupons) (two_working_frame l r c1 c2)
        (to_e_list [BI_local_get 2%N; BI_call 10%N; BI_call 7%N]) (option_result (Api.create_application_thunk_api cl1)).
    Proof.
      intro Lhs1; eapply runs_trans with (mid := v_to_e_list [extern_value cl1] ++ [AI_basic (BI_call 7%N)]).
      - apply (runs_context _ _ [AI_basic (BI_local_get 2%N); AI_basic (BI_call 10%N)] _ [] [AI_basic (BI_call 7%N)]).
        eapply (coupon_get_endpoint true); [reflexivity|reflexivity|exact Lhs1].
      - destruct (Api.create_application_thunk_api cl1) as [th|] eqn:Create.
        + eapply call_host with (hf := fixpoint_create_application_thunk) (addr := 7%N); try reflexivity.
          unfold host_values, extern_value; rewrite R.to_handle_to_externref, Create; reflexivity.
        + eapply call_host_none with (hf := fixpoint_create_application_thunk) (addr := 7%N); try reflexivity.
          unfold host_values, extern_value; rewrite R.to_handle_to_externref, Create; reflexivity.
    Qed.

    Lemma create_run coupons l r c1 c2 : runs (coupon_store coupons) (two_working_frame l r c1 c2)
      (to_e_list think_create_body) (result l r).
    Proof.
      eapply local_pair_host with (hf := fixpoint_create_think_coupon) (addr := 14%N); try reflexivity.
      unfold host_values, extern_value; rewrite !R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma last_some coupons l r c1 c2 cl1 cr2 th : C.get_coupon_lhs c1 = Some cl1 -> C.get_coupon_rhs c2 = Some cr2 ->
      Api.create_application_thunk_api cl1 = Some th ->
      runs (coupon_store coupons) (two_working_frame l r c1 c2) (to_e_list think_application_last)
        (if Api.is_equal th l && Api.is_equal cr2 r then result l r else [AI_trap]).
    Proof.
      intros Lhs1 Rhs2 Create.
      assert (Guards : Forall2 (fun guard bit => runs (coupon_store coupons) (two_working_frame l r c1 c2) (to_e_list guard)
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool bit))])) think_application_last_guards
        [Api.is_equal th l; Api.is_equal cr2 r]).
      { constructor.
        - apply (equal_runs _ _ (to_e_list [BI_local_get 2%N; BI_call 10%N; BI_call 7%N]) [AI_basic (BI_local_get 0%N)] th l);
            [reflexivity| |apply runs_step, r_local_get; reflexivity].
          pose proof (application_value coupons l r c1 c2 cl1 Lhs1) as Run; rewrite Create in Run; exact Run.
        - constructor; [|constructor].
          eapply getter_compare with (hf := fixpoint_get_coupon_rhs) (getter_addr := 11%N); try reflexivity.
          unfold host_values, extern_value; rewrite R.to_handle_to_externref, Rhs2; reflexivity. }
      assert (Terminal : terminal_form (result l r)) by (left; reflexivity).
      pose proof (guard_chain_runs _ _ _ _ think_create_body _ Guards (create_run coupons l r c1 c2) Terminal) as Run.
      cbn [forallb] in Run; rewrite Bool.andb_true_r in Run; exact Run.
    Qed.

    Lemma last_none coupons l r c1 c2 cl1 : C.get_coupon_lhs c1 = Some cl1 -> Api.create_application_thunk_api cl1 = None ->
      runs (coupon_store coupons) (two_working_frame l r c1 c2) (to_e_list think_application_last) [AI_trap].
    Proof.
      intros Lhs1 Create; pose proof (application_value coupons l r c1 c2 cl1 Lhs1) as Run; rewrite Create in Run.
      eapply runs_trans with (mid := AI_trap :: AI_basic (BI_local_get 0%N) :: AI_basic (BI_call 0%N) ::
        [AI_basic (BI_if (BT_valtype (Some (T_ref T_externref)))
          (guard_chain [[BI_local_get 3%N; BI_call 11%N; BI_local_get 1%N; BI_call 0%N]] think_create_body) self_failure_body)]).
      - apply (runs_context _ _ (to_e_list [BI_local_get 2%N; BI_call 10%N; BI_call 7%N]) [AI_trap] [] _); exact Run.
      - apply (trap_context _ _ [] _); discriminate.
    Qed.

    Lemma tests_result coupons l r c1 c2 cl1 cr1 cl2 cr2 : Api.is_eval_coupon c1 = true -> Api.is_apply_coupon c2 = true ->
      C.get_coupon_lhs c1 = Some cl1 -> C.get_coupon_rhs c1 = Some cr1 ->
      C.get_coupon_lhs c2 = Some cl2 -> C.get_coupon_rhs c2 = Some cr2 ->
      runs (coupon_store coupons) (two_working_frame l r c1 c2) (to_e_list think_application_tests)
        (option_result (abstract_result cl1 cr1 cl2 cr2 l r)).
    Proof.
      intros Tag1 Tag2 Lhs1 Rhs1 Lhs2 Rhs2; pose proof (prefix_guards_run coupons l r c1 c2 cr1 cl2 Rhs1 Lhs2) as Guards.
      rewrite Tag1, Tag2 in Guards; unfold abstract_result.
      destruct (Api.is_equal cr1 cl2) eqn:Middle.
      - destruct (Api.create_application_thunk_api cl1) as [th|] eqn:Create.
        + pose proof (last_some coupons l r c1 c2 cl1 cr2 th Lhs1 Rhs2 Create) as Last.
          assert (Terminal : terminal_form (if Api.is_equal th l && Api.is_equal cr2 r then result l r else [AI_trap])).
          { destruct (Api.is_equal th l && Api.is_equal cr2 r); [left; reflexivity|right; reflexivity]. }
          pose proof (guard_chain_runs _ _ _ _ think_application_last _ Guards Last Terminal) as Run.
          destruct (Api.is_equal th l && Api.is_equal cr2 r); exact Run.
        + pose proof (guard_chain_runs _ _ _ _ think_application_last [AI_trap] Guards
            (last_none coupons l r c1 c2 cl1 Lhs1 Create) (or_intror eq_refl)) as Run; exact Run.
      - apply (guard_chain_false _ _ _ _ think_application_last Guards); reflexivity.
    Qed.

    Lemma tests_execution c1 c2 rest l r : runs (coupon_store (c1 :: c2 :: rest)) (two_working_frame l r c1 c2)
      (to_e_list think_application_tests) (option_result (B.make_think_application_coupon (c1 :: c2 :: rest) l r)).
    Proof.
      cbn [B.make_think_application_coupon]; unfold B.read_coupon.
      destruct (B.API.is_type C.Eval c1) eqn:Tag1.
      - change (Api.is_eval_coupon c1 = true) in Tag1.
        destruct (B.type_lhs_exist _ _ Tag1) as [cl1 Lhs1], (B.type_rhs_exist _ _ Tag1) as [cr1 Rhs1]; rewrite Lhs1, Rhs1.
        destruct (B.API.is_type C.Apply c2) eqn:Tag2.
        + destruct (B.type_lhs_exist _ _ Tag2) as [cl2 Lhs2], (B.type_rhs_exist _ _ Tag2) as [cr2 Rhs2]; rewrite Lhs2, Rhs2.
          exact (tests_result (c1 :: c2 :: rest) l r c1 c2 cl1 cr1 cl2 cr2 Tag1 Tag2 Lhs1 Rhs1 Lhs2 Rhs2).
        + change (Api.is_apply_coupon c2 = false) in Tag2; apply guard_chain_good_head.
          * pose proof (eval_tag (c1 :: c2 :: rest) l r c1 c2) as Run; rewrite Tag1 in Run; exact Run.
          * apply guard_chain_bad_head; pose proof (apply_tag (c1 :: c2 :: rest) l r c1 c2) as Run; rewrite Tag2 in Run; exact Run.
          * right; reflexivity.
      - change (Api.is_eval_coupon c1 = false) in Tag1.
        apply guard_chain_bad_head; pose proof (eval_tag (c1 :: c2 :: rest) l r c1 c2) as Run; rewrite Tag1 in Run; exact Run.
    Qed.

    Theorem make_think_application_coupon_execution coupons l r caller : runs (coupon_store coupons) caller
      (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_think_application_coupon_idx])
      (option_result (B.make_think_application_coupon coupons l r)).
    Proof.
      apply invoke_two_coupons with (tests := think_application_tests).
      - rewrite coupon_store_functions; reflexivity.
      - destruct (B.make_think_application_coupon coupons l r); [left; split; reflexivity|right; reflexivity].
      - destruct coupons as [|c1 [|c2 rest]]; [reflexivity|reflexivity|apply tests_execution].
    Qed.

    Theorem make_think_application_coupon_success coupons l r res caller : Forall B.coupon_good coupons ->
      B.make_think_application_coupon coupons l r = Some res ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_think_application_coupon_idx])
        (tt, coupon_store coupons, caller, v_to_e_list [extern_value res]) /\ B.coupon_good res.
    Proof.
      intros Good Make; split.
      - apply runs_sound; pose proof (make_think_application_coupon_execution coupons l r caller) as Run; rewrite Make in Run; exact Run.
      - eapply B.make_think_application_coupon_good; eassumption.
    Qed.

    Theorem make_think_application_coupon_trap coupons l r caller : B.make_think_application_coupon coupons l r = None ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_think_application_coupon_idx])
        (tt, coupon_store coupons, caller, [AI_trap]).
    Proof. intro Make; apply runs_sound; pose proof (make_think_application_coupon_execution coupons l r caller) as Run; rewrite Make in Run; exact Run. Qed.
  End Execution.
End ThinkApplicationCoupon.
