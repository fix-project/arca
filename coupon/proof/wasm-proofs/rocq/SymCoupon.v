From Stdlib Require Import List Bool Arith NArith Lia.
From Wasm Require Import datatypes operations opsem.
From FixProof Require Import Handle ApplyTree CouponApi CouponConstructors Host ModuleLayout CouponTable.
Import ListNotations.
Open Scope list_scope.

Module SymCoupon (S : STORAGE) (P : PROGRAM S) (C : COUPON_STORAGE S) (R : EXTERNREFS S).
  Module T := CouponTable S C R.
  Module B := CouponConstructors S P C.
  Import T T.U T.U.W.

  Lemma ms_some coupons l r res : B.make_sym_coupon coupons l r = Some res ->
    exists c rest cl cr, coupons = c :: rest /\ Api.is_eq_coupon c = true /\
      C.get_coupon_lhs c = Some cl /\ C.get_coupon_rhs c = Some cr /\
      Api.is_equal cr l = true /\ Api.is_equal cl r = true /\ res = C.create_coupon C.Eq l r.
  Proof.
    destruct coupons as [|c rest]; cbn [B.make_sym_coupon]; [discriminate|].
    destruct (B.read_coupon C.Eq c) as [[cl cr]|] eqn:Read; [|discriminate].
    destruct (B.API.is_equal cr l && B.API.is_equal cl r)%bool eqn:Check; [|discriminate].
    intro Make; inversion Make; subst; apply andb_true_iff in Check as [Left Right].
    destruct (B.read_coupon_type _ _ _ _ Read) as [Tag [Lhs Rhs]].
    exists c, rest, cl, cr; repeat split; try assumption; try reflexivity.
    apply Api.is_type_match; exact Tag.
  Qed.

  Lemma ms_none coupons l r : B.make_sym_coupon coupons l r = None ->
    coupons = [] \/ exists c rest, coupons = c :: rest /\
      (Api.is_eq_coupon c = false \/ exists cl cr,
        C.get_coupon_lhs c = Some cl /\ C.get_coupon_rhs c = Some cr /\
        (Api.is_equal cr l = false \/ Api.is_equal cl r = false)).
  Proof.
    destruct coupons as [|c rest]; [intro; left; reflexivity|].
    cbn [B.make_sym_coupon]; intro Make; right; exists c, rest; split; [reflexivity|].
    destruct (B.API.is_type C.Eq c) eqn:Tag; [|left; exact Tag].
    destruct (B.type_lhs_exist _ _ Tag) as [cl Lhs], (B.type_rhs_exist _ _ Tag) as [cr Rhs].
    unfold B.read_coupon in Make; rewrite Tag, Lhs, Rhs in Make.
    destruct (B.API.is_equal cr l && B.API.is_equal cl r)%bool eqn:Check; [discriminate|].
    apply andb_false_iff in Check; right; exists cl, cr; auto.
  Qed.

  Section Execution.
    Context `{mem : BlockUpdateMemory}.
    Definition initial_frame l r := Build_frame
      [extern_value l; extern_value r; VAL_ref (VAL_ref_null T_externref)] coupon_instance.
    Definition working_frame l r c := Build_frame
      [extern_value l; extern_value r; extern_value c] coupon_instance.

    Definition sym_result l r := v_to_e_list [extern_value (C.create_coupon C.Eq l r)].

    Lemma sym_create coupons l r c : runs (coupon_store coupons) (working_frame l r c)
      (to_e_list self_success_body) (sym_result l r).
    Proof.
      eapply local_pair_host with (hf := fixpoint_create_eq_coupon) (addr := 12%N); try reflexivity.
      unfold host_values, extern_value; rewrite !R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma sym_lhs_prologue coupons l r c cl : C.get_coupon_lhs c = Some cl ->
      runs (coupon_store coupons) (working_frame l r c) (to_e_list sym_lhs_body)
        [v_to_e (VAL_num (VAL_int32 (wasm_bool (Api.is_equal cl r))));
         AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) self_success_body self_failure_body)].
    Proof.
      intro Lhs; change (runs (coupon_store coupons) (working_frame l r c)
        ([] ++ [AI_basic (BI_local_get 2%N); AI_basic (BI_call 10%N); AI_basic (BI_local_get 1%N); AI_basic (BI_call 0%N)] ++
          [AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) self_success_body self_failure_body)])
        ([] ++ v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_equal cl r)))] ++
          [AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) self_success_body self_failure_body)])).
      apply (runs_context _ _ _ _ [] _).
      eapply getter_compare with (hf := fixpoint_get_coupon_lhs) (getter_addr := 10%N); try reflexivity.
      unfold host_values, extern_value; rewrite R.to_handle_to_externref, Lhs; reflexivity.
    Qed.

    Lemma sym_rhs_prologue coupons l r c cr : C.get_coupon_rhs c = Some cr ->
      runs (coupon_store coupons) (working_frame l r c) (to_e_list sym_rhs_body)
        [v_to_e (VAL_num (VAL_int32 (wasm_bool (Api.is_equal cr l))));
         AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) sym_lhs_body self_failure_body)].
    Proof.
      intro Rhs; change (runs (coupon_store coupons) (working_frame l r c)
        ([] ++ [AI_basic (BI_local_get 2%N); AI_basic (BI_call 11%N); AI_basic (BI_local_get 0%N); AI_basic (BI_call 0%N)] ++
          [AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) sym_lhs_body self_failure_body)])
        ([] ++ v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_equal cr l)))] ++
          [AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) sym_lhs_body self_failure_body)])).
      apply (runs_context _ _ _ _ [] _).
      eapply getter_compare with (hf := fixpoint_get_coupon_rhs) (getter_addr := 11%N); try reflexivity.
      unfold host_values, extern_value; rewrite R.to_handle_to_externref, Rhs; reflexivity.
    Qed.

    Lemma sym_test_prologue coupons l r c : runs (coupon_store coupons) (working_frame l r c) (to_e_list sym_test_body)
        [v_to_e (VAL_num (VAL_int32 (wasm_bool (Api.is_eq_coupon c))));
         AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) sym_rhs_body self_failure_body)].
    Proof.
      change (runs (coupon_store coupons) (working_frame l r c)
        ([] ++ [AI_basic (BI_local_get 2%N); AI_basic (BI_call 3%N)] ++
          [AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) sym_rhs_body self_failure_body)])
        ([] ++ v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_eq_coupon c)))] ++
          [AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) sym_rhs_body self_failure_body)])).
      apply (runs_context _ _ _ _ [] _).
      eapply local_index_host with (hf := fixpoint_is_eq_coupon) (addr := 3%N); try reflexivity.
      unfold host_values, extern_value; rewrite R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma sym_tests_success coupons l r c cl cr : Api.is_eq_coupon c = true ->
      C.get_coupon_lhs c = Some cl -> C.get_coupon_rhs c = Some cr ->
      Api.is_equal cr l = true -> Api.is_equal cl r = true ->
      runs (coupon_store coupons) (working_frame l r c) (to_e_list sym_test_body) (sym_result l r).
    Proof.
      intros Tag Lhs Rhs Left Right; eapply runs_trans; [apply sym_test_prologue|].
      rewrite Tag; apply if_externref; [left; reflexivity|].
      eapply runs_trans; [apply sym_rhs_prologue; exact Rhs|].
      rewrite Left; apply if_externref; [left; reflexivity|].
      eapply runs_trans; [apply sym_lhs_prologue; exact Lhs|].
      rewrite Right; apply if_externref; [left; reflexivity|apply sym_create].
    Qed.

    Lemma sym_tests_bad_type coupons l r c : Api.is_eq_coupon c = false ->
      runs (coupon_store coupons) (working_frame l r c) (to_e_list sym_test_body) [AI_trap].
    Proof.
      intro Tag; eapply runs_trans; [apply sym_test_prologue|].
      rewrite Tag; apply if_externref; [right; reflexivity|].
      apply runs_step, r_simple, rs_unreachable.
    Qed.

    Lemma sym_tests_bad_endpoint coupons l r c cl cr :
      C.get_coupon_lhs c = Some cl -> C.get_coupon_rhs c = Some cr ->
      (Api.is_equal cr l = false \/ Api.is_equal cl r = false) ->
      runs (coupon_store coupons) (working_frame l r c) (to_e_list sym_test_body) [AI_trap].
    Proof.
      intros Lhs Rhs Fail; eapply runs_trans; [apply sym_test_prologue|].
      apply if_externref; [right; reflexivity|].
      destruct (Api.is_eq_coupon c); [|apply runs_step, r_simple, rs_unreachable].
      eapply runs_trans; [apply sym_rhs_prologue; exact Rhs|].
      apply if_externref; [right; reflexivity|].
      destruct (Api.is_equal cr l) eqn:Left; [|apply runs_step, r_simple, rs_unreachable].
      destruct Fail as [Fail|Right]; [congruence|].
      eapply runs_trans; [apply sym_lhs_prologue; exact Lhs|].
      rewrite Right; apply if_externref; [right; reflexivity|].
      apply runs_step, r_simple, rs_unreachable.
    Qed.

    Lemma sym_prefix_some c coupons l r : frame_runs (coupon_store (c :: coupons))
      (initial_frame l r, to_e_list sym_body) (working_frame l r c, to_e_list sym_test_body).
    Proof.
      eapply frame_runs_trans with (middle := (initial_frame l r,
        [v_to_e (extern_value c); AI_basic (BI_local_set 2%N)] ++ to_e_list sym_test_body)).
      - change (frame_runs (coupon_store (c :: coupons))
          (initial_frame l r, [] ++ [v_to_e (VAL_num (VAL_int32 (i32_of_nat 0))); AI_basic (BI_table_get 0%N)] ++
            (AI_basic (BI_local_set 2%N) :: to_e_list sym_test_body))
          (initial_frame l r, [] ++ [v_to_e (extern_value c)] ++
            (AI_basic (BI_local_set 2%N) :: to_e_list sym_test_body))).
        apply (frame_runs_context _ (initial_frame l r,
          [v_to_e (VAL_num (VAL_int32 (i32_of_nat 0))); AI_basic (BI_table_get 0%N)])
          (initial_frame l r, [v_to_e (extern_value c)]) [] _).
        apply frame_runs_step, get_first_coupon; reflexivity.
      - apply frame_runs_step; eapply r_label with (lh := LH_base [] (to_e_list sym_test_body)).
        + eapply r_local_set with (f := initial_frame l r) (f' := working_frame l r c)
            (i := 2%N) (v := extern_value c) (vd := VAL_ref (VAL_ref_null T_externref));
            [reflexivity|reflexivity|reflexivity].
        + reflexivity.
        + reflexivity.
    Qed.

    Lemma sym_prefix_empty l r : frame_runs (coupon_store [])
      (initial_frame l r, to_e_list sym_body) (initial_frame l r, [AI_trap]).
    Proof.
      eapply frame_runs_trans with (middle := (initial_frame l r,
        AI_trap :: AI_basic (BI_local_set 2%N) :: to_e_list sym_test_body)).
      - change (frame_runs (coupon_store [])
          (initial_frame l r, [] ++ [v_to_e (VAL_num (VAL_int32 (i32_of_nat 0))); AI_basic (BI_table_get 0%N)] ++
            (AI_basic (BI_local_set 2%N) :: to_e_list sym_test_body))
          (initial_frame l r, [] ++ [AI_trap] ++
            (AI_basic (BI_local_set 2%N) :: to_e_list sym_test_body))).
        apply (frame_runs_context _ (initial_frame l r,
          [v_to_e (VAL_num (VAL_int32 (i32_of_nat 0))); AI_basic (BI_table_get 0%N)])
          (initial_frame l r, [AI_trap]) [] _).
        apply frame_runs_step, get_empty_coupon; reflexivity.
      - apply frame_runs_step, r_simple; eapply rs_trap with
          (lh := LH_base [] (AI_basic (BI_local_set 2%N) :: to_e_list sym_test_body));
          [discriminate|reflexivity].
    Qed.

    Theorem make_sym_coupon_empty_trap l r caller :
      reduce_trans (tt, coupon_store [], caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_sym_coupon_idx])
        (tt, coupon_store [], caller, [AI_trap]).
    Proof.
      apply runs_sound; eapply invoke_pair_locals with (inst := coupon_instance)
        (typeidx := 0%N) (locals := [T_ref T_externref]) (body := sym_body)
        (defaults := [VAL_ref (VAL_ref_null T_externref)]) (final_frame := initial_frame l r).
      - rewrite coupon_store_functions; reflexivity.
      - reflexivity.
      - apply sym_prefix_empty.
      - right; reflexivity.
    Qed.

    Lemma make_sym_coupon_raw_run_invoke_some coupons l r res caller :
      B.make_sym_coupon coupons l r = Some res ->
      runs (coupon_store coupons) caller
        (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_sym_coupon_idx])
        (v_to_e_list [extern_value res]).
    Proof.
      intro Make; destruct (ms_some _ _ _ _ Make) as [c [rest [cl [cr [-> [Tag [Lhs [Rhs [Left [Right ->]]]]]]]]]].
      eapply invoke_pair_locals with (inst := coupon_instance)
        (typeidx := 0%N) (locals := [T_ref T_externref]) (body := sym_body)
        (defaults := [VAL_ref (VAL_ref_null T_externref)]) (final_frame := working_frame l r c).
      - rewrite coupon_store_functions; reflexivity.
      - reflexivity.
      - eapply frame_runs_trans with (middle := (working_frame l r c, to_e_list sym_test_body)).
        + apply sym_prefix_some.
        + apply runs_to_frame_runs; eapply sym_tests_success; eassumption.
      - left; split; reflexivity.
    Qed.

    Lemma make_sym_coupon_raw_run_invoke_none coupons l r caller :
      B.make_sym_coupon coupons l r = None ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_sym_coupon_idx])
        (tt, coupon_store coupons, caller, [AI_trap]).
    Proof.
      intro Make; destruct (ms_none _ _ _ Make) as [->|[c [rest [-> Fail]]]].
      - apply make_sym_coupon_empty_trap.
      - apply runs_sound; eapply invoke_pair_locals with (inst := coupon_instance)
          (typeidx := 0%N) (locals := [T_ref T_externref]) (body := sym_body)
          (defaults := [VAL_ref (VAL_ref_null T_externref)]) (final_frame := working_frame l r c).
        + rewrite coupon_store_functions; reflexivity.
        + reflexivity.
        + eapply frame_runs_trans with (middle := (working_frame l r c, to_e_list sym_test_body)).
          * apply sym_prefix_some.
          * apply runs_to_frame_runs; destruct Fail as [Tag|[cl [cr [Lhs [Rhs Endpoints]]]]].
            -- apply sym_tests_bad_type; exact Tag.
            -- eapply sym_tests_bad_endpoint; eassumption.
        + right; reflexivity.
    Qed.

    Theorem make_sym_coupon_success coupons l r res caller :
      Forall B.coupon_good coupons -> B.make_sym_coupon coupons l r = Some res ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_sym_coupon_idx])
        (tt, coupon_store coupons, caller, v_to_e_list [extern_value res]) /\ B.coupon_good res.
    Proof.
      intros Good Make; split.
      - apply runs_sound; eapply make_sym_coupon_raw_run_invoke_some; exact Make.
      - eapply B.make_sym_coupon_good; eassumption.
    Qed.

    Theorem make_sym_coupon_trap coupons l r caller : B.make_sym_coupon coupons l r = None ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_sym_coupon_idx])
        (tt, coupon_store coupons, caller, [AI_trap]).
    Proof. apply make_sym_coupon_raw_run_invoke_none. Qed.
  End Execution.
End SymCoupon.
