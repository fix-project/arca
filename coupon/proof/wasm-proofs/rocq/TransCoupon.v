From Stdlib Require Import List Bool Arith NArith.
From Wasm Require Import datatypes operations opsem.
From FixProof Require Import Handle ApplyTree CouponApi CouponConstructors Host ModuleLayout CouponTable.
Import ListNotations.
Open Scope list_scope.

Module TransCoupon (S : STORAGE) (P : PROGRAM S) (C : COUPON_STORAGE S) (R : EXTERNREFS S).
  Module T := CouponTable S C R.
  Module B := CouponConstructors S P C.
  Import T T.U T.U.W.

  Lemma mt_some coupons l r res : B.make_trans_coupon coupons l r = Some res ->
    exists e1 e2 rest e1l e1r e2l e2r, coupons = e1 :: e2 :: rest /\
      Api.is_eq_coupon e1 = true /\ Api.is_eq_coupon e2 = true /\
      C.get_coupon_lhs e1 = Some e1l /\ C.get_coupon_rhs e1 = Some e1r /\
      C.get_coupon_lhs e2 = Some e2l /\ C.get_coupon_rhs e2 = Some e2r /\
      Api.is_equal e1r e2l = true /\ Api.is_equal l e1l = true /\ Api.is_equal r e2r = true /\
      res = C.create_coupon C.Eq l r.
  Proof.
    destruct coupons as [|e1 [|e2 rest]]; cbn [B.make_trans_coupon]; try discriminate.
    destruct (B.read_coupon C.Eq e1) as [[e1l e1r]|] eqn:Read1,
      (B.read_coupon C.Eq e2) as [[e2l e2r]|] eqn:Read2; try discriminate.
    destruct (B.API.is_equal e1r e2l && B.API.is_equal l e1l && B.API.is_equal r e2r)%bool eqn:Check; [|discriminate].
    intro Make; inversion Make; subst; apply andb_true_iff in Check as [Checks Right];
      apply andb_true_iff in Checks as [Middle Left].
    destruct (B.read_coupon_type _ _ _ _ Read1) as [Tag1 [Lhs1 Rhs1]],
      (B.read_coupon_type _ _ _ _ Read2) as [Tag2 [Lhs2 Rhs2]].
    exists e1, e2, rest, e1l, e1r, e2l, e2r; repeat split; try assumption; try reflexivity;
      apply Api.is_type_match; assumption.
  Qed.

  Lemma mt_none coupons l r : B.make_trans_coupon coupons l r = None ->
    coupons = [] \/ (exists e1, coupons = [e1]) \/
    exists e1 e2 rest, coupons = e1 :: e2 :: rest /\
      (Api.is_eq_coupon e1 = false \/ Api.is_eq_coupon e2 = false \/
       exists e1l e1r e2l e2r,
         C.get_coupon_lhs e1 = Some e1l /\ C.get_coupon_rhs e1 = Some e1r /\
         C.get_coupon_lhs e2 = Some e2l /\ C.get_coupon_rhs e2 = Some e2r /\
         (Api.is_equal e1r e2l = false \/ Api.is_equal l e1l = false \/ Api.is_equal r e2r = false)).
  Proof.
    destruct coupons as [|e1 [|e2 rest]]; intro Make.
    - left; reflexivity.
    - right; left; exists e1; reflexivity.
    - right; right; exists e1, e2, rest; split; [reflexivity|].
      destruct (B.API.is_type C.Eq e1) eqn:Tag1; [|left; exact Tag1].
      destruct (B.API.is_type C.Eq e2) eqn:Tag2; [|right; left; exact Tag2].
      destruct (B.type_lhs_exist _ _ Tag1) as [e1l Lhs1], (B.type_rhs_exist _ _ Tag1) as [e1r Rhs1],
        (B.type_lhs_exist _ _ Tag2) as [e2l Lhs2], (B.type_rhs_exist _ _ Tag2) as [e2r Rhs2].
      cbn [B.make_trans_coupon] in Make; unfold B.read_coupon in Make;
        rewrite Tag1, Tag2, Lhs1, Rhs1, Lhs2, Rhs2 in Make.
      destruct (B.API.is_equal e1r e2l && B.API.is_equal l e1l && B.API.is_equal r e2r)%bool eqn:Check; [discriminate|].
      apply andb_false_iff in Check as [Check|Right];
        [apply andb_false_iff in Check as [Middle|Left]|];
        right; right; exists e1l, e1r, e2l, e2r; repeat split; try assumption; tauto.
  Qed.

  Section Execution.
    Context `{mem : BlockUpdateMemory}.
    Definition null_ref := VAL_ref (VAL_ref_null T_externref).
    Definition initial_frame l r := Build_frame [extern_value l; extern_value r; null_ref; null_ref] coupon_instance.
    Definition first_frame l r e1 := Build_frame [extern_value l; extern_value r; extern_value e1; null_ref] coupon_instance.
    Definition working_frame l r e1 e2 := Build_frame
      [extern_value l; extern_value r; extern_value e1; extern_value e2] coupon_instance.
    Definition trans_result l r := v_to_e_list [extern_value (C.create_coupon C.Eq l r)].

    Lemma trans_prefix_first e1 rest l r : frame_runs (coupon_store (e1 :: rest))
      (initial_frame l r, to_e_list trans_body)
      (first_frame l r e1, to_e_list (trans_second_prefix ++ trans_test_body)).
    Proof.
      eapply frame_runs_trans with (middle := (initial_frame l r,
        [v_to_e (extern_value e1); AI_basic (BI_local_set 2%N)] ++ to_e_list (trans_second_prefix ++ trans_test_body))).
      - unfold trans_body; unfold to_e_list; rewrite map_app.
        apply (frame_runs_context _ (initial_frame l r,
          [v_to_e (VAL_num (VAL_int32 (i32_of_nat 0))); AI_basic (BI_table_get 0%N)])
          (initial_frame l r, [v_to_e (extern_value e1)]) []
          (AI_basic (BI_local_set 2%N) :: to_e_list (trans_second_prefix ++ trans_test_body))).
        apply frame_runs_step, get_first_coupon; reflexivity.
      - apply frame_runs_step; eapply r_label with (lh := LH_base [] (to_e_list (trans_second_prefix ++ trans_test_body))).
        + eapply r_local_set with (f := initial_frame l r) (f' := first_frame l r e1)
            (i := 2%N) (v := extern_value e1) (vd := null_ref); reflexivity.
        + reflexivity.
        + reflexivity.
    Qed.

    Lemma trans_prefix_second e1 e2 rest l r : frame_runs (coupon_store (e1 :: e2 :: rest))
      (first_frame l r e1, to_e_list (trans_second_prefix ++ trans_test_body))
      (working_frame l r e1 e2, to_e_list trans_test_body).
    Proof.
      eapply frame_runs_trans with (middle := (first_frame l r e1,
        [v_to_e (extern_value e2); AI_basic (BI_local_set 3%N)] ++ to_e_list trans_test_body)).
      - unfold trans_second_prefix; unfold to_e_list; rewrite map_app.
        apply (frame_runs_context _ (first_frame l r e1,
          [v_to_e (VAL_num (VAL_int32 (i32_of_nat 1))); AI_basic (BI_table_get 0%N)])
          (first_frame l r e1, [v_to_e (extern_value e2)]) []
          (AI_basic (BI_local_set 3%N) :: to_e_list trans_test_body)).
        apply frame_runs_step, get_second_coupon; reflexivity.
      - apply frame_runs_step; eapply r_label with (lh := LH_base [] (to_e_list trans_test_body)).
        + eapply r_local_set with (f := first_frame l r e1) (f' := working_frame l r e1 e2)
            (i := 3%N) (v := extern_value e2) (vd := null_ref); reflexivity.
        + reflexivity.
        + reflexivity.
    Qed.

    Lemma trans_tag coupons f j c : f_inst f = coupon_instance -> lookup_N (f_locs f) j = Some (extern_value c) ->
      runs (coupon_store coupons) f [AI_basic (BI_local_get j); AI_basic (BI_call 3%N)]
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_eq_coupon c)))]).
    Proof.
      intros Instance Local; eapply local_index_host with (hf := fixpoint_is_eq_coupon) (addr := 3%N) (v := extern_value c); try reflexivity; try assumption.
      - rewrite Instance; reflexivity.
      - unfold host_values, extern_value; rewrite R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma trans_get_lhs coupons f j c got : f_inst f = coupon_instance -> lookup_N (f_locs f) j = Some (extern_value c) ->
      C.get_coupon_lhs c = Some got -> runs (coupon_store coupons) f
        [AI_basic (BI_local_get j); AI_basic (BI_call 10%N)] (v_to_e_list [extern_value got]).
    Proof.
      intros Instance Local Getter; eapply local_index_host with (hf := fixpoint_get_coupon_lhs) (addr := 10%N) (v := extern_value c);
        try reflexivity; try assumption.
      - rewrite Instance; reflexivity.
      - unfold host_values, extern_value; rewrite R.to_handle_to_externref, Getter; reflexivity.
    Qed.

    Lemma trans_get_rhs coupons f j c got : f_inst f = coupon_instance -> lookup_N (f_locs f) j = Some (extern_value c) ->
      C.get_coupon_rhs c = Some got -> runs (coupon_store coupons) f
        [AI_basic (BI_local_get j); AI_basic (BI_call 11%N)] (v_to_e_list [extern_value got]).
    Proof.
      intros Instance Local Getter; eapply local_index_host with (hf := fixpoint_get_coupon_rhs) (addr := 11%N) (v := extern_value c);
        try reflexivity; try assumption.
      - rewrite Instance; reflexivity.
      - unfold host_values, extern_value; rewrite R.to_handle_to_externref, Getter; reflexivity.
    Qed.

    Lemma trans_guards_run coupons l r e1 e2 e1l e1r e2l e2r :
      C.get_coupon_lhs e1 = Some e1l -> C.get_coupon_rhs e1 = Some e1r ->
      C.get_coupon_lhs e2 = Some e2l -> C.get_coupon_rhs e2 = Some e2r ->
      Forall2 (fun guard bit => runs (coupon_store coupons) (working_frame l r e1 e2) (to_e_list guard)
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool bit))])) trans_guards
        [Api.is_eq_coupon e1; Api.is_eq_coupon e2; Api.is_equal e1r e2l; Api.is_equal l e1l; Api.is_equal r e2r].
    Proof.
      intros Lhs1 Rhs1 Lhs2 Rhs2; constructor; [apply trans_tag; reflexivity|].
      constructor; [apply trans_tag; reflexivity|].
      constructor.
      - apply (equal_runs _ _
          [AI_basic (BI_local_get 2%N); AI_basic (BI_call 11%N)]
          [AI_basic (BI_local_get 3%N); AI_basic (BI_call 10%N)] e1r e2l); [reflexivity| |].
        + eapply trans_get_rhs; [reflexivity|reflexivity|exact Rhs1].
        + eapply trans_get_lhs; [reflexivity|reflexivity|exact Lhs2].
      - constructor.
        + apply (equal_runs _ _ [AI_basic (BI_local_get 0%N)]
            [AI_basic (BI_local_get 2%N); AI_basic (BI_call 10%N)] l e1l); [reflexivity| |].
          * apply runs_step, r_local_get; reflexivity.
          * eapply trans_get_lhs; [reflexivity|reflexivity|exact Lhs1].
        + constructor.
          * apply (equal_runs _ _ [AI_basic (BI_local_get 1%N)]
              [AI_basic (BI_local_get 3%N); AI_basic (BI_call 11%N)] r e2r); [reflexivity| |].
            -- apply runs_step, r_local_get; reflexivity.
            -- eapply trans_get_rhs; [reflexivity|reflexivity|exact Rhs2].
          * constructor.
    Qed.

    Lemma trans_create coupons l r e1 e2 : runs (coupon_store coupons) (working_frame l r e1 e2)
      (to_e_list self_success_body) (trans_result l r).
    Proof.
      eapply local_pair_host with (hf := fixpoint_create_eq_coupon) (addr := 12%N); try reflexivity.
      unfold host_values, extern_value; rewrite !R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma trans_tests_result coupons l r e1 e2 e1l e1r e2l e2r :
      C.get_coupon_lhs e1 = Some e1l -> C.get_coupon_rhs e1 = Some e1r ->
      C.get_coupon_lhs e2 = Some e2l -> C.get_coupon_rhs e2 = Some e2r ->
      runs (coupon_store coupons) (working_frame l r e1 e2) (to_e_list trans_test_body)
        (if forallb (fun b => b)
          [Api.is_eq_coupon e1; Api.is_eq_coupon e2; Api.is_equal e1r e2l; Api.is_equal l e1l; Api.is_equal r e2r]
         then trans_result l r else [AI_trap]).
    Proof.
      intros Lhs1 Rhs1 Lhs2 Rhs2; apply guard_chain_runs.
      - eapply trans_guards_run; eassumption.
      - apply trans_create.
      - left; reflexivity.
    Qed.

    Lemma trans_tests_bad_first coupons l r e1 e2 : Api.is_eq_coupon e1 = false ->
      runs (coupon_store coupons) (working_frame l r e1 e2) (to_e_list trans_test_body) [AI_trap].
    Proof.
      intro Tag; apply guard_chain_bad_head.
      pose proof (trans_tag coupons (working_frame l r e1 e2) 2%N e1 eq_refl eq_refl) as Guard.
      rewrite Tag in Guard; exact Guard.
    Qed.

    Lemma trans_tests_bad_second coupons l r e1 e2 : Api.is_eq_coupon e2 = false ->
      runs (coupon_store coupons) (working_frame l r e1 e2) (to_e_list trans_test_body) [AI_trap].
    Proof.
      intro Tag2; destruct (Api.is_eq_coupon e1) eqn:Tag1; [|apply trans_tests_bad_first; exact Tag1].
      apply guard_chain_good_head.
      - pose proof (trans_tag coupons (working_frame l r e1 e2) 2%N e1 eq_refl eq_refl) as Guard.
        rewrite Tag1 in Guard; exact Guard.
      - apply guard_chain_bad_head.
        pose proof (trans_tag coupons (working_frame l r e1 e2) 3%N e2 eq_refl eq_refl) as Guard.
        rewrite Tag2 in Guard; exact Guard.
      - right; reflexivity.
    Qed.

    Lemma trans_tests_bad_endpoints coupons l r e1 e2 e1l e1r e2l e2r :
      C.get_coupon_lhs e1 = Some e1l -> C.get_coupon_rhs e1 = Some e1r ->
      C.get_coupon_lhs e2 = Some e2l -> C.get_coupon_rhs e2 = Some e2r ->
      (Api.is_equal e1r e2l = false \/ Api.is_equal l e1l = false \/ Api.is_equal r e2r = false) ->
      runs (coupon_store coupons) (working_frame l r e1 e2) (to_e_list trans_test_body) [AI_trap].
    Proof.
      intros Lhs1 Rhs1 Lhs2 Rhs2 Fail.
      pose proof (trans_tests_result coupons l r e1 e2 e1l e1r e2l e2r Lhs1 Rhs1 Lhs2 Rhs2) as Run.
      destruct Fail as [Middle|[Left|Right]]; rewrite ?Middle, ?Left, ?Right in Run;
        destruct (Api.is_eq_coupon e1), (Api.is_eq_coupon e2),
          (Api.is_equal e1r e2l), (Api.is_equal l e1l), (Api.is_equal r e2r); exact Run.
    Qed.

    Lemma trans_prefix_empty l r : frame_runs (coupon_store [])
      (initial_frame l r, to_e_list trans_body) (initial_frame l r, [AI_trap]).
    Proof.
      eapply frame_runs_trans with (middle := (initial_frame l r,
        AI_trap :: AI_basic (BI_local_set 2%N) :: to_e_list (trans_second_prefix ++ trans_test_body))).
      - unfold trans_body; unfold to_e_list; rewrite map_app.
        apply (frame_runs_context _ (initial_frame l r,
          [v_to_e (VAL_num (VAL_int32 (i32_of_nat 0))); AI_basic (BI_table_get 0%N)])
          (initial_frame l r, [AI_trap]) []
          (AI_basic (BI_local_set 2%N) :: to_e_list (trans_second_prefix ++ trans_test_body))).
        apply frame_runs_step, get_empty_coupon; reflexivity.
      - apply frame_runs_step, r_simple; eapply rs_trap with
          (lh := LH_base [] (AI_basic (BI_local_set 2%N) :: to_e_list (trans_second_prefix ++ trans_test_body)));
          [discriminate|reflexivity].
    Qed.

    Lemma trans_prefix_single e1 l r : frame_runs (coupon_store [e1])
      (initial_frame l r, to_e_list trans_body) (first_frame l r e1, [AI_trap]).
    Proof.
      eapply frame_runs_trans; [apply trans_prefix_first|].
      eapply frame_runs_trans with (middle := (first_frame l r e1,
        AI_trap :: AI_basic (BI_local_set 3%N) :: to_e_list trans_test_body)).
      - unfold trans_second_prefix; unfold to_e_list; rewrite map_app.
        apply (frame_runs_context _ (first_frame l r e1,
          [v_to_e (VAL_num (VAL_int32 (i32_of_nat 1))); AI_basic (BI_table_get 0%N)])
          (first_frame l r e1, [AI_trap]) [] (AI_basic (BI_local_set 3%N) :: to_e_list trans_test_body)).
        apply frame_runs_step, r_table_get_failure; reflexivity.
      - apply frame_runs_step, r_simple; eapply rs_trap with
          (lh := LH_base [] (AI_basic (BI_local_set 3%N) :: to_e_list trans_test_body));
          [discriminate|reflexivity].
    Qed.

    Lemma make_trans_coupon_raw_run_invoke_some coupons l r res caller :
      B.make_trans_coupon coupons l r = Some res ->
      runs (coupon_store coupons) caller
        (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_trans_coupon_idx])
        (v_to_e_list [extern_value res]).
    Proof.
      intro Make; destruct (mt_some _ _ _ _ Make) as
        [e1 [e2 [rest [e1l [e1r [e2l [e2r [-> [Tag1 [Tag2 [Lhs1 [Rhs1 [Lhs2 [Rhs2 [Middle [Left [Right ->]]]]]]]]]]]]]]]]].
      eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
        (locals := [T_ref T_externref; T_ref T_externref]) (body := trans_body)
        (defaults := [null_ref; null_ref]) (final_frame := working_frame l r e1 e2).
      - rewrite coupon_store_functions; reflexivity.
      - reflexivity.
      - eapply frame_runs_trans; [apply trans_prefix_first|].
        eapply frame_runs_trans; [apply trans_prefix_second|].
        apply runs_to_frame_runs.
        pose proof (trans_tests_result (e1 :: e2 :: rest) l r e1 e2 e1l e1r e2l e2r Lhs1 Rhs1 Lhs2 Rhs2) as Run.
        rewrite Tag1, Tag2, Middle, Left, Right in Run; exact Run.
      - left; split; reflexivity.
    Qed.

    Lemma make_trans_coupon_raw_run_invoke_none coupons l r caller :
      B.make_trans_coupon coupons l r = None ->
      runs (coupon_store coupons) caller
        (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_trans_coupon_idx]) [AI_trap].
    Proof.
      intro Make; destruct (mt_none _ _ _ Make) as [->|[[e1 ->]|[e1 [e2 [rest [-> Fail]]]]]].
      - eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
          (locals := [T_ref T_externref; T_ref T_externref]) (body := trans_body)
          (defaults := [null_ref; null_ref]) (final_frame := initial_frame l r).
        + rewrite coupon_store_functions; reflexivity.
        + reflexivity.
        + apply trans_prefix_empty.
        + right; reflexivity.
      - eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
          (locals := [T_ref T_externref; T_ref T_externref]) (body := trans_body)
          (defaults := [null_ref; null_ref]) (final_frame := first_frame l r e1).
        + rewrite coupon_store_functions; reflexivity.
        + reflexivity.
        + apply trans_prefix_single.
        + right; reflexivity.
      - eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
          (locals := [T_ref T_externref; T_ref T_externref]) (body := trans_body)
          (defaults := [null_ref; null_ref]) (final_frame := working_frame l r e1 e2).
        + rewrite coupon_store_functions; reflexivity.
        + reflexivity.
        + eapply frame_runs_trans; [apply trans_prefix_first|].
          eapply frame_runs_trans; [apply trans_prefix_second|].
          apply runs_to_frame_runs; destruct Fail as [Tag|[Tag|[e1l [e1r [e2l [e2r [Lhs1 [Rhs1 [Lhs2 [Rhs2 Endpoints]]]]]]]]]].
          * apply trans_tests_bad_first; exact Tag.
          * apply trans_tests_bad_second; exact Tag.
          * eapply trans_tests_bad_endpoints; eassumption.
        + right; reflexivity.
    Qed.

    Theorem make_trans_coupon_success coupons l r res caller :
      Forall B.coupon_good coupons -> B.make_trans_coupon coupons l r = Some res ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_trans_coupon_idx])
        (tt, coupon_store coupons, caller, v_to_e_list [extern_value res]) /\ B.coupon_good res.
    Proof.
      intros Good Make; split; [apply runs_sound; eapply make_trans_coupon_raw_run_invoke_some|eapply B.make_trans_coupon_good]; eassumption.
    Qed.

    Theorem make_trans_coupon_trap coupons l r caller : B.make_trans_coupon coupons l r = None ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_trans_coupon_idx])
        (tt, coupon_store coupons, caller, [AI_trap]).
    Proof. intro Make; apply runs_sound; eapply make_trans_coupon_raw_run_invoke_none; exact Make. Qed.
  End Execution.
End TransCoupon.
