From Stdlib Require Import List NArith Relation_Operators Lia.
From Wasm Require Import datatypes operations opsem.
From FixProof Require Import Handle CouponApi Host ExecutionUtil ModuleLayout.
Import ListNotations.
Open Scope list_scope.

Module CouponTable (S : STORAGE) (C : COUPON_STORAGE S) (R : EXTERNREFS S).
  Module U := ExecutionUtil S C R.
  Import U U.W.

  Section Execution.
    Context `{mem : BlockUpdateMemory}.

    (** The client supplies this exported table before invoking a builder,
        just as in the original interpreter configurations. *)
    Definition coupon_store (coupons : list S.handle) := Build_store_record (s_funcs allocated_store)
      [Build_tableinst (Build_table_type (Build_limits 0%N None) T_externref)
         (map (fun h => VAL_ref_extern (R.to_externref h)) coupons);
       Build_tableinst (Build_table_type (Build_limits 13%N (Some 13%N)) T_funcref)
         (map VAL_ref_func dispatch_indices)]
      [] [] [Build_eleminst T_funcref []] [].

    Lemma coupon_store_functions coupons : s_funcs (coupon_store coupons) = s_funcs allocated_store.
    Proof. apply store_functions_projection. Qed.

    Lemma coupon_store_empty : coupon_store [] = ready_store.
    Proof. reflexivity. Qed.

    Lemma coupon_table_element coupons idx :
      stab_elem (coupon_store coupons) coupon_instance 0%N idx =
      lookup_N (map (fun h => VAL_ref_extern (R.to_externref h)) coupons) idx.
    Proof. reflexivity. Qed.

    Lemma coupon_table_lookup coupons idx c : lookup_N coupons idx = Some c ->
      stab_elem (coupon_store coupons) coupon_instance 0%N idx = Some (VAL_ref_extern (R.to_externref c)).
    Proof.
      intro Lookup; rewrite coupon_table_element; unfold lookup_N in *.
      rewrite nth_error_map, Lookup; reflexivity.
    Qed.

    Lemma get_first_coupon c coupons f : f_inst f = coupon_instance ->
      reduce tt (coupon_store (c :: coupons)) f
        [v_to_e (VAL_num (VAL_int32 (i32_of_nat 0))); AI_basic (BI_table_get 0%N)]
        tt (coupon_store (c :: coupons)) f [v_to_e (extern_value c)].
    Proof.
      intro Instance; apply r_table_get_success; rewrite Instance.
      apply coupon_table_lookup; reflexivity.
    Qed.

    Lemma get_second_coupon c1 c2 coupons f : f_inst f = coupon_instance ->
      reduce tt (coupon_store (c1 :: c2 :: coupons)) f
        [v_to_e (VAL_num (VAL_int32 (i32_of_nat 1))); AI_basic (BI_table_get 0%N)]
        tt (coupon_store (c1 :: c2 :: coupons)) f [v_to_e (extern_value c2)].
    Proof.
      intro Instance; apply r_table_get_success; rewrite Instance.
      apply coupon_table_lookup; reflexivity.
    Qed.

    Lemma get_empty_coupon f : f_inst f = coupon_instance ->
      reduce tt (coupon_store []) f
        [v_to_e (VAL_num (VAL_int32 (i32_of_nat 0))); AI_basic (BI_table_get 0%N)]
        tt (coupon_store []) f [AI_trap].
    Proof. intro Instance; apply r_table_get_failure; rewrite Instance; reflexivity. Qed.

    Lemma equal_symmetric l r : Api.is_equal l r = Api.is_equal r l.
    Proof.
      destruct (Api.is_equal l r) eqn:Left, (Api.is_equal r l) eqn:Right; try reflexivity.
      - pose proof (proj1 (Api.is_equal_match l r) Left) as Eq; subst; congruence.
      - pose proof (proj1 (Api.is_equal_match r l) Right) as Eq; subst; congruence.
    Qed.

    Lemma call_host_none s f idx addr hf args :
      lookup_N (inst_funcs (f_inst f)) idx = Some addr ->
      lookup_N (s_funcs s) addr = Some (FC_func_host (function_signature hf) hf) ->
      host_values hf args = None ->
      (match function_signature hf with Tf params _ => List.length args = List.length params end) ->
      runs s f (v_to_e_list args ++ [AI_basic (BI_call idx)]) [AI_trap].
    Proof.
      intros Index Closure Values Arity.
      eapply runs_trans with (mid := v_to_e_list args ++ [AI_invoke addr]).
      - apply runs_step; change (reduce tt s f (v_to_e_list args ++ [AI_basic (BI_call idx)] ++ [])
          tt s f (v_to_e_list args ++ [AI_invoke addr] ++ [])).
        apply step_context, r_call; exact Index.
      - destruct (function_signature hf) as [params rets] eqn:Sig.
        apply runs_step; eapply r_invoke_host_diverge with (h := hf) (vcs := args)
          (n := List.length params) (m := List.length rets); try reflexivity; try eassumption.
        left; split; [symmetry; exact Sig|].
        change (None = U.W.H.omap (fun values => (s, result_values values)) (host_values hf args)).
        rewrite Values; reflexivity.
    Qed.

    (** Locals change during a native function, while the store stays fixed.
        Preserve those frames in the inner execution relation. *)
    Definition frame_runs (s : store_record) := clos_refl_trans
      (frame * list administrative_instruction)
      (fun start finish => reduce tt s (fst start) (snd start) tt s (fst finish) (snd finish)).

    Lemma frame_runs_step s f es f' es' : reduce tt s f es tt s f' es' ->
      frame_runs s (f, es) (f', es').
    Proof. intro Step; apply rt_step; exact Step. Qed.

    Lemma frame_runs_trans s start middle finish : frame_runs s start middle ->
      frame_runs s middle finish -> frame_runs s start finish.
    Proof. intros First Second; eapply rt_trans; eassumption. Qed.

    Lemma runs_to_frame_runs s f es es' : runs s f es es' -> frame_runs s (f, es) (f, es').
    Proof.
      intro Run; induction Run; [apply frame_runs_step; exact H|apply rt_refl|].
      eapply frame_runs_trans; eassumption.
    Qed.

    Lemma frame_runs_sound s start finish : frame_runs s start finish ->
      reduce_trans (tt, s, fst start, snd start) (tt, s, fst finish, snd finish).
    Proof.
      intro Run; induction Run.
      - apply rt_step; exact H.
      - apply rt_refl.
      - eapply rt_trans with (y := (tt, s, fst y, snd y)); eassumption.
    Qed.

    Lemma frame_runs_context s start finish vs suffix : frame_runs s start finish ->
      frame_runs s (fst start, v_to_e_list vs ++ snd start ++ suffix)
        (fst finish, v_to_e_list vs ++ snd finish ++ suffix).
    Proof.
      intro Run; induction Run.
      - apply frame_runs_step; eapply r_label with (lh := LH_base vs suffix); [exact H|reflexivity|reflexivity].
      - apply rt_refl.
      - eapply frame_runs_trans; eassumption.
    Qed.

    Lemma frame_runs_label s start finish n cont : frame_runs s start finish ->
      frame_runs s (fst start, [AI_label n cont (snd start)]) (fst finish, [AI_label n cont (snd finish)]).
    Proof.
      intro Run; induction Run.
      - apply frame_runs_step; eapply r_label with (lh := LH_rec [] n cont (LH_base [] []) []);
          [exact H|simpl; now rewrite app_nil_r|simpl; now rewrite app_nil_r].
      - apply rt_refl.
      - eapply frame_runs_trans; eassumption.
    Qed.

    Lemma frame_runs_frame s start finish n caller : frame_runs s start finish ->
      runs s caller [AI_frame n (fst start) (snd start)] [AI_frame n (fst finish) (snd finish)].
    Proof.
      intro Run; induction Run.
      - apply runs_step, r_frame; exact H.
      - apply runs_refl.
      - eapply runs_trans; eassumption.
    Qed.

    Lemma trap_context s f vs suffix : v_to_e_list vs ++ [AI_trap] ++ suffix <> [AI_trap] ->
      runs s f (v_to_e_list vs ++ [AI_trap] ++ suffix) [AI_trap].
    Proof.
      intro Nontrivial; apply runs_step, r_simple; eapply rs_trap with (lh := LH_base vs suffix);
        [exact Nontrivial|reflexivity].
    Qed.

    Definition one_initial_frame l r := Build_frame
      [extern_value l; extern_value r; VAL_ref (VAL_ref_null T_externref)] coupon_instance.
    Definition one_working_frame l r c := Build_frame
      [extern_value l; extern_value r; extern_value c] coupon_instance.

    Lemma one_coupon_prefix c coupons l r tests : frame_runs (coupon_store (c :: coupons))
      (one_initial_frame l r, to_e_list (one_coupon_body tests)) (one_working_frame l r c, to_e_list tests).
    Proof.
      eapply frame_runs_trans with (middle := (one_initial_frame l r,
        [v_to_e (extern_value c); AI_basic (BI_local_set 2%N)] ++ to_e_list tests)).
      - unfold one_coupon_body, to_e_list; rewrite map_app.
        apply (frame_runs_context _ (one_initial_frame l r,
          [v_to_e (VAL_num (VAL_int32 (i32_of_nat 0))); AI_basic (BI_table_get 0%N)])
          (one_initial_frame l r, [v_to_e (extern_value c)]) [] (AI_basic (BI_local_set 2%N) :: to_e_list tests)).
        apply frame_runs_step, get_first_coupon; reflexivity.
      - apply frame_runs_step; eapply r_label with (lh := LH_base [] (to_e_list tests)).
        + eapply r_local_set with (f := one_initial_frame l r) (f' := one_working_frame l r c)
            (i := 2%N) (v := extern_value c) (vd := VAL_ref (VAL_ref_null T_externref)); reflexivity.
        + reflexivity.
        + reflexivity.
    Qed.

    Lemma one_coupon_empty l r tests : frame_runs (coupon_store [])
      (one_initial_frame l r, to_e_list (one_coupon_body tests)) (one_initial_frame l r, [AI_trap]).
    Proof.
      eapply frame_runs_trans with (middle := (one_initial_frame l r, AI_trap :: AI_basic (BI_local_set 2%N) :: to_e_list tests)).
      - unfold one_coupon_body, to_e_list; rewrite map_app.
        apply (frame_runs_context _ (one_initial_frame l r,
          [v_to_e (VAL_num (VAL_int32 (i32_of_nat 0))); AI_basic (BI_table_get 0%N)])
          (one_initial_frame l r, [AI_trap]) [] (AI_basic (BI_local_set 2%N) :: to_e_list tests)).
        apply frame_runs_step, get_empty_coupon; reflexivity.
      - apply runs_to_frame_runs; apply (trap_context _ _ [] (AI_basic (BI_local_set 2%N) :: to_e_list tests)); discriminate.
    Qed.

    Definition null_extern := VAL_ref (VAL_ref_null T_externref).
    Lemma table_read_set coupons f f' n j h suffix : f_inst f = coupon_instance ->
      lookup_N coupons (Wasm_int.N_of_uint i32m (i32_of_nat n)) = Some h ->
      f_inst f' = f_inst f -> ssrbool.pred_of_simpl (ssrnat.ltn (N.to_nat j)) (List.length (f_locs f)) = true ->
      f_locs f' = seq.set_nth null_extern (f_locs f) (N.to_nat j) (extern_value h) ->
      frame_runs (coupon_store coupons)
        (f, [v_to_e (VAL_num (VAL_int32 (i32_of_nat n))); AI_basic (BI_table_get 0%N); AI_basic (BI_local_set j)] ++ suffix)
        (f', suffix).
    Proof.
      intros Instance Entry SameInstance Bound Locals.
      eapply frame_runs_trans with (middle := (f, v_to_e (extern_value h) :: AI_basic (BI_local_set j) :: suffix)).
      - apply (frame_runs_context _ (f, [v_to_e (VAL_num (VAL_int32 (i32_of_nat n))); AI_basic (BI_table_get 0%N)])
          (f, [v_to_e (extern_value h)]) [] (AI_basic (BI_local_set j) :: suffix)).
        apply frame_runs_step, r_table_get_success; rewrite Instance; apply coupon_table_lookup; exact Entry.
      - apply frame_runs_step; eapply r_label with (lh := LH_base [] suffix).
        + eapply r_local_set with (f := f) (f' := f') (i := j) (v := extern_value h) (vd := null_extern); eassumption.
        + reflexivity.
        + reflexivity.
    Qed.

    Lemma table_read_set_failure coupons f n j suffix : f_inst f = coupon_instance ->
      lookup_N coupons (Wasm_int.N_of_uint i32m (i32_of_nat n)) = None ->
      frame_runs (coupon_store coupons)
        (f, [v_to_e (VAL_num (VAL_int32 (i32_of_nat n))); AI_basic (BI_table_get 0%N); AI_basic (BI_local_set j)] ++ suffix)
        (f, [AI_trap]).
    Proof.
      intros Instance Entry.
      eapply frame_runs_trans with (middle := (f, AI_trap :: AI_basic (BI_local_set j) :: suffix)).
      - apply (frame_runs_context _ (f, [v_to_e (VAL_num (VAL_int32 (i32_of_nat n))); AI_basic (BI_table_get 0%N)])
          (f, [AI_trap]) [] (AI_basic (BI_local_set j) :: suffix)).
        apply frame_runs_step, r_table_get_failure; rewrite Instance, coupon_table_element.
        unfold lookup_N in *; rewrite nth_error_map, Entry; reflexivity.
      - apply runs_to_frame_runs, (trap_context _ _ [] (AI_basic (BI_local_set j) :: suffix)); discriminate.
    Qed.
    Definition two_initial_frame l r := Build_frame [extern_value l; extern_value r; null_extern; null_extern] coupon_instance.
    Definition two_first_frame l r c := Build_frame [extern_value l; extern_value r; extern_value c; null_extern] coupon_instance.
    Definition two_working_frame l r c1 c2 := Build_frame
      [extern_value l; extern_value r; extern_value c1; extern_value c2] coupon_instance.

    Lemma two_prefix_first c rest l r tests : frame_runs (coupon_store (c :: rest))
      (two_initial_frame l r, to_e_list (two_coupon_body tests))
      (two_first_frame l r c, to_e_list (trans_second_prefix ++ tests)).
    Proof.
      eapply frame_runs_trans with (middle := (two_initial_frame l r,
        [v_to_e (extern_value c); AI_basic (BI_local_set 2%N)] ++ to_e_list (trans_second_prefix ++ tests))).
      - unfold two_coupon_body, one_coupon_body, to_e_list; rewrite map_app.
        apply (frame_runs_context _ (two_initial_frame l r,
          [v_to_e (VAL_num (VAL_int32 (i32_of_nat 0))); AI_basic (BI_table_get 0%N)])
          (two_initial_frame l r, [v_to_e (extern_value c)]) []
          (AI_basic (BI_local_set 2%N) :: to_e_list (trans_second_prefix ++ tests))).
        apply frame_runs_step, get_first_coupon; reflexivity.
      - apply frame_runs_step; eapply r_label with (lh := LH_base [] (to_e_list (trans_second_prefix ++ tests))).
        + eapply r_local_set with (f := two_initial_frame l r) (f' := two_first_frame l r c)
            (i := 2%N) (v := extern_value c) (vd := null_extern); reflexivity.
        + reflexivity.
        + reflexivity.
    Qed.

    Lemma two_prefix_second c1 c2 rest l r tests : frame_runs (coupon_store (c1 :: c2 :: rest))
      (two_first_frame l r c1, to_e_list (trans_second_prefix ++ tests))
      (two_working_frame l r c1 c2, to_e_list tests).
    Proof.
      eapply frame_runs_trans with (middle := (two_first_frame l r c1,
        [v_to_e (extern_value c2); AI_basic (BI_local_set 3%N)] ++ to_e_list tests)).
      - unfold trans_second_prefix, to_e_list; rewrite map_app.
        apply (frame_runs_context _ (two_first_frame l r c1,
          [v_to_e (VAL_num (VAL_int32 (i32_of_nat 1))); AI_basic (BI_table_get 0%N)])
          (two_first_frame l r c1, [v_to_e (extern_value c2)]) [] (AI_basic (BI_local_set 3%N) :: to_e_list tests)).
        apply frame_runs_step, get_second_coupon; reflexivity.
      - apply frame_runs_step; eapply r_label with (lh := LH_base [] (to_e_list tests)).
        + eapply r_local_set with (f := two_first_frame l r c1) (f' := two_working_frame l r c1 c2)
            (i := 3%N) (v := extern_value c2) (vd := null_extern); reflexivity.
        + reflexivity.
        + reflexivity.
    Qed.

    Lemma two_prefix_empty l r tests : frame_runs (coupon_store [])
      (two_initial_frame l r, to_e_list (two_coupon_body tests)) (two_initial_frame l r, [AI_trap]).
    Proof.
      eapply frame_runs_trans with (middle := (two_initial_frame l r,
        AI_trap :: AI_basic (BI_local_set 2%N) :: to_e_list (trans_second_prefix ++ tests))).
      - unfold two_coupon_body, one_coupon_body, to_e_list; rewrite map_app.
        apply (frame_runs_context _ (two_initial_frame l r,
          [v_to_e (VAL_num (VAL_int32 (i32_of_nat 0))); AI_basic (BI_table_get 0%N)])
          (two_initial_frame l r, [AI_trap]) []
          (AI_basic (BI_local_set 2%N) :: to_e_list (trans_second_prefix ++ tests))).
        apply frame_runs_step, get_empty_coupon; reflexivity.
      - apply runs_to_frame_runs, (trap_context _ _ []
          (AI_basic (BI_local_set 2%N) :: to_e_list (trans_second_prefix ++ tests))); discriminate.
    Qed.

    Lemma two_prefix_single c l r tests : frame_runs (coupon_store [c])
      (two_initial_frame l r, to_e_list (two_coupon_body tests)) (two_first_frame l r c, [AI_trap]).
    Proof.
      eapply frame_runs_trans; [apply two_prefix_first|].
      eapply frame_runs_trans with (middle := (two_first_frame l r c,
        AI_trap :: AI_basic (BI_local_set 3%N) :: to_e_list tests)).
      - unfold trans_second_prefix, to_e_list; rewrite map_app.
        apply (frame_runs_context _ (two_first_frame l r c,
          [v_to_e (VAL_num (VAL_int32 (i32_of_nat 1))); AI_basic (BI_table_get 0%N)])
          (two_first_frame l r c, [AI_trap]) [] (AI_basic (BI_local_set 3%N) :: to_e_list tests)).
        apply frame_runs_step, r_table_get_failure; reflexivity.
      - apply runs_to_frame_runs, (trap_context _ _ [] (AI_basic (BI_local_set 3%N) :: to_e_list tests)); discriminate.
    Qed.

    Lemma local_index_host s f j v idx addr hf out :
      lookup_N (f_locs f) j = Some v ->
      lookup_N (inst_funcs (f_inst f)) idx = Some addr ->
      lookup_N (s_funcs s) addr = Some (FC_func_host (function_signature hf) hf) ->
      host_values hf [v] = Some out ->
      (match function_signature hf with Tf params _ => List.length params = 1%nat end) ->
      runs s f [AI_basic (BI_local_get j); AI_basic (BI_call idx)] (v_to_e_list out).
    Proof.
      intros Local Index Closure Values Arity.
      eapply runs_trans with (mid := v_to_e_list [v] ++ [AI_basic (BI_call idx)]).
      - apply runs_step; change (reduce tt s f ([] ++ [AI_basic (BI_local_get j)] ++ [AI_basic (BI_call idx)])
          tt s f ([] ++ [v_to_e v] ++ [AI_basic (BI_call idx)])).
        apply (step_context s f _ _ [] _), r_local_get; assumption.
      - eapply call_host; try eassumption.
        destruct (function_signature hf); simpl in *; symmetry; exact Arity.
    Qed.

    Definition coupon_getter_idx (lhs : bool) := if lhs then 10%N else 11%N.
    Definition coupon_getter (lhs : bool) := if lhs then fixpoint_get_coupon_lhs else fixpoint_get_coupon_rhs.
    Definition coupon_endpoint (lhs : bool) := if lhs then C.get_coupon_lhs else C.get_coupon_rhs.
    Lemma coupon_get_endpoint lhs coupons f j c h : f_inst f = coupon_instance ->
      lookup_N (f_locs f) j = Some (extern_value c) -> coupon_endpoint lhs c = Some h ->
      runs (coupon_store coupons) f [AI_basic (BI_local_get j); AI_basic (BI_call (coupon_getter_idx lhs))]
        (v_to_e_list [extern_value h]).
    Proof.
      intros Instance Local Getter; eapply local_index_host with (hf := coupon_getter lhs)
        (addr := coupon_getter_idx lhs) (v := extern_value c).
      - exact Local.
      - rewrite Instance; destruct lhs; reflexivity.
      - rewrite coupon_store_functions; destruct lhs; reflexivity.
      - destruct lhs; unfold coupon_endpoint in Getter; unfold coupon_getter, host_values, extern_value;
          rewrite R.to_handle_to_externref, Getter; reflexivity.
      - destruct lhs; reflexivity.
    Qed.

    Lemma getter_compare coupons f source_idx c getter_idx getter_addr hf target_idx target got :
      f_inst f = coupon_instance ->
      lookup_N (f_locs f) source_idx = Some (extern_value c) ->
      lookup_N (f_locs f) target_idx = Some (extern_value target) ->
      lookup_N (inst_funcs coupon_instance) getter_idx = Some getter_addr ->
      lookup_N (s_funcs (coupon_store coupons)) getter_addr = Some (FC_func_host (function_signature hf) hf) ->
      host_values hf [extern_value c] = Some [extern_value got] ->
      (match function_signature hf with Tf params _ => List.length params = 1%nat end) ->
      runs (coupon_store coupons) f
        [AI_basic (BI_local_get source_idx); AI_basic (BI_call getter_idx);
         AI_basic (BI_local_get target_idx); AI_basic (BI_call 0%N)]
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_equal got target))) ]).
    Proof.
      intros Instance Source Target Index Closure Values Arity.
      eapply runs_trans with (mid := v_to_e_list [extern_value got] ++
        [AI_basic (BI_local_get target_idx); AI_basic (BI_call 0%N)]).
      - change (runs (coupon_store coupons) f
          ([] ++ [AI_basic (BI_local_get source_idx); AI_basic (BI_call getter_idx)] ++
            [AI_basic (BI_local_get target_idx); AI_basic (BI_call 0%N)])
          ([] ++ v_to_e_list [extern_value got] ++
            [AI_basic (BI_local_get target_idx); AI_basic (BI_call 0%N)])).
        apply (runs_context _ _ _ _ [] _).
        eapply local_index_host; try eassumption; rewrite Instance; exact Index.
      - eapply runs_trans with (mid := v_to_e_list [extern_value got; extern_value target] ++ [AI_basic (BI_call 0%N)]).
        + apply runs_step; change (reduce tt (coupon_store coupons) f
            (v_to_e_list [extern_value got] ++ [AI_basic (BI_local_get target_idx)] ++ [AI_basic (BI_call 0%N)])
            tt (coupon_store coupons) f
            (v_to_e_list [extern_value got] ++ [v_to_e (extern_value target)] ++ [AI_basic (BI_call 0%N)])).
          apply step_context, r_local_get; exact Target.
        + eapply call_host with (hf := fixpoint_is_equal) (addr := 0%N); try reflexivity.
          * rewrite Instance; reflexivity.
          * unfold host_values, extern_value; rewrite !R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma equal_runs coupons f left_code right_code left right : f_inst f = coupon_instance ->
      runs (coupon_store coupons) f left_code (v_to_e_list [extern_value left]) ->
      runs (coupon_store coupons) f right_code (v_to_e_list [extern_value right]) ->
      runs (coupon_store coupons) f (left_code ++ right_code ++ [AI_basic (BI_call 0%N)])
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Api.is_equal left right)))]).
    Proof.
      intros Instance Left Right.
      eapply runs_trans with (mid := v_to_e_list [extern_value left] ++ right_code ++ [AI_basic (BI_call 0%N)]).
      - apply (runs_context _ _ _ _ [] _); exact Left.
      - eapply runs_trans with (mid := v_to_e_list [extern_value left; extern_value right] ++ [AI_basic (BI_call 0%N)]).
        + apply (runs_context _ _ _ _ [extern_value left] _); exact Right.
        + eapply call_host with (hf := fixpoint_is_equal) (addr := 0%N); try reflexivity.
          * rewrite Instance; reflexivity.
          * unfold host_values, extern_value; rewrite !R.to_handle_to_externref; reflexivity.
    Qed.

    Lemma guard_prologue s f guard b yes no :
      runs s f (to_e_list guard) (v_to_e_list [VAL_num (VAL_int32 (wasm_bool b))]) ->
      runs s f (to_e_list (guard ++ [BI_if (BT_valtype (Some (T_ref T_externref))) yes no]))
        [v_to_e (VAL_num (VAL_int32 (wasm_bool b)));
         AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) yes no)].
    Proof.
      intro Guard; unfold to_e_list; rewrite map_app.
      apply (runs_context s f (to_e_list guard)
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool b))]) []
        [AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) yes no)]); exact Guard.
    Qed.

    Lemma guard_chain_runs s f guards bits result out :
      Forall2 (fun guard bit => runs s f (to_e_list guard)
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool bit))])) guards bits ->
      runs s f (to_e_list result) out -> terminal_form out ->
      runs s f (to_e_list (guard_chain guards result))
        (if forallb (fun b => b) bits then out else [AI_trap]).
    Proof.
      intro Guards; induction Guards as [|guard bit guards bits Guard Guards IH]; intros Result Terminal.
      - exact Result.
      - cbn [guard_chain]; eapply runs_trans; [apply guard_prologue; exact Guard|].
        destruct bit; cbn [forallb andb].
        + apply if_externref.
          * destruct (forallb (fun b => b) bits); [exact Terminal|right; reflexivity].
          * apply IH; assumption.
        + apply if_externref; [right; reflexivity|].
          apply runs_step, r_simple, rs_unreachable.
    Qed.

    Lemma guard_chain_false s f guards bits result :
      Forall2 (fun guard bit => runs s f (to_e_list guard)
        (v_to_e_list [VAL_num (VAL_int32 (wasm_bool bit))])) guards bits ->
      forallb (fun b => b) bits = false ->
      runs s f (to_e_list (guard_chain guards result)) [AI_trap].
    Proof.
      intro Guards; induction Guards as [|guard bit guards bits Guard Guards IH]; intro Fail; [discriminate|].
      cbn [guard_chain]; eapply runs_trans; [apply guard_prologue; exact Guard|].
      destruct bit.
      - apply if_externref; [right; reflexivity|apply IH; exact Fail].
      - apply if_externref; [right; reflexivity|apply runs_step, r_simple, rs_unreachable].
    Qed.

    Lemma guard_chain_bad_head s f guard rest result :
      runs s f (to_e_list guard) (v_to_e_list [VAL_num (VAL_int32 (wasm_bool false))]) ->
      runs s f (to_e_list (guard_chain (guard :: rest) result)) [AI_trap].
    Proof.
      intro Guard; cbn [guard_chain]; eapply runs_trans; [apply guard_prologue; exact Guard|].
      apply if_externref; [right; reflexivity|apply runs_step, r_simple, rs_unreachable].
    Qed.

    Lemma guard_chain_good_head s f guard rest result out :
      runs s f (to_e_list guard) (v_to_e_list [VAL_num (VAL_int32 (wasm_bool true))]) ->
      runs s f (to_e_list (guard_chain rest result)) out -> terminal_form out ->
      runs s f (to_e_list (guard_chain (guard :: rest) result)) out.
    Proof.
      intros Guard Rest Terminal; cbn [guard_chain]; eapply runs_trans; [apply guard_prologue; exact Guard|].
      apply if_externref; assumption.
    Qed.

    Lemma invoke_pair_locals s inst addr typeidx locals body defaults l r final_frame caller out :
      lookup_N (s_funcs s) addr = Some (FC_func_native type_rr_r inst (Build_module_func typeidx locals body)) ->
      default_vals locals = Some defaults ->
      frame_runs s (Build_frame ([l; r] ++ defaults) inst, to_e_list body) (final_frame, out) ->
      (const_list out = true /\ List.length out = 1%nat) \/ out = [AI_trap] ->
      runs s caller (v_to_e_list [l; r] ++ [AI_invoke addr]) out.
    Proof.
      intros Closure Defaults Body Terminal.
      eapply runs_trans with (mid := [AI_frame 1 (Build_frame ([l; r] ++ defaults) inst) [AI_label 1 [] (to_e_list body)]]).
      - apply runs_step; eapply r_invoke_native with (vs := [l; r])
          (ts1 := [T_ref T_externref; T_ref T_externref]) (ts2 := [T_ref T_externref])
          (ts := locals) (defaults := defaults) (n := 2%nat) (k := List.length locals); try eassumption; reflexivity.
      - eapply runs_trans with (mid := [AI_frame 1 final_frame [AI_label 1 [] out]]).
        + apply (frame_runs_frame s
            (Build_frame ([l; r] ++ defaults) inst, [AI_label 1 [] (to_e_list body)])
            (final_frame, [AI_label 1 [] out]) 1 caller).
          apply (frame_runs_label s (Build_frame ([l; r] ++ defaults) inst, to_e_list body)
            (final_frame, out) 1 []); exact Body.
        + destruct Terminal as [[Const Len]| ->].
          * eapply runs_trans with (mid := [AI_frame 1 final_frame out]).
            -- apply runs_frame, runs_step, r_simple, rs_label_const; exact Const.
            -- apply runs_step, r_simple, rs_local_const; assumption.
          * eapply runs_trans with (mid := [AI_frame 1 final_frame [AI_trap]]).
            -- apply runs_frame, runs_step, r_simple, rs_label_trap.
            -- apply runs_step, r_simple, rs_local_trap.
    Qed.
    Lemma invoke_two_coupons coupons l r caller idx tests out :
      lookup_N (s_funcs (coupon_store coupons)) idx = Some (FC_func_native type_rr_r coupon_instance
        (Build_module_func 0%N [T_ref T_externref; T_ref T_externref] (two_coupon_body tests))) ->
      ((const_list out = true /\ List.length out = 1%nat) \/ out = [AI_trap]) ->
      (match coupons with
       | [] | [_] => out = [AI_trap]
       | c1 :: c2 :: _ => runs (coupon_store coupons) (two_working_frame l r c1 c2) (to_e_list tests) out end) ->
      runs (coupon_store coupons) caller (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke idx]) out.
    Proof.
      intros Closure Terminal Tests; destruct coupons as [|c1 [|c2 rest]].
      - subst out; eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
          (locals := [T_ref T_externref; T_ref T_externref]) (body := two_coupon_body tests)
          (defaults := [null_extern; null_extern]) (final_frame := two_initial_frame l r).
        + exact Closure.
        + reflexivity.
        + apply two_prefix_empty.
        + right; reflexivity.
      - subst out; eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
          (locals := [T_ref T_externref; T_ref T_externref]) (body := two_coupon_body tests)
          (defaults := [null_extern; null_extern]) (final_frame := two_first_frame l r c1).
        + exact Closure.
        + reflexivity.
        + apply two_prefix_single.
        + right; reflexivity.
      - eapply invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
          (locals := [T_ref T_externref; T_ref T_externref]) (body := two_coupon_body tests)
          (defaults := [null_extern; null_extern]) (final_frame := two_working_frame l r c1 c2).
        + exact Closure.
        + reflexivity.
        + eapply frame_runs_trans; [apply two_prefix_first|].
          eapply frame_runs_trans; [apply two_prefix_second|apply runs_to_frame_runs; exact Tests].
        + exact Terminal.
    Qed.
  End Execution.
End CouponTable.
