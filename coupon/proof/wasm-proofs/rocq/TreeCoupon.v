From Stdlib Require Import List Bool Arith NArith ZArith Lia.
From Wasm Require Import datatypes operations opsem.
From FixProof Require Import Handle ApplyTree CouponApi CouponConstructors Host
  ModuleLayout CouponTable TreeLayout WasmNatural LoopUtil.
Import ListNotations.
Open Scope list_scope.

(** Eq and Eval use the same two-loop program, differing only in the tag
    predicate and the final coupon constructor. *)
Module TreeCoupon (S : STORAGE) (P : PROGRAM S) (C : COUPON_STORAGE S) (R : EXTERNREFS S).
  Module L := LoopUtil S C R.
  Module B := CouponConstructors S P C.
  Import L L.T L.T.U L.T.U.W.
  Inductive tree_kind := TreeEq | TreeEval.
  Definition tree_tag kind := match kind with TreeEq => C.Eq | TreeEval => C.Eval end.
  Definition tag_index kind := match kind with TreeEq => 3%N | TreeEval => 4%N end.
  Definition tag_host kind := match kind with TreeEq => fixpoint_is_eq_coupon | TreeEval => fixpoint_is_eval_coupon end.
  Definition tag_check kind c := Api.is_type (tree_tag kind) c.
  Definition create_index kind := match kind with TreeEq => 12%N | TreeEval => 13%N end.
  Definition create_host kind := match kind with TreeEq => fixpoint_create_eq_coupon | TreeEval => fixpoint_create_eval_coupon end.
  Definition native_index kind := match kind with TreeEq => 21%N | TreeEval => 22%N end.
  Definition size_guard (left : bool) := [BI_local_get (if left then 0%N else 1%N); BI_call 16%N;
    BI_local_get 3%N; BI_relop T_i32 (Relop_i ROI_eq)].
  Definition tree_size_bounded h := forall n, Api.get_tree_size_api h = Some n -> (Z.of_nat n < 2 ^ 31)%Z.

  Lemma make_some kind coupons l r c : B.make_tree_coupon (tree_tag kind) coupons l r = Some c ->
    forallb (tag_check kind) coupons = true /\ B.has_tree_size l (List.length coupons) = true /\
    B.has_tree_size r (List.length coupons) = true /\
    forallb (B.tree_entry_match coupons l r) (seq 0 (List.length coupons)) = true /\
    c = C.create_coupon (tree_tag kind) l r.
  Proof.
    unfold B.make_tree_coupon; change
      ((if forallb (tag_check kind) coupons then
        if B.has_tree_size l (List.length coupons) && B.has_tree_size r (List.length coupons) then
          if forallb (B.tree_entry_match coupons l r) (seq 0 (List.length coupons)) then
            Some (C.create_coupon (tree_tag kind) l r) else None else None else None) = Some c ->
        forallb (tag_check kind) coupons = true /\ B.has_tree_size l (List.length coupons) = true /\
        B.has_tree_size r (List.length coupons) = true /\
        forallb (B.tree_entry_match coupons l r) (seq 0 (List.length coupons)) = true /\
        c = C.create_coupon (tree_tag kind) l r).
    destruct (forallb (tag_check kind) coupons) eqn:Tags,
      (B.has_tree_size l (List.length coupons)) eqn:SizeL,
      (B.has_tree_size r (List.length coupons)) eqn:SizeR,
      (forallb (B.tree_entry_match coupons l r) (seq 0 (List.length coupons))) eqn:Entries;
        cbn; intro Make; try discriminate; inversion Make; repeat split; reflexivity.
  Qed.

  Lemma make_some_rev kind coupons l r : forallb (tag_check kind) coupons = true ->
    B.has_tree_size l (List.length coupons) = true -> B.has_tree_size r (List.length coupons) = true ->
    forallb (B.tree_entry_match coupons l r) (seq 0 (List.length coupons)) = true ->
    B.make_tree_coupon (tree_tag kind) coupons l r = Some (C.create_coupon (tree_tag kind) l r).
  Proof.
    intros Tags SizeL SizeR Entries; unfold B.make_tree_coupon.
    change (forallb (B.API.is_type (tree_tag kind)) coupons = true) in Tags.
    rewrite Tags, SizeL, SizeR, Entries; reflexivity.
  Qed.

  Lemma make_none kind coupons l r : B.make_tree_coupon (tree_tag kind) coupons l r = None ->
    forallb (tag_check kind) coupons = false \/ B.has_tree_size l (List.length coupons) = false \/
    B.has_tree_size r (List.length coupons) = false \/
    forallb (B.tree_entry_match coupons l r) (seq 0 (List.length coupons)) = false.
  Proof.
    destruct (forallb (tag_check kind) coupons) eqn:Tags; [|auto].
    destruct (B.has_tree_size l (List.length coupons)) eqn:SizeL; [|auto].
    destruct (B.has_tree_size r (List.length coupons)) eqn:SizeR; [|auto].
    destruct (forallb (B.tree_entry_match coupons l r) (seq 0 (List.length coupons))) eqn:Entries; [|auto].
    rewrite (make_some_rev kind coupons l r Tags SizeL SizeR Entries); discriminate.
  Qed.

  Lemma has_tree_size_bounded h n : (Z.of_nat n < 2 ^ 31)%Z ->
    B.has_tree_size h n = true -> tree_size_bounded h.
  Proof.
    intros Bound Has m Size; unfold B.has_tree_size in Has.
    change (match Api.get_tree_size_api h with Some m => Nat.eqb m n | None => false end = true) in Has.
    rewrite Size in Has; apply Nat.eqb_eq in Has; subst m; exact Bound.
  Qed.

  Section Execution.
    Context `{mem : BlockUpdateMemory}.

    Lemma counter_coupon_read coupons l r old n i c :
      (Z.of_nat i < 2 ^ 32)%Z -> nth_error coupons i = Some c ->
      runs (coupon_store coupons) (loop_frame l r old n i)
        (to_e_list [BI_local_get 4%N; BI_table_get 0%N]) (v_to_e_list [extern_value c]).
    Proof.
      intros Bound Lookup.
      eapply runs_trans with (mid := [v_to_e (VAL_num (VAL_int32 (wasm_nat i))); AI_basic (BI_table_get 0%N)]).
      - apply runs_step, (step_context _ _ [AI_basic (BI_local_get 4%N)]
          [v_to_e (VAL_num (VAL_int32 (wasm_nat i)))] [] [AI_basic (BI_table_get 0%N)]), r_local_get; reflexivity.
      - apply runs_step, r_table_get_success; change
          (stab_elem (coupon_store coupons) coupon_instance 0%N (Wasm_int.N_of_uint i32m (wasm_nat i)) =
            Some (VAL_ref_extern (R.to_externref c))).
        apply coupon_table_lookup; unfold lookup_N; rewrite (wasm_nat_to_N i Bound), Nat2N.id; exact Lookup.
    Qed.

    Lemma tag_value_run kind coupons f c : f_inst f = coupon_instance ->
      runs (coupon_store coupons) f (v_to_e_list [extern_value c] ++ [AI_basic (BI_call (tag_index kind))])
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (tag_check kind c)))]).
    Proof.
      intro Instance; eapply call_host with (hf := tag_host kind) (addr := tag_index kind).
      - rewrite Instance; destruct kind; reflexivity.
      - rewrite coupon_store_functions; destruct kind; reflexivity.
      - destruct kind; unfold host_values, extern_value; rewrite R.to_handle_to_externref; reflexivity.
      - destruct kind; reflexivity.
    Qed.

    Lemma tag_guard_run kind coupons l r old n i c :
      (Z.of_nat i < 2 ^ 32)%Z -> nth_error coupons i = Some c ->
      runs (coupon_store coupons) (loop_frame l r old n i)
        (to_e_list [BI_local_get 4%N; BI_table_get 0%N; BI_call (tag_index kind)])
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (tag_check kind c)))]).
    Proof.
      intros Bound Lookup.
      eapply runs_trans with (mid := v_to_e_list [extern_value c] ++ [AI_basic (BI_call (tag_index kind))]).
      - apply (runs_context _ _ (to_e_list [BI_local_get 4%N; BI_table_get 0%N])
          (v_to_e_list [extern_value c]) [] [AI_basic (BI_call (tag_index kind))]).
        apply counter_coupon_read; assumption.
      - apply tag_value_run; reflexivity.
    Qed.

    Lemma first_loop_body kind coupons l r old n i c :
      (Z.of_nat i < 2 ^ 32)%Z -> nth_error coupons i = Some c ->
      runs (coupon_store coupons) (loop_frame l r old n i) (to_e_list (tree_tag_body (tag_index kind)))
        (if tag_check kind c then [] else [AI_trap]).
    Proof.
      intros Bound Lookup; eapply runs_trans with (mid :=
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool (tag_check kind c)))] ++
          [AI_basic (BI_if (BT_valtype None) [BI_nop] self_failure_body)]).
      - apply (runs_context _ _ (to_e_list [BI_local_get 4%N; BI_table_get 0%N; BI_call (tag_index kind)])
          (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (tag_check kind c)))]) []
          [AI_basic (BI_if (BT_valtype None) [BI_nop] self_failure_body)]).
        apply tag_guard_run with (c := c); assumption.
      - apply if_void.
        + destruct (tag_check kind c); [left|right]; reflexivity.
        + destruct (tag_check kind c); apply runs_step, r_simple; [apply rs_nop|apply rs_unreachable].
    Qed.

    Lemma first_loop_internal_good kind coupons l r old n i c :
      n = List.length coupons -> (i < n)%nat -> (Z.of_nat n < 2 ^ 31)%Z ->
      nth_error coupons i = Some c -> tag_check kind c = true ->
      frame_runs (coupon_store coupons)
        (loop_frame l r old n i, loop_state (tree_tag_body (tag_index kind)) (to_e_list (tree_loop_code (tree_tag_body (tag_index kind)))))
        (loop_frame l r old n (S i), loop_state (tree_tag_body (tag_index kind)) (to_e_list (tree_loop_code (tree_tag_body (tag_index kind))))).
    Proof.
      intros Size Less Bound Lookup Tag; apply loop_iteration; try assumption.
      apply runs_to_frame_runs; pose proof (first_loop_body kind coupons l r old n i c ltac:(cbn in Bound; cbn; lia) Lookup) as Run.
      rewrite Tag in Run; exact Run.
    Qed.

    Lemma first_loop_internal_bad kind coupons l r old n i c :
      (i < n)%nat -> (Z.of_nat n < 2 ^ 31)%Z -> nth_error coupons i = Some c -> tag_check kind c = false ->
      frame_runs (coupon_store coupons)
        (loop_frame l r old n i, loop_state (tree_tag_body (tag_index kind)) (to_e_list (tree_loop_code (tree_tag_body (tag_index kind)))))
        (loop_frame l r old n i, [AI_trap]).
    Proof.
      intros Less Bound Lookup Tag; apply loop_body_trap; try assumption.
      apply runs_to_frame_runs; pose proof (first_loop_body kind coupons l r old n i c ltac:(cbn in Bound; cbn; lia) Lookup) as Run.
      rewrite Tag in Run; exact Run.
    Qed.

    (** Induction over the remaining suffix is the Rocq counterpart of the
        Isabelle induction over the number of successful loop iterations. *)
    Lemma first_loop_suffix kind remaining : forall prefix coupons l r old,
      coupons = prefix ++ remaining -> (Z.of_nat (List.length coupons) < 2 ^ 31)%Z ->
      exists i, frame_runs (coupon_store coupons)
        (loop_frame l r old (List.length coupons) (List.length prefix),
          loop_state (tree_tag_body (tag_index kind)) (to_e_list (tree_loop_code (tree_tag_body (tag_index kind)))))
        (if forallb (tag_check kind) remaining then
          (loop_frame l r old (List.length coupons) (List.length coupons), [])
         else (loop_frame l r old (List.length coupons) i, [AI_trap])).
    Proof.
      induction remaining as [|c rest IH]; intros prefix coupons l r old Layout Bound.
      - exists (List.length coupons); cbn [forallb]; assert (List.length prefix = List.length coupons) as Size by (rewrite Layout, app_nil_r; reflexivity).
        rewrite Size; apply loop_stop; exact Bound.
      - assert (Less : (List.length prefix < List.length coupons)%nat) by (rewrite Layout, length_app; cbn; lia).
        assert (Lookup : nth_error coupons (List.length prefix) = Some c).
        { rewrite Layout, nth_error_app2 by lia; rewrite Nat.sub_diag; reflexivity. }
        destruct (tag_check kind c) eqn:Tag.
        + pose proof (first_loop_internal_good kind coupons l r old _ _ c eq_refl Less Bound Lookup Tag) as Step.
          destruct (IH (prefix ++ [c]) coupons l r old ltac:(rewrite Layout, <- app_assoc; reflexivity) Bound) as [i Run].
          replace (List.length (prefix ++ [c])) with (S (List.length prefix)) in Run by (rewrite length_app; cbn; lia).
          exists i; cbn [forallb]; rewrite Tag; cbn [andb].
          eapply frame_runs_trans; eassumption.
        + exists (List.length prefix); cbn [forallb]; rewrite Tag; cbn [andb].
          apply first_loop_internal_bad with (c := c); assumption.
    Qed.

    Lemma first_loop_good kind coupons l r old : Forall (fun c => tag_check kind c = true) coupons ->
      (Z.of_nat (List.length coupons) < 2 ^ 31)%Z ->
      frame_runs (coupon_store coupons)
        (loop_frame l r old (List.length coupons) 0, to_e_list (tree_loop (tree_tag_body (tag_index kind))))
        (loop_frame l r old (List.length coupons) (List.length coupons), []).
    Proof.
      intros Tags Bound; assert (forallb (tag_check kind) coupons = true) as AllTags.
      { apply forallb_forall; intros c InC; rewrite Forall_forall in Tags; apply Tags; exact InC. }
      eapply frame_runs_trans; [apply loop_enter|].
      destruct (first_loop_suffix kind coupons [] coupons l r old eq_refl Bound) as [i Run].
      rewrite AllTags in Run; exact Run.
    Qed.

    Lemma first_loop_bad kind coupons l r old : forallb (tag_check kind) coupons = false ->
      (Z.of_nat (List.length coupons) < 2 ^ 31)%Z -> exists i,
      frame_runs (coupon_store coupons)
        (loop_frame l r old (List.length coupons) 0, to_e_list (tree_loop (tree_tag_body (tag_index kind))))
        (loop_frame l r old (List.length coupons) i, [AI_trap]).
    Proof.
      intros Bad Bound; destruct (first_loop_suffix kind coupons [] coupons l r old eq_refl Bound) as [i Run].
      rewrite Bad in Run; exists i; eapply frame_runs_trans; [apply loop_enter|exact Run].
    Qed.
    Definition entry_left_guard := [BI_local_get 0%N; BI_local_get 4%N; BI_call 17%N;
      BI_local_get 2%N; BI_call 10%N; BI_call 0%N].
    Definition entry_right_guard := [BI_local_get 1%N; BI_local_get 4%N; BI_call 17%N;
      BI_local_get 2%N; BI_call 11%N; BI_call 0%N].

    Lemma tree_data_run coupons l r old n i (left : bool) h :
      (Z.of_nat i < 2 ^ 32)%Z -> Api.get_tree_data_api (if left then l else r) i = Some h ->
      runs (coupon_store coupons) (loop_frame l r old n i)
        (to_e_list [BI_local_get (if left then 0%N else 1%N); BI_local_get 4%N; BI_call 17%N])
        (v_to_e_list [extern_value h]).
    Proof.
      intros Bound Data; eapply runs_trans with (mid :=
        v_to_e_list [extern_value (if left then l else r); VAL_num (VAL_int32 (wasm_nat i))] ++ [AI_basic (BI_call 17%N)]).
      - apply (runs_context _ _ (to_e_list [BI_local_get (if left then 0%N else 1%N); BI_local_get 4%N])
          (v_to_e_list [extern_value (if left then l else r); VAL_num (VAL_int32 (wasm_nat i))]) [] [AI_basic (BI_call 17%N)]).
        apply local_indices; destruct left; reflexivity.
      - eapply call_host with (hf := fixpoint_get_tree_data) (addr := 17%N); try reflexivity.
        unfold host_values, extern_value; rewrite R.to_handle_to_externref.
        change (L.T.U.W.H.omap (fun h => [extern_value h]) (Api.get_tree_data_api (if left then l else r)
          (Wasm_int.nat_of_uint i32m (wasm_nat i))) = Some [extern_value h]).
        rewrite (wasm_nat_to_nat i Bound), Data; reflexivity.
    Qed.

    Lemma entry_guard_run coupons l r n i c (left : bool) data endpoint :
      (Z.of_nat i < 2 ^ 32)%Z -> Api.get_tree_data_api (if left then l else r) i = Some data ->
      (if left then C.get_coupon_lhs c else C.get_coupon_rhs c) = Some endpoint ->
      runs (coupon_store coupons) (loop_frame l r (extern_value c) n i)
        (to_e_list (if left then entry_left_guard else entry_right_guard))
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_equal data endpoint))) ]).
    Proof.
      intros Bound Data Endpoint.
      replace (to_e_list (if left then entry_left_guard else entry_right_guard)) with
        (to_e_list [BI_local_get (if left then 0%N else 1%N); BI_local_get 4%N; BI_call 17%N] ++
          to_e_list [BI_local_get 2%N; BI_call (coupon_getter_idx left)] ++ [AI_basic (BI_call 0%N)])
        by (destruct left; reflexivity).
      apply (equal_runs _ _
        (to_e_list [BI_local_get (if left then 0%N else 1%N); BI_local_get 4%N; BI_call 17%N])
        (to_e_list [BI_local_get 2%N; BI_call (coupon_getter_idx left)]) data endpoint).
      - destruct left; reflexivity.
      - apply tree_data_run; assumption.
      - apply (coupon_get_endpoint left _ _ 2%N c endpoint); try reflexivity;
          destruct left; exact Endpoint.
    Qed.

    Lemma second_loop_checks coupons l r n i c li ri cl cr :
      (Z.of_nat i < 2 ^ 32)%Z -> Api.get_tree_data_api l i = Some li -> Api.get_tree_data_api r i = Some ri ->
      C.get_coupon_lhs c = Some cl -> C.get_coupon_rhs c = Some cr ->
      runs (coupon_store coupons) (loop_frame l r (extern_value c) n i)
        (to_e_list (entry_left_guard ++ [BI_if (BT_valtype None) tree_rhs_entry self_failure_body]))
        (if Api.is_equal li cl && Api.is_equal ri cr then [] else [AI_trap]).
    Proof.
      intros Bound Left Right Lhs Rhs.
      eapply runs_trans with (mid := v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_equal li cl)))] ++
        [AI_basic (BI_if (BT_valtype None) tree_rhs_entry self_failure_body)]).
      - apply (runs_context _ _ (to_e_list entry_left_guard)
          (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_equal li cl)))]) []
          [AI_basic (BI_if (BT_valtype None) tree_rhs_entry self_failure_body)]).
        apply (entry_guard_run _ _ _ _ _ _ true li cl); assumption.
      - apply if_void; [destruct (Api.is_equal li cl), (Api.is_equal ri cr); [left|right|right|right]; reflexivity|].
        destruct (Api.is_equal li cl); [|apply runs_step, r_simple, rs_unreachable].
        eapply runs_trans with (mid := v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_equal ri cr)))] ++
          [AI_basic (BI_if (BT_valtype None) [BI_nop] self_failure_body)]).
        + apply (runs_context _ _ (to_e_list entry_right_guard)
            (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_equal ri cr)))]) []
            [AI_basic (BI_if (BT_valtype None) [BI_nop] self_failure_body)]).
          apply (entry_guard_run _ _ _ _ _ _ false ri cr); assumption.
        + apply if_void; [destruct (Api.is_equal ri cr); [left|right]; reflexivity|].
          destruct (Api.is_equal ri cr); apply runs_step, r_simple; [apply rs_nop|apply rs_unreachable].
    Qed.

    Lemma entry_coupon_set coupons l r old n i c :
      (Z.of_nat i < 2 ^ 32)%Z -> nth_error coupons i = Some c ->
      frame_runs (coupon_store coupons) (loop_frame l r old n i,
        to_e_list [BI_local_get 4%N; BI_table_get 0%N; BI_local_set 2%N])
        (loop_frame l r (extern_value c) n i, []).
    Proof.
      intros Bound Lookup; eapply frame_runs_trans with (middle := (loop_frame l r old n i,
        [v_to_e (VAL_num (VAL_int32 (wasm_nat i))); AI_basic (BI_table_get 0%N); AI_basic (BI_local_set 2%N)])).
      - apply runs_to_frame_runs, runs_step, (step_context _ _ [AI_basic (BI_local_get 4%N)]
          [v_to_e (VAL_num (VAL_int32 (wasm_nat i)))] [] [AI_basic (BI_table_get 0%N); AI_basic (BI_local_set 2%N)]), r_local_get; reflexivity.
      - apply (table_read_set coupons _ _ i 2%N c []); try reflexivity.
        unfold lookup_N; change (nth_error coupons (N.to_nat (Wasm_int.N_of_uint i32m (wasm_nat i))) = Some c).
        rewrite (wasm_nat_to_N i Bound), Nat2N.id; exact Lookup.
    Qed.

    Lemma second_loop_body coupons l r old n i c li ri cl cr :
      (Z.of_nat i < 2 ^ 32)%Z -> nth_error coupons i = Some c ->
      Api.get_tree_data_api l i = Some li -> Api.get_tree_data_api r i = Some ri ->
      C.get_coupon_lhs c = Some cl -> C.get_coupon_rhs c = Some cr ->
      frame_runs (coupon_store coupons) (loop_frame l r old n i, to_e_list tree_entry_body)
        (loop_frame l r (extern_value c) n i, if Api.is_equal li cl && Api.is_equal ri cr then [] else [AI_trap]).
    Proof.
      intros Bound Lookup Left Right Lhs Rhs.
      eapply frame_runs_trans with (middle := (loop_frame l r (extern_value c) n i,
        to_e_list (entry_left_guard ++ [BI_if (BT_valtype None) tree_rhs_entry self_failure_body]))).
      - apply (frame_runs_context _ (loop_frame l r old n i,
          to_e_list [BI_local_get 4%N; BI_table_get 0%N; BI_local_set 2%N])
          (loop_frame l r (extern_value c) n i, []) []
          (to_e_list (entry_left_guard ++ [BI_if (BT_valtype None) tree_rhs_entry self_failure_body]))).
        apply entry_coupon_set; assumption.
      - apply runs_to_frame_runs, second_loop_checks; assumption.
    Qed.

    Lemma entry_inputs kind coupons l r i c :
      forallb (tag_check kind) coupons = true -> B.has_tree_size l (List.length coupons) = true ->
      B.has_tree_size r (List.length coupons) = true -> nth_error coupons i = Some c ->
      exists li ri cl cr, Api.get_tree_data_api l i = Some li /\ Api.get_tree_data_api r i = Some ri /\
        C.get_coupon_lhs c = Some cl /\ C.get_coupon_rhs c = Some cr.
    Proof.
      intros Tags SizeL SizeR Lookup.
      assert (Less : (i < List.length coupons)%nat) by (apply nth_error_Some; rewrite Lookup; discriminate).
      assert (Tag : B.API.is_type (tree_tag kind) c = true).
      { apply forallb_forall with (x := c) in Tags; [exact Tags|eapply nth_error_In; exact Lookup]. }
      destruct (B.type_lhs_exist _ _ Tag) as [cl Lhs], (B.type_rhs_exist _ _ Tag) as [cr Rhs].
      destruct (B.has_tree_size_some _ _ SizeL) as [tl [-> LenL]], (B.has_tree_size_some _ _ SizeR) as [tr [-> LenR]].
      exists (L.T.U.W.H.get_tree_data tl i), (L.T.U.W.H.get_tree_data tr i), cl, cr; repeat split; try assumption;
        apply B.tree_api_in_bounds; rewrite LenL || rewrite LenR; exact Less.
    Qed.

    Lemma second_loop_body_match kind coupons l r old i c :
      forallb (tag_check kind) coupons = true -> B.has_tree_size l (List.length coupons) = true ->
      B.has_tree_size r (List.length coupons) = true -> (Z.of_nat (List.length coupons) < 2 ^ 31)%Z ->
      nth_error coupons i = Some c ->
      frame_runs (coupon_store coupons) (loop_frame l r old (List.length coupons) i, to_e_list tree_entry_body)
        (loop_frame l r (extern_value c) (List.length coupons) i,
          if B.tree_entry_match coupons l r i then [] else [AI_trap]).
    Proof.
      intros Tags SizeL SizeR Bound Lookup.
      destruct (entry_inputs kind coupons l r i c Tags SizeL SizeR Lookup) as [li [ri [cl [cr [Left [Right [Lhs Rhs]]]]]]].
      change (B.API.get_tree_data_api l i = Some li) in Left.
      change (B.API.get_tree_data_api r i = Some ri) in Right.
      unfold B.tree_entry_match; rewrite Lookup, Left, Right, Lhs, Rhs.
      apply second_loop_body; try assumption.
      assert (Less : (i < List.length coupons)%nat) by (apply nth_error_Some; rewrite Lookup; discriminate).
      cbn in Bound; cbn; lia.
    Qed.
    Lemma second_loop_suffix kind remaining : forall prefix coupons l r old,
      coupons = prefix ++ remaining -> forallb (tag_check kind) coupons = true ->
      B.has_tree_size l (List.length coupons) = true -> B.has_tree_size r (List.length coupons) = true ->
      (Z.of_nat (List.length coupons) < 2 ^ 31)%Z -> exists final_old final_i,
      frame_runs (coupon_store coupons)
        (loop_frame l r old (List.length coupons) (List.length prefix),
          loop_state tree_entry_body (to_e_list (tree_loop_code tree_entry_body)))
        (if forallb (B.tree_entry_match coupons l r) (seq (List.length prefix) (List.length remaining)) then
          (loop_frame l r final_old (List.length coupons) (List.length coupons), [])
         else (loop_frame l r final_old (List.length coupons) final_i, [AI_trap])).
    Proof.
      induction remaining as [|c rest IH]; intros prefix coupons l r old Layout Tags SizeL SizeR Bound.
      - exists old, (List.length coupons); cbn [seq forallb].
        assert (List.length prefix = List.length coupons) as Size by (rewrite Layout, app_nil_r; reflexivity).
        rewrite Size; apply loop_stop; exact Bound.
      - assert (Less : (List.length prefix < List.length coupons)%nat) by (rewrite Layout, length_app; cbn; lia).
        assert (Lookup : nth_error coupons (List.length prefix) = Some c).
        { rewrite Layout, nth_error_app2 by lia; rewrite Nat.sub_diag; reflexivity. }
        pose proof (second_loop_body_match kind coupons l r old _ c Tags SizeL SizeR Bound Lookup) as Body.
        destruct (B.tree_entry_match coupons l r (List.length prefix)) eqn:Entry.
        + assert (Step : frame_runs (coupon_store coupons)
            (loop_frame l r old (List.length coupons) (List.length prefix), loop_state tree_entry_body (to_e_list (tree_loop_code tree_entry_body)))
            (loop_frame l r (extern_value c) (List.length coupons) (S (List.length prefix)), loop_state tree_entry_body (to_e_list (tree_loop_code tree_entry_body)))).
          { apply loop_iteration; assumption. }
          destruct (IH (prefix ++ [c]) coupons l r (extern_value c)
            ltac:(rewrite Layout, <- app_assoc; reflexivity) Tags SizeL SizeR Bound) as [final_old [final_i Run]].
          replace (List.length (prefix ++ [c])) with (S (List.length prefix)) in Run by (rewrite length_app; cbn; lia).
          exists final_old, final_i; cbn [length seq forallb]; rewrite Entry; cbn [andb].
          eapply frame_runs_trans; eassumption.
        + exists (extern_value c), (List.length prefix); cbn [length seq forallb]; rewrite Entry; cbn [andb].
          apply loop_body_trap; assumption.
    Qed.

    Lemma second_loop_good kind coupons l r old :
      forallb (tag_check kind) coupons = true -> B.has_tree_size l (List.length coupons) = true ->
      B.has_tree_size r (List.length coupons) = true ->
      forallb (B.tree_entry_match coupons l r) (seq 0 (List.length coupons)) = true ->
      (Z.of_nat (List.length coupons) < 2 ^ 31)%Z -> exists final_old,
      frame_runs (coupon_store coupons)
        (loop_frame l r old (List.length coupons) 0, to_e_list (tree_loop tree_entry_body))
        (loop_frame l r final_old (List.length coupons) (List.length coupons), []).
    Proof.
      intros Tags SizeL SizeR Entries Bound.
      destruct (second_loop_suffix kind coupons [] coupons l r old eq_refl Tags SizeL SizeR Bound) as [final_old [final_i Run]].
      cbn [length] in Run; rewrite Entries in Run; exists final_old.
      eapply frame_runs_trans; [apply loop_enter|exact Run].
    Qed.

    Lemma second_loop_bad kind coupons l r old :
      forallb (tag_check kind) coupons = true -> B.has_tree_size l (List.length coupons) = true ->
      B.has_tree_size r (List.length coupons) = true ->
      forallb (B.tree_entry_match coupons l r) (seq 0 (List.length coupons)) = false ->
      (Z.of_nat (List.length coupons) < 2 ^ 31)%Z -> exists final_old final_i,
      frame_runs (coupon_store coupons)
        (loop_frame l r old (List.length coupons) 0, to_e_list (tree_loop tree_entry_body))
        (loop_frame l r final_old (List.length coupons) final_i, [AI_trap]).
    Proof.
      intros Tags SizeL SizeR Entries Bound.
      destruct (second_loop_suffix kind coupons [] coupons l r old eq_refl Tags SizeL SizeR Bound) as [final_old [final_i Run]].
      cbn [length] in Run; rewrite Entries in Run; exists final_old, final_i.
      eapply frame_runs_trans; [apply loop_enter|exact Run].
    Qed.
    Lemma counter_reset coupons l r old n i : frame_runs (coupon_store coupons)
      (loop_frame l r old n i, to_e_list tree_reset) (loop_frame l r old n 0, []).
    Proof.
      apply frame_runs_step; eapply r_local_set with (i := 4%N)
        (v := VAL_num (VAL_int32 (wasm_nat 0))) (vd := null_extern); reflexivity.
    Qed.

    Lemma tree_create_run kind coupons l r old n i :
      runs (coupon_store coupons) (loop_frame l r old n i) (to_e_list (tree_create (create_index kind)))
        (v_to_e_list [extern_value (C.create_coupon (tree_tag kind) l r)]).
    Proof.
      eapply local_pair_host with (hf := create_host kind) (addr := create_index kind); try reflexivity.
      - destruct kind; reflexivity.
      - rewrite coupon_store_functions; destruct kind; reflexivity.
      - destruct kind; unfold host_values, extern_value; rewrite !R.to_handle_to_externref; reflexivity.
      - destruct kind; reflexivity.
    Qed.

    Lemma entries_create_execution kind coupons l r old i :
      forallb (tag_check kind) coupons = true -> B.has_tree_size l (List.length coupons) = true ->
      B.has_tree_size r (List.length coupons) = true -> (Z.of_nat (List.length coupons) < 2 ^ 31)%Z ->
      exists f', frame_runs (coupon_store coupons)
        (loop_frame l r old (List.length coupons) i,
          to_e_list (tree_reset ++ tree_loop tree_entry_body ++ tree_create (create_index kind)))
        (f', if forallb (B.tree_entry_match coupons l r) (seq 0 (List.length coupons)) then
          v_to_e_list [extern_value (C.create_coupon (tree_tag kind) l r)] else [AI_trap]).
    Proof.
      intros Tags SizeL SizeR Bound.
      assert (Reset : frame_runs (coupon_store coupons)
        (loop_frame l r old (List.length coupons) i,
          to_e_list (tree_reset ++ tree_loop tree_entry_body ++ tree_create (create_index kind)))
        (loop_frame l r old (List.length coupons) 0,
          to_e_list (tree_loop tree_entry_body ++ tree_create (create_index kind)))).
      { unfold to_e_list at 1; rewrite map_app.
        exact (frame_runs_context _ _ _ [] (to_e_list (tree_loop tree_entry_body ++ tree_create (create_index kind)))
          (counter_reset coupons l r old (List.length coupons) i)). }
      destruct (forallb (B.tree_entry_match coupons l r) (seq 0 (List.length coupons))) eqn:Entries.
      - destruct (second_loop_good kind coupons l r old Tags SizeL SizeR Entries Bound) as [final_old Loop].
        exists (loop_frame l r final_old (List.length coupons) (List.length coupons)).
        eapply frame_runs_trans; [exact Reset|].
        eapply frame_runs_trans with (middle := (loop_frame l r final_old (List.length coupons) (List.length coupons),
          to_e_list (tree_create (create_index kind)))).
        + unfold to_e_list at 1; rewrite map_app.
          exact (frame_runs_context _ _ _ [] (to_e_list (tree_create (create_index kind))) Loop).
        + apply runs_to_frame_runs, tree_create_run.
      - destruct (second_loop_bad kind coupons l r old Tags SizeL SizeR Entries Bound) as [final_old [final_i Loop]].
        exists (loop_frame l r final_old (List.length coupons) final_i).
        eapply frame_runs_trans; [exact Reset|].
        eapply frame_runs_trans with (middle := (loop_frame l r final_old (List.length coupons) final_i,
          AI_trap :: to_e_list (tree_create (create_index kind)))).
        + unfold to_e_list at 1; rewrite map_app.
          exact (frame_runs_context _ _ _ [] (to_e_list (tree_create (create_index kind))) Loop).
        + apply runs_to_frame_runs, (trap_context _ _ [] (to_e_list (tree_create (create_index kind))));
            destruct kind; discriminate.
    Qed.
    Lemma size_guard_run coupons l r old n i (left : bool) m :
      (Z.of_nat n < 2 ^ 31)%Z -> (Z.of_nat m < 2 ^ 31)%Z ->
      Api.get_tree_size_api (if left then l else r) = Some m ->
      runs (coupon_store coupons) (loop_frame l r old n i) (to_e_list (size_guard left))
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Nat.eqb m n)))]).
    Proof.
      intros NBound MBound Size.
      eapply runs_trans with (mid := v_to_e_list [VAL_num (VAL_int32 (wasm_nat m))] ++
        to_e_list [BI_local_get 3%N; BI_relop T_i32 (Relop_i ROI_eq)]).
      - apply (runs_context _ _ (to_e_list [BI_local_get (if left then 0%N else 1%N); BI_call 16%N])
          (v_to_e_list [VAL_num (VAL_int32 (wasm_nat m))]) [] (to_e_list [BI_local_get 3%N; BI_relop T_i32 (Relop_i ROI_eq)])).
        eapply local_index_host with (hf := fixpoint_get_tree_size) (addr := 16%N)
          (v := extern_value (if left then l else r)); try (destruct left; reflexivity).
        unfold host_values, extern_value; rewrite R.to_handle_to_externref, Size; reflexivity.
      - eapply runs_trans with (mid := v_to_e_list [VAL_num (VAL_int32 (wasm_nat m)); VAL_num (VAL_int32 (wasm_nat n))] ++
          [AI_basic (BI_relop T_i32 (Relop_i ROI_eq))]).
        + apply runs_step, (step_context _ _ [AI_basic (BI_local_get 3%N)]
            [v_to_e (VAL_num (VAL_int32 (wasm_nat n)))] [VAL_num (VAL_int32 (wasm_nat m))]
            [AI_basic (BI_relop T_i32 (Relop_i ROI_eq))]), r_local_get; reflexivity.
        + rewrite <- bool_word, <- (wasm_nat_equal m n ltac:(cbn in MBound; cbn; lia) ltac:(cbn in NBound; cbn; lia)).
          apply runs_step, r_simple, rs_relop; reflexivity.
    Qed.

    Lemma size_guard_none coupons l r old n i (left : bool) :
      Api.get_tree_size_api (if left then l else r) = None ->
      runs (coupon_store coupons) (loop_frame l r old n i) (to_e_list (size_guard left)) [AI_trap].
    Proof.
      intro Size; eapply runs_trans with (mid := v_to_e_list [extern_value (if left then l else r)] ++
        [AI_basic (BI_call 16%N); AI_basic (BI_local_get 3%N); AI_basic (BI_relop T_i32 (Relop_i ROI_eq))]).
      - apply runs_step, (step_context _ _ [AI_basic (BI_local_get (if left then 0%N else 1%N))]
          [v_to_e (extern_value (if left then l else r))] []
          [AI_basic (BI_call 16%N); AI_basic (BI_local_get 3%N); AI_basic (BI_relop T_i32 (Relop_i ROI_eq))]), r_local_get;
          destruct left; reflexivity.
      - eapply runs_trans with (mid := AI_trap :: to_e_list [BI_local_get 3%N; BI_relop T_i32 (Relop_i ROI_eq)]).
        + apply (runs_context _ _ (v_to_e_list [extern_value (if left then l else r)] ++ [AI_basic (BI_call 16%N)])
            [AI_trap] [] (to_e_list [BI_local_get 3%N; BI_relop T_i32 (Relop_i ROI_eq)])).
          eapply call_host_none with (hf := fixpoint_get_tree_size) (addr := 16%N); try reflexivity.
          unfold host_values, extern_value; rewrite R.to_handle_to_externref, Size; reflexivity.
        + apply (trap_context _ _ [] (to_e_list [BI_local_get 3%N; BI_relop T_i32 (Relop_i ROI_eq)])); discriminate.
    Qed.

    Lemma size_guard_branch coupons l r old n i (left : bool) yes out :
      (Z.of_nat n < 2 ^ 31)%Z -> tree_size_bounded (if left then l else r) -> terminal_form out ->
      (B.has_tree_size (if left then l else r) n = true -> exists f',
        frame_runs (coupon_store coupons) (loop_frame l r old n i, to_e_list yes) (f', out)) ->
      exists f', frame_runs (coupon_store coupons)
        (loop_frame l r old n i, to_e_list (size_guard left ++
          [BI_if (BT_valtype (Some (T_ref T_externref))) yes self_failure_body]))
        (f', if B.has_tree_size (if left then l else r) n then out else [AI_trap]).
    Proof.
      intros Bound SizeBound Terminal Branch.
      destruct (Api.get_tree_size_api (if left then l else r)) as [m|] eqn:Size.
      - assert (Has : B.has_tree_size (if left then l else r) n = Nat.eqb m n).
        { unfold B.has_tree_size; change
            (match Api.get_tree_size_api (if left then l else r) with Some sz => Nat.eqb sz n | None => false end = Nat.eqb m n).
          rewrite Size; reflexivity. }
        pose proof (size_guard_run coupons l r old n i left m Bound (SizeBound m Size) Size) as Guard.
        destruct (Nat.eqb m n) eqn:Check.
        + destruct (Branch Has) as [f' Run]; exists f'; rewrite Has.
          eapply frame_runs_trans with (middle := (loop_frame l r old n i,
            [v_to_e (VAL_num (VAL_int32 (wasm_bool true)));
              AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) yes self_failure_body)])).
          * apply runs_to_frame_runs, guard_prologue; exact Guard.
          * apply frame_if_externref; assumption.
        + exists (loop_frame l r old n i); rewrite Has.
          eapply frame_runs_trans with (middle := (loop_frame l r old n i,
            [v_to_e (VAL_num (VAL_int32 (wasm_bool false)));
              AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) yes self_failure_body)])).
          * apply runs_to_frame_runs, guard_prologue; exact Guard.
          * apply runs_to_frame_runs, if_externref; [right; reflexivity|apply runs_step, r_simple, rs_unreachable].
      - assert (Has : B.has_tree_size (if left then l else r) n = false).
        { unfold B.has_tree_size; change
            (match Api.get_tree_size_api (if left then l else r) with Some sz => Nat.eqb sz n | None => false end = false).
          rewrite Size; reflexivity. }
        exists (loop_frame l r old n i); rewrite Has; apply runs_to_frame_runs.
        eapply runs_trans with (mid := [AI_trap; AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) yes self_failure_body)]).
        + unfold to_e_list; rewrite map_app.
          apply (runs_context _ _ (to_e_list (size_guard left)) [AI_trap] []
            [AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) yes self_failure_body)]).
          apply size_guard_none; exact Size.
        + apply (trap_context _ _ [] [AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) yes self_failure_body)]); discriminate.
    Qed.

    Lemma size_tests_execution kind coupons l r old i : forallb (tag_check kind) coupons = true ->
      (Z.of_nat (List.length coupons) < 2 ^ 31)%Z -> tree_size_bounded l -> tree_size_bounded r ->
      exists f', frame_runs (coupon_store coupons)
        (loop_frame l r old (List.length coupons) i, to_e_list (tree_size_tests (create_index kind)))
        (f', if B.has_tree_size l (List.length coupons) then
          if B.has_tree_size r (List.length coupons) then
            if forallb (B.tree_entry_match coupons l r) (seq 0 (List.length coupons)) then
              v_to_e_list [extern_value (C.create_coupon (tree_tag kind) l r)] else [AI_trap]
          else [AI_trap] else [AI_trap]).
    Proof.
      intros Tags Bound BoundL BoundR.
      apply (size_guard_branch _ _ _ _ _ _ true (tree_size_right (create_index kind))); try assumption.
      - destruct (B.has_tree_size r (List.length coupons)),
          (forallb (B.tree_entry_match coupons l r) (seq 0 (List.length coupons))); [left|right|right|right]; reflexivity.
      - intro SizeL; apply (size_guard_branch _ _ _ _ _ _ false
          (tree_reset ++ tree_loop tree_entry_body ++ tree_create (create_index kind))); try assumption.
        + destruct (forallb (B.tree_entry_match coupons l r) (seq 0 (List.length coupons))); [left|right]; reflexivity.
        + intro SizeR; apply entries_create_execution; assumption.
    Qed.
    Lemma tree_prologue coupons l r : frame_runs (coupon_store coupons)
      (loop_frame l r null_extern 0 0, to_e_list ([BI_table_size 0%N; BI_local_set 3%N] ++ tree_reset))
      (loop_frame l r null_extern (List.length coupons) 0, []).
    Proof.
      eapply frame_runs_trans with (middle := (loop_frame l r null_extern 0 0,
        [v_to_e (VAL_num (VAL_int32 (wasm_nat (List.length coupons)))); AI_basic (BI_local_set 3%N)] ++ to_e_list tree_reset)).
      - apply runs_to_frame_runs, runs_step, (step_context _ _ [AI_basic (BI_table_size 0%N)]
          [v_to_e (VAL_num (VAL_int32 (wasm_nat (List.length coupons))))] []
          (AI_basic (BI_local_set 3%N) :: to_e_list tree_reset)).
        eapply r_table_size with (sz := List.length coupons); [reflexivity|].
        change (List.length (map (fun h => VAL_ref_extern (R.to_externref h)) coupons) = List.length coupons).
        apply length_map.
      - eapply frame_runs_trans with (middle := (loop_frame l r null_extern (List.length coupons) 0, to_e_list tree_reset)).
        + eapply frame_runs_context with (start := (loop_frame l r null_extern 0 0,
            [v_to_e (VAL_num (VAL_int32 (wasm_nat (List.length coupons)))); AI_basic (BI_local_set 3%N)]))
            (finish := (loop_frame l r null_extern (List.length coupons) 0, [])) (vs := []).
          apply frame_runs_step; eapply r_local_set with (i := 3%N)
            (v := VAL_num (VAL_int32 (wasm_nat (List.length coupons)))) (vd := null_extern); reflexivity.
        + apply counter_reset.
    Qed.

    Lemma tree_body_execution kind coupons l r : (Z.of_nat (List.length coupons) < 2 ^ 31)%Z ->
      tree_size_bounded l -> tree_size_bounded r -> exists f',
      frame_runs (coupon_store coupons)
        (loop_frame l r null_extern 0 0, to_e_list (tree_body (tag_index kind) (create_index kind)))
        (f', match B.make_tree_coupon (tree_tag kind) coupons l r with
          Some c => v_to_e_list [extern_value c] | None => [AI_trap] end).
    Proof.
      intros Bound BoundL BoundR.
      assert (Prefix : frame_runs (coupon_store coupons)
        (loop_frame l r null_extern 0 0, to_e_list (tree_body (tag_index kind) (create_index kind)))
        (loop_frame l r null_extern (List.length coupons) 0,
          to_e_list (tree_loop (tree_tag_body (tag_index kind)) ++ tree_size_tests (create_index kind)))).
      { change (frame_runs (coupon_store coupons)
          (loop_frame l r null_extern 0 0,
            to_e_list (([BI_table_size 0%N; BI_local_set 3%N] ++ tree_reset) ++
              tree_loop (tree_tag_body (tag_index kind)) ++ tree_size_tests (create_index kind)))
          (loop_frame l r null_extern (List.length coupons) 0,
            to_e_list (tree_loop (tree_tag_body (tag_index kind)) ++ tree_size_tests (create_index kind)))).
        unfold to_e_list at 1; rewrite map_app.
        exact (frame_runs_context _ _ _ [] (to_e_list (tree_loop (tree_tag_body (tag_index kind)) ++
          tree_size_tests (create_index kind))) (tree_prologue coupons l r)). }
      unfold B.make_tree_coupon.
      change (exists f', frame_runs (coupon_store coupons)
        (loop_frame l r null_extern 0 0, to_e_list (tree_body (tag_index kind) (create_index kind)))
        (f', match (if forallb (tag_check kind) coupons then
          if B.has_tree_size l (List.length coupons) && B.has_tree_size r (List.length coupons) then
            if forallb (B.tree_entry_match coupons l r) (seq 0 (List.length coupons)) then
              Some (C.create_coupon (tree_tag kind) l r) else None else None else None) with
          Some c => v_to_e_list [extern_value c] | None => [AI_trap] end)).
      destruct (forallb (tag_check kind) coupons) eqn:Tags.
      - assert (AllTags : Forall (fun c => tag_check kind c = true) coupons).
        { apply Forall_forall; intros c InC; apply forallb_forall with (x := c) in Tags; assumption. }
        pose proof (first_loop_good kind coupons l r null_extern AllTags Bound) as Loop.
        destruct (size_tests_execution kind coupons l r null_extern (List.length coupons) Tags Bound BoundL BoundR) as [f' Tests].
        exists f'; eapply frame_runs_trans; [exact Prefix|].
        eapply frame_runs_trans with (middle := (loop_frame l r null_extern (List.length coupons) (List.length coupons),
          to_e_list (tree_size_tests (create_index kind)))).
        + unfold to_e_list at 1; rewrite map_app.
          exact (frame_runs_context _ _ _ [] (to_e_list (tree_size_tests (create_index kind))) Loop).
        + destruct (B.has_tree_size l (List.length coupons)), (B.has_tree_size r (List.length coupons)),
            (forallb (B.tree_entry_match coupons l r) (seq 0 (List.length coupons))); exact Tests.
      - destruct (first_loop_bad kind coupons l r null_extern Tags Bound) as [i Loop].
        exists (loop_frame l r null_extern (List.length coupons) i); eapply frame_runs_trans; [exact Prefix|].
        eapply frame_runs_trans with (middle := (loop_frame l r null_extern (List.length coupons) i,
          AI_trap :: to_e_list (tree_size_tests (create_index kind)))).
        + unfold to_e_list at 1; rewrite map_app.
          exact (frame_runs_context _ _ _ [] (to_e_list (tree_size_tests (create_index kind))) Loop).
        + apply runs_to_frame_runs, (trap_context _ _ [] (to_e_list (tree_size_tests (create_index kind))));
            destruct kind; discriminate.
    Qed.

    Theorem make_tree_coupon_execution kind coupons l r caller :
      (Z.of_nat (List.length coupons) < 2 ^ 31)%Z -> tree_size_bounded l -> tree_size_bounded r ->
      runs (coupon_store coupons) caller
        (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke (native_index kind)])
        (match B.make_tree_coupon (tree_tag kind) coupons l r with
          Some c => v_to_e_list [extern_value c] | None => [AI_trap] end).
    Proof.
      intros Bound BoundL BoundR; destruct (tree_body_execution kind coupons l r Bound BoundL BoundR) as [f' Body].
      eapply invoke_pair_locals with (locals := [T_ref T_externref; T_num T_i32; T_num T_i32])
        (defaults := [null_extern; VAL_num (VAL_int32 (wasm_nat 0)); VAL_num (VAL_int32 (wasm_nat 0))])
        (typeidx := 0%N) (body := tree_body (tag_index kind) (create_index kind)) (final_frame := f'); try reflexivity.
      - rewrite coupon_store_functions; destruct kind; reflexivity.
      - exact Body.
      - destruct (B.make_tree_coupon (tree_tag kind) coupons l r); [left; split; reflexivity|right; reflexivity].
    Qed.

    Theorem make_tree_coupon_success kind coupons l r c caller : Forall B.coupon_good coupons ->
      B.make_tree_coupon (tree_tag kind) coupons l r = Some c ->
      (Z.of_nat (List.length coupons) < 2 ^ 31)%Z ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke (native_index kind)])
        (tt, coupon_store coupons, caller, v_to_e_list [extern_value c]) /\ B.coupon_good c.
    Proof.
      intros Goods Make Bound.
      destruct (make_some kind coupons l r c Make) as [_ [SizeL [SizeR _]]].
      pose proof (has_tree_size_bounded _ _ Bound SizeL) as BoundL.
      pose proof (has_tree_size_bounded _ _ Bound SizeR) as BoundR; split.
      - apply runs_sound; pose proof (make_tree_coupon_execution kind coupons l r caller Bound BoundL BoundR) as Run.
        rewrite Make in Run; exact Run.
      - destruct kind; [eapply B.make_eq_tree_coupon_good|eapply B.make_eval_tree_coupon_good]; eassumption.
    Qed.

    Theorem make_tree_coupon_failure kind coupons l r caller :
      B.make_tree_coupon (tree_tag kind) coupons l r = None ->
      (Z.of_nat (List.length coupons) < 2 ^ 31)%Z -> tree_size_bounded l -> tree_size_bounded r ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke (native_index kind)])
        (tt, coupon_store coupons, caller, [AI_trap]).
    Proof.
      intros Make Bound BoundL BoundR; apply runs_sound.
      pose proof (make_tree_coupon_execution kind coupons l r caller Bound BoundL BoundR) as Run; rewrite Make in Run; exact Run.
    Qed.
  End Execution.
End TreeCoupon.
