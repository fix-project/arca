From Stdlib Require Import List Bool Arith NArith ZArith Logic.ProofIrrelevance.
From Wasm Require Import datatypes operations opsem.
From FixProof Require Import Handle ApplyTree CouponApi CouponConstructors Host ModuleLayout
  CouponTable Dispatcher SelfCoupon EvalBlobCoupon SymCoupon TransCoupon MappedEqCoupon
  ThinkToForceCoupon ForceToEncodeStrictCoupon EvalEqCoupon ThinkApplicationCoupon ForceResultEqCoupon TreeCoupon.
Import ListNotations.
Open Scope list_scope.

Module MakeCoupon (S : STORAGE) (P : PROGRAM S) (C : COUPON_STORAGE S) (R : EXTERNREFS S).
  Module D := Dispatcher S P C R.
  Module Self := SelfCoupon S P C R.
  Module Blob := EvalBlobCoupon S P C R.
  Module Sym := SymCoupon S P C R.
  Module Trans := TransCoupon S P C R.
  Module Mapped := MappedEqCoupon S P C R.
  Module ThinkForce := ThinkToForceCoupon S P C R.
  Module ForceStrict := ForceToEncodeStrictCoupon S P C R.
  Module EvalEq := EvalEqCoupon S P C R.
  Module ThinkApp := ThinkApplicationCoupon S P C R.
  Module ForceEq := ForceResultEqCoupon S P C R.
  Module Tree := TreeCoupon S P C R.
  Import D D.T D.T.U D.T.U.W.

  Section Execution.
    Context `{mem : BlockUpdateMemory}.

    Lemma self_host_eq : Self.U.W.fixpoint_host = D.T.U.W.fixpoint_host.
    Proof. unfold Self.U.W.fixpoint_host, D.T.U.W.fixpoint_host; f_equal; apply proof_irrelevance. Qed.
    Lemma sym_host_eq : Sym.T.U.W.fixpoint_host = D.T.U.W.fixpoint_host.
    Proof. unfold Sym.T.U.W.fixpoint_host, D.T.U.W.fixpoint_host; f_equal; apply proof_irrelevance. Qed.
    Lemma trans_host_eq : Trans.T.U.W.fixpoint_host = D.T.U.W.fixpoint_host.
    Proof. unfold Trans.T.U.W.fixpoint_host, D.T.U.W.fixpoint_host; f_equal; apply proof_irrelevance. Qed.
    Lemma mapped_host_eq : Mapped.T.U.W.fixpoint_host = D.T.U.W.fixpoint_host.
    Proof. unfold Mapped.T.U.W.fixpoint_host, D.T.U.W.fixpoint_host; f_equal; apply proof_irrelevance. Qed.
    Lemma think_force_host_eq : ThinkForce.T.U.W.fixpoint_host = D.T.U.W.fixpoint_host.
    Proof. unfold ThinkForce.T.U.W.fixpoint_host, D.T.U.W.fixpoint_host; f_equal; apply proof_irrelevance. Qed.
    Lemma force_strict_host_eq : ForceStrict.M.T.U.W.fixpoint_host = D.T.U.W.fixpoint_host.
    Proof. unfold ForceStrict.M.T.U.W.fixpoint_host, D.T.U.W.fixpoint_host; f_equal; apply proof_irrelevance. Qed.
    Lemma eval_eq_host_eq : EvalEq.T.U.W.fixpoint_host = D.T.U.W.fixpoint_host.
    Proof. unfold EvalEq.T.U.W.fixpoint_host, D.T.U.W.fixpoint_host; f_equal; apply proof_irrelevance. Qed.
    Lemma think_app_host_eq : ThinkApp.T.U.W.fixpoint_host = D.T.U.W.fixpoint_host.
    Proof. unfold ThinkApp.T.U.W.fixpoint_host, D.T.U.W.fixpoint_host; f_equal; apply proof_irrelevance. Qed.
    Lemma force_eq_host_eq : ForceEq.T.U.W.fixpoint_host = D.T.U.W.fixpoint_host.
    Proof. unfold ForceEq.T.U.W.fixpoint_host, D.T.U.W.fixpoint_host; f_equal; apply proof_irrelevance. Qed.
    Lemma tree_host_eq : Tree.L.T.U.W.fixpoint_host = D.T.U.W.fixpoint_host.
    Proof. unfold Tree.L.T.U.W.fixpoint_host, D.T.U.W.fixpoint_host; f_equal; apply proof_irrelevance. Qed.
    Lemma runs_host_transport (source : @host fixpoint_host_functions mem)
      (Same : source = D.T.U.W.fixpoint_host) s f es es' :
      (forall hs : @host_state fixpoint_host_functions mem source,
        Relation_Operators.clos_refl_trans _
          (fun a b => @reduce fixpoint_host_functions mem source hs s f a hs s f b) es es') ->
      D.T.U.runs s f es es'.
    Proof. intro All; rewrite Same in All; exact (All tt). Qed.

    Lemma self_runs_shared s f es es' : Self.U.runs s f es es' -> D.T.U.runs s f es es'.
    Proof.
      intro Run; apply (runs_host_transport _ self_host_eq).
      intro hs; destruct hs; exact Run.
    Qed.
    Lemma sym_runs_shared s f es es' : Sym.T.U.runs s f es es' -> D.T.U.runs s f es es'.
    Proof.
      intro Run; apply (runs_host_transport _ sym_host_eq).
      intro hs; destruct hs; exact Run.
    Qed.
    Lemma trans_runs_shared s f es es' : Trans.T.U.runs s f es es' -> D.T.U.runs s f es es'.
    Proof.
      intro Run; apply (runs_host_transport _ trans_host_eq).
      intro hs; destruct hs; exact Run.
    Qed.
    Lemma mapped_runs_shared s f es es' : Mapped.T.U.runs s f es es' -> D.T.U.runs s f es es'.
    Proof.
      intro Run; apply (runs_host_transport _ mapped_host_eq).
      intro hs; destruct hs; exact Run.
    Qed.
    Lemma think_force_runs_shared s f es es' : ThinkForce.T.U.runs s f es es' -> D.T.U.runs s f es es'.
    Proof.
      intro Run; apply (runs_host_transport _ think_force_host_eq).
      intro hs; destruct hs; exact Run.
    Qed.
    Lemma force_strict_runs_shared s f es es' : ForceStrict.M.T.U.runs s f es es' -> D.T.U.runs s f es es'.
    Proof.
      intro Run; apply (runs_host_transport _ force_strict_host_eq).
      intro hs; destruct hs; exact Run.
    Qed.
    Lemma eval_eq_runs_shared s f es es' : EvalEq.T.U.runs s f es es' -> D.T.U.runs s f es es'.
    Proof.
      intro Run; apply (runs_host_transport _ eval_eq_host_eq).
      intro hs; destruct hs; exact Run.
    Qed.
    Lemma think_app_runs_shared s f es es' : ThinkApp.T.U.runs s f es es' -> D.T.U.runs s f es es'.
    Proof.
      intro Run; apply (runs_host_transport _ think_app_host_eq).
      intro hs; destruct hs; exact Run.
    Qed.
    Lemma force_eq_runs_shared s f es es' : ForceEq.T.U.runs s f es es' -> D.T.U.runs s f es es'.
    Proof.
      intro Run; apply (runs_host_transport _ force_eq_host_eq).
      intro hs; destruct hs; exact Run.
    Qed.
    Lemma tree_runs_shared s f es es' : Tree.L.T.U.runs s f es es' -> D.T.U.runs s f es es'.
    Proof.
      intro Run; apply (runs_host_transport _ tree_host_eq).
      intro hs; destruct hs; exact Run.
    Qed.

    (** Self and blob evaluation do not read the coupon table. Their native
        executions also apply to the store containing the client's table. *)
    Lemma self_execution coupons l r caller : runs (coupon_store coupons) caller
      (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_self_coupon_idx])
      (match D.B.make_self_coupon coupons l r with Some c => v_to_e_list [extern_value c] | None => [AI_trap] end).
    Proof.
      apply self_runs_shared.
      destruct (D.B.make_self_coupon coupons l r) as [c|] eqn:Make.
      - eapply Self.make_self_coupon_raw_run_invoke_some with (inst := coupon_instance)
          (equal_addr := 0%N) (create_addr := 12%N) (coupons := coupons).
        + reflexivity.
        + reflexivity.
        + rewrite coupon_store_functions; reflexivity.
        + rewrite coupon_store_functions; reflexivity.
        + rewrite coupon_store_functions; reflexivity.
        + exact Make.
      - eapply Self.make_self_coupon_raw_run_invoke_none with (inst := coupon_instance)
          (equal_addr := 0%N) (coupons := coupons).
        + reflexivity.
        + rewrite coupon_store_functions; reflexivity.
        + rewrite coupon_store_functions; reflexivity.
        + exact Make.
    Qed.

    Lemma blob_execution coupons l r caller : runs (coupon_store coupons) caller
      (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_eval_blobobj_coupon_idx])
      (match D.B.make_eval_blob_coupon coupons l r with Some c => v_to_e_list [extern_value c] | None => [AI_trap] end).
    Proof.
      assert (Guards : Forall2 (fun guard bit => runs (coupon_store coupons) (Blob.argument_frame l r)
        (to_e_list guard) (v_to_e_list [VAL_num (VAL_int32 (wasm_bool bit))]))
        [[BI_local_get 0%N; BI_call 18%N]; [BI_local_get 0%N; BI_local_get 1%N; BI_call 0%N]]
        [D.T.U.W.H.get_type l =? 0; Api.is_equal l r]).
      { constructor.
        - eapply local_first_host with (hf := fixpoint_is_blob_obj) (addr := 18%N); try reflexivity.
          unfold host_values, extern_value, Blob.U.W.extern_value; rewrite R.to_handle_to_externref; reflexivity.
        - constructor; [|constructor].
          eapply local_pair_host with (hf := fixpoint_is_equal) (addr := 0%N); try reflexivity.
          unfold host_values, extern_value, Blob.U.W.extern_value; rewrite !R.to_handle_to_externref; reflexivity. }
      assert (Create : runs (coupon_store coupons) (Blob.argument_frame l r) (to_e_list eval_blob_create_body)
        (v_to_e_list [extern_value (C.create_coupon C.Eval l r)])).
      { eapply local_pair_host with (hf := fixpoint_create_eval_coupon) (addr := 13%N); try reflexivity.
        unfold host_values, extern_value, Blob.U.W.extern_value; rewrite !R.to_handle_to_externref; reflexivity. }
      assert (Terminal : terminal_form (v_to_e_list [extern_value (C.create_coupon C.Eval l r)])) by (left; reflexivity).
      pose proof (guard_chain_runs _ _ _ _ eval_blob_create_body _ Guards Create Terminal) as Body.
      cbn [forallb] in Body; rewrite andb_true_r in Body.
      change (runs (coupon_store coupons) (Blob.argument_frame l r) (to_e_list eval_blob_body)
        (if (D.B.D.EC.E.EP.E.H.get_type l =? 0) && D.B.API.is_equal l r then
          v_to_e_list [extern_value (C.create_coupon C.Eval l r)] else [AI_trap])) in Body.
      unfold D.B.make_eval_blob_coupon.
      eapply invoke_pair with (inst := coupon_instance) (typeidx := 0%N) (body := eval_blob_body).
      - rewrite coupon_store_functions; reflexivity.
      - destruct ((D.B.D.EC.E.EP.E.H.get_type l =? 0) && D.B.API.is_equal l r); exact Body.
      - destruct ((D.B.D.EC.E.EP.E.H.get_type l =? 0) && D.B.API.is_equal l r); [left; split; reflexivity|right; reflexivity].
    Qed.

    (** The original symmetry failure theorem is a reduction closure. Retain
        its case split here in the fixed-store closure used by the dispatcher. *)
    Lemma sym_execution coupons l r caller : runs (coupon_store coupons) caller
      (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_sym_coupon_idx])
      (match D.B.make_sym_coupon coupons l r with Some c => v_to_e_list [extern_value c] | None => [AI_trap] end).
    Proof.
      apply sym_runs_shared.
      destruct (D.B.make_sym_coupon coupons l r) as [c|] eqn:Make.
      - apply Sym.make_sym_coupon_raw_run_invoke_some; exact Make.
      - destruct (Sym.ms_none _ _ _ Make) as [->|[c [rest [-> Fail]]]].
        + eapply Sym.T.invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
            (locals := [T_ref T_externref]) (body := sym_body) (defaults := [null_extern])
            (final_frame := Sym.initial_frame l r).
          * rewrite coupon_store_functions; reflexivity.
          * reflexivity.
          * apply Sym.sym_prefix_empty.
          * right; reflexivity.
        + eapply Sym.T.invoke_pair_locals with (inst := coupon_instance) (typeidx := 0%N)
            (locals := [T_ref T_externref]) (body := sym_body) (defaults := [null_extern])
            (final_frame := Sym.working_frame l r c).
          * rewrite coupon_store_functions; reflexivity.
          * reflexivity.
          * eapply Sym.T.frame_runs_trans with (middle := (Sym.working_frame l r c, to_e_list sym_test_body)).
            -- apply Sym.sym_prefix_some.
            -- apply Sym.T.runs_to_frame_runs; destruct Fail as [Tag|[cl [cr [Lhs [Rhs Endpoints]]]]].
               ++ apply Sym.sym_tests_bad_type; exact Tag.
               ++ eapply Sym.sym_tests_bad_endpoint; eassumption.
          * right; reflexivity.
    Qed.

    Lemma trans_execution coupons l r caller : runs (coupon_store coupons) caller
      (v_to_e_list [extern_value l; extern_value r] ++ [AI_invoke func_make_trans_coupon_idx])
      (match D.B.make_trans_coupon coupons l r with Some c => v_to_e_list [extern_value c] | None => [AI_trap] end).
    Proof.
      apply trans_runs_shared.
      destruct (D.B.make_trans_coupon coupons l r) as [c|] eqn:Make;
        [apply Trans.make_trans_coupon_raw_run_invoke_some|apply Trans.make_trans_coupon_raw_run_invoke_none]; exact Make.
    Qed.

    Lemma native_execution coupons req l r caller :
      (Z.of_nat (List.length coupons) < 2 ^ 31)%Z -> Tree.tree_size_bounded l -> Tree.tree_size_bounded r ->
      runs (coupon_store coupons) caller (child_call req l r)
        (match D.B.make_coupon req coupons l r with Some c => v_to_e_list [extern_value c] | None => [AI_trap] end).
    Proof.
      intros Bound BoundL BoundR; destruct req; cbn [child_call request_to_func_idx D.B.make_coupon].
      - apply tree_runs_shared; apply (Tree.make_tree_coupon_execution Tree.TreeEq); assumption.
      - apply force_eq_runs_shared; apply ForceEq.make_force_result_eq_coupon_execution.
      - apply think_app_runs_shared; apply ThinkApp.make_think_application_coupon_execution.
      - apply think_force_runs_shared; apply ThinkForce.make_think_to_force_coupon_execution.
      - apply mapped_runs_shared; apply (Mapped.mapped_execution Mapped.ApplicationMap).
      - apply blob_execution.
      - apply tree_runs_shared; apply (Tree.make_tree_coupon_execution Tree.TreeEval); assumption.
      - apply force_strict_runs_shared; apply ForceStrict.make_force_to_encode_strict_coupon_execution.
      - apply eval_eq_runs_shared; apply EvalEq.make_eval_eq_coupon_execution.
      - apply mapped_runs_shared; apply (Mapped.mapped_execution Mapped.StrictMap).
      - apply sym_execution.
      - apply trans_execution.
      - apply self_execution.
    Qed.

    Theorem make_coupon_execution coupons req l r caller :
      (Z.of_nat (List.length coupons) < 2 ^ 31)%Z -> Tree.tree_size_bounded l -> Tree.tree_size_bounded r ->
      runs (coupon_store coupons) caller
        (v_to_e_list [request_value req; extern_value l; extern_value r] ++ [AI_invoke func_make_coupon_idx])
        (match D.B.make_coupon req coupons l r with Some c => v_to_e_list [extern_value c] | None => [AI_trap] end).
    Proof.
      intros Bound BoundL BoundR; apply make_coupon_from_native.
      - apply native_execution; assumption.
      - destruct (D.B.make_coupon req coupons l r); [left; split; reflexivity|right; reflexivity].
    Qed.

    Theorem make_coupon_success coupons req l r c caller : Forall D.B.coupon_good coupons ->
      D.B.make_coupon req coupons l r = Some c ->
      (Z.of_nat (List.length coupons) < 2 ^ 31)%Z -> Tree.tree_size_bounded l -> Tree.tree_size_bounded r ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [request_value req; extern_value l; extern_value r] ++ [AI_invoke func_make_coupon_idx])
        (tt, coupon_store coupons, caller, v_to_e_list [extern_value c]) /\ D.B.coupon_good c.
    Proof.
      intros Goods Make Bound BoundL BoundR; split.
      - apply runs_sound; pose proof (make_coupon_execution coupons req l r caller Bound BoundL BoundR) as Run.
        rewrite Make in Run; exact Run.
      - eapply D.B.make_coupon_good; eassumption.
    Qed.

    Theorem make_coupon_failure coupons req l r caller : D.B.make_coupon req coupons l r = None ->
      (Z.of_nat (List.length coupons) < 2 ^ 31)%Z -> Tree.tree_size_bounded l -> Tree.tree_size_bounded r ->
      reduce_trans (tt, coupon_store coupons, caller,
        v_to_e_list [request_value req; extern_value l; extern_value r] ++ [AI_invoke func_make_coupon_idx])
        (tt, coupon_store coupons, caller, [AI_trap]).
    Proof.
      intros Make Bound BoundL BoundR; apply runs_sound.
      pose proof (make_coupon_execution coupons req l r caller Bound BoundL BoundR) as Run; rewrite Make in Run; exact Run.
    Qed.
  End Execution.
End MakeCoupon.
