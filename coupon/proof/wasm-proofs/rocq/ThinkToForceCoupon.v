From Stdlib Require Import List Bool Arith NArith.
From Wasm Require Import datatypes operations opsem.
From FixProof Require Import Handle ApplyTree CouponApi CouponConstructors Host ModuleLayout CouponTable.
Import ListNotations.
Open Scope list_scope.

Module ThinkToForceCoupon (S : STORAGE) (P : PROGRAM S) (C : COUPON_STORAGE S) (R : EXTERNREFS S).
  Module T := CouponTable S C R.
  Module B := CouponConstructors S P C.
  Import T T.U T.U.W.

  Definition data_guard h := (Nat.eqb (H.get_type h) 0 || Nat.eqb (H.get_type h) 1 ||
    Nat.eqb (H.get_type h) 2 || Nat.eqb (H.get_type h) 3)%bool.
  Definition endpoint_guard cl cr l r := (data_guard cr && (Api.is_equal cl l && Api.is_equal cr r))%bool.

  Lemma make_some coupons l r res : B.make_think_to_force_coupon coupons l r = Some res ->
    exists c rest, coupons = c :: rest /\ Api.is_think_coupon c = true /\
      C.get_coupon_lhs c = Some l /\ C.get_coupon_rhs c = Some r /\
      data_guard r = true /\ res = C.create_coupon C.Force l r.
  Proof.
    destruct coupons as [|c rest]; cbn [B.make_think_to_force_coupon]; [discriminate|].
    destruct (B.read_coupon C.Think c) as [[cl cr]|] eqn:Read; [|discriminate].
    intro Make.
    change ((if endpoint_guard cl cr l r then Some (C.create_coupon C.Force l r) else None) = Some res) in Make.
    destruct (endpoint_guard cl cr l r) eqn:Check; [|discriminate].
    inversion Make; subst res.
    unfold endpoint_guard in Check; apply andb_true_iff in Check as [Data Ends].
    apply andb_true_iff in Ends as [Left Right].
    apply Api.is_equal_match in Left; apply Api.is_equal_match in Right; subst cl cr.
    destruct (B.read_coupon_type _ _ _ _ Read) as [Tag [Lhs Rhs]].
    exists c, rest; repeat split; try assumption; try reflexivity.
    apply Api.is_type_match; exact Tag.
  Qed.

  Lemma make_none coupons l r : B.make_think_to_force_coupon coupons l r = None ->
    coupons = [] \/ exists c rest, coupons = c :: rest /\
      (Api.is_think_coupon c = false \/ exists cl cr,
        C.get_coupon_lhs c = Some cl /\ C.get_coupon_rhs c = Some cr /\
        (data_guard cr = false \/ Api.is_equal cl l = false \/ Api.is_equal cr r = false)).
  Proof.
    destruct coupons as [|c rest]; [intro; left; reflexivity|].
    intro Make; right; exists c, rest; split; [reflexivity|].
    destruct (B.API.is_type C.Think c) eqn:Tag; [|left; exact Tag].
    destruct (B.type_lhs_exist _ _ Tag) as [cl Lhs], (B.type_rhs_exist _ _ Tag) as [cr Rhs].
    cbn [B.make_think_to_force_coupon] in Make; unfold B.read_coupon in Make; rewrite Tag, Lhs, Rhs in Make.
    change ((if endpoint_guard cl cr l r then Some (C.create_coupon C.Force l r) else None) = None) in Make.
    destruct (endpoint_guard cl cr l r) eqn:Check; [discriminate|].
    unfold endpoint_guard in Check; apply andb_false_iff in Check as [Data|Ends].
    - right; exists cl, cr; auto.
    - apply andb_false_iff in Ends; right; exists cl, cr; auto.
  Qed.

  Section Execution.
    Context `{mem : BlockUpdateMemory}.
    Definition result l r := v_to_e_list [extern_value (C.create_coupon C.Force l r)].

    Lemma think_tag_run coupons l r c : runs (coupon_store coupons) (one_working_frame l r c)
      (to_e_list [BI_local_get 2%N; BI_call 6%N])
      (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_think_coupon c)))]).
    Proof.
      eapply local_index_host with (hf := fixpoint_is_think_coupon) (addr := 6%N); try reflexivity.
      unfold host_values, extern_value; rewrite R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma rhs_data_run coupons l r c cr : C.get_coupon_rhs c = Some cr ->
      runs (coupon_store coupons) (one_working_frame l r c)
        (to_e_list [BI_local_get 2%N; BI_call 11%N; BI_call 19%N])
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (data_guard cr))) ]).
    Proof.
      intro Rhs; eapply runs_trans with (mid := v_to_e_list [extern_value cr] ++ [AI_basic (BI_call 19%N)]).
      - change (runs (coupon_store coupons) (one_working_frame l r c)
          ([] ++ [AI_basic (BI_local_get 2%N); AI_basic (BI_call 11%N)] ++ [AI_basic (BI_call 19%N)])
          ([] ++ v_to_e_list [extern_value cr] ++ [AI_basic (BI_call 19%N)])).
        apply (runs_context _ _ _ _ [] _).
        eapply local_index_host with (hf := fixpoint_get_coupon_rhs) (addr := 11%N); try reflexivity.
        unfold host_values, extern_value; rewrite R.to_handle_to_externref, Rhs; reflexivity.
      - eapply call_host with (hf := fixpoint_is_data) (addr := 19%N); try reflexivity.
        unfold host_values, extern_value; rewrite R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma create_run coupons l r c : runs (coupon_store coupons) (one_working_frame l r c)
      (to_e_list force_create_body) (result l r).
    Proof.
      eapply local_pair_host with (hf := fixpoint_create_force_coupon) (addr := 15%N); try reflexivity.
      unfold host_values, extern_value; rewrite !R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma tests_bad_type coupons l r c : Api.is_think_coupon c = false ->
      runs (coupon_store coupons) (one_working_frame l r c) (to_e_list think_to_force_tests) [AI_trap].
    Proof.
      intro Tag; apply guard_chain_bad_head; pose proof (think_tag_run coupons l r c) as Run; rewrite Tag in Run; exact Run.
    Qed.

    Lemma tests_result coupons l r c cl cr : Api.is_think_coupon c = true ->
      C.get_coupon_lhs c = Some cl -> C.get_coupon_rhs c = Some cr ->
      runs (coupon_store coupons) (one_working_frame l r c) (to_e_list think_to_force_tests)
        (if endpoint_guard cl cr l r then result l r else [AI_trap]).
    Proof.
      intros Tag Lhs Rhs.
      assert (Guards : Forall2 (fun guard bit => runs (coupon_store coupons) (one_working_frame l r c)
        (to_e_list guard) (v_to_e_list [VAL_num (VAL_int32 (wasm_bool bit))]))
        think_to_force_guards [true; data_guard cr; Api.is_equal cl l; Api.is_equal cr r]).
      { constructor.
        - pose proof (think_tag_run coupons l r c) as Run; rewrite Tag in Run; exact Run.
        - constructor; [apply rhs_data_run; exact Rhs|].
          constructor.
          + eapply getter_compare with (hf := fixpoint_get_coupon_lhs) (getter_addr := 10%N); try reflexivity.
            unfold host_values, extern_value; rewrite R.to_handle_to_externref, Lhs; reflexivity.
          + constructor.
            * eapply getter_compare with (hf := fixpoint_get_coupon_rhs) (getter_addr := 11%N); try reflexivity.
              unfold host_values, extern_value; rewrite R.to_handle_to_externref, Rhs; reflexivity.
            * constructor. }
      assert (Terminal : terminal_form (result l r)) by (left; split; reflexivity).
      pose proof (guard_chain_runs _ _ _ _ force_create_body (result l r) Guards
        (create_run coupons l r c) Terminal) as Run.
      cbn [forallb] in Run; rewrite !andb_true_r in Run; exact Run.
    Qed.

    Theorem make_think_to_force_coupon_execution coupons l r caller :
      runs (coupon_store coupons) caller
        (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_think_to_force_coupon_idx])
        (match B.make_think_to_force_coupon coupons l r with
         | Some res => v_to_e_list [extern_value res] | None => [AI_trap] end).
    Proof.
      destruct coupons as [|c rest].
      - cbn [B.make_think_to_force_coupon].
        eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
          (locals := [T_ref T_externref]) (body := think_to_force_body)
          (defaults := [VAL_ref (VAL_ref_null T_externref)]) (final_frame := one_initial_frame l r).
        + rewrite coupon_store_functions; reflexivity.
        + reflexivity.
        + apply one_coupon_empty.
        + right; reflexivity.
      - destruct (B.API.is_type C.Think c) eqn:Tag.
        + destruct (B.type_lhs_exist _ _ Tag) as [cl Lhs], (B.type_rhs_exist _ _ Tag) as [cr Rhs].
          cbn [B.make_think_to_force_coupon]; unfold B.read_coupon; rewrite Tag, Lhs, Rhs.
          change (runs (coupon_store (c :: rest)) caller
            (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_think_to_force_coupon_idx])
            (match (if endpoint_guard cl cr l r then Some (C.create_coupon C.Force l r) else None) with
             | Some res => v_to_e_list [extern_value res] | None => [AI_trap] end)).
          eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
            (locals := [T_ref T_externref]) (body := think_to_force_body)
            (defaults := [VAL_ref (VAL_ref_null T_externref)]) (final_frame := one_working_frame l r c).
          * rewrite coupon_store_functions; reflexivity.
          * reflexivity.
          * eapply frame_runs_trans; [apply one_coupon_prefix|].
            apply runs_to_frame_runs; pose proof (tests_result (c :: rest) l r c cl cr Tag Lhs Rhs) as Run.
            destruct (endpoint_guard cl cr l r); exact Run.
          * destruct (endpoint_guard cl cr l r); [left; split; reflexivity|right; reflexivity].
        + cbn [B.make_think_to_force_coupon]; unfold B.read_coupon; rewrite Tag.
          eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
            (locals := [T_ref T_externref]) (body := think_to_force_body)
            (defaults := [VAL_ref (VAL_ref_null T_externref)]) (final_frame := one_working_frame l r c).
          * rewrite coupon_store_functions; reflexivity.
          * reflexivity.
          * eapply frame_runs_trans; [apply one_coupon_prefix|].
            apply runs_to_frame_runs, tests_bad_type; exact Tag.
          * right; reflexivity.
    Qed.

    Theorem make_think_to_force_coupon_success coupons l r res caller :
      Forall B.coupon_good coupons -> B.make_think_to_force_coupon coupons l r = Some res ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_think_to_force_coupon_idx])
        (tt, coupon_store coupons, caller, v_to_e_list [extern_value res]) /\ B.coupon_good res.
    Proof.
      intros Good Make; split.
      - apply runs_sound; pose proof (make_think_to_force_coupon_execution coupons l r caller) as Run;
          rewrite Make in Run; exact Run.
      - eapply B.make_think_to_force_coupon_good; eassumption.
    Qed.

    Theorem make_think_to_force_coupon_trap coupons l r caller : B.make_think_to_force_coupon coupons l r = None ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_think_to_force_coupon_idx])
        (tt, coupon_store coupons, caller, [AI_trap]).
    Proof.
      intro Make; apply runs_sound; pose proof (make_think_to_force_coupon_execution coupons l r caller) as Run;
        rewrite Make in Run; exact Run.
    Qed.
  End Execution.
End ThinkToForceCoupon.
