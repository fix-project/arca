From Wasm Require Import typing properties.
(** Proves native witness authentication and premise checking under explicit
    comparison and resource conditions. *)

From FixProof.Collector.wasm Require Import Backend Support.
Import ListNotations.
Local Open Scope list_scope.

Module WasmWitnesses (C : COLLECTOR_CONTEXT).
  Import C S R.
  Module Support := WasmSupport C.
  Import Support.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.

  Local Notation ext := G.E.H.W.extern_value.
  Local Notation treeh := G.E.H.W.H.HTreeObj.
  Local Notation blobh := G.E.H.W.H.HBlobObj.
  Local Notation lower := X.D.EC.E.EP.E.lower.
  Local Notation lift := X.D.EC.E.EP.E.lift.
  Local Notation local_get_run := (G.E.collector_local_get combination true true handle_bytes).

  Definition authenticated_claim witness claim :=
    exists bundle tag author,
      witness = treeh bundle /\ get_tree_raw bundle = [treeh tag; treeh claim] /\
      get_tree_raw tag = [author; G.E.H.W.H.HTreeRef claim] /\ tree_tag tag = true /\
      List.length (get_tree_raw claim) = 3.

  Definition claim_views_ready store := forall witness,
    List.length (get_tree_raw witness) = 2 ->
    exists first,
      G.E.H.tree_attachment_result G.E.collector_instance store
        [ext (treeh witness); word 3] = Some (first, result_values []) /\
      forall tag, List.length (get_tree_raw tag) = 2 ->
        exists second, G.E.H.tree_attachment_result G.E.collector_instance first
          [ext (treeh tag); word 6] = Some (second, result_values []).

  Definition claim_store_transition store witness claim next :=
    exists bundle tag first,
      witness = treeh bundle /\ get_tree_raw bundle = [treeh tag; treeh claim] /\
      G.E.H.tree_attachment_result G.E.collector_instance store
        [ext (treeh bundle); word 3] = Some (first, result_values []) /\
      G.E.H.tree_attachment_result G.E.collector_instance first
        [ext (treeh tag); word 6] = Some (next, result_values []).

  Lemma authenticated_claim_supplied witness claim :
    authenticated_claim witness claim -> supplied_claim witness = treeh claim.
  Proof.
    intros [bundle [tag [author [Witness [Bundle [Tag [Tagged Arity]]]]]]].
    subst witness; eapply supplied_claim_bundle; exact Bundle.
  Qed.

  Section Execution.
    Lemma collector_claim_prefix_executes state store attached witness coupon assertion tagged :
      s_funcs store = s_funcs G.E.allocated_store ->
      get_tree_raw witness = [coupon; assertion] -> G.E.H.W.Api.is_tag coupon = tagged ->
      G.E.H.tree_attachment_result G.E.collector_instance store
        [ext (treeh witness); word 3] = Some (attached, result_values []) ->
      let empty := VAL_ref (VAL_ref_null T_externref) in
      reduce_trans
        (state, store, Build_frame [ext (treeh witness); empty; empty] G.E.collector_instance,
          to_e_list (firstn 15 (modfunc_body evaluation_claim)))
        (state, attached, Build_frame [ext (treeh witness); ext coupon; ext assertion] G.E.collector_instance,
          if tagged then [] else [AI_trap]).
    Proof.
      intros Functions Witness Tagged Attached empty.
      set (locals0 := [ext (treeh witness); empty; empty]).
      set (locals1 := [ext (treeh witness); ext coupon; empty]).
      set (locals2 := [ext (treeh witness); ext coupon; ext assertion]).
      assert (Functions' : s_funcs attached = s_funcs G.E.allocated_store).
      { rewrite (G.E.H.tree_attachment_result_functions _ _ _ _ _ Attached); exact Functions. }
      assert (Coupon : stab_elem attached G.E.collector_instance 3%N 0%N =
        Some (G.E.H.Tables.externref coupon)).
      { eapply G.E.H.tree_attachment_result_elements with (offset := 0) in Attached;
          [exact Attached|rewrite Witness; reflexivity]. }
      assert (Assertion : stab_elem attached G.E.collector_instance 3%N 1%N =
        Some (G.E.H.Tables.externref assertion)).
      { eapply G.E.H.tree_attachment_result_elements with (offset := 1) in Attached;
          [exact Attached|rewrite Witness; reflexivity]. }
      rewrite evaluation_claim_body; cbn [firstn].
      execute (local_get_run state store locals0 0%N (ext (treeh witness)) eq_refl).
      execute (arity_run state store locals0 witness 2 Functions ltac:(rewrite Witness; reflexivity)
        ltac:(vm_compute; congruence)).
      execute (local_get_run state store locals0 0%N (ext (treeh witness)) eq_refl).
      execute (attach_tree_run state store locals0 witness 3 attached Functions Attached).
      execute (table_get_run state attached locals0 3%N (wasm_nat 0) _ Coupon).
      execute (local_set_run state attached locals0 1%N (ext coupon) ltac:(cbn; lia)).
      execute (table_get_run state attached locals1 3%N (wasm_nat 1) _ Assertion).
      execute (local_set_run state attached locals1 2%N (ext assertion) ltac:(cbn; lia)).
      execute (local_get_run state attached locals2 1%N (ext coupon) eq_refl).
      exact (tag_run state attached locals2 coupon tagged Functions' Tagged).
    Qed.

    Lemma collector_claim_body_executes state store first second witness tag claim authors reference
        reference_bytes claim_bytes equal :
      s_funcs store = s_funcs G.E.allocated_store ->
      get_tree_raw witness = [treeh tag; treeh claim] ->
      get_tree_raw tag = [authors; reference] -> tree_tag tag = true ->
      List.length (get_tree_raw claim) = 3 ->
      G.E.H.tree_attachment_result G.E.collector_instance store
        [ext (treeh witness); word 3] = Some (first, result_values []) ->
      G.E.H.tree_attachment_result G.E.collector_instance first
        [ext (treeh tag); word 6] = Some (second, result_values []) ->
      handle_bytes state reference = Some reference_bytes ->
      handle_bytes (Datatypes.S state) (Data (Ref (TreeRef claim))) = Some claim_bytes ->
      G.E.H.Tables.T.O.compare_handles (blobh reference_bytes) (blobh claim_bytes) = Some equal ->
      reduce_trans
        (state, store, Build_frame [ext (treeh witness); VAL_ref (VAL_ref_null T_externref);
          VAL_ref (VAL_ref_null T_externref)] G.E.collector_instance,
          to_e_list (modfunc_body evaluation_claim))
        (Datatypes.S (Datatypes.S state), second,
          Build_frame [ext (treeh witness); ext (treeh tag); ext (treeh claim)] G.E.collector_instance,
          if equal then v_to_e_list [ext (treeh claim)] else [AI_trap]).
    Proof.
      intros Functions Witness Fields Tagged Length First Second Reference ClaimBytes Compared.
      set (locals2 := [ext (treeh witness); ext (treeh tag); ext (treeh claim)]).
      assert (Functions1 : s_funcs first = s_funcs G.E.allocated_store).
      { rewrite (G.E.H.tree_attachment_result_functions _ _ _ _ _ First); exact Functions. }
      assert (Functions2 : s_funcs second = s_funcs G.E.allocated_store).
      { rewrite (G.E.H.tree_attachment_result_functions _ _ _ _ _ Second); exact Functions1. }
      assert (ReferenceChild : stab_elem second G.E.collector_instance 6%N 1%N =
        Some (G.E.H.Tables.externref reference)).
      { eapply G.E.H.tree_attachment_result_elements with (offset := 1) in Second;
          [exact Second|rewrite Fields; reflexivity]. }
      pose proof (collector_claim_prefix_executes state store first witness (treeh tag) (treeh claim) true
        Functions Witness Tagged First) as Prefix.
      rewrite evaluation_claim_body in Prefix; cbn [firstn] in Prefix.
      rewrite evaluation_claim_body; execute Prefix.
      execute (local_get_run state first locals2 1%N (ext (treeh tag)) eq_refl).
      execute (arity_run state first locals2 tag 2 Functions1 ltac:(rewrite Fields; reflexivity)
        ltac:(vm_compute; congruence)).
      execute (local_get_run state first locals2 1%N (ext (treeh tag)) eq_refl).
      execute (attach_tree_run state first locals2 tag 6 second Functions1 Second).
      execute (local_get_run state second locals2 2%N (ext (treeh claim)) eq_refl).
      execute (arity_run state second locals2 claim 3 Functions2 Length ltac:(vm_compute; congruence)).
      execute (table_get_run state second locals2 6%N (wasm_nat 1) _ ReferenceChild).
      execute_under [ext reference] (local_get_run state second locals2 2%N (ext (treeh claim)) eq_refl).
      execute (G.collector_object_executes combination handle_bytes state second locals2 reference (TreeObj claim)
        reference_bytes claim_bytes equal Functions2 Reference ClaimBytes Compared).
      destruct equal.
      - execute (local_get_run (Datatypes.S (Datatypes.S state)) second locals2 2%N (ext (treeh claim)) eq_refl).
        apply rt_refl.
      - apply rt_step, r_simple; eapply rs_trap with
          (lh := LH_base [] [AI_basic (BI_local_get 2%N)]); [discriminate|reflexivity].
    Qed.

    Lemma collector_claim_executes state store first second locals witness tag claim authors reference
        reference_bytes claim_bytes equal :
      s_funcs store = s_funcs G.E.allocated_store ->
      get_tree_raw witness = [treeh tag; treeh claim] ->
      get_tree_raw tag = [authors; reference] -> tree_tag tag = true ->
      List.length (get_tree_raw claim) = 3 ->
      G.E.H.tree_attachment_result G.E.collector_instance store
        [ext (treeh witness); word 3] = Some (first, result_values []) ->
      G.E.H.tree_attachment_result G.E.collector_instance first
        [ext (treeh tag); word 6] = Some (second, result_values []) ->
      handle_bytes state reference = Some reference_bytes ->
      handle_bytes (Datatypes.S state) (Data (Ref (TreeRef claim))) = Some claim_bytes ->
      G.E.H.Tables.T.O.compare_handles (blobh reference_bytes) (blobh claim_bytes) = Some equal ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [ext (treeh witness)] ++ [AI_basic (BI_call 29%N)])
        (Datatypes.S (Datatypes.S state), second, Build_frame locals G.E.collector_instance,
          if equal then v_to_e_list [ext (treeh claim)] else [AI_trap]).
    Proof.
      intros Functions Witness Fields Tagged Length First Second Reference ClaimBytes Compared.
      eapply G.E.collector_call with (code := evaluation_claim)
        (parameters := [T_ref T_externref]) (results := [T_ref T_externref])
        (defaults := [VAL_ref (VAL_ref_null T_externref); VAL_ref (VAL_ref_null T_externref)])
        (index := 29%N) (address := 29%N) (arguments := [ext (treeh witness)])
        (values := [ext (treeh claim)]) (accepted := equal)
        (final_frame := Build_frame [ext (treeh witness); ext (treeh tag); ext (treeh claim)]
          G.E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - eapply collector_claim_body_executes; eassumption.
    Qed.

    Lemma collector_claim_rejects_container state store locals witness :
      s_funcs store = s_funcs G.E.allocated_store ->
      (forall tree, witness = treeh tree ->
        (Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 64)%Z /\ List.length (get_tree_raw tree) <> 2) ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [ext witness] ++ [AI_basic (BI_call 29%N)])
        (state, store, Build_frame locals G.E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Invalid.
      set (defaults := [VAL_ref (VAL_ref_null T_externref); VAL_ref (VAL_ref_null T_externref)]).
      set (inside := ext witness :: defaults).
      eapply G.E.collector_call with (code := evaluation_claim)
        (parameters := [T_ref T_externref]) (results := [T_ref T_externref]) (defaults := defaults)
        (index := 29%N) (address := 29%N) (arguments := [ext witness]) (values := [ext witness])
        (accepted := false) (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - rewrite evaluation_claim_body.
        execute (local_get_run state store inside 0%N (ext witness) eq_refl).
        eapply G.E.collector_trap_suffix with
          (instructions := v_to_e_list [ext witness; word 2] ++ [AI_basic (BI_call 25%N)]).
        apply G.collector_arity_rejects_shape with (expected := 2);
          [exact Functions|vm_compute; congruence|exact Invalid].
    Qed.

    Lemma collector_claim_rejects_untagged state store first locals witness coupon assertion :
      s_funcs store = s_funcs G.E.allocated_store ->
      get_tree_raw witness = [coupon; assertion] -> G.E.H.W.Api.is_tag coupon = false ->
      G.E.H.tree_attachment_result G.E.collector_instance store
        [ext (treeh witness); word 3] = Some (first, result_values []) ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [ext (treeh witness)] ++ [AI_basic (BI_call 29%N)])
        (state, first, Build_frame locals G.E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Witness Untagged First.
      set (empty := VAL_ref (VAL_ref_null T_externref)).
      set (locals2 := [ext (treeh witness); ext coupon; ext assertion]).
      eapply G.E.collector_call with (code := evaluation_claim)
        (parameters := [T_ref T_externref]) (results := [T_ref T_externref]) (defaults := [empty; empty])
        (index := 29%N) (address := 29%N) (arguments := [ext (treeh witness)])
        (values := [ext assertion]) (accepted := false)
        (final_frame := Build_frame locals2 G.E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - eapply G.E.collector_trap_suffix with
          (instructions := to_e_list (firstn 15 (modfunc_body evaluation_claim)))
          (suffix := to_e_list (skipn 15 (modfunc_body evaluation_claim))).
        exact (collector_claim_prefix_executes state store first witness coupon assertion false
          Functions Witness Untagged First).
    Qed.

    Lemma collector_claim_rejects_tag_shape state store first locals witness tag assertion :
      s_funcs store = s_funcs G.E.allocated_store ->
      get_tree_raw witness = [treeh tag; assertion] -> tree_tag tag = true ->
      G.E.H.tree_attachment_result G.E.collector_instance store
        [ext (treeh witness); word 3] = Some (first, result_values []) ->
      (Z.of_nat (List.length (get_tree_raw tag)) < 2 ^ 64)%Z ->
      List.length (get_tree_raw tag) <> 2 ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [ext (treeh witness)] ++ [AI_basic (BI_call 29%N)])
        (state, first, Build_frame locals G.E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Witness Tagged First Bound Different.
      set (empty := VAL_ref (VAL_ref_null T_externref)).
      set (inside := [ext (treeh witness); ext (treeh tag); ext assertion]).
      assert (Functions1 : s_funcs first = s_funcs G.E.allocated_store).
      { rewrite (G.E.H.tree_attachment_result_functions _ _ _ _ _ First); exact Functions. }
      eapply G.E.collector_call with (code := evaluation_claim)
        (parameters := [T_ref T_externref]) (results := [T_ref T_externref]) (defaults := [empty; empty])
        (index := 29%N) (address := 29%N) (arguments := [ext (treeh witness)])
        (values := [ext assertion]) (accepted := false)
        (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - pose proof (collector_claim_prefix_executes state store first witness (treeh tag) assertion true
          Functions Witness Tagged First) as Prefix.
        rewrite evaluation_claim_body in Prefix; cbn [firstn] in Prefix.
        rewrite evaluation_claim_body; execute Prefix.
        execute (local_get_run state first inside 1%N (ext (treeh tag)) eq_refl).
        eapply G.E.collector_trap_suffix with
          (instructions := v_to_e_list [ext (treeh tag); word 2] ++ [AI_basic (BI_call 25%N)]).
        apply G.collector_arity_rejects_shape with (expected := 2);
          [exact Functions1|vm_compute; congruence|].
        intros other Same; inversion Same; subst other; auto.
    Qed.

    Lemma collector_claim_rejects_assertion_shape state store first second locals witness tag assertion :
      s_funcs store = s_funcs G.E.allocated_store ->
      get_tree_raw witness = [treeh tag; assertion] -> tree_tag tag = true ->
      List.length (get_tree_raw tag) = 2 ->
      G.E.H.tree_attachment_result G.E.collector_instance store
        [ext (treeh witness); word 3] = Some (first, result_values []) ->
      G.E.H.tree_attachment_result G.E.collector_instance first
        [ext (treeh tag); word 6] = Some (second, result_values []) ->
      (forall claim, assertion = treeh claim ->
        (Z.of_nat (List.length (get_tree_raw claim)) < 2 ^ 64)%Z /\ List.length (get_tree_raw claim) <> 3) ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [ext (treeh witness)] ++ [AI_basic (BI_call 29%N)])
        (state, second, Build_frame locals G.E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Witness Tagged Arity First Second Invalid.
      set (empty := VAL_ref (VAL_ref_null T_externref)).
      set (inside := [ext (treeh witness); ext (treeh tag); ext assertion]).
      assert (Functions1 : s_funcs first = s_funcs G.E.allocated_store).
      { rewrite (G.E.H.tree_attachment_result_functions _ _ _ _ _ First); exact Functions. }
      assert (Functions2 : s_funcs second = s_funcs G.E.allocated_store).
      { rewrite (G.E.H.tree_attachment_result_functions _ _ _ _ _ Second); exact Functions1. }
      eapply G.E.collector_call with (code := evaluation_claim)
        (parameters := [T_ref T_externref]) (results := [T_ref T_externref]) (defaults := [empty; empty])
        (index := 29%N) (address := 29%N) (arguments := [ext (treeh witness)])
        (values := [ext assertion]) (accepted := false)
        (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - pose proof (collector_claim_prefix_executes state store first witness (treeh tag) assertion true
          Functions Witness Tagged First) as Prefix.
        rewrite evaluation_claim_body in Prefix; cbn [firstn] in Prefix.
        rewrite evaluation_claim_body; execute Prefix.
        execute (local_get_run state first inside 1%N (ext (treeh tag)) eq_refl).
        execute (arity_run state first inside tag 2 Functions1 Arity ltac:(vm_compute; congruence)).
        execute (local_get_run state first inside 1%N (ext (treeh tag)) eq_refl).
        execute (attach_tree_run state first inside tag 6 second Functions1 Second).
        execute (local_get_run state second inside 2%N (ext assertion) eq_refl).
        eapply G.E.collector_trap_suffix with
          (instructions := v_to_e_list [ext assertion; word 3] ++ [AI_basic (BI_call 25%N)]).
        apply G.collector_arity_rejects_shape with (expected := 3);
          [exact Functions2|vm_compute; congruence|exact Invalid].
    Qed.

    Lemma collector_premise_input_executes state store attached witness claim opcode
        claimed_lhs claimed_rhs lhs rhs kind :
      s_funcs store = s_funcs G.E.allocated_store ->
      supplied_claim witness = treeh claim ->
      get_tree_raw claim = [opcode; claimed_lhs; claimed_rhs] ->
      G.E.H.tree_attachment_result G.E.collector_instance store
        [ext (treeh claim); word 4] = Some (attached, result_values []) ->
      let arguments := [VAL_num (VAL_int64 (wasm_nat64 kind));
        ext (supplied_claim witness); ext lhs; ext rhs] in
      reduce_trans
        (state, store, Build_frame arguments G.E.collector_instance,
          to_e_list (firstn 5 (modfunc_body evaluation_premise)))
        (state, attached, Build_frame arguments G.E.collector_instance, v_to_e_list [ext opcode]).
    Proof.
      intros Functions Supplied Fields Attached arguments.
      assert (Code : stab_elem attached G.E.collector_instance 4%N 0%N =
        Some (G.E.H.Tables.externref opcode)).
      { eapply G.E.H.tree_attachment_result_elements with (offset := 0) in Attached;
          [exact Attached|rewrite Fields; reflexivity]. }
      rewrite evaluation_premise_header; cbn [firstn].
      execute (local_get_run state store arguments 1%N (ext (treeh claim)) ltac:(unfold arguments; rewrite Supplied; reflexivity)).
      execute (attach_tree_run state store arguments claim 4 attached Functions Attached).
      exact (table_get_run state attached arguments 4%N (wasm_nat 0) _ Code).
    Qed.

    Lemma collector_premise_header_executes state store attached copied witness claim code
        claimed_lhs claimed_rhs lhs rhs n page left_equal right_equal :
      s_funcs store = s_funcs G.E.allocated_store ->
      supplied_claim witness = treeh claim ->
      get_tree_raw claim = [blobh code; claimed_lhs; claimed_rhs] ->
      G.E.H.tree_attachment_result G.E.collector_instance store
        [ext (treeh claim); word 4] = Some (attached, result_values []) ->
      G.E.H.blob_read_result G.E.collector_instance attached
        [ext (blobh code); word 0; word 0; word 8] = Some (copied, result_values [word 8]) ->
      s_mems copied = [page] -> load page 0%N 0%N 8%N = Some (get_blob_data code) ->
      decode_u64 (get_blob_data code) = Some n ->
      observed_comparison state claimed_lhs lhs left_equal ->
      (if left_equal then observed_comparison (state + 2) claimed_rhs rhs right_equal else True) ->
      let arguments := [VAL_num (VAL_int64 (wasm_nat64 n));
        ext (supplied_claim witness); ext lhs; ext rhs] in
      reduce_trans
        (state, store, Build_frame arguments G.E.collector_instance,
          to_e_list (modfunc_body evaluation_premise))
        (state + (if left_equal then 4 else 2), copied,
          Build_frame arguments G.E.collector_instance,
          if andb left_equal right_equal then [] else [AI_trap]).
    Proof.
      intros Functions Supplied Fields Attached Read Memory Loaded Decoded Left Right arguments.
      assert (Functions1 : s_funcs attached = s_funcs G.E.allocated_store).
      { rewrite (G.E.H.tree_attachment_result_functions _ _ _ _ _ Attached); exact Functions. }
      assert (Functions2 : s_funcs copied = s_funcs G.E.allocated_store).
      { rewrite (G.E.H.blob_read_result_functions _ _ _ _ _ Read); exact Functions1. }
      assert (LeftEntry : stab_elem copied G.E.collector_instance 4%N 1%N =
        Some (G.E.H.Tables.externref claimed_lhs)).
      { unfold stab_elem; rewrite (G.E.H.blob_read_result_tables _ _ _ _ _ Read).
        eapply G.E.H.tree_attachment_result_elements with (offset := 1) in Attached;
          [exact Attached|rewrite Fields; reflexivity]. }
      assert (RightEntry : stab_elem copied G.E.collector_instance 4%N 2%N =
        Some (G.E.H.Tables.externref claimed_rhs)).
      { unfold stab_elem; rewrite (G.E.H.blob_read_result_tables _ _ _ _ _ Read).
        eapply G.E.H.tree_attachment_result_elements with (offset := 2) in Attached;
          [exact Attached|rewrite Fields; reflexivity]. }
      pose proof (collector_premise_input_executes state store attached witness claim (blobh code)
        claimed_lhs claimed_rhs lhs rhs n Functions Supplied Fields Attached) as Input.
      rewrite evaluation_premise_header in Input; cbn [firstn] in Input.
      rewrite evaluation_premise_header; execute Input.
      execute (collector_integer_executes state attached arguments code n copied page
        Functions1 Read Functions2 Memory Loaded Decoded).
      execute_under [VAL_num (VAL_int64 (wasm_nat64 n))]
        (local_get_run state copied arguments 0%N (VAL_num (VAL_int64 (wasm_nat64 n))) eq_refl).
      assert (Equal : reduce_trans
        (state, copied, Build_frame arguments G.E.collector_instance,
          [v_to_e (VAL_num (VAL_int64 (wasm_nat64 n)));
           v_to_e (VAL_num (VAL_int64 (wasm_nat64 n))); AI_basic (BI_relop T_i64 (Relop_i ROI_eq))])
        (state, copied, Build_frame arguments G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int32 (wasm_bool true))])).
      { assert (Same : app_relop (Relop_i ROI_eq)
          (VAL_int64 (wasm_nat64 n)) (VAL_int64 (wasm_nat64 n)) = true).
        { change (Wasm_int.Int64.eq (wasm_nat64 n) (wasm_nat64 n) = true).
          apply Wasm_int.Int64.eq_true. }
        rewrite <- Same; apply rt_step, r_simple; eapply rs_relop with
          (v1 := VAL_int64 (wasm_nat64 n)) (v2 := VAL_int64 (wasm_nat64 n))
          (t := T_i64) (op := Relop_i ROI_eq); reflexivity. }
      execute Equal.
      execute (G.E.collector_require_executes combination true true handle_bytes state copied arguments true
        ltac:(rewrite Functions2; reflexivity)).
      execute (table_get_run state copied arguments 4%N (wasm_nat 1) _ LeftEntry).
      execute_under [ext claimed_lhs] (local_get_run state copied arguments 2%N (ext lhs) eq_refl).
      execute (same_run state copied arguments claimed_lhs lhs left_equal Functions2 Left).
      replace (Datatypes.S (Datatypes.S state)) with (state + 2) by lia.
      destruct left_equal.
      - execute (table_get_run (state + 2) copied arguments 4%N (wasm_nat 2) _ RightEntry).
        execute_under [ext claimed_rhs] (local_get_run (state + 2) copied arguments 3%N (ext rhs) eq_refl).
        execute (same_run (state + 2) copied arguments claimed_rhs rhs right_equal Functions2 Right).
        replace (Datatypes.S (Datatypes.S (state + 2))) with (state + 4) by lia.
        destruct right_equal; apply rt_refl.
      - apply rt_step, r_simple; eapply rs_trap with (lh := LH_base [] (to_e_list
          [BI_const_num (VAL_int32 (wasm_nat 2)); BI_table_get 4%N; BI_local_get 3%N; BI_call 22%N]));
          [discriminate|reflexivity].
    Qed.

    Lemma collector_premise_executes state store attached copied locals witness claim code
        claimed_lhs claimed_rhs lhs rhs n page :
      s_funcs store = s_funcs G.E.allocated_store ->
      supplied_claim witness = treeh claim ->
      get_tree_raw claim = [blobh code; claimed_lhs; claimed_rhs] ->
      G.E.H.tree_attachment_result G.E.collector_instance store
        [ext (treeh claim); word 4] = Some (attached, result_values []) ->
      G.E.H.blob_read_result G.E.collector_instance attached
        [ext (blobh code); word 0; word 0; word 8] = Some (copied, result_values [word 8]) ->
      s_mems copied = [page] -> load page 0%N 0%N 8%N = Some (get_blob_data code) ->
      decode_u64 (get_blob_data code) = Some n ->
      observed_same state claimed_lhs lhs -> observed_same (state + 2) claimed_rhs rhs ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 n)); ext (supplied_claim witness); ext lhs; ext rhs] ++
            [AI_basic (BI_call 30%N)])
        (state + 4, copied, Build_frame locals G.E.collector_instance, []).
    Proof.
      intros Functions Supplied Fields Attached Read Memory Loaded Decoded Left Right.
      set (arguments := [VAL_num (VAL_int64 (wasm_nat64 n)); ext (supplied_claim witness); ext lhs; ext rhs]).
      eapply G.E.collector_call_void with (code := evaluation_premise)
        (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
        (defaults := []) (index := 30%N) (address := 30%N)
        (arguments := arguments) (accepted := true)
        (final_frame := Build_frame arguments G.E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - exact (collector_premise_header_executes state store attached copied witness claim code
          claimed_lhs claimed_rhs lhs rhs n page true true Functions Supplied Fields Attached Read
          Memory Loaded Decoded Left Right).
    Qed.

    Lemma collector_premise_rejects_endpoints state store attached copied locals witness claim code
        claimed_lhs claimed_rhs lhs rhs n page left_equal right_equal :
      s_funcs store = s_funcs G.E.allocated_store -> supplied_claim witness = treeh claim ->
      get_tree_raw claim = [blobh code; claimed_lhs; claimed_rhs] ->
      G.E.H.tree_attachment_result G.E.collector_instance store
        [ext (treeh claim); word 4] = Some (attached, result_values []) ->
      G.E.H.blob_read_result G.E.collector_instance attached
        [ext (blobh code); word 0; word 0; word 8] = Some (copied, result_values [word 8]) ->
      s_mems copied = [page] -> load page 0%N 0%N 8%N = Some (get_blob_data code) ->
      decode_u64 (get_blob_data code) = Some n ->
      observed_comparison state claimed_lhs lhs left_equal ->
      (if left_equal then observed_comparison (state + 2) claimed_rhs rhs right_equal else True) ->
      andb left_equal right_equal = false ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 n)); ext (supplied_claim witness); ext lhs; ext rhs] ++
            [AI_basic (BI_call 30%N)])
        (state + (if left_equal then 4 else 2), copied, Build_frame locals G.E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Supplied Fields Attached Read Memory Loaded Decoded Left Right Rejected.
      set (arguments := [VAL_num (VAL_int64 (wasm_nat64 n)); ext (supplied_claim witness); ext lhs; ext rhs]).
      eapply G.E.collector_call_void with (code := evaluation_premise)
        (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
        (defaults := []) (index := 30%N) (address := 30%N)
        (arguments := arguments) (accepted := false)
        (final_frame := Build_frame arguments G.E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - pose proof (collector_premise_header_executes state store attached copied witness claim code
          claimed_lhs claimed_rhs lhs rhs n page left_equal right_equal Functions Supplied Fields Attached Read
          Memory Loaded Decoded Left Right) as Header.
        rewrite Rejected in Header; exact Header.
    Qed.

    Lemma collector_premise_rejects_opcode_format state store attached locals witness claim opcode
        claimed_lhs claimed_rhs lhs rhs kind :
      s_funcs store = s_funcs G.E.allocated_store -> supplied_claim witness = treeh claim ->
      get_tree_raw claim = [opcode; claimed_lhs; claimed_rhs] ->
      G.E.H.tree_attachment_result G.E.collector_instance store
        [ext (treeh claim); word 4] = Some (attached, result_values []) ->
      (forall blob, opcode = blobh blob ->
        (Z.of_nat (List.length (get_blob_data blob)) < 2 ^ 64)%Z /\ List.length (get_blob_data blob) <> 8) ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 kind)); ext (supplied_claim witness); ext lhs; ext rhs] ++
            [AI_basic (BI_call 30%N)])
        (state, attached, Build_frame locals G.E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Supplied Fields Attached Invalid.
      set (arguments := [VAL_num (VAL_int64 (wasm_nat64 kind)); ext (supplied_claim witness); ext lhs; ext rhs]).
      assert (Functions1 : s_funcs attached = s_funcs G.E.allocated_store).
      { rewrite (G.E.H.tree_attachment_result_functions _ _ _ _ _ Attached); exact Functions. }
      eapply G.E.collector_call_void with (code := evaluation_premise)
        (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
        (defaults := []) (index := 30%N) (address := 30%N) (arguments := arguments)
        (accepted := false) (final_frame := Build_frame arguments G.E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - pose proof (collector_premise_input_executes state store attached witness claim opcode
          claimed_lhs claimed_rhs lhs rhs kind Functions Supplied Fields Attached) as Input.
        rewrite evaluation_premise_header in Input; cbn [firstn] in Input.
        rewrite evaluation_premise_header; execute Input.
        eapply G.E.collector_trap_suffix with
          (instructions := v_to_e_list [ext opcode] ++ [AI_basic (BI_call 26%N)]).
        destruct opcode as [[object|reference]|thunk|encoded].
        + destruct object as [blob|tree].
          * destruct (Invalid blob eq_refl) as [Bound Different].
            eapply collector_integer_rejects_width; eassumption.
          * apply collector_integer_rejects_type; [exact Functions1|intros blob Same; discriminate].
        + apply collector_integer_rejects_type; [exact Functions1|intros blob Same; discriminate].
        + apply collector_integer_rejects_type; [exact Functions1|intros blob Same; discriminate].
        + apply collector_integer_rejects_type; [exact Functions1|intros blob Same; discriminate].
    Qed.

    Lemma collector_premise_rejects_opcode_value state store attached copied locals witness claim code
        claimed_lhs claimed_rhs lhs rhs number kind page :
      s_funcs store = s_funcs G.E.allocated_store -> supplied_claim witness = treeh claim ->
      get_tree_raw claim = [blobh code; claimed_lhs; claimed_rhs] ->
      G.E.H.tree_attachment_result G.E.collector_instance store
        [ext (treeh claim); word 4] = Some (attached, result_values []) ->
      G.E.H.blob_read_result G.E.collector_instance attached
        [ext (blobh code); word 0; word 0; word 8] = Some (copied, result_values [word 8]) ->
      s_mems copied = [page] -> load page 0%N 0%N 8%N = Some (get_blob_data code) ->
      decode_u64 (get_blob_data code) = Some number ->
      (Z.of_nat kind < 2 ^ 64)%Z -> number <> kind ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 kind)); ext (supplied_claim witness); ext lhs; ext rhs] ++
            [AI_basic (BI_call 30%N)])
        (state, copied, Build_frame locals G.E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Supplied Fields Attached Read Memory Loaded Decoded KindBound Different.
      set (arguments := [VAL_num (VAL_int64 (wasm_nat64 kind)); ext (supplied_claim witness); ext lhs; ext rhs]).
      assert (Functions1 : s_funcs attached = s_funcs G.E.allocated_store).
      { rewrite (G.E.H.tree_attachment_result_functions _ _ _ _ _ Attached); exact Functions. }
      assert (Functions2 : s_funcs copied = s_funcs G.E.allocated_store).
      { rewrite (G.E.H.blob_read_result_functions _ _ _ _ _ Read); exact Functions1. }
      eapply G.E.collector_call_void with (code := evaluation_premise)
        (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
        (defaults := []) (index := 30%N) (address := 30%N) (arguments := arguments)
        (accepted := false) (final_frame := Build_frame arguments G.E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - pose proof (collector_premise_input_executes state store attached witness claim (blobh code)
          claimed_lhs claimed_rhs lhs rhs kind Functions Supplied Fields Attached) as Input.
        rewrite evaluation_premise_header in Input; cbn [firstn] in Input.
        rewrite evaluation_premise_header; execute Input.
        execute (collector_integer_executes state attached arguments code number copied page
          Functions1 Read Functions2 Memory Loaded Decoded).
        execute_under [VAL_num (VAL_int64 (wasm_nat64 number))]
          (local_get_run state copied arguments 0%N (VAL_num (VAL_int64 (wasm_nat64 kind))) eq_refl).
        assert (Mismatch : app_relop (Relop_i ROI_eq)
          (VAL_int64 (wasm_nat64 number)) (VAL_int64 (wasm_nat64 kind)) = false).
        { change (Wasm_int.Int64.eq (wasm_nat64 number) (wasm_nat64 kind) = false).
          apply Wasm_int.Int64.eq_false.
          intro Same; apply (f_equal Wasm_int.Int64.unsigned) in Same.
          rewrite (wasm_nat64_unsigned number (decode_u64_bound _ _ Decoded)),
            (wasm_nat64_unsigned kind KindBound) in Same; lia. }
        assert (Compared : reduce_trans
          (state, copied, Build_frame arguments G.E.collector_instance,
            [v_to_e (VAL_num (VAL_int64 (wasm_nat64 number)));
             v_to_e (VAL_num (VAL_int64 (wasm_nat64 kind))); AI_basic (BI_relop T_i64 (Relop_i ROI_eq))])
          (state, copied, Build_frame arguments G.E.collector_instance,
            v_to_e_list [VAL_num (VAL_int32 (wasm_bool false))])).
        { rewrite <- Mismatch; apply rt_step, r_simple; eapply rs_relop with
            (v1 := VAL_int64 (wasm_nat64 number)) (v2 := VAL_int64 (wasm_nat64 kind))
            (t := T_i64) (op := Relop_i ROI_eq); reflexivity. }
        execute Compared.
        eapply G.E.collector_trap_suffix with
          (instructions := [v_to_e (VAL_num (VAL_int32 (wasm_bool false))); AI_basic (BI_call 21%N)]).
        apply G.E.collector_require_executes with (condition := false); rewrite Functions2; reflexivity.
    Qed.

    Definition premise_ready state store witness kind lhs rhs state' store' :=
      exists claim code claimed_lhs claimed_rhs attached page,
        authenticated_claim witness claim /\
        s_funcs store = s_funcs G.E.allocated_store /\
        get_tree_raw claim = [blobh code; claimed_lhs; claimed_rhs] /\
        decode_u64 (get_blob_data code) = Some kind /\
        G.E.H.tree_attachment_result G.E.collector_instance store
          [ext (treeh claim); word 4] = Some (attached, result_values []) /\
        G.E.H.blob_read_result G.E.collector_instance attached
          [ext (blobh code); word 0; word 0; word 8] = Some (store', result_values [word 8]) /\
        s_mems store' = [page] /\ load page 0%N 0%N 8%N = Some (get_blob_data code) /\
        observed_same state claimed_lhs lhs /\ observed_same (state + 2) claimed_rhs rhs /\
        state' = state + 4.

    Lemma collector_ready_premise_executes state store witness kind lhs rhs state' store' locals :
      premise_ready state store witness kind lhs rhs state' store' ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 kind)); ext (supplied_claim witness); ext lhs; ext rhs] ++
            [AI_basic (BI_call 30%N)])
        (state', store', Build_frame locals G.E.collector_instance, []).
    Proof.
      intros [claim [code [claimed_lhs [claimed_rhs [attached [page Ready]]]]]].
      destruct Ready as [Authenticated [Functions [Fields [Decoded [Attached [Read [Memory [Loaded [Left [Right State]]]]]]]]]].
      subst state'; eapply collector_premise_executes; try eassumption.
      apply authenticated_claim_supplied; exact Authenticated.
    Qed.

    Lemma collector_ready_premise_decodes state store witness kind lhs rhs state' store' claim_kind :
      premise_ready state store witness kind lhs rhs state' store' ->
      Claims.Code.statement_kind kind = Some claim_kind ->
      exists authors, Claims.decode witness = Some (claim_kind, authors, lhs, rhs).
    Proof.
      intros [claim [code [claimed_lhs [claimed_rhs [attached [page Ready]]]]]] Kind.
      destruct Ready as [Authenticated [Functions [Fields [Decoded [Attached [Read [Memory [Loaded [Left [Right State]]]]]]]]]].
      destruct Authenticated as [bundle [tag [author [Witness [Bundle [Tag [Tagged Arity]]]]]]].
      apply observed_same_identifies in Left; apply observed_same_identifies in Right.
      subst witness claimed_lhs claimed_rhs; exists author.
      unfold Claims.decode; cbn [treeh]; rewrite Bundle.
      cbn -[Claims.API.handle_eqb to_nat Claims.Code.statement_kind]; rewrite Tagged, Tag.
      cbn -[Claims.API.handle_eqb to_nat Claims.Code.statement_kind].
      rewrite (proj2 (Claims.API.handle_eqb_spec _ _) eq_refl), Fields.
      change (to_nat (get_blob_data code) = Some kind) in Decoded.
      cbn -[to_nat Claims.Code.statement_kind]; rewrite Decoded.
      cbn -[Claims.Code.statement_kind]; rewrite Kind; reflexivity.
    Qed.

    Lemma collector_ready_premise_functions state store witness kind lhs rhs state' store' :
      premise_ready state store witness kind lhs rhs state' store' -> s_funcs store' = s_funcs store.
    Proof.
      intros [claim [code [claimed_lhs [claimed_rhs [attached [page Ready]]]]]].
      destruct Ready as [_ [_ [_ [_ [Attached [Read _]]]]]].
      rewrite (G.E.H.blob_read_result_functions _ _ _ _ _ Read).
      exact (G.E.H.tree_attachment_result_functions _ _ _ _ _ Attached).
    Qed.

    Lemma collector_ready_premise_other_table state store witness kind lhs rhs state' store' other address offset :
      premise_ready state store witness kind lhs rhs state' store' ->
      lookup_N (inst_tables G.E.collector_instance) other = Some address -> 4%N <> address ->
      stab_elem store' G.E.collector_instance other offset = stab_elem store G.E.collector_instance other offset.
    Proof.
      intros [claim [code [claimed_lhs [claimed_rhs [attached [page Ready]]]]]] Address Different.
      destruct Ready as [_ [_ [_ [_ [Attached [Read _]]]]]].
      unfold stab_elem at 1; rewrite (G.E.H.blob_read_result_tables _ _ _ _ _ Read).
      change (stab_elem attached G.E.collector_instance other offset =
        stab_elem store G.E.collector_instance other offset).
      eapply G.E.H.tree_attachment_result_other_table; [exact Attached|reflexivity|exact Address|exact Different].
    Qed.

    Lemma premise_ready_retained state store witness kind lhs rhs state' store' witnesses :
      premise_ready state store witness kind lhs rhs state' store' ->
      retained_claims store witnesses -> retained_claims store' witnesses.
    Proof.
      intros Ready Retained index original Entry.
      rewrite (collector_ready_premise_other_table _ _ _ _ _ _ _ _ 9%N 9%N _ Ready eq_refl ltac:(discriminate)).
      apply Retained; exact Entry.
    Qed.

    Lemma premise_ready_tree_view state store witness kind lhs rhs state' store' other address node :
      premise_ready state store witness kind lhs rhs state' store' ->
      lookup_N (inst_tables G.E.collector_instance) other = Some address -> 4%N <> address ->
      tree_view store other node -> tree_view store' other node.
    Proof.
      intros Ready Address Different View index value Entry.
      rewrite (collector_ready_premise_other_table _ _ _ _ _ _ _ _ _ _ _ Ready Address Different).
      apply View; exact Entry.
    Qed.

    Lemma premise_ready_typing state store witness kind lhs rhs state' store' :
      premise_ready state store witness kind lhs rhs state' store' ->
      store_typing store -> store_typing store'.
    Proof.
      intros [claim [code [claimed_lhs [claimed_rhs [attached [page Ready]]]]]] Typed.
      destruct Ready as [_ [_ [_ [_ [Attached [Read _]]]]]].
      apply (proj1 (proj2 (G.E.H.blob_read_result_correct _ _ _ _ _ Read))).
      apply (proj1 (proj2 (G.E.H.tree_attachment_result_correct _ _ _ _ _ Attached))).
      exact Typed.
    Qed.

    Definition claim_ready state witness claim :=
      exists bundle tag authors reference,
        witness = treeh bundle /\ get_tree_raw bundle = [treeh tag; treeh claim] /\
        get_tree_raw tag = [authors; reference] /\ tree_tag tag = true /\
        List.length (get_tree_raw claim) = 3 /\
        observed_same state reference (Data (Ref (TreeRef claim))).

    Lemma claim_ready_authenticates state witness claim :
      claim_ready state witness claim -> authenticated_claim witness claim.
    Proof.
      intros [bundle [tag [author [reference [Witness [Bundle [Tag [Tagged [Arity Same]]]]]]]]].
      apply observed_same_identifies in Same; subst reference.
      exists bundle, tag, author; repeat split; assumption.
    Qed.

    Fixpoint lengths_bounded depth value := match depth with
      | 0 => True
      | Datatypes.S remaining => match value with
        | Data (Object (BlobObj blob)) => (Z.of_nat (List.length (get_blob_data blob)) < 2 ^ 64)%Z
        | Data (Object (TreeObj t)) =>
          (Z.of_nat (List.length (get_tree_raw t)) < 2 ^ 64)%Z /\
          Forall (lengths_bounded remaining) (get_tree_raw t)
        | _ => True end
      end.

    (** A bounded-length property at one more level implies the property at the shallower
        level. *)
    Lemma lengths_bounded_weaken depth value :
      lengths_bounded (Datatypes.S depth) value -> lengths_bounded depth value.
    Proof.
      revert value; induction depth as [|depth IH]; intro value; [intros; exact I|].
      destruct value as [[[blob|tree]|reference]|thunk|encoded]; cbn [lengths_bounded]; try tauto.
      intros [Length Children]; split; [exact Length|].
      eapply Forall_impl; [|exact Children]; intros child Bound; apply IH; exact Bound.
    Qed.

    (** A child of a bounded Tree satisfies the bound with one less level of depth. *)
    Lemma lengths_bounded_child depth tree child :
      lengths_bounded (Datatypes.S depth) (treeh tree) ->
      In child (get_tree_raw tree) -> lengths_bounded depth child.
    Proof.
      intros [_ Children] Member; rewrite Forall_forall in Children; apply Children; exact Member.
    Qed.

    Lemma collector_claim_execution state store witness :
      s_funcs store = s_funcs G.E.allocated_store ->
      claim_views_ready store -> lengths_bounded 2 witness ->
      (forall index value, exists bytes, handle_bytes index value = Some bytes) ->
      exists state' store' returned,
        (forall locals, reduce_trans
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [ext witness] ++ [AI_basic (BI_call 29%N)])
          (state', store', Build_frame locals G.E.collector_instance,
            match returned with Some claim => v_to_e_list [ext (treeh claim)] | None => [AI_trap] end)) /\
        (forall claim, returned = Some claim -> state' = state + 2 /\
          claim_ready state witness claim /\ claim_store_transition store witness claim store').
    Proof.
      intros Functions Views Bounds Bytes.
      destruct witness as [[[blob|bundle]|reference]|thunk|encoded];
        try solve [exists state, store, None; split;
          [intro locals; apply collector_claim_rejects_container; [exact Functions|intros tree Same; discriminate]
          |intros claim Returned; discriminate]].
      pose proof (proj1 Bounds) as RootLength.
      destruct (get_tree_raw bundle) as [|coupon [|assertion [|extra rest]]] eqn:Envelope;
        try solve [exists state, store, None; split;
          [intro locals; apply collector_claim_rejects_container; [exact Functions|];
            intros tree Same; inversion Same; subst tree; split;
              [rewrite Envelope; exact RootLength|rewrite Envelope; cbn; lia]
          |intros claim Returned; discriminate]].
      destruct (Views bundle ltac:(rewrite Envelope; reflexivity)) as [first [First NextView]].
      pose proof (lengths_bounded_child 1 bundle coupon Bounds ltac:(rewrite Envelope; cbn; auto)) as CouponBound.
      pose proof (lengths_bounded_child 1 bundle assertion Bounds ltac:(rewrite Envelope; cbn; auto)) as AssertionBound.
      destruct coupon as [[[blob|tag]|reference]|thunk|encoded];
        try solve [exists state, first, None; split;
          [intro locals; eapply collector_claim_rejects_untagged; [exact Functions|exact Envelope|reflexivity|exact First]
          |intros claim Returned; discriminate]].
      destruct (tree_tag tag) eqn:Tagged; [|exists state, first, None; split;
        [intro locals; eapply collector_claim_rejects_untagged; [exact Functions|exact Envelope|exact Tagged|exact First]
        |intros claim Returned; discriminate]].
      destruct (get_tree_raw tag) as [|authors [|reference [|extra rest]]] eqn:Fields;
        try solve [exists state, first, None; split;
          [intro locals; eapply collector_claim_rejects_tag_shape; try eassumption;
            [exact (proj1 CouponBound)|rewrite Fields; cbn; lia]
          |intros claim Returned; discriminate]].
      assert (Arity : List.length (get_tree_raw tag) = 2) by (rewrite Fields; reflexivity).
      destruct (NextView tag Arity) as [second Second].
      destruct assertion as [[[blob|claim]|ref]|thunk|encoded];
        try solve [exists state, second, None; split;
          [intro locals; eapply collector_claim_rejects_assertion_shape; try eassumption;
            intros tree Same; discriminate
          |intros claim Returned; discriminate]].
      destruct (Nat.eq_dec (List.length (get_tree_raw claim)) 3) as [ClaimArity|Invalid];
        [|exists state, second, None; split;
          [intro locals; eapply collector_claim_rejects_assertion_shape; try eassumption;
            intros tree Same; inversion Same; subst tree; split; [exact (proj1 AssertionBound)|exact Invalid]
          |intros actual Returned; discriminate]].
      destruct (observed_comparison_total state reference (Data (Ref (TreeRef claim))) Bytes) as [equal Compared].
      exists (state + 2), second, (if equal then Some claim else None); split.
      - intro locals; destruct Compared as [reference_bytes [claim_bytes [Reference [ClaimBytes Equal]]]].
        replace (state + 2) with (Datatypes.S (Datatypes.S state)) by lia.
        destruct equal;
          [eapply collector_claim_executes with (equal := true)
          |eapply collector_claim_executes with (equal := false)]; eassumption.
      - destruct equal; intros actual Returned; [inversion Returned; subst actual|discriminate].
        split; [reflexivity|]; split.
        + exists bundle, tag, authors, reference; repeat split; assumption || reflexivity.
        + exists bundle, tag, first; repeat split; assumption || reflexivity.
    Qed.

    Definition integer_reads_ready store := forall code,
      List.length (get_blob_data code) = 8 ->
      exists copied page,
        G.E.H.blob_read_result G.E.collector_instance store
          [ext (blobh code); word 0; word 0; word 8] = Some (copied, result_values [word 8]) /\
        s_mems copied = [page] /\ load page 0%N 0%N 8%N = Some (get_blob_data code).

    Lemma memory_capacity_integer_reads store :
      store_typing store -> memory_capacity store -> integer_reads_ready store.
    Proof.
      intros Typed [before [Memory [TypeOf Length]]] code Width.
      assert (CopyBound : (0 + N.of_nat (List.length (get_blob_data code)) <= mem_length before)%N).
      { rewrite Width, Length; vm_compute; congruence. }
      destruct (blob_copy_exists before 0%N (get_blob_data code) CopyBound) as [after Copied].
      exists (upd_s_mem store [after]), after; split.
      - eapply G.E.H.blob_read_zero_bounded; [reflexivity|exact Memory|exact Typed|exact Width|exact Copied].
      - split; [reflexivity|].
        pose proof (blob_copy_read _ _ _ _ Copied) as Read.
        destruct (blob_copy_shape _ _ _ _ Copied) as [_ AfterLength].
        rewrite Length in AfterLength; unfold load; rewrite AfterLength; cbn.
        replace 8 with (List.length (get_blob_data code)) by exact Width.
        exact Read.
    Qed.

    Definition premise_views_ready store := forall claim,
      List.length (get_tree_raw claim) = 3 ->
      exists attached,
        G.E.H.tree_attachment_result G.E.collector_instance store
          [ext (treeh claim); word 4] = Some (attached, result_values []) /\
        integer_reads_ready attached.

    Lemma collector_premise_execution state store witness kind claim_kind lhs rhs :
      s_funcs store = s_funcs G.E.allocated_store -> lengths_bounded 3 witness ->
      (forall index value, exists bytes, handle_bytes index value = Some bytes) ->
      (exists claim, authenticated_claim witness claim) -> premise_views_ready store ->
      (Z.of_nat kind < 2 ^ 64)%Z -> Claims.Code.statement_kind kind = Some claim_kind ->
      exists state' store' (accepted : bool),
        (forall locals, reduce_trans
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 kind)); ext (supplied_claim witness); ext lhs; ext rhs] ++
              [AI_basic (BI_call 30%N)])
          (state', store', Build_frame locals G.E.collector_instance, if accepted then [] else [AI_trap])) /\
        (accepted = true -> premise_ready state store witness kind lhs rhs state' store').
    Proof.
      intros Functions Bounds Bytes [claim Authenticated] Views KindBound Kind.
      pose proof (authenticated_claim_supplied _ _ Authenticated) as Supplied.
      destruct Authenticated as [bundle [tag [author [Witness [Envelope [Tag [Tagged ClaimArity]]]]]]].
      assert (Authentication : authenticated_claim witness claim).
      { exists bundle, tag, author; repeat split; assumption. }
      destruct (Views claim ClaimArity) as [attached [Attached Reads]].
      destruct (get_tree_raw claim) as [|opcode [|claimed_lhs [|claimed_rhs tail]]] eqn:Assertion;
        cbn in ClaimArity; try discriminate.
      destruct tail; cbn in ClaimArity; [|discriminate].
      destruct opcode as [[[code|tree]|reference]|thunk|encoded];
        try solve [exists state, attached, false; split;
          [intro locals; eapply collector_premise_rejects_opcode_format; try eassumption; intros blob Same; discriminate
          |intro Accepted; discriminate]].
      assert (CodeLength : (Z.of_nat (List.length (get_blob_data code)) < 2 ^ 64)%Z).
      { subst witness; exact (lengths_bounded_child 1 claim (blobh code)
          (lengths_bounded_child 2 bundle (treeh claim) Bounds ltac:(rewrite Envelope; cbn; auto))
          ltac:(rewrite Assertion; cbn; auto)). }
      destruct (Nat.eq_dec (List.length (get_blob_data code)) 8) as [Width|Invalid].
      2: { exists state, attached, false; split.
        - intro locals; eapply collector_premise_rejects_opcode_format; try eassumption.
          intros blob Same; inversion Same; subst blob; auto.
        - intro Accepted; discriminate. }
      assert (Decoded : exists number, decode_u64 (get_blob_data code) = Some number).
      { change (exists number,
          (if Nat.eqb (List.length (get_blob_data code)) 8
           then Some (Z.to_nat (Memdata.int_of_bytes (get_blob_data code))) else None) = Some number).
        rewrite Width; eexists; reflexivity. }
      destruct Decoded as [number Decoded].
      destruct (Reads code Width) as [copied [page [Read [Memory Loaded]]]].
      destruct (Nat.eq_dec number kind) as [Same|Different].
      2: { exists state, copied, false; split.
        - intro locals; eapply collector_premise_rejects_opcode_value; eassumption.
        - intro Accepted; discriminate. }
      subst number.
      destruct (observed_comparison_total state claimed_lhs lhs Bytes) as [left_equal Left].
      destruct left_equal.
      2: { exists (state + 2), copied, false; split.
        - intro locals; eapply collector_premise_rejects_endpoints with (left_equal := false) (right_equal := false);
            try eassumption; reflexivity || exact I.
        - intro Accepted; discriminate. }
      destruct (observed_comparison_total (state + 2) claimed_rhs rhs Bytes) as [right_equal Right].
      destruct right_equal.
      2: { exists (state + 4), copied, false; split.
        - intro locals; eapply collector_premise_rejects_endpoints with (left_equal := true) (right_equal := false);
            try eassumption; reflexivity.
        - intro Accepted; discriminate. }
      exists (state + 4), copied, true; split.
      - intro locals; eapply collector_premise_executes; eassumption.
      - intro Accepted; exists claim, code, claimed_lhs, claimed_rhs, attached, page;
          repeat split; assumption || reflexivity.
    Qed.

  End Execution.
End WasmWitnesses.
