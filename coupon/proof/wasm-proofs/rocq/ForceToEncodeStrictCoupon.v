From Stdlib Require Import List Bool Arith NArith.
From Wasm Require Import datatypes operations opsem.
From FixProof Require Import Handle ApplyTree CouponApi CouponConstructors Host ModuleLayout MappedEqCoupon.
Import ListNotations.
Open Scope list_scope.

Module ForceToEncodeStrictCoupon (S : STORAGE) (P : PROGRAM S) (C : COUPON_STORAGE S) (R : EXTERNREFS S).
  Module M := MappedEqCoupon S P C R.
  Import M.T M.T.U M.T.U.W.

  Definition object_guard h := (Nat.eqb (H.get_type h) 0 || Nat.eqb (H.get_type h) 1)%bool.
  Definition endpoint_guard encoded cr l r :=
    (Api.is_equal encoded l && Api.is_equal cr r && object_guard cr)%bool.

  Lemma make_some coupons l r res : M.B.make_force_to_encode_strict_coupon coupons l r = Some res ->
    exists c rest cl, coupons = c :: rest /\ Api.is_force_coupon c = true /\
      C.get_coupon_lhs c = Some cl /\ C.get_coupon_rhs c = Some r /\
      M.B.API.create_strict_encode_api cl = Some l /\ object_guard r = true /\
      res = C.create_coupon C.Eq l r.
  Proof.
    destruct coupons as [|c rest]; cbn [M.B.make_force_to_encode_strict_coupon]; [discriminate|].
    destruct (M.B.read_coupon C.Force c) as [[cl cr]|] eqn:Read; [|discriminate].
    destruct (M.B.API.create_strict_encode_api cl) as [encoded|] eqn:Encode; [|discriminate].
    intro Make; change ((if endpoint_guard encoded cr l r then Some (C.create_coupon C.Eq l r) else None) = Some res) in Make.
    destruct (endpoint_guard encoded cr l r) eqn:Check; [|discriminate].
    inversion Make; subst res; unfold endpoint_guard in Check.
    apply andb_true_iff in Check as [Ends Object]; apply andb_true_iff in Ends as [Left Right].
    apply Api.is_equal_match in Left; apply Api.is_equal_match in Right; subst encoded cr.
    destruct (M.B.read_coupon_type _ _ _ _ Read) as [Tag [Lhs Rhs]].
    exists c, rest, cl; repeat split; try assumption; try reflexivity.
    apply Api.is_type_match; exact Tag.
  Qed.

  Lemma make_none coupons l r : M.B.make_force_to_encode_strict_coupon coupons l r = None ->
    coupons = [] \/ exists c rest, coupons = c :: rest /\
      (Api.is_force_coupon c = false \/ Api.is_force_coupon c = true /\ exists cl cr,
        C.get_coupon_lhs c = Some cl /\ C.get_coupon_rhs c = Some cr /\
        (M.B.API.create_strict_encode_api cl = None \/ exists encoded,
          M.B.API.create_strict_encode_api cl = Some encoded /\
          (object_guard cr = false \/ Api.is_equal encoded l = false \/ Api.is_equal cr r = false))).
  Proof.
    destruct coupons as [|c rest]; [intro; left; reflexivity|].
    intro Make; right; exists c, rest; split; [reflexivity|].
    destruct (M.B.API.is_type C.Force c) eqn:Tag; [|left; exact Tag].
    right; split; [exact Tag|].
    destruct (M.B.type_lhs_exist _ _ Tag) as [cl Lhs], (M.B.type_rhs_exist _ _ Tag) as [cr Rhs].
    exists cl, cr; split; [exact Lhs|]; split; [exact Rhs|].
    cbn [M.B.make_force_to_encode_strict_coupon] in Make; unfold M.B.read_coupon in Make; rewrite Tag, Lhs, Rhs in Make.
    destruct (M.B.API.create_strict_encode_api cl) as [encoded|] eqn:Encode; [|left; reflexivity].
    change ((if endpoint_guard encoded cr l r then Some (C.create_coupon C.Eq l r) else None) = None) in Make.
    destruct (endpoint_guard encoded cr l r) eqn:Check; [discriminate|].
    right; exists encoded; split; [reflexivity|].
    unfold endpoint_guard in Check; apply andb_false_iff in Check as [Ends|Object]; [|auto].
    apply andb_false_iff in Ends; auto.
  Qed.

  Section Execution.
    Context `{mem : BlockUpdateMemory}.

    Lemma force_tag_run coupons l r c : runs (coupon_store coupons) (one_working_frame l r c)
      (to_e_list [BI_local_get 2%N; BI_call 2%N])
      (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_force_coupon c)))]).
    Proof.
      eapply local_index_host with (hf := fixpoint_is_force_coupon) (addr := 2%N); try reflexivity.
      unfold host_values, extern_value; rewrite R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma rhs_object_run coupons l r c cr : C.get_coupon_rhs c = Some cr ->
      runs (coupon_store coupons) (one_working_frame l r c)
        (to_e_list [BI_local_get 2%N; BI_call 11%N; BI_call 20%N])
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (object_guard cr))) ]).
    Proof.
      intro Rhs; eapply runs_trans with (mid := v_to_e_list [extern_value cr] ++ [AI_basic (BI_call 20%N)]).
      - change (runs (coupon_store coupons) (one_working_frame l r c)
          ([] ++ [AI_basic (BI_local_get 2%N); AI_basic (BI_call 11%N)] ++ [AI_basic (BI_call 20%N)])
          ([] ++ v_to_e_list [extern_value cr] ++ [AI_basic (BI_call 20%N)])).
        apply (runs_context _ _ _ _ [] _).
        eapply local_index_host with (hf := fixpoint_get_coupon_rhs) (addr := 11%N); try reflexivity.
        unfold host_values, extern_value; rewrite R.to_handle_to_externref, Rhs; reflexivity.
      - eapply call_host with (hf := fixpoint_is_object) (addr := 20%N); try reflexivity.
        unfold host_values, extern_value; rewrite R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma prefix_guards_run coupons l r c cr : C.get_coupon_rhs c = Some cr ->
      Forall2 (fun guard bit => runs (coupon_store coupons) (one_working_frame l r c)
        (to_e_list guard) (v_to_e_list [VAL_num (VAL_int32 (wasm_bool bit))]))
        force_to_encode_prefix_guards
        [Api.is_force_coupon c; object_guard cr; Api.is_equal cr r].
    Proof.
      intro Rhs; constructor; [apply force_tag_run|].
      constructor; [apply rhs_object_run; exact Rhs|].
      constructor; [|constructor].
      eapply getter_compare with (hf := fixpoint_get_coupon_rhs) (getter_addr := 11%N); try reflexivity.
      unfold host_values, extern_value; rewrite R.to_handle_to_externref, Rhs; reflexivity.
    Qed.

    Lemma tests_bad_type coupons l r c : Api.is_force_coupon c = false ->
      runs (coupon_store coupons) (one_working_frame l r c) (to_e_list force_to_encode_tests) [AI_trap].
    Proof.
      intro Tag; apply guard_chain_bad_head; pose proof (force_tag_run coupons l r c) as Run; rewrite Tag in Run; exact Run.
    Qed.

    Lemma last_some coupons l r c cl encoded : C.get_coupon_lhs c = Some cl ->
      M.B.API.create_strict_encode_api cl = Some encoded ->
      runs (coupon_store coupons) (one_working_frame l r c) (to_e_list force_to_encode_last)
        (if Api.is_equal encoded l then M.result l r else [AI_trap]).
    Proof.
      intros Lhs Encode; unfold force_to_encode_last; eapply runs_trans.
      - apply guard_prologue with (b := Api.is_equal encoded l).
        rewrite (equal_symmetric encoded l).
        eapply (M.mapped_comparison_some M.StrictMap true 0%N l coupons l r c cl encoded);
          [exact Lhs|exact Encode|reflexivity].
      - destruct (Api.is_equal encoded l).
        + apply if_externref; [left; split; reflexivity|apply M.create_run].
        + apply if_externref; [right; reflexivity|apply runs_step, r_simple, rs_unreachable].
    Qed.

    Lemma last_none coupons l r c cl : C.get_coupon_lhs c = Some cl ->
      M.B.API.create_strict_encode_api cl = None ->
      runs (coupon_store coupons) (one_working_frame l r c) (to_e_list force_to_encode_last) [AI_trap].
    Proof.
      intros Lhs Encode; apply (M.comparison_failure M.StrictMap true 0%N l coupons l r c cl self_success_body);
        [exact Lhs|exact Encode|reflexivity].
    Qed.

    Lemma tests_result coupons l r c cl cr : Api.is_force_coupon c = true ->
      C.get_coupon_lhs c = Some cl -> C.get_coupon_rhs c = Some cr ->
      runs (coupon_store coupons) (one_working_frame l r c) (to_e_list force_to_encode_tests)
        (match M.B.API.create_strict_encode_api cl with
         | Some encoded => if endpoint_guard encoded cr l r then M.result l r else [AI_trap]
         | None => [AI_trap] end).
    Proof.
      intros Tag Lhs Rhs; pose proof (prefix_guards_run coupons l r c cr Rhs) as Guards.
      rewrite Tag in Guards; destruct (M.B.API.create_strict_encode_api cl) as [encoded|] eqn:Encode.
      - pose proof (last_some coupons l r c cl encoded Lhs Encode) as Last.
        assert (Terminal : terminal_form (if Api.is_equal encoded l then M.result l r else [AI_trap])).
        { destruct (Api.is_equal encoded l); [left; split; reflexivity|right; reflexivity]. }
        pose proof (guard_chain_runs _ _ _ _ force_to_encode_last _ Guards Last Terminal) as Run.
        cbn [forallb] in Run; rewrite !andb_true_r in Run.
        unfold endpoint_guard; destruct (object_guard cr), (Api.is_equal cr r), (Api.is_equal encoded l); exact Run.
      - pose proof (guard_chain_runs _ _ _ _ force_to_encode_last [AI_trap] Guards
          (last_none coupons l r c cl Lhs Encode) (or_intror eq_refl)) as Run.
        destruct (forallb (fun b => b) [true; object_guard cr; Api.is_equal cr r]); exact Run.
    Qed.

    Theorem make_force_to_encode_strict_coupon_execution coupons l r caller :
      runs (coupon_store coupons) caller
        (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_force_to_encode_strict_coupon_idx])
        (match M.B.make_force_to_encode_strict_coupon coupons l r with
         | Some res => v_to_e_list [extern_value res] | None => [AI_trap] end).
    Proof.
      destruct coupons as [|c rest].
      - cbn [M.B.make_force_to_encode_strict_coupon].
        eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
          (locals := [T_ref T_externref]) (body := force_to_encode_body)
          (defaults := [VAL_ref (VAL_ref_null T_externref)]) (final_frame := one_initial_frame l r).
        + rewrite coupon_store_functions; reflexivity.
        + reflexivity.
        + apply one_coupon_empty.
        + right; reflexivity.
      - destruct (M.B.API.is_type C.Force c) eqn:Tag.
        + destruct (M.B.type_lhs_exist _ _ Tag) as [cl Lhs], (M.B.type_rhs_exist _ _ Tag) as [cr Rhs].
          cbn [M.B.make_force_to_encode_strict_coupon]; unfold M.B.read_coupon; rewrite Tag, Lhs, Rhs.
          change (runs (coupon_store (c :: rest)) caller
            (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_force_to_encode_strict_coupon_idx])
            (match (match M.B.API.create_strict_encode_api cl with
             | Some encoded => if endpoint_guard encoded cr l r then Some (C.create_coupon C.Eq l r) else None
             | None => None end) with
             | Some res => v_to_e_list [extern_value res] | None => [AI_trap] end)).
          eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
            (locals := [T_ref T_externref]) (body := force_to_encode_body)
            (defaults := [VAL_ref (VAL_ref_null T_externref)]) (final_frame := one_working_frame l r c).
          * rewrite coupon_store_functions; reflexivity.
          * reflexivity.
          * eapply frame_runs_trans; [apply one_coupon_prefix|].
            apply runs_to_frame_runs; pose proof (tests_result (c :: rest) l r c cl cr Tag Lhs Rhs) as Run.
            destruct (M.B.API.create_strict_encode_api cl) as [encoded|]; [|exact Run].
            destruct (endpoint_guard encoded cr l r); exact Run.
          * destruct (M.B.API.create_strict_encode_api cl) as [encoded|]; [|right; reflexivity].
            destruct (endpoint_guard encoded cr l r); [left; split; reflexivity|right; reflexivity].
        + cbn [M.B.make_force_to_encode_strict_coupon]; unfold M.B.read_coupon; rewrite Tag.
          eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
            (locals := [T_ref T_externref]) (body := force_to_encode_body)
            (defaults := [VAL_ref (VAL_ref_null T_externref)]) (final_frame := one_working_frame l r c).
          * rewrite coupon_store_functions; reflexivity.
          * reflexivity.
          * eapply frame_runs_trans; [apply one_coupon_prefix|].
            apply runs_to_frame_runs, tests_bad_type; exact Tag.
          * right; reflexivity.
    Qed.

    Theorem make_force_to_encode_strict_coupon_success coupons l r res caller :
      Forall M.B.coupon_good coupons -> M.B.make_force_to_encode_strict_coupon coupons l r = Some res ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_force_to_encode_strict_coupon_idx])
        (tt, coupon_store coupons, caller, v_to_e_list [extern_value res]) /\ M.B.coupon_good res.
    Proof.
      intros Good Make; split.
      - apply runs_sound; pose proof (make_force_to_encode_strict_coupon_execution coupons l r caller) as Run;
          rewrite Make in Run; exact Run.
      - eapply M.B.make_force_to_encode_strict_coupon_good; eassumption.
    Qed.

    Theorem make_force_to_encode_strict_coupon_trap coupons l r caller : M.B.make_force_to_encode_strict_coupon coupons l r = None ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_force_to_encode_strict_coupon_idx])
        (tt, coupon_store coupons, caller, [AI_trap]).
    Proof.
      intro Make; apply runs_sound; pose proof (make_force_to_encode_strict_coupon_execution coupons l r caller) as Run;
        rewrite Make in Run; exact Run.
    Qed.
  End Execution.
End ForceToEncodeStrictCoupon.
