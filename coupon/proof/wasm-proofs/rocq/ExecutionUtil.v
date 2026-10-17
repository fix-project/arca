From Stdlib Require Import List NArith Relation_Operators String Lia.
From Wasm Require Import datatypes operations opsem instantiation_func interp_instantiate_sound.
From FixProof Require Import Handle CouponApi Host Init ModuleLayout.
Import ListNotations.
Open Scope list_scope.

Module ExecutionUtil (S : STORAGE) (C : COUPON_STORAGE S) (R : EXTERNREFS S).
  Module W := Host S C R.
  Import W.

  Section Execution.
    Context `{mem : BlockUpdateMemory}.

    (** The constructors preserve the store and the caller's frame. This
        specialization of Wasm's reflexive transitive reduction is useful
        for the individual function proofs. *)
    Definition runs (s : store_record) (f : frame) :=
      clos_refl_trans (list administrative_instruction)
        (fun es es' => reduce tt s f es tt s f es').

    Lemma runs_step s f es es' : reduce tt s f es tt s f es' -> runs s f es es'.
    Proof. intro Step; apply rt_step; exact Step. Qed.
    Lemma runs_refl s f es : runs s f es es.
    Proof. apply rt_refl. Qed.
    Lemma runs_trans s f es mid es' : runs s f es mid -> runs s f mid es' -> runs s f es es'.
    Proof. intros First Second; eapply rt_trans; eassumption. Qed.

    Lemma runs_sound s f es es' : runs s f es es' ->
      reduce_trans (tt, s, f, es) (tt, s, f, es').
    Proof.
      intro Run; induction Run.
      - apply rt_step; exact H.
      - apply rt_refl.
      - eapply rt_trans; eassumption.
    Qed.

    Lemma step_context s f es es' vs suffix : reduce tt s f es tt s f es' ->
      reduce tt s f (v_to_e_list vs ++ es ++ suffix) tt s f (v_to_e_list vs ++ es' ++ suffix).
    Proof. intro Step; eapply r_label with (lh := LH_base vs suffix); [exact Step|reflexivity|reflexivity]. Qed.

    Lemma runs_context s f es es' vs suffix : runs s f es es' ->
      runs s f (v_to_e_list vs ++ es ++ suffix) (v_to_e_list vs ++ es' ++ suffix).
    Proof.
      intro Run; induction Run; [apply runs_step, step_context; assumption|apply runs_refl|].
      eapply runs_trans; eassumption.
    Qed.

    Lemma runs_label s f es es' n cont : runs s f es es' ->
      runs s f [AI_label n cont es] [AI_label n cont es'].
    Proof.
      intro Run; induction Run.
      - apply runs_step; eapply r_label with (lh := LH_rec [] n cont (LH_base [] []) []);
          [exact H|simpl; now rewrite app_nil_r|simpl; now rewrite app_nil_r].
      - apply runs_refl.
      - eapply runs_trans; eassumption.
    Qed.

    Lemma runs_frame s f es es' n caller : runs s f es es' ->
      runs s caller [AI_frame n f es] [AI_frame n f es'].
    Proof.
      intro Run; induction Run; [apply runs_step, r_frame; exact H|apply runs_refl|].
      eapply runs_trans; eassumption.
    Qed.

    Lemma call_host s f idx addr hf args out :
      lookup_N (inst_funcs (f_inst f)) idx = Some addr ->
      lookup_N (s_funcs s) addr = Some (FC_func_host (function_signature hf) hf) ->
      host_values hf args = Some out ->
      (match function_signature hf with Tf params _ => List.length args = List.length params end) ->
      runs s f (v_to_e_list args ++ [AI_basic (BI_call idx)]) (v_to_e_list out).
    Proof.
      intros Index Closure Values Arity.
      eapply runs_trans with (mid := v_to_e_list args ++ [AI_invoke addr]).
      - apply runs_step; change (reduce tt s f (v_to_e_list args ++ [AI_basic (BI_call idx)] ++ [])
          tt s f (v_to_e_list args ++ [AI_invoke addr] ++ [])).
        apply step_context, r_call; exact Index.
      - destruct (function_signature hf) as [params rets] eqn:Sig.
        apply runs_step.
        eapply r_invoke_host_success with (h := hf) (r := result_values out) (vcs := args)
          (n := List.length params) (m := List.length rets); try reflexivity; try eassumption.
        left; split; [symmetry; exact Sig|].
        change (Some (s, result_values out) = W.H.omap (fun values => (s, result_values values)) (host_values hf args)).
        rewrite Values; reflexivity.
    Qed.

    Lemma local_pair s f l r :
      lookup_N (f_locs f) 0%N = Some l -> lookup_N (f_locs f) 1%N = Some r ->
      runs s f [AI_basic (BI_local_get 0%N); AI_basic (BI_local_get 1%N)] (v_to_e_list [l; r]).
    Proof.
      intros Lhs Rhs.
      eapply runs_trans with (mid := [v_to_e l; AI_basic (BI_local_get 1%N)]).
      - apply runs_step; change (reduce tt s f ([] ++ [AI_basic (BI_local_get 0%N)] ++ [AI_basic (BI_local_get 1%N)])
          tt s f ([] ++ [v_to_e l] ++ [AI_basic (BI_local_get 1%N)])).
        apply (step_context s f _ _ [] _), r_local_get; exact Lhs.
      - apply runs_step; change (reduce tt s f (v_to_e_list [l] ++ [AI_basic (BI_local_get 1%N)] ++ [])
          tt s f (v_to_e_list [l] ++ [v_to_e r] ++ [])).
        apply step_context, r_local_get; exact Rhs.
    Qed.

    Lemma local_pair_host s f l r idx addr hf out :
      lookup_N (f_locs f) 0%N = Some l -> lookup_N (f_locs f) 1%N = Some r ->
      lookup_N (inst_funcs (f_inst f)) idx = Some addr ->
      lookup_N (s_funcs s) addr = Some (FC_func_host (function_signature hf) hf) ->
      host_values hf [l; r] = Some out ->
      (match function_signature hf with Tf params _ => List.length params = 2%nat end) ->
      runs s f [AI_basic (BI_local_get 0%N); AI_basic (BI_local_get 1%N); AI_basic (BI_call idx)] (v_to_e_list out).
    Proof.
      intros Lhs Rhs Index Closure Values Arity.
      eapply runs_trans with (mid := v_to_e_list [l; r] ++ [AI_basic (BI_call idx)]).
      - change (runs s f ([] ++ [AI_basic (BI_local_get 0%N); AI_basic (BI_local_get 1%N)] ++ [AI_basic (BI_call idx)])
          ([] ++ v_to_e_list [l; r] ++ [AI_basic (BI_call idx)])).
        apply (runs_context s f _ _ [] _), local_pair; assumption.
      - eapply call_host; try eassumption.
        destruct (function_signature hf); simpl in *; symmetry; exact Arity.
    Qed.

    Lemma local_first_host s f l idx addr hf out :
      lookup_N (f_locs f) 0%N = Some l ->
      lookup_N (inst_funcs (f_inst f)) idx = Some addr ->
      lookup_N (s_funcs s) addr = Some (FC_func_host (function_signature hf) hf) ->
      host_values hf [l] = Some out ->
      (match function_signature hf with Tf params _ => List.length params = 1%nat end) ->
      runs s f [AI_basic (BI_local_get 0%N); AI_basic (BI_call idx)] (v_to_e_list out).
    Proof.
      intros Lhs Index Closure Values Arity.
      eapply runs_trans with (mid := v_to_e_list [l] ++ [AI_basic (BI_call idx)]).
      - apply runs_step; change (reduce tt s f ([] ++ [AI_basic (BI_local_get 0%N)] ++ [AI_basic (BI_call idx)])
          tt s f ([] ++ [v_to_e l] ++ [AI_basic (BI_call idx)])).
        apply (step_context s f _ _ [] _), r_local_get; assumption.
      - eapply call_host; try eassumption.
        destruct (function_signature hf); simpl in *; symmetry; exact Arity.
    Qed.

    Lemma if_externref s f (b : bool) yes no out : terminal_form out ->
      runs s f (to_e_list (if b then yes else no)) out ->
      runs s f [v_to_e (VAL_num (VAL_int32 (W.wasm_bool b)));
        AI_basic (BI_if (BT_valtype (Some (T_ref T_externref))) yes no)] out.
    Proof.
      intros Terminal Branch.
      eapply runs_trans with (mid := [AI_basic (BI_block (BT_valtype (Some (T_ref T_externref))) (if b then yes else no))]).
      - apply runs_step, r_simple; destruct b.
        + apply rs_if_true; vm_compute; discriminate.
        + apply rs_if_false; reflexivity.
      - eapply runs_trans with (mid := [AI_label 1 [] (to_e_list (if b then yes else no))]).
        + apply runs_step; eapply r_block with (vs := []) (n := 0%nat) (t1s := []) (t2s := [T_ref T_externref]); reflexivity.
        + eapply runs_trans with (mid := [AI_label 1 [] out]); [apply runs_label; exact Branch|].
          apply runs_step, r_simple; destruct Terminal as [Const| ->];
            [apply rs_label_const; exact Const|apply rs_label_trap].
    Qed.

    Lemma invoke_pair s inst addr typeidx body l r caller out :
      lookup_N (s_funcs s) addr = Some (FC_func_native type_rr_r inst (Build_module_func typeidx [] body)) ->
      runs s (Build_frame [l; r] inst) (to_e_list body) out ->
      (const_list out = true /\ List.length out = 1%nat) \/ out = [AI_trap] ->
      runs s caller (v_to_e_list [l; r] ++ [AI_invoke addr]) out.
    Proof.
      intros Closure Body Terminal.
      eapply runs_trans with (mid := [AI_frame 1 (Build_frame [l; r] inst) [AI_label 1 [] (to_e_list body)]]).
      - apply runs_step; eapply r_invoke_native with (vs := [l; r])
          (ts1 := [T_ref T_externref; T_ref T_externref]) (ts2 := [T_ref T_externref])
          (ts := []) (defaults := []) (n := 2%nat) (k := 0%nat); try eassumption; reflexivity.
      - eapply runs_trans with (mid := [AI_frame 1 (Build_frame [l; r] inst) [AI_label 1 [] out]]).
        + apply runs_frame, runs_label; exact Body.
        + destruct Terminal as [[Const Len]| ->].
          * eapply runs_trans with (mid := [AI_frame 1 (Build_frame [l; r] inst) out]).
            -- apply runs_frame, runs_step, r_simple, rs_label_const; exact Const.
            -- apply runs_step, r_simple, rs_local_const; assumption.
          * eapply runs_trans with (mid := [AI_frame 1 (Build_frame [l; r] inst) [AI_trap]]).
            -- apply runs_frame, runs_step, r_simple, rs_label_trap.
            -- apply runs_step, r_simple, rs_local_trap.
    Qed.

    (** The import order is checked by the executable instantiator below.
        A host store contains only these closures; Wasm allocates the native
        functions and tables from the parsed module. *)
    Definition fixpoint_functions := [fixpoint_is_equal; fixpoint_is_storage_coupon;
      fixpoint_is_force_coupon; fixpoint_is_eq_coupon; fixpoint_is_eval_coupon;
      fixpoint_is_apply_coupon; fixpoint_is_think_coupon; fixpoint_create_application_thunk;
      fixpoint_create_strict_encode; fixpoint_create_shallow_encode; fixpoint_get_coupon_lhs;
      fixpoint_get_coupon_rhs; fixpoint_create_eq_coupon; fixpoint_create_eval_coupon;
      fixpoint_create_think_coupon; fixpoint_create_force_coupon; fixpoint_get_tree_size;
      fixpoint_get_tree_data; fixpoint_is_blob_obj; fixpoint_is_data; fixpoint_is_object].
    Definition host_store : store_record := Build_store_record
      (map (fun hf => FC_func_host (function_signature hf) hf) fixpoint_functions) [] [] [] [] [].
    Definition import_values := map (fun n => EV_func (N.of_nat n)) (seq 0 21).
    Definition dispatch_indices := [func_make_eq_tree_coupon_idx; func_make_eq_application_coupon_idx;
      func_make_force_result_eq_coupon_idx; func_make_eq_encode_strict_coupon_idx;
      func_make_think_application_coupon_idx; func_make_think_to_force_coupon_idx;
      func_make_force_to_encode_strict_coupon_idx; func_make_eval_eq_coupon_idx;
      func_make_eval_blobobj_coupon_idx; func_make_eval_tree_coupon_idx;
      func_make_sym_coupon_idx; func_make_trans_coupon_idx; func_make_self_coupon_idx].
    Definition module_allocation := interp_alloc_module host_store coupon_module import_values []
      [map VAL_ref_func dispatch_indices].
    Definition allocated_store := fst module_allocation.
    Definition coupon_instance := snd module_allocation.
    Definition initialization_code := instantiation_spec.get_init_expr_elems (mod_elems coupon_module).

    Lemma coupon_module_instantiated :
      interp_instantiate tt host_store coupon_module import_values =
      (Some (tt, allocated_store, Build_frame [] coupon_instance, initialization_code), ""%string).
    Proof. vm_compute; reflexivity. Qed.

    Lemma coupon_module_instantiation_sound :
      instantiation_spec.instantiate host_store coupon_module import_values
        (allocated_store, Build_frame [] coupon_instance, initialization_code).
    Proof. eapply interp_instantiate_imp_instantiate; exact coupon_module_instantiated. Qed.

    Lemma coupon_equal_index : lookup_N (inst_funcs coupon_instance) 0%N = Some 0%N.
    Proof. reflexivity. Qed.
    Lemma coupon_create_eq_index : lookup_N (inst_funcs coupon_instance) 12%N = Some 12%N.
    Proof. reflexivity. Qed.
    Lemma coupon_equal_closure : lookup_N (s_funcs allocated_store) 0%N =
      Some (FC_func_host type_rr_i32 fixpoint_is_equal).
    Proof. reflexivity. Qed.
    Lemma coupon_create_eq_closure : lookup_N (s_funcs allocated_store) 12%N =
      Some (FC_func_host type_rr_r fixpoint_create_eq_coupon).
    Proof. reflexivity. Qed.
    Lemma coupon_self_closure : lookup_N (s_funcs allocated_store) func_make_self_coupon_idx =
      Some (FC_func_native type_rr_r coupon_instance self_code).
    Proof. reflexivity. Qed.

    (** Retain the final configuration, which WasmCert's convenience fuel
        runner discards. Every step still comes from its certified interpreter.
        Both values and traps are terminal configurations. *)
    Fixpoint certified_run_ctx (fuel : nat) (hs : unit) (cfg : cfg_tuple_ctx) (depth : N)
      : option (unit * store_record * frame * list administrative_instruction) :=
      match fuel with
      | O => None
      | Datatypes.S remaining =>
        match run_one_step_ctx (ho := fixpoint_host) (host_application_impl := host_execute) host_execute_correct hs cfg depth with
        | RSC_normal hs' cfg' depth' _ _ =>
          match run_step_cfg_ctx_reform cfg' with
          | Some cfg'' => certified_run_ctx remaining hs' cfg'' depth'
          | None => None
          end
        | RSC_value s f vs _ => Some (hs, s, f, v_to_e_list vs)
        | RSC_trap s f _ => Some (hs, s, f, [AI_trap])
        | _ => None
        end
      end.

    Lemma reform_cfg_sound cfg cfg' : valid_ccs_cfg cfg ->
      run_step_cfg_ctx_reform cfg = Some cfg' -> ctx_to_cfg cfg = ctx_to_cfg cfg'.
    Proof.
      destruct cfg as [[[s ccs] sc] oe]; intros Valid Reform.
      assert (valid_ccs ccs = true) as Nonempty by (unfold valid_ccs_cfg in Valid; destruct ccs; [contradiction|reflexivity]).
      destruct (run_step_reform_valid s sc oe Nonempty) as [cfg'' [Eq [Same Valid']]].
      rewrite Reform in Eq; inversion Eq; subst; exact Same.
    Qed.

    Lemma certified_run_ctx_sound fuel hs cfg depth hs' s' f' es' :
      certified_run_ctx fuel hs cfg depth = Some (hs', s', f', es') ->
      forall s f es, ctx_to_cfg cfg = Some (s, (f, es)) ->
      reduce_trans (hs, s, f, es) (hs', s', f', es').
    Proof.
      revert hs cfg depth hs' s' f' es'.
      induction fuel as [|fuel IH]; intros hs cfg depth hs' s' f' es' Run s f es Cfg;
        [discriminate|].
      cbn [certified_run_ctx] in Run.
      destruct (run_one_step_ctx (ho := fixpoint_host) (host_application_impl := host_execute) host_execute_correct hs cfg depth)
        as [hs1 cfg1 depth1 Step Valid|s1 f1 vs Terminal|s1 f1 Terminal|Invalid|Error].
      - destruct (run_step_cfg_ctx_reform cfg1) as [cfg2|] eqn:Reform; [|discriminate].
        pose proof (reform_cfg_sound _ _ Valid Reform) as Same.
        unfold reduce_ctx in Step; rewrite Cfg in Step.
        destruct (ctx_to_cfg cfg1) as [[s1 [f1 es1]]|] eqn:Next; [|contradiction].
        eapply rt_trans with (y := (hs1, s1, f1, es1)); [apply rt_step; exact Step|].
        eapply IH; [exact Run|rewrite <- Same; reflexivity].
      - inversion Run; subst; rewrite Cfg in Terminal; inversion Terminal; subst; apply rt_refl.
      - inversion Run; subst; rewrite Cfg in Terminal; inversion Terminal; subst; apply rt_refl.
      - discriminate.
      - discriminate.
    Qed.

    Definition certified_run fuel s f es :=
      let '(exist cfg _) := interp_cfg_of_wasm (s, (f, es)) in
      certified_run_ctx fuel tt cfg 0%N.

    Lemma certified_run_sound fuel s f es hs' s' f' es' :
      certified_run fuel s f es = Some (hs', s', f', es') ->
      reduce_trans (tt, s, f, es) (hs', s', f', es').
    Proof.
      unfold certified_run; destruct (interp_cfg_of_wasm (s, (f, es))) as [cfg [Cfg Valid]].
      intro Run; eapply certified_run_ctx_sound; eassumption.
    Qed.

    Definition ready_store := Build_store_record (s_funcs allocated_store)
      [Build_tableinst (Build_table_type (Build_limits 0%N None) T_externref) [];
       Build_tableinst (Build_table_type (Build_limits 13%N (Some 13%N)) T_funcref)
         (map VAL_ref_func dispatch_indices)]
      [] [] [Build_eleminst T_funcref []] [].

    Definition partial_store (filled : nat) := Build_store_record (s_funcs allocated_store)
      [Build_tableinst (Build_table_type (Build_limits 0%N None) T_externref) [];
       Build_tableinst (Build_table_type (Build_limits 13%N (Some 13%N)) T_funcref)
         (firstn filled (map VAL_ref_func dispatch_indices) ++ repeat (VAL_ref_null T_funcref) (13 - filled))]
      [] [] [Build_eleminst T_funcref (map VAL_ref_func dispatch_indices)] [].
    Definition init_config (filled : nat) :=
      (tt, partial_store filled, Build_frame [] coupon_instance,
       [v_to_e (VAL_num (VAL_int32 (i32_of_nat filled)));
        v_to_e (VAL_num (VAL_int32 (i32_of_nat filled)));
        v_to_e (VAL_num (VAL_int32 (i32_of_nat (13 - filled))));
        AI_basic (BI_table_init 1%N 0%N); AI_basic (BI_elem_drop 0%N)]).
    Definition init_intermediate (filled : nat) :=
      (tt, partial_store filled, Build_frame [] coupon_instance,
       [v_to_e (VAL_num (VAL_int32 (i32_of_nat filled)));
        v_to_e (VAL_ref (VAL_ref_func (nth filled dispatch_indices 0%N)));
        AI_basic (BI_table_set 1%N);
        v_to_e (VAL_num (VAL_int32 (i32_of_nat (Datatypes.S filled))));
        v_to_e (VAL_num (VAL_int32 (i32_of_nat (Datatypes.S filled))));
        v_to_e (VAL_num (VAL_int32 (i32_of_nat (13 - Datatypes.S filled))));
        AI_basic (BI_table_init 1%N 0%N); AI_basic (BI_elem_drop 0%N)]).

    Ltac init_entry_step :=
      match goal with
      | |- reduce_trans (init_config ?filled_count) (init_config _) =>
        eapply rt_trans with (y := init_intermediate filled_count);
        [ apply rt_step; unfold reduce_tuple, init_config, init_intermediate;
          eapply r_label with (lh := LH_base [] [AI_basic (BI_elem_drop 0%N)]);
          [ eapply r_table_init_step with (x := 1%N) (y := 0%N)
              (src := i32_of_nat filled_count) (dst := i32_of_nat filled_count) (n := i32_of_nat (13 - filled_count))
              (src' := i32_of_nat (Datatypes.S filled_count)) (dst' := i32_of_nat (Datatypes.S filled_count))
              (n' := i32_of_nat (13 - Datatypes.S filled_count))
              (v := VAL_ref_func (nth filled_count dispatch_indices 0%N));
              vm_compute; try reflexivity; try lia; try discriminate
          | reflexivity | reflexivity ]
        | apply rt_step; unfold reduce_tuple, init_config, init_intermediate;
          eapply r_label with (lh := LH_base []
            [v_to_e (VAL_num (VAL_int32 (i32_of_nat (Datatypes.S filled_count))));
             v_to_e (VAL_num (VAL_int32 (i32_of_nat (Datatypes.S filled_count))));
             v_to_e (VAL_num (VAL_int32 (i32_of_nat (13 - Datatypes.S filled_count))));
             AI_basic (BI_table_init 1%N 0%N); AI_basic (BI_elem_drop 0%N)]);
          [ eapply r_table_set_success with (x := 1%N) (i := i32_of_nat filled_count)
              (tabv := VAL_ref_func (nth filled_count dispatch_indices 0%N)); vm_compute; reflexivity
          | reflexivity | reflexivity ] ]
      end.

    Lemma initialization_entry filled : (filled < 13)%nat ->
      reduce_trans (init_config filled) (init_config (Datatypes.S filled)).
    Proof.
      intro Bound.
      do 13 (destruct filled as [|filled]; [init_entry_step|]).
      lia.
    Qed.

    Lemma coupon_module_initialization_sound :
      reduce_trans (tt, allocated_store, Build_frame [] coupon_instance, to_e_list initialization_code)
        (tt, ready_store, Build_frame [] coupon_instance, []).
    Proof.
      change (reduce_trans (init_config 0) (tt, ready_store, Build_frame [] coupon_instance, [])).
      do 13 (eapply rt_trans; [apply initialization_entry; lia|]).
      eapply rt_trans with (y := (tt, partial_store 13, Build_frame [] coupon_instance, [AI_basic (BI_elem_drop 0%N)])).
      - apply rt_step; unfold reduce_tuple, init_config.
        eapply r_label with (lh := LH_base [] [AI_basic (BI_elem_drop 0%N)]);
          [eapply r_table_init_return with (x := 1%N) (y := 0%N)
            (src := i32_of_nat 13) (dst := i32_of_nat 13) (n := i32_of_nat 0)
            (tab := Build_tableinst (Build_table_type (Build_limits 13%N (Some 13%N)) T_funcref)
              (map VAL_ref_func dispatch_indices))
            (elem := Build_eleminst T_funcref (map VAL_ref_func dispatch_indices));
            vm_compute; try reflexivity; try discriminate; lia|reflexivity|reflexivity].
      - apply rt_step; eapply r_elem_drop; vm_compute; reflexivity.
    Qed.

    Lemma store_functions_projection functions tabs mems globals elems datas :
      s_funcs (Build_store_record functions tabs mems globals elems datas) = functions.
    Proof. reflexivity. Qed.

    Lemma ready_functions : s_funcs ready_store = s_funcs allocated_store.
    Proof. apply store_functions_projection. Qed.

    Lemma ready_dispatch : stab ready_store coupon_instance 1%N =
      Some (Build_tableinst (Build_table_type (Build_limits 13%N (Some 13%N)) T_funcref)
        (map VAL_ref_func dispatch_indices)).
    Proof. reflexivity. Qed.
  End Execution.
End ExecutionUtil.
