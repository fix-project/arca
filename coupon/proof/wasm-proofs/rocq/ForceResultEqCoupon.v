From Stdlib Require Import List Bool Arith NArith Lia.
From Wasm Require Import datatypes operations opsem.
From FixProof Require Import Handle ApplyTree CouponApi CouponConstructors Host ModuleLayout CouponTable.
Import ListNotations.
Open Scope list_scope.

Module ForceResultEqCoupon (S : STORAGE) (P : PROGRAM S) (C : COUPON_STORAGE S) (R : EXTERNREFS S).
  Module T := CouponTable S C R.
  Module B := CouponConstructors S P C.
  Import T T.U T.U.W.
  Definition endpoint_guard f1l f1r f2l f2r el er l r :=
    (Api.is_equal f1r el && Api.is_equal f2r er && Api.is_equal f1l l && Api.is_equal f2l r)%bool.

  Definition success_inputs coupons l r := exists f1 f2 e rest f1l f1r f2l f2r el er,
    coupons = f1 :: f2 :: e :: rest /\ Api.is_force_coupon f1 = true /\ Api.is_force_coupon f2 = true /\ Api.is_eq_coupon e = true /\
    C.get_coupon_lhs f1 = Some f1l /\ C.get_coupon_rhs f1 = Some f1r /\
    C.get_coupon_lhs f2 = Some f2l /\ C.get_coupon_rhs f2 = Some f2r /\
    C.get_coupon_lhs e = Some el /\ C.get_coupon_rhs e = Some er /\
    Api.is_equal f1r el = true /\ Api.is_equal f2r er = true /\ Api.is_equal f1l l = true /\ Api.is_equal f2l r = true.

  Lemma mt_some coupons l r res : B.make_force_result_eq_coupon coupons l r = Some res ->
    success_inputs coupons l r /\ res = C.create_coupon C.Eq l r.
  Proof.
    destruct coupons as [|f1 [|f2 [|e rest]]]; cbn [B.make_force_result_eq_coupon]; try discriminate.
    destruct (B.read_coupon C.Force f1) as [[f1l f1r]|] eqn:Read1,
      (B.read_coupon C.Force f2) as [[f2l f2r]|] eqn:Read2,
      (B.read_coupon C.Eq e) as [[el er]|] eqn:Read3; try discriminate.
    destruct (B.API.is_equal f1r el && B.API.is_equal f2r er && B.API.is_equal f1l l && B.API.is_equal f2l r) eqn:Check; [|discriminate].
    intro Make; inversion Make; subst res; split; [|reflexivity].
    apply andb_true_iff in Check as [Checks Right]; apply andb_true_iff in Checks as [Checks Left];
      apply andb_true_iff in Checks as [First Second].
    destruct (B.read_coupon_type _ _ _ _ Read1) as [Tag1 [Lhs1 Rhs1]],
      (B.read_coupon_type _ _ _ _ Read2) as [Tag2 [Lhs2 Rhs2]], (B.read_coupon_type _ _ _ _ Read3) as [Tag3 [Lhs3 Rhs3]].
    exists f1, f2, e, rest, f1l, f1r, f2l, f2r, el, er; repeat split; try assumption; try reflexivity;
      apply Api.is_type_match; assumption.
  Qed.

  Lemma mt_some_rev coupons l r : success_inputs coupons l r ->
    B.make_force_result_eq_coupon coupons l r = Some (C.create_coupon C.Eq l r).
  Proof.
    intros (f1 & f2 & e & rest & f1l & f1r & f2l & f2r & el & er & -> & Tag1 & Tag2 & Tag3 &
      Lhs1 & Rhs1 & Lhs2 & Rhs2 & Lhs3 & Rhs3 & First & Second & Left & Right).
    cbn [B.make_force_result_eq_coupon]; unfold B.read_coupon.
    change (B.API.is_type C.Force f1 = true) in Tag1; change (B.API.is_type C.Force f2 = true) in Tag2;
      change (B.API.is_type C.Eq e = true) in Tag3.
    change (B.API.is_equal f1r el = true) in First; change (B.API.is_equal f2r er = true) in Second;
      change (B.API.is_equal f1l l = true) in Left; change (B.API.is_equal f2l r = true) in Right.
    rewrite Tag1, Tag2, Tag3, Lhs1, Rhs1, Lhs2, Rhs2, Lhs3, Rhs3, First, Second, Left, Right; reflexivity.
  Qed.

  Lemma mt_none coupons l r : B.make_force_result_eq_coupon coupons l r = None ->
    (List.length coupons < 3)%nat \/ exists f1 f2 e rest, coupons = f1 :: f2 :: e :: rest /\
      (Api.is_force_coupon f1 = false \/ Api.is_force_coupon f2 = false \/ Api.is_eq_coupon e = false \/
       exists f1l f1r f2l f2r el er, C.get_coupon_lhs f1 = Some f1l /\ C.get_coupon_rhs f1 = Some f1r /\
         C.get_coupon_lhs f2 = Some f2l /\ C.get_coupon_rhs f2 = Some f2r /\
         C.get_coupon_lhs e = Some el /\ C.get_coupon_rhs e = Some er /\
         (Api.is_equal f1r el = false \/ Api.is_equal f2r er = false \/ Api.is_equal f1l l = false \/ Api.is_equal f2l r = false)).
  Proof.
    destruct coupons as [|f1 [|f2 [|e rest]]]; intro Make; try (left; cbn; lia).
    right; exists f1, f2, e, rest; split; [reflexivity|].
    destruct (B.API.is_type C.Force f1) eqn:Tag1; [|left; exact Tag1].
    destruct (B.API.is_type C.Force f2) eqn:Tag2; [|right; left; exact Tag2].
    destruct (B.API.is_type C.Eq e) eqn:Tag3; [|right; right; left; exact Tag3].
    destruct (B.type_lhs_exist _ _ Tag1) as [f1l Lhs1], (B.type_rhs_exist _ _ Tag1) as [f1r Rhs1],
      (B.type_lhs_exist _ _ Tag2) as [f2l Lhs2], (B.type_rhs_exist _ _ Tag2) as [f2r Rhs2],
      (B.type_lhs_exist _ _ Tag3) as [el Lhs3], (B.type_rhs_exist _ _ Tag3) as [er Rhs3].
    cbn [B.make_force_result_eq_coupon] in Make; unfold B.read_coupon in Make;
      rewrite Tag1, Tag2, Tag3, Lhs1, Rhs1, Lhs2, Rhs2, Lhs3, Rhs3 in Make.
    destruct (B.API.is_equal f1r el && B.API.is_equal f2r er && B.API.is_equal f1l l && B.API.is_equal f2l r) eqn:Check; [discriminate|].
    apply andb_false_iff in Check as [Checks|Right]; [apply andb_false_iff in Checks as [Checks|Left]|];
      try (apply andb_false_iff in Checks as [First|Second]);
      right; right; right; exists f1l, f1r, f2l, f2r, el, er; repeat split; try assumption; tauto.
  Qed.

  Section Execution.
    Context `{mem : BlockUpdateMemory}.
    Definition initial_frame l r := Build_frame [extern_value l; extern_value r; null_extern; null_extern; null_extern] coupon_instance.
    Definition first_frame l r f1 := Build_frame [extern_value l; extern_value r; extern_value f1; null_extern; null_extern] coupon_instance.
    Definition second_frame l r f1 f2 := Build_frame [extern_value l; extern_value r; extern_value f1; extern_value f2; null_extern] coupon_instance.
    Definition working_frame l r f1 f2 e := Build_frame [extern_value l; extern_value r; extern_value f1; extern_value f2; extern_value e] coupon_instance.
    Definition result l r := v_to_e_list [extern_value (C.create_coupon C.Eq l r)].

    Lemma prefix_first f1 rest l r : frame_runs (coupon_store (f1 :: rest))
      (initial_frame l r, to_e_list force_result_eq_body)
      (first_frame l r f1, to_e_list (trans_second_prefix ++ third_coupon_prefix ++ force_result_eq_tests)).
    Proof. apply table_read_set with (n := 0%nat) (j := 2%N) (h := f1); reflexivity. Qed.
    Lemma prefix_second f1 f2 rest l r : frame_runs (coupon_store (f1 :: f2 :: rest))
      (first_frame l r f1, to_e_list (trans_second_prefix ++ third_coupon_prefix ++ force_result_eq_tests))
      (second_frame l r f1 f2, to_e_list (third_coupon_prefix ++ force_result_eq_tests)).
    Proof. apply table_read_set with (n := 1%nat) (j := 3%N) (h := f2); reflexivity. Qed.
    Lemma prefix_third f1 f2 e rest l r : frame_runs (coupon_store (f1 :: f2 :: e :: rest))
      (second_frame l r f1 f2, to_e_list (third_coupon_prefix ++ force_result_eq_tests))
      (working_frame l r f1 f2 e, to_e_list force_result_eq_tests).
    Proof. apply table_read_set with (n := 2%nat) (j := 4%N) (h := e); reflexivity. Qed.
    Lemma prefix_empty l r : frame_runs (coupon_store []) (initial_frame l r, to_e_list force_result_eq_body)
      (initial_frame l r, [AI_trap]).
    Proof. apply table_read_set_failure with (n := 0%nat) (j := 2%N); reflexivity. Qed.
    Lemma prefix_single f1 l r : frame_runs (coupon_store [f1]) (initial_frame l r, to_e_list force_result_eq_body)
      (first_frame l r f1, [AI_trap]).
    Proof.
      eapply frame_runs_trans; [apply prefix_first|].
      apply table_read_set_failure with (n := 1%nat) (j := 3%N); reflexivity.
    Qed.
    Lemma prefix_pair f1 f2 l r : frame_runs (coupon_store [f1; f2]) (initial_frame l r, to_e_list force_result_eq_body)
      (second_frame l r f1 f2, [AI_trap]).
    Proof.
      eapply frame_runs_trans; [apply prefix_first|]; eapply frame_runs_trans; [apply prefix_second|].
      apply table_read_set_failure with (n := 2%nat) (j := 4%N); reflexivity.
    Qed.

    Lemma force_tag coupons f j c : f_inst f = coupon_instance -> lookup_N (f_locs f) j = Some (extern_value c) ->
      runs (coupon_store coupons) f [AI_basic (BI_local_get j); AI_basic (BI_call 2%N)]
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_force_coupon c))) ]).
    Proof.
      intros Instance Local; eapply local_index_host with (hf := fixpoint_is_force_coupon) (addr := 2%N)
        (v := extern_value c); try reflexivity; try assumption.
      - rewrite Instance; reflexivity.
      - unfold host_values, extern_value; rewrite R.to_handle_to_externref; reflexivity.
    Qed.
    Lemma eq_tag coupons l r f1 f2 e : runs (coupon_store coupons) (working_frame l r f1 f2 e)
      (to_e_list [BI_local_get 4%N; BI_call 3%N]) (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_eq_coupon e))) ]).
    Proof.
      eapply local_index_host with (hf := fixpoint_is_eq_coupon) (addr := 3%N); try reflexivity.
      unfold host_values, extern_value; rewrite R.to_handle_to_externref; reflexivity.
    Qed.
    Lemma tags_run coupons l r f1 f2 e :
      Forall2 (fun guard bit => runs (coupon_store coupons) (working_frame l r f1 f2 e) (to_e_list guard)
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool bit))])) force_result_eq_tag_guards
        [Api.is_force_coupon f1; Api.is_force_coupon f2; Api.is_eq_coupon e].
    Proof.
      constructor; [apply force_tag; reflexivity|]; constructor; [apply force_tag; reflexivity|].
      constructor; [apply eq_tag|constructor].
    Qed.

    Lemma tests_bad_tags coupons l r f1 f2 e :
      forallb (fun b => b) [Api.is_force_coupon f1; Api.is_force_coupon f2; Api.is_eq_coupon e] = false ->
      runs (coupon_store coupons) (working_frame l r f1 f2 e) (to_e_list force_result_eq_tests) [AI_trap].
    Proof. apply guard_chain_false; apply tags_run. Qed.

    Lemma endpoint_guards_run coupons l r f1 f2 e f1l f1r f2l f2r el er :
      C.get_coupon_lhs f1 = Some f1l -> C.get_coupon_rhs f1 = Some f1r ->
      C.get_coupon_lhs f2 = Some f2l -> C.get_coupon_rhs f2 = Some f2r ->
      C.get_coupon_lhs e = Some el -> C.get_coupon_rhs e = Some er ->
      Forall2 (fun guard bit => runs (coupon_store coupons) (working_frame l r f1 f2 e) (to_e_list guard)
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool bit))])) force_result_eq_endpoint_guards
        [Api.is_equal f1r el; Api.is_equal f2r er; Api.is_equal f1l l; Api.is_equal f2l r].
    Proof.
      intros Lhs1 Rhs1 Lhs2 Rhs2 Lhs3 Rhs3; constructor.
      - apply (equal_runs _ _ [AI_basic (BI_local_get 2%N); AI_basic (BI_call 11%N)]
          [AI_basic (BI_local_get 4%N); AI_basic (BI_call 10%N)] f1r el); [reflexivity| |].
        + eapply (coupon_get_endpoint false); [reflexivity|reflexivity|exact Rhs1].
        + eapply (coupon_get_endpoint true); [reflexivity|reflexivity|exact Lhs3].
      - constructor.
        + apply (equal_runs _ _ [AI_basic (BI_local_get 3%N); AI_basic (BI_call 11%N)]
            [AI_basic (BI_local_get 4%N); AI_basic (BI_call 11%N)] f2r er); [reflexivity| |].
          * eapply (coupon_get_endpoint false); [reflexivity|reflexivity|exact Rhs2].
          * eapply (coupon_get_endpoint false); [reflexivity|reflexivity|exact Rhs3].
        + constructor.
          * eapply getter_compare with (hf := fixpoint_get_coupon_lhs) (getter_addr := 10%N); try reflexivity.
            unfold host_values, extern_value; rewrite R.to_handle_to_externref, Lhs1; reflexivity.
          * constructor; [|constructor].
            eapply getter_compare with (hf := fixpoint_get_coupon_lhs) (getter_addr := 10%N); try reflexivity.
            unfold host_values, extern_value; rewrite R.to_handle_to_externref, Lhs2; reflexivity.
    Qed.
    Lemma create_run coupons l r f1 f2 e : runs (coupon_store coupons) (working_frame l r f1 f2 e)
      (to_e_list self_success_body) (result l r).
    Proof.
      eapply local_pair_host with (hf := fixpoint_create_eq_coupon) (addr := 12%N); try reflexivity.
      unfold host_values, extern_value; rewrite !R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma tests_result coupons l r f1 f2 e f1l f1r f2l f2r el er :
      Api.is_force_coupon f1 = true -> Api.is_force_coupon f2 = true -> Api.is_eq_coupon e = true ->
      C.get_coupon_lhs f1 = Some f1l -> C.get_coupon_rhs f1 = Some f1r ->
      C.get_coupon_lhs f2 = Some f2l -> C.get_coupon_rhs f2 = Some f2r ->
      C.get_coupon_lhs e = Some el -> C.get_coupon_rhs e = Some er ->
      runs (coupon_store coupons) (working_frame l r f1 f2 e) (to_e_list force_result_eq_tests)
        (if endpoint_guard f1l f1r f2l f2r el er l r then result l r else [AI_trap]).
    Proof.
      intros Tag1 Tag2 Tag3 Lhs1 Rhs1 Lhs2 Rhs2 Lhs3 Rhs3.
      pose proof (endpoint_guards_run coupons l r f1 f2 e f1l f1r f2l f2r el er Lhs1 Rhs1 Lhs2 Rhs2 Lhs3 Rhs3) as Guards.
      assert (Terminal : terminal_form (result l r)) by (left; reflexivity).
      pose proof (guard_chain_runs _ _ _ _ self_success_body _ Guards (create_run coupons l r f1 f2 e) Terminal) as Endpoints.
      pose proof (tags_run coupons l r f1 f2 e) as Tags; rewrite Tag1, Tag2, Tag3 in Tags.
      assert (Final : terminal_form (if forallb (fun b => b)
        [Api.is_equal f1r el; Api.is_equal f2r er; Api.is_equal f1l l; Api.is_equal f2l r] then result l r else [AI_trap])).
      { destruct (forallb (fun b => b) [Api.is_equal f1r el; Api.is_equal f2r er; Api.is_equal f1l l; Api.is_equal f2l r]);
          [left; reflexivity|right; reflexivity]. }
      pose proof (guard_chain_runs _ _ _ _ (guard_chain force_result_eq_endpoint_guards self_success_body) _ Tags Endpoints Final) as Run.
      unfold endpoint_guard; destruct (Api.is_equal f1r el), (Api.is_equal f2r er), (Api.is_equal f1l l), (Api.is_equal f2l r); exact Run.
    Qed.

    Lemma tests_execution f1 f2 e rest l r : runs (coupon_store (f1 :: f2 :: e :: rest)) (working_frame l r f1 f2 e)
      (to_e_list force_result_eq_tests) (match B.make_force_result_eq_coupon (f1 :: f2 :: e :: rest) l r with
        | Some res => v_to_e_list [extern_value res] | None => [AI_trap] end).
    Proof.
      cbn [B.make_force_result_eq_coupon]; unfold B.read_coupon.
      destruct (B.API.is_type C.Force f1) eqn:Tag1.
      - destruct (B.type_lhs_exist _ _ Tag1) as [f1l Lhs1], (B.type_rhs_exist _ _ Tag1) as [f1r Rhs1]; rewrite Lhs1, Rhs1.
        destruct (B.API.is_type C.Force f2) eqn:Tag2.
        + destruct (B.type_lhs_exist _ _ Tag2) as [f2l Lhs2], (B.type_rhs_exist _ _ Tag2) as [f2r Rhs2]; rewrite Lhs2, Rhs2.
          destruct (B.API.is_type C.Eq e) eqn:Tag3.
          * destruct (B.type_lhs_exist _ _ Tag3) as [el Lhs3], (B.type_rhs_exist _ _ Tag3) as [er Rhs3]; rewrite Lhs3, Rhs3.
            pose proof (tests_result (f1 :: f2 :: e :: rest) l r f1 f2 e f1l f1r f2l f2r el er Tag1 Tag2 Tag3 Lhs1 Rhs1 Lhs2 Rhs2 Lhs3 Rhs3) as Run.
            change (runs (coupon_store (f1 :: f2 :: e :: rest)) (working_frame l r f1 f2 e) (to_e_list force_result_eq_tests)
              (if B.API.is_equal f1r el && B.API.is_equal f2r er && B.API.is_equal f1l l && B.API.is_equal f2l r
               then result l r else [AI_trap])) in Run.
            destruct (B.API.is_equal f1r el && B.API.is_equal f2r er && B.API.is_equal f1l l && B.API.is_equal f2l r); exact Run.
          * apply tests_bad_tags; change (Api.is_eq_coupon e = false) in Tag3.
            rewrite Tag3; destruct (Api.is_force_coupon f1), (Api.is_force_coupon f2); reflexivity.
        + apply tests_bad_tags; change (Api.is_force_coupon f2 = false) in Tag2.
          rewrite Tag2; destruct (Api.is_force_coupon f1); reflexivity.
      - apply tests_bad_tags; change (Api.is_force_coupon f1 = false) in Tag1; rewrite Tag1; reflexivity.
    Qed.

    Theorem make_force_result_eq_coupon_execution coupons l r caller : runs (coupon_store coupons) caller
      (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_force_result_eq_coupon_idx])
      (match B.make_force_result_eq_coupon coupons l r with Some res => v_to_e_list [extern_value res] | None => [AI_trap] end).
    Proof.
      destruct coupons as [|f1 [|f2 [|e rest]]].
      - eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
          (locals := [T_ref T_externref; T_ref T_externref; T_ref T_externref]) (body := force_result_eq_body)
          (defaults := [null_extern; null_extern; null_extern]) (final_frame := initial_frame l r).
        + rewrite coupon_store_functions; reflexivity.
        + reflexivity.
        + apply prefix_empty.
        + right; reflexivity.
      - eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
          (locals := [T_ref T_externref; T_ref T_externref; T_ref T_externref]) (body := force_result_eq_body)
          (defaults := [null_extern; null_extern; null_extern]) (final_frame := first_frame l r f1).
        + rewrite coupon_store_functions; reflexivity.
        + reflexivity.
        + apply prefix_single.
        + right; reflexivity.
      - eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
          (locals := [T_ref T_externref; T_ref T_externref; T_ref T_externref]) (body := force_result_eq_body)
          (defaults := [null_extern; null_extern; null_extern]) (final_frame := second_frame l r f1 f2).
        + rewrite coupon_store_functions; reflexivity.
        + reflexivity.
        + apply prefix_pair.
        + right; reflexivity.
      - eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
          (locals := [T_ref T_externref; T_ref T_externref; T_ref T_externref]) (body := force_result_eq_body)
          (defaults := [null_extern; null_extern; null_extern]) (final_frame := working_frame l r f1 f2 e).
        + rewrite coupon_store_functions; reflexivity.
        + reflexivity.
        + eapply frame_runs_trans; [apply prefix_first|]; eapply frame_runs_trans; [apply prefix_second|].
          eapply frame_runs_trans; [apply prefix_third|apply runs_to_frame_runs, tests_execution].
        + destruct (B.make_force_result_eq_coupon (f1 :: f2 :: e :: rest) l r); [left; split; reflexivity|right; reflexivity].
    Qed.

    Theorem make_force_result_eq_coupon_success coupons l r res caller : Forall B.coupon_good coupons ->
      B.make_force_result_eq_coupon coupons l r = Some res ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_force_result_eq_coupon_idx])
        (tt, coupon_store coupons, caller, v_to_e_list [extern_value res]) /\ B.coupon_good res.
    Proof.
      intros Good Make; split.
      - apply runs_sound; pose proof (make_force_result_eq_coupon_execution coupons l r caller) as Run; rewrite Make in Run; exact Run.
      - eapply B.make_force_result_eq_coupon_good; eassumption.
    Qed.
    Theorem make_force_result_eq_coupon_trap coupons l r caller : B.make_force_result_eq_coupon coupons l r = None ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_force_result_eq_coupon_idx])
        (tt, coupon_store coupons, caller, [AI_trap]).
    Proof. intro Make; apply runs_sound; pose proof (make_force_result_eq_coupon_execution coupons l r caller) as Run; rewrite Make in Run; exact Run. Qed.
  End Execution.
End ForceResultEqCoupon.
