From Stdlib Require Import List Bool Arith NArith.
From Wasm Require Import datatypes operations opsem.
From FixProof Require Import Handle ApplyTree CouponApi CouponConstructors Host ModuleLayout CouponTable.
Import ListNotations.
Open Scope list_scope.

Module MappedEqCoupon (S : STORAGE) (P : PROGRAM S) (C : COUPON_STORAGE S) (R : EXTERNREFS S).
  Module T := CouponTable S C R.
  Module B := CouponConstructors S P C.
  Import T T.U T.U.W.
  Inductive mapping := ApplicationMap | StrictMap.
  Definition mapper k := match k with ApplicationMap => fixpoint_create_application_thunk | StrictMap => fixpoint_create_strict_encode end.
  Definition mapper_idx k := match k with ApplicationMap => 7%N | StrictMap => 8%N end.
  Definition reverse k := match k with ApplicationMap => false | StrictMap => true end.
  Definition function_idx k := match k with ApplicationMap => func_make_eq_application_coupon_idx | StrictMap => func_make_eq_encode_strict_coupon_idx end.
  Definition convert k := match k with ApplicationMap => B.API.create_application_thunk_api | StrictMap => B.API.create_strict_encode_api end.
  Definition make_mapped k := B.make_eq_mapped_coupon (convert k).
  Definition getter_idx (lhs : bool) := if lhs then 10%N else 11%N.
  Definition getter (lhs : bool) := if lhs then fixpoint_get_coupon_lhs else fixpoint_get_coupon_rhs.
  Definition get_endpoint (lhs : bool) := if lhs then C.get_coupon_lhs else C.get_coupon_rhs.
  Definition mapped_value_code k lhs := [BI_local_get 2%N; BI_call (getter_idx lhs); BI_call (mapper_idx k)].

  Lemma make_some k coupons l r res : make_mapped k coupons l r = Some res ->
    exists c rest cl cr vl vr, coupons = c :: rest /\ Api.is_eq_coupon c = true /\
      C.get_coupon_lhs c = Some cl /\ C.get_coupon_rhs c = Some cr /\
      convert k cl = Some vl /\ convert k cr = Some vr /\
      Api.is_equal l vl = true /\ Api.is_equal r vr = true /\ res = C.create_coupon C.Eq l r.
  Proof.
    destruct coupons as [|c rest]; cbn [make_mapped B.make_eq_mapped_coupon]; [discriminate|].
    destruct (B.read_coupon C.Eq c) as [[cl cr]|] eqn:Read; [|discriminate].
    destruct (convert k cl) as [vl|] eqn:LeftMap, (convert k cr) as [vr|] eqn:RightMap; try discriminate.
    destruct (B.API.is_equal l vl && B.API.is_equal r vr) eqn:Check; [|discriminate].
    intro Make; inversion Make; subst res; apply andb_true_iff in Check as [Left Right].
    destruct (B.read_coupon_type _ _ _ _ Read) as [Tag [Lhs Rhs]].
    exists c, rest, cl, cr, vl, vr; repeat split; try assumption; try reflexivity.
    apply Api.is_type_match; exact Tag.
  Qed.

  Lemma make_some_rev k coupons l r :
    (exists c rest cl cr vl vr, coupons = c :: rest /\ Api.is_eq_coupon c = true /\
      C.get_coupon_lhs c = Some cl /\ C.get_coupon_rhs c = Some cr /\
      convert k cl = Some vl /\ convert k cr = Some vr /\
      Api.is_equal l vl = true /\ Api.is_equal r vr = true) ->
    make_mapped k coupons l r = Some (C.create_coupon C.Eq l r).
  Proof.
    intros (c & rest & cl & cr & vl & vr & -> & Tag & Lhs & Rhs & LeftMap & RightMap & Left & Right).
    cbn [make_mapped B.make_eq_mapped_coupon]; unfold B.read_coupon.
    change (B.API.is_type C.Eq c = true) in Tag.
    change (B.API.is_equal l vl = true) in Left; change (B.API.is_equal r vr = true) in Right.
    rewrite Tag, Lhs, Rhs, LeftMap, RightMap, Left, Right; reflexivity.
  Qed.

  Lemma make_none k coupons l r : make_mapped k coupons l r = None ->
    coupons = [] \/ exists c rest, coupons = c :: rest /\
      (Api.is_eq_coupon c = false \/ Api.is_eq_coupon c = true /\ exists cl cr,
        C.get_coupon_lhs c = Some cl /\ C.get_coupon_rhs c = Some cr /\
        (convert k cl = None \/ convert k cr = None \/ exists vl vr,
          convert k cl = Some vl /\ convert k cr = Some vr /\
          (Api.is_equal l vl = false \/ Api.is_equal r vr = false))).
  Proof.
    destruct coupons as [|c rest]; [intro; left; reflexivity|].
    intro Make; right; exists c, rest; split; [reflexivity|].
    destruct (B.API.is_type C.Eq c) eqn:Tag; [|left; exact Tag].
    right; split; [exact Tag|].
    destruct (B.type_lhs_exist _ _ Tag) as [cl Lhs], (B.type_rhs_exist _ _ Tag) as [cr Rhs].
    exists cl, cr; split; [exact Lhs|]; split; [exact Rhs|].
    cbn [make_mapped B.make_eq_mapped_coupon] in Make; unfold B.read_coupon in Make; rewrite Tag, Lhs, Rhs in Make.
    destruct (convert k cl) as [vl|] eqn:LeftMap; [|left; reflexivity].
    destruct (convert k cr) as [vr|] eqn:RightMap; [|right; left; reflexivity].
    destruct (B.API.is_equal l vl && B.API.is_equal r vr) eqn:Check; [discriminate|].
    apply andb_false_iff in Check; right; right; exists vl, vr; auto.
  Qed.

  Section Execution.
    Context `{mem : BlockUpdateMemory}.
    Definition result l r := v_to_e_list [extern_value (C.create_coupon C.Eq l r)].
    Definition value_result value := match value with Some h => v_to_e_list [extern_value h] | None => [AI_trap] end.

    Lemma mapper_values k h : host_values (mapper k) [extern_value h] = handle_result (convert k h).
    Proof. destruct k; unfold host_values, extern_value; rewrite R.to_handle_to_externref; reflexivity. Qed.

    Lemma mapped_value_run k lhs coupons l r c h : get_endpoint lhs c = Some h ->
      runs (coupon_store coupons) (one_working_frame l r c) (to_e_list (mapped_value_code k lhs))
        (value_result (convert k h)).
    Proof.
      intro Getter; eapply runs_trans with (mid := v_to_e_list [extern_value h] ++ [AI_basic (BI_call (mapper_idx k))]).
      - change (runs (coupon_store coupons) (one_working_frame l r c)
          ([] ++ [AI_basic (BI_local_get 2%N); AI_basic (BI_call (getter_idx lhs))] ++ [AI_basic (BI_call (mapper_idx k))])
          ([] ++ v_to_e_list [extern_value h] ++ [AI_basic (BI_call (mapper_idx k))])).
        apply (runs_context _ _ _ _ [] _).
        eapply local_index_host with (hf := getter lhs) (addr := getter_idx lhs) (v := extern_value c);
          try (destruct lhs; reflexivity).
        destruct lhs; unfold get_endpoint in Getter; unfold getter, host_values, extern_value;
          rewrite R.to_handle_to_externref, Getter; reflexivity.
      - destruct (convert k h) as [v|] eqn:Converted.
        + eapply call_host with (hf := mapper k) (addr := mapper_idx k); try (destruct k; reflexivity).
          rewrite mapper_values, Converted; reflexivity.
        + eapply call_host_none with (hf := mapper k) (addr := mapper_idx k); try (destruct k; reflexivity).
          rewrite mapper_values, Converted; reflexivity.
    Qed.

    Lemma mapped_comparison_some k lhs target_idx target coupons l r c h v :
      get_endpoint lhs c = Some h -> convert k h = Some v ->
      lookup_N (f_locs (one_working_frame l r c)) target_idx = Some (extern_value target) ->
      runs (coupon_store coupons) (one_working_frame l r c)
        (to_e_list (mapped_comparison (reverse k) (getter_idx lhs) (mapper_idx k) target_idx))
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_equal target v))) ]).
    Proof.
      intros Getter Converted Local.
      assert (Mapped : runs (coupon_store coupons) (one_working_frame l r c)
        (to_e_list (mapped_value_code k lhs)) (v_to_e_list [extern_value v])).
      { pose proof (mapped_value_run k lhs coupons l r c h Getter) as Run; rewrite Converted in Run; exact Run. }
      assert (Target : runs (coupon_store coupons) (one_working_frame l r c)
        [AI_basic (BI_local_get target_idx)] (v_to_e_list [extern_value target])).
      { apply runs_step, r_local_get; exact Local. }
      destruct k.
      - apply (equal_runs _ _ [AI_basic (BI_local_get target_idx)] (to_e_list (mapped_value_code ApplicationMap lhs)) target v);
          [reflexivity|exact Target|exact Mapped].
      - rewrite (equal_symmetric target v).
        apply (equal_runs _ _ (to_e_list (mapped_value_code StrictMap lhs)) [AI_basic (BI_local_get target_idx)] v target);
          [reflexivity|exact Mapped|exact Target].
    Qed.

    Lemma mapped_comparison_none k lhs target_idx target coupons l r c h :
      get_endpoint lhs c = Some h -> convert k h = None ->
      lookup_N (f_locs (one_working_frame l r c)) target_idx = Some (extern_value target) ->
      runs (coupon_store coupons) (one_working_frame l r c)
        (to_e_list (mapped_comparison (reverse k) (getter_idx lhs) (mapper_idx k) target_idx)) [AI_trap].
    Proof.
      intros Getter Converted Local.
      pose proof (mapped_value_run k lhs coupons l r c h Getter) as Mapped; rewrite Converted in Mapped.
      destruct k.
      - eapply runs_trans with (mid := v_to_e_list [extern_value target] ++
          to_e_list (mapped_value_code ApplicationMap lhs) ++ [AI_basic (BI_call 0%N)]).
        + apply runs_step; change (reduce tt (coupon_store coupons) (one_working_frame l r c)
            ([] ++ [AI_basic (BI_local_get target_idx)] ++ (to_e_list (mapped_value_code ApplicationMap lhs) ++ [AI_basic (BI_call 0%N)]))
            tt (coupon_store coupons) (one_working_frame l r c)
            ([] ++ [v_to_e (extern_value target)] ++ (to_e_list (mapped_value_code ApplicationMap lhs) ++ [AI_basic (BI_call 0%N)]))).
          apply (step_context _ _ _ _ [] _), r_local_get; exact Local.
        + eapply runs_trans with (mid := v_to_e_list [extern_value target] ++ [AI_trap; AI_basic (BI_call 0%N)]).
          * apply (runs_context _ _ (to_e_list (mapped_value_code ApplicationMap lhs)) [AI_trap]
              [extern_value target] [AI_basic (BI_call 0%N)]); exact Mapped.
          * apply (trap_context _ _ [extern_value target] [AI_basic (BI_call 0%N)]); discriminate.
      - eapply runs_trans with (mid := AI_trap :: AI_basic (BI_local_get target_idx) :: AI_basic (BI_call 0%N) :: []).
        + apply (runs_context _ _ (to_e_list (mapped_value_code StrictMap lhs)) [AI_trap] []
            [AI_basic (BI_local_get target_idx); AI_basic (BI_call 0%N)]); exact Mapped.
        + apply (trap_context _ _ [] [AI_basic (BI_local_get target_idx); AI_basic (BI_call 0%N)]); discriminate.
    Qed.

    Lemma tag_run coupons l r c : runs (coupon_store coupons) (one_working_frame l r c)
      (to_e_list [BI_local_get 2%N; BI_call 3%N])
      (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_eq_coupon c)))]).
    Proof.
      eapply local_index_host with (hf := fixpoint_is_eq_coupon) (addr := 3%N); try reflexivity.
      unfold host_values, extern_value; rewrite R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma create_run coupons l r c : runs (coupon_store coupons) (one_working_frame l r c)
      (to_e_list self_success_body) (result l r).
    Proof.
      eapply local_pair_host with (hf := fixpoint_create_eq_coupon) (addr := 12%N); try reflexivity.
      unfold host_values, extern_value; rewrite !R.to_handle_to_externref; reflexivity.
    Qed.

    Definition tests k := guard_chain (mapped_guards (mapper_idx k) (reverse k)) self_success_body.
    Definition left_tests k := guard_chain
      [mapped_comparison (reverse k) 10%N (mapper_idx k) 0%N;
       mapped_comparison (reverse k) 11%N (mapper_idx k) 1%N] self_success_body.
    Definition right_tests k := guard_chain [mapped_comparison (reverse k) 11%N (mapper_idx k) 1%N] self_success_body.

    Lemma comparison_failure k lhs idx target coupons l r c h body :
      get_endpoint lhs c = Some h -> convert k h = None ->
      lookup_N (f_locs (one_working_frame l r c)) idx = Some (extern_value target) ->
      runs (coupon_store coupons) (one_working_frame l r c)
        (to_e_list (mapped_comparison (reverse k) (getter_idx lhs) (mapper_idx k) idx ++
          [BI_if (BT_valtype (Some (T_ref T_externref))) body self_failure_body])) [AI_trap].
    Proof.
      intros Getter Converted Local.
      unfold to_e_list at 1; rewrite map_app.
      eapply runs_trans with (mid := AI_trap :: AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) body self_failure_body) :: []).
      - apply (runs_context _ _
          (to_e_list (mapped_comparison (reverse k) (getter_idx lhs) (mapper_idx k) idx)) [AI_trap] []
          [AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) body self_failure_body)]).
        eapply mapped_comparison_none; eassumption.
      - apply (trap_context _ _ [] [AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) body self_failure_body)]); discriminate.
    Qed.

    Lemma tests_bad_type k coupons l r c : Api.is_eq_coupon c = false ->
      runs (coupon_store coupons) (one_working_frame l r c) (to_e_list (tests k)) [AI_trap].
    Proof.
      intro Tag; apply guard_chain_bad_head.
      pose proof (tag_run coupons l r c) as Run; rewrite Tag in Run; exact Run.
    Qed.

    Lemma tests_result k coupons l r c cl cr : Api.is_eq_coupon c = true ->
      C.get_coupon_lhs c = Some cl -> C.get_coupon_rhs c = Some cr ->
      runs (coupon_store coupons) (one_working_frame l r c) (to_e_list (tests k))
        (match convert k cl, convert k cr with
         | Some vl, Some vr => if Api.is_equal l vl && Api.is_equal r vr then result l r else [AI_trap]
         | _, _ => [AI_trap] end).
    Proof.
      intros Tag Lhs Rhs; apply guard_chain_good_head.
      - pose proof (tag_run coupons l r c) as Run; rewrite Tag in Run; exact Run.
      - destruct (convert k cl) as [vl|] eqn:Left, (convert k cr) as [vr|] eqn:Right.
        + pose proof (guard_chain_runs (coupon_store coupons) (one_working_frame l r c)
            [mapped_comparison (reverse k) 10%N (mapper_idx k) 0%N;
             mapped_comparison (reverse k) 11%N (mapper_idx k) 1%N]
            [Api.is_equal l vl; Api.is_equal r vr] self_success_body (result l r)) as Run.
          assert (Guards : Forall2 (fun guard bit => runs (coupon_store coupons) (one_working_frame l r c)
            (to_e_list guard) (v_to_e_list [VAL_num (VAL_int32 (wasm_bool bit))]))
            [mapped_comparison (reverse k) 10%N (mapper_idx k) 0%N; mapped_comparison (reverse k) 11%N (mapper_idx k) 1%N]
            [Api.is_equal l vl; Api.is_equal r vr]).
          { constructor.
            - eapply mapped_comparison_some with (lhs := true) (h := cl); try reflexivity; eassumption.
            - constructor; [eapply mapped_comparison_some with (lhs := false) (h := cr); try reflexivity; eassumption|constructor]. }
          specialize (Run Guards (create_run coupons l r c) (or_introl eq_refl)).
          destruct (Api.is_equal l vl), (Api.is_equal r vr); exact Run.
        + cbn [guard_chain]; eapply runs_trans; [apply guard_prologue; eapply mapped_comparison_some with (lhs := true) (h := cl); try reflexivity; eassumption|].
          apply if_externref; [right; reflexivity|].
          destruct (Api.is_equal l vl); [|apply runs_step, r_simple, rs_unreachable].
          apply comparison_failure with (lhs := false) (target := r) (h := cr); try reflexivity; assumption.
        + cbn [guard_chain]; apply comparison_failure with (lhs := true) (target := l) (h := cl); try reflexivity; assumption.
        + cbn [guard_chain]; apply comparison_failure with (lhs := true) (target := l) (h := cl); try reflexivity; assumption.
      - destruct (convert k cl), (convert k cr); [destruct (Api.is_equal l h && Api.is_equal r h0)| | |];
          [left; reflexivity|right; reflexivity|right; reflexivity|right; reflexivity|right; reflexivity].
    Qed.

    Theorem mapped_execution k coupons l r caller :
      runs (coupon_store coupons) caller
        (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke (function_idx k)])
        (match make_mapped k coupons l r with Some res => v_to_e_list [extern_value res] | None => [AI_trap] end).
    Proof.
      destruct coupons as [|c rest].
      - cbn [make_mapped B.make_eq_mapped_coupon].
        eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
          (locals := [T_ref T_externref]) (body := mapped_body (mapper_idx k) (reverse k))
          (defaults := [VAL_ref (VAL_ref_null T_externref)]) (final_frame := one_initial_frame l r).
        + destruct k; rewrite coupon_store_functions; reflexivity.
        + reflexivity.
        + apply one_coupon_empty.
        + right; reflexivity.
      - destruct (B.API.is_type C.Eq c) eqn:Tag.
        + destruct (B.type_lhs_exist _ _ Tag) as [cl Lhs], (B.type_rhs_exist _ _ Tag) as [cr Rhs].
          cbn [make_mapped B.make_eq_mapped_coupon]; unfold B.read_coupon; rewrite Tag, Lhs, Rhs.
          eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
            (locals := [T_ref T_externref]) (body := mapped_body (mapper_idx k) (reverse k))
            (defaults := [VAL_ref (VAL_ref_null T_externref)]) (final_frame := one_working_frame l r c).
          * destruct k; rewrite coupon_store_functions; reflexivity.
          * reflexivity.
          * eapply frame_runs_trans; [apply one_coupon_prefix|].
            apply runs_to_frame_runs; pose proof (tests_result k (c :: rest) l r c cl cr Tag Lhs Rhs) as Run.
            destruct (convert k cl), (convert k cr); try exact Run.
            change (runs (coupon_store (c :: rest)) (one_working_frame l r c) (to_e_list (tests k))
              (if B.API.is_equal l h && B.API.is_equal r h0 then result l r else [AI_trap])) in Run.
            destruct (B.API.is_equal l h && B.API.is_equal r h0); exact Run.
          * destruct (convert k cl), (convert k cr); try (right; reflexivity).
            destruct (B.API.is_equal l h && B.API.is_equal r h0); [left; split; reflexivity|right; reflexivity].
        + cbn [make_mapped B.make_eq_mapped_coupon]; unfold B.read_coupon; rewrite Tag.
          eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
            (locals := [T_ref T_externref]) (body := mapped_body (mapper_idx k) (reverse k))
            (defaults := [VAL_ref (VAL_ref_null T_externref)]) (final_frame := one_working_frame l r c).
          * destruct k; rewrite coupon_store_functions; reflexivity.
          * reflexivity.
          * eapply frame_runs_trans; [apply one_coupon_prefix|].
            apply runs_to_frame_runs, tests_bad_type; exact Tag.
          * right; reflexivity.
    Qed.

    Lemma make_mapped_good k coupons l r res : Forall B.coupon_good coupons ->
      make_mapped k coupons l r = Some res -> B.coupon_good res.
    Proof.
      destruct k; [apply B.make_eq_application_coupon_good|apply B.make_eq_encode_strict_coupon_good].
    Qed.

    Theorem mapped_success k coupons l r res caller : Forall B.coupon_good coupons ->
      make_mapped k coupons l r = Some res ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke (function_idx k)])
        (tt, coupon_store coupons, caller, v_to_e_list [extern_value res]) /\ B.coupon_good res.
    Proof.
      intros Good Make; split.
      - apply runs_sound; pose proof (mapped_execution k coupons l r caller) as Run; rewrite Make in Run; exact Run.
      - eapply make_mapped_good; eassumption.
    Qed.

    Theorem mapped_trap k coupons l r caller : make_mapped k coupons l r = None ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke (function_idx k)])
        (tt, coupon_store coupons, caller, [AI_trap]).
    Proof.
      intro Make; apply runs_sound; pose proof (mapped_execution k coupons l r caller) as Run; rewrite Make in Run; exact Run.
    Qed.

    Theorem make_eq_application_coupon_success coupons l r res caller : Forall B.coupon_good coupons ->
      B.make_eq_application_coupon coupons l r = Some res ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_eq_application_coupon_idx])
        (tt, coupon_store coupons, caller, v_to_e_list [extern_value res]) /\ B.coupon_good res.
    Proof. apply (mapped_success ApplicationMap). Qed.

    Theorem make_eq_application_coupon_trap coupons l r caller : B.make_eq_application_coupon coupons l r = None ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_eq_application_coupon_idx])
        (tt, coupon_store coupons, caller, [AI_trap]).
    Proof. apply (mapped_trap ApplicationMap). Qed.

    Theorem make_eq_encode_strict_coupon_success coupons l r res caller : Forall B.coupon_good coupons ->
      B.make_eq_encode_strict_coupon coupons l r = Some res ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_eq_encode_strict_coupon_idx])
        (tt, coupon_store coupons, caller, v_to_e_list [extern_value res]) /\ B.coupon_good res.
    Proof. apply (mapped_success StrictMap). Qed.

    Theorem make_eq_encode_strict_coupon_trap coupons l r caller : B.make_eq_encode_strict_coupon coupons l r = None ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_eq_encode_strict_coupon_idx])
        (tt, coupon_store coupons, caller, [AI_trap]).
    Proof. apply (mapped_trap StrictMap). Qed.
  End Execution.
End MappedEqCoupon.
