(** Proves native witness authentication, premise checking, and author
    accumulation under explicit comparison and resource conditions. *)

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
  Local Notation local_get_run := (G.E.collector_local_get combination true handle_bytes).

  Section Execution.
    (** The claim helper's prefix extracts the supplied Tag and assertion from a correctly
        shaped witness. *)
    Lemma collector_claim_prefix_executes state store witness coupon assertion tagged :
      s_funcs store = s_funcs G.E.allocated_store ->
      get_tree_raw witness = [coupon; assertion] -> G.E.H.W.Api.is_tag coupon = tagged ->
      let empty := VAL_ref (VAL_ref_null T_externref) in
      reduce_trans
        (state, store, Build_frame [ext (treeh witness); empty; empty] G.E.collector_instance,
          to_e_list (firstn 14 (modfunc_body evaluation_claim)))
        (state, store, Build_frame [ext (treeh witness); ext coupon; ext assertion] G.E.collector_instance,
          if tagged then [] else [AI_trap]).
    Proof.
      intros Functions Witness Tagged empty.
      set (locals0 := [ext (treeh witness); empty; empty]).
      set (locals1 := [ext (treeh witness); ext coupon; empty]).
      set (locals2 := [ext (treeh witness); ext coupon; ext assertion]).
      assert (Length : List.length (get_tree_raw witness) = 2) by now rewrite Witness.
      assert (Coupon : nth_error (get_tree_raw witness) 0 = Some coupon) by now rewrite Witness.
      assert (Assertion : nth_error (get_tree_raw witness) 1 = Some assertion) by now rewrite Witness.
      rewrite evaluation_claim_body; cbn [firstn].
      execute (local_get_run state store locals0 0%N (ext (treeh witness)) eq_refl).
      execute (arity_run state store locals0 witness 2 Functions Length ltac:(vm_compute; congruence)).
      execute (local_get_run state store locals0 0%N (ext (treeh witness)) eq_refl).
      execute (child_run state store locals0 witness 0 coupon Functions ltac:(vm_compute; congruence) Coupon).
      execute (local_set_run state store locals0 1%N (ext coupon) ltac:(cbn; lia)).
      execute (local_get_run state store locals1 0%N (ext (treeh witness)) eq_refl).
      execute (child_run state store locals1 witness 1 assertion Functions ltac:(vm_compute; congruence) Assertion).
      execute (local_set_run state store locals1 2%N (ext assertion) ltac:(cbn; lia)).
      execute (local_get_run state store locals2 1%N (ext coupon) eq_refl).
      exact (tag_run state store locals2 coupon tagged Functions Tagged).
    Qed.

    (** The claim helper body authenticates a supplied assertion against the Tag's Ref
        under the stated comparison readiness conditions. *)
    Lemma collector_claim_body_executes state store witness tag claim authors reference
        reference_bytes claim_bytes equal :
      s_funcs store = s_funcs G.E.allocated_store ->
      get_tree_raw witness = [treeh tag; treeh claim] ->
      get_tree_raw tag = [authors; reference] -> tree_tag tag = true ->
      List.length (get_tree_raw claim) = 3 ->
      handle_bytes state reference = Some reference_bytes ->
      handle_bytes (Datatypes.S state) (Data (Ref (TreeRef claim))) = Some claim_bytes ->
      G.E.H.Tables.T.O.compare_handles (blobh reference_bytes) (blobh claim_bytes) = Some equal ->
      reduce_trans
        (state, store, Build_frame [ext (treeh witness); VAL_ref (VAL_ref_null T_externref);
          VAL_ref (VAL_ref_null T_externref)] G.E.collector_instance,
          to_e_list (modfunc_body evaluation_claim))
        (Datatypes.S (Datatypes.S state), store,
          Build_frame [ext (treeh witness); ext (treeh tag); ext (treeh claim)] G.E.collector_instance,
          if equal then v_to_e_list [ext (treeh claim)] else [AI_trap]).
    Proof.
      intros Functions Witness Fields Tagged Length Reference ClaimBytes Compared.
      assert (TagLength : List.length (get_tree_raw tag) = 2) by now rewrite Fields.
      assert (ReferenceChild : nth_error (get_tree_raw tag) 1 = Some reference) by now rewrite Fields.
      set (locals2 := [ext (treeh witness); ext (treeh tag); ext (treeh claim)]).
      pose proof (collector_claim_prefix_executes state store witness (treeh tag) (treeh claim) true
        Functions Witness Tagged) as Prefix.
      rewrite evaluation_claim_body in Prefix; cbn [firstn] in Prefix.
      rewrite evaluation_claim_body; execute Prefix.
      execute (local_get_run state store locals2 1%N (ext (treeh tag)) eq_refl).
      execute (arity_run state store locals2 tag 2 Functions TagLength ltac:(vm_compute; congruence)).
      execute (local_get_run state store locals2 2%N (ext (treeh claim)) eq_refl).
      execute (arity_run state store locals2 claim 3 Functions Length ltac:(vm_compute; congruence)).
      execute (local_get_run state store locals2 1%N (ext (treeh tag)) eq_refl).
      execute (child_run state store locals2 tag 1 reference Functions ltac:(vm_compute; congruence) ReferenceChild).
      execute_under [ext reference] (local_get_run state store locals2 2%N (ext (treeh claim)) eq_refl).
      execute (G.collector_object_executes combination handle_bytes state store locals2 reference (TreeObj claim)
        reference_bytes claim_bytes equal Functions Reference ClaimBytes Compared).
      destruct equal.
      - execute (local_get_run (Datatypes.S (Datatypes.S state)) store locals2 2%N (ext (treeh claim)) eq_refl).
        apply rt_refl.
      - apply rt_step, r_simple; eapply rs_trap with
          (lh := LH_base [] [AI_basic (BI_local_get 2%N)]); [discriminate|reflexivity].
    Qed.
    (** A ready witness executes the claim helper and returns its authenticated assertion
        Tree. *)
    Lemma collector_claim_executes state store locals witness tag claim authors reference
        reference_bytes claim_bytes equal :
      s_funcs store = s_funcs G.E.allocated_store ->
      get_tree_raw witness = [treeh tag; treeh claim] ->
      get_tree_raw tag = [authors; reference] -> tree_tag tag = true ->
      List.length (get_tree_raw claim) = 3 ->
      handle_bytes state reference = Some reference_bytes ->
      handle_bytes (Datatypes.S state) (Data (Ref (TreeRef claim))) = Some claim_bytes ->
      G.E.H.Tables.T.O.compare_handles (blobh reference_bytes) (blobh claim_bytes) = Some equal ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [ext (treeh witness)] ++ [AI_basic (BI_call 26%N)])
        (Datatypes.S (Datatypes.S state), store, Build_frame locals G.E.collector_instance,
          if equal then v_to_e_list [ext (treeh claim)] else [AI_trap]).
    Proof.
      intros Functions Witness Fields Tagged Length Reference ClaimBytes Compared.
      eapply G.E.collector_call with (code := evaluation_claim)
        (parameters := [T_ref T_externref]) (results := [T_ref T_externref])
        (defaults := [VAL_ref (VAL_ref_null T_externref); VAL_ref (VAL_ref_null T_externref)])
        (index := 26%N) (address := 26%N) (arguments := [ext (treeh witness)])
        (values := [ext (treeh claim)]) (accepted := equal)
        (final_frame := Build_frame [ext (treeh witness); ext (treeh tag); ext (treeh claim)]
          G.E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - eapply collector_claim_body_executes; eassumption.
    Qed.
    (** The claim helper traps when the outer witness is not the required two-element Tree. *)
    Lemma collector_claim_rejects_container state store locals witness :
      s_funcs store = s_funcs G.E.allocated_store ->
      (forall tree, witness = treeh tree ->
        (Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 64)%Z /\ List.length (get_tree_raw tree) <> 2) ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [ext witness] ++ [AI_basic (BI_call 26%N)])
        (state, store, Build_frame locals G.E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Invalid.
      set (defaults := [VAL_ref (VAL_ref_null T_externref); VAL_ref (VAL_ref_null T_externref)]).
      set (inside := ext witness :: defaults).
      eapply G.E.collector_call with (code := evaluation_claim)
        (parameters := [T_ref T_externref]) (results := [T_ref T_externref]) (defaults := defaults)
        (index := 26%N) (address := 26%N) (arguments := [ext witness]) (values := [ext witness])
        (accepted := false) (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - rewrite evaluation_claim_body.
        execute (local_get_run state store inside 0%N (ext witness) eq_refl).
        eapply G.E.collector_trap_suffix with
          (instructions := v_to_e_list [ext witness; word 2] ++ [AI_basic (BI_call 22%N)]).
        apply G.collector_arity_rejects_shape with (expected := 2);
          [exact Functions|vm_compute; congruence|exact Invalid].
    Qed.

    (** The claim helper traps when the supplied author-and-claim Tree is not marked as a
        Tag. *)
    Lemma collector_claim_rejects_untagged state store locals witness coupon assertion :
      s_funcs store = s_funcs G.E.allocated_store ->
      get_tree_raw witness = [coupon; assertion] -> G.E.H.W.Api.is_tag coupon = false ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [ext (treeh witness)] ++ [AI_basic (BI_call 26%N)])
        (state, store, Build_frame locals G.E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Witness Untagged.
      set (empty := VAL_ref (VAL_ref_null T_externref)).
      set (locals2 := [ext (treeh witness); ext coupon; ext assertion]).
      eapply G.E.collector_call with (code := evaluation_claim)
        (parameters := [T_ref T_externref]) (results := [T_ref T_externref]) (defaults := [empty; empty])
        (index := 26%N) (address := 26%N) (arguments := [ext (treeh witness)])
        (values := [ext assertion]) (accepted := false)
        (final_frame := Build_frame locals2 G.E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - eapply G.E.collector_trap_suffix with
          (instructions := to_e_list (firstn 14 (modfunc_body evaluation_claim)))
          (suffix := to_e_list (skipn 14 (modfunc_body evaluation_claim))).
        exact (collector_claim_prefix_executes state store witness coupon assertion false Functions Witness Untagged).
    Qed.

    (** The claim helper traps when the Tag or assertion fields have invalid shape. *)
    Lemma collector_claim_rejects_inner_shape state store locals witness tag assertion :
      s_funcs store = s_funcs G.E.allocated_store ->
      get_tree_raw witness = [treeh tag; assertion] -> tree_tag tag = true ->
      (((Z.of_nat (List.length (get_tree_raw tag)) < 2 ^ 64)%Z /\ List.length (get_tree_raw tag) <> 2) \/
        (List.length (get_tree_raw tag) = 2 /\ forall claim, assertion = treeh claim ->
          (Z.of_nat (List.length (get_tree_raw claim)) < 2 ^ 64)%Z /\ List.length (get_tree_raw claim) <> 3)) ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [ext (treeh witness)] ++ [AI_basic (BI_call 26%N)])
        (state, store, Build_frame locals G.E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Witness Tagged Invalid.
      set (empty := VAL_ref (VAL_ref_null T_externref)).
      set (inside := [ext (treeh witness); ext (treeh tag); ext assertion]).
      eapply G.E.collector_call with (code := evaluation_claim)
        (parameters := [T_ref T_externref]) (results := [T_ref T_externref]) (defaults := [empty; empty])
        (index := 26%N) (address := 26%N) (arguments := [ext (treeh witness)])
        (values := [ext assertion]) (accepted := false)
        (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - pose proof (collector_claim_prefix_executes state store witness (treeh tag) assertion true
          Functions Witness Tagged) as Prefix.
        rewrite evaluation_claim_body in Prefix; cbn [firstn] in Prefix.
        rewrite evaluation_claim_body; execute Prefix.
        execute (local_get_run state store inside 1%N (ext (treeh tag)) eq_refl).
        destruct Invalid as [[Bound Different]|[TagLength Invalid]].
        + eapply G.E.collector_trap_suffix with
            (instructions := v_to_e_list [ext (treeh tag); word 2] ++ [AI_basic (BI_call 22%N)]).
          apply G.collector_arity_rejects_shape with (expected := 2);
            [exact Functions|vm_compute; congruence|].
          intros other Same; inversion Same; subst other; auto.
        + execute (arity_run state store inside tag 2 Functions TagLength ltac:(vm_compute; congruence)).
          execute (local_get_run state store inside 2%N (ext assertion) eq_refl).
          eapply G.E.collector_trap_suffix with
            (instructions := v_to_e_list [ext assertion; word 3] ++ [AI_basic (BI_call 22%N)]).
          apply G.collector_arity_rejects_shape with (expected := 3);
            [exact Functions|vm_compute; congruence|exact Invalid].
    Qed.

    (** The premise helper's input instructions obtain the authenticated claim and its
        fields. *)
    Lemma collector_premise_input_executes state store witness tag claim authors reference opcode
        claimed_lhs claimed_rhs lhs rhs kind :
      s_funcs store = s_funcs G.E.allocated_store ->
      get_tree_raw witness = [treeh tag; treeh claim] ->
      get_tree_raw tag = [authors; reference] -> tree_tag tag = true ->
      get_tree_raw claim = [opcode; claimed_lhs; claimed_rhs] ->
      observed_same state reference (Data (Ref (TreeRef claim))) ->
      let arguments := [VAL_num (VAL_int64 (wasm_nat64 kind)); ext (treeh witness); ext lhs; ext rhs] in
      let empty := VAL_ref (VAL_ref_null T_externref) in
      reduce_trans
        (state, store, Build_frame (arguments ++ [empty; empty; empty; word 0; word 0; word 0])
          G.E.collector_instance, to_e_list (firstn 10 (modfunc_body evaluation_premise)))
        (state + 2, store,
          Build_frame (arguments ++ [ext (treeh tag); ext (treeh claim); empty; word 0; word 0; word 0])
            G.E.collector_instance, v_to_e_list [ext opcode]).
    Proof.
      intros Functions Witness Fields Tagged Assertion Reference arguments empty.
      set (locals0 := arguments ++ [empty; empty; empty; word 0; word 0; word 0]).
      set (locals1 := arguments ++ [empty; ext (treeh claim); empty; word 0; word 0; word 0]).
      set (locals2 := arguments ++ [ext (treeh tag); ext (treeh claim); empty; word 0; word 0; word 0]).
      assert (Length : List.length (get_tree_raw claim) = 3) by now rewrite Assertion.
      assert (TagChild : nth_error (get_tree_raw witness) 0 = Some (treeh tag)) by now rewrite Witness.
      assert (Code : nth_error (get_tree_raw claim) 0 = Some opcode) by now rewrite Assertion.
      replace (firstn 10 (modfunc_body evaluation_premise)) with
        (firstn 10 (firstn 24 (modfunc_body evaluation_premise))) by reflexivity.
      rewrite evaluation_premise_header; cbn [firstn].
      execute (local_get_run state store locals0 1%N (ext (treeh witness)) eq_refl).
      destruct Reference as [reference_bytes [claim_bytes [Reference [ClaimBytes Compared]]]].
      execute (collector_claim_executes state store locals0 witness tag claim authors reference
        reference_bytes claim_bytes true Functions Witness Fields Tagged Length Reference ClaimBytes Compared).
      replace (Datatypes.S (Datatypes.S state)) with (state + 2) by lia.
      execute (local_set_run (state + 2) store locals0 5%N (ext (treeh claim)) ltac:(cbn; lia)).
      execute (local_get_run (state + 2) store locals1 1%N (ext (treeh witness)) eq_refl).
      execute (child_run (state + 2) store locals1 witness 0 (treeh tag) Functions
        ltac:(vm_compute; congruence) TagChild).
      execute (local_set_run (state + 2) store locals1 4%N (ext (treeh tag)) ltac:(cbn; lia)).
      execute (local_get_run (state + 2) store locals2 5%N (ext (treeh claim)) eq_refl).
      execute (child_run (state + 2) store locals2 claim 0 opcode Functions ltac:(vm_compute; congruence) Code).
      apply rt_refl.
    Qed.

    (** The premise helper header authenticates the claim and checks its requested code and
        endpoints. *)
    Lemma collector_premise_header_executes state store witness tag claim authors reference code
        claimed_lhs claimed_rhs lhs rhs n before page left_equal right_equal :
      s_funcs store = s_funcs G.E.allocated_store ->
      s_mems store = [before] ->
      G.E.H.attachment_allowed before (get_blob_data code) = true ->
      attachment_page (get_blob_data code) = Some page ->
      get_tree_raw witness = [treeh tag; treeh claim] ->
      get_tree_raw tag = [authors; reference] -> tree_tag tag = true ->
      get_tree_raw claim = [blobh code; claimed_lhs; claimed_rhs] ->
      decode_u64 (get_blob_data code) = Some n ->
      observed_same state reference (Data (Ref (TreeRef claim))) ->
      observed_comparison (state + 2) claimed_lhs lhs left_equal ->
      (if left_equal then observed_comparison (state + 4) claimed_rhs rhs right_equal else True) ->
      let arguments := [VAL_num (VAL_int64 (wasm_nat64 n)); ext (treeh witness); ext lhs; ext rhs] in
      let empty := VAL_ref (VAL_ref_null T_externref) in
      reduce_trans
        (state, store, Build_frame (arguments ++ [empty; empty; empty; word 0; word 0; word 0])
          G.E.collector_instance, to_e_list (firstn 24 (modfunc_body evaluation_premise)))
        (state + (if left_equal then 6 else 4), upd_s_mem store [page],
          Build_frame (arguments ++ [ext (treeh tag); ext (treeh claim); empty; word 0; word 0; word 0])
            G.E.collector_instance, if andb left_equal right_equal then [] else [AI_trap]).
    Proof.
      intros Functions Memory Allowed Page Witness Fields Tagged Assertion Decoded
        Reference Left Right arguments empty.
      set (locals2 := arguments ++ [ext (treeh tag); ext (treeh claim); empty; word 0; word 0; word 0]).
      assert (Lhs : nth_error (get_tree_raw claim) 1 = Some claimed_lhs) by now rewrite Assertion.
      assert (Rhs : nth_error (get_tree_raw claim) 2 = Some claimed_rhs) by now rewrite Assertion.
      assert (Functions' : s_funcs (upd_s_mem store [page]) = s_funcs G.E.allocated_store)
        by exact Functions.
      pose proof (collector_premise_input_executes state store witness tag claim authors reference
        (blobh code) claimed_lhs claimed_rhs lhs rhs n Functions Witness Fields Tagged Assertion Reference) as Input.
      replace (firstn 10 (modfunc_body evaluation_premise)) with
        (firstn 10 (firstn 24 (modfunc_body evaluation_premise))) in Input by reflexivity.
      rewrite evaluation_premise_header in Input; cbn [firstn] in Input.
      rewrite evaluation_premise_header; execute Input.
      execute (collector_integer_executes (state + 2) store locals2 code n before page
        Functions Memory Allowed Page Decoded).
      execute_under [VAL_num (VAL_int64 (wasm_nat64 n))]
        (local_get_run (state + 2) (upd_s_mem store [page]) locals2 0%N
          (VAL_num (VAL_int64 (wasm_nat64 n))) eq_refl).
      assert (Equal : reduce_trans
        (state + 2, upd_s_mem store [page], Build_frame locals2 G.E.collector_instance,
          [v_to_e (VAL_num (VAL_int64 (wasm_nat64 n)));
           v_to_e (VAL_num (VAL_int64 (wasm_nat64 n))); AI_basic (BI_relop T_i64 (Relop_i ROI_eq))])
        (state + 2, upd_s_mem store [page], Build_frame locals2 G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int32 (wasm_bool true))])).
      { assert (Same : app_relop (Relop_i ROI_eq)
          (VAL_int64 (wasm_nat64 n)) (VAL_int64 (wasm_nat64 n)) = true).
        { change (Wasm_int.Int64.eq (wasm_nat64 n) (wasm_nat64 n) = true).
          apply Wasm_int.Int64.eq_true. }
        rewrite <- Same.
        apply rt_step, r_simple; eapply rs_relop with
          (v1 := VAL_int64 (wasm_nat64 n)) (v2 := VAL_int64 (wasm_nat64 n))
          (t := T_i64) (op := Relop_i ROI_eq); reflexivity. }
      execute Equal.
      execute (G.E.collector_require_executes combination true handle_bytes (state + 2)
        (upd_s_mem store [page]) locals2 true ltac:(rewrite Functions'; reflexivity)).
      execute (local_get_run (state + 2) (upd_s_mem store [page]) locals2 5%N (ext (treeh claim)) eq_refl).
      execute (child_run (state + 2) (upd_s_mem store [page]) locals2 claim 1 claimed_lhs Functions'
        ltac:(vm_compute; congruence) Lhs).
      execute_under [ext claimed_lhs]
        (local_get_run (state + 2) (upd_s_mem store [page]) locals2 2%N (ext lhs) eq_refl).
      execute (same_run (state + 2) (upd_s_mem store [page]) locals2 claimed_lhs lhs left_equal Functions' Left).
      replace (Datatypes.S (Datatypes.S (state + 2))) with (state + 4) by lia.
      destruct left_equal.
      - execute (local_get_run (state + 4) (upd_s_mem store [page]) locals2 5%N (ext (treeh claim)) eq_refl).
        execute (child_run (state + 4) (upd_s_mem store [page]) locals2 claim 2 claimed_rhs Functions'
          ltac:(vm_compute; congruence) Rhs).
        execute_under [ext claimed_rhs]
          (local_get_run (state + 4) (upd_s_mem store [page]) locals2 3%N (ext rhs) eq_refl).
        execute (same_run (state + 4) (upd_s_mem store [page]) locals2 claimed_rhs rhs right_equal Functions' Right).
        replace (Datatypes.S (Datatypes.S (state + 4))) with (state + 6) by lia.
        destruct right_equal; apply rt_refl.
      - apply rt_step, r_simple; eapply rs_trap with (lh := LH_base [] (to_e_list
          [BI_local_get 5%N; BI_const_num (VAL_int32 (wasm_nat 2)); BI_call 7%N; BI_local_get 3%N; BI_call 19%N]));
          [discriminate|reflexivity].
    Qed.

    (** The premise helper traps when its exact-identity endpoint checks fail. *)
    Lemma collector_premise_rejects_endpoints state store locals witness tag claim authors reference code
        claimed_lhs claimed_rhs lhs rhs n before page left_equal right_equal :
      s_funcs store = s_funcs G.E.allocated_store ->
      s_mems store = [before] ->
      G.E.H.attachment_allowed before (get_blob_data code) = true ->
      attachment_page (get_blob_data code) = Some page ->
      get_tree_raw witness = [treeh tag; treeh claim] ->
      get_tree_raw tag = [authors; reference] -> tree_tag tag = true ->
      get_tree_raw claim = [blobh code; claimed_lhs; claimed_rhs] ->
      decode_u64 (get_blob_data code) = Some n ->
      observed_same state reference (Data (Ref (TreeRef claim))) ->
      observed_comparison (state + 2) claimed_lhs lhs left_equal ->
      (if left_equal then observed_comparison (state + 4) claimed_rhs rhs right_equal else True) ->
      andb left_equal right_equal = false ->
      let arguments := [VAL_num (VAL_int64 (wasm_nat64 n)); ext (treeh witness); ext lhs; ext rhs] in
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list arguments ++ [AI_basic (BI_call 27%N)])
        (state + (if left_equal then 6 else 4), upd_s_mem store [page],
          Build_frame locals G.E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Memory Allowed Page Witness Fields Tagged Assertion Decoded Ref Left Right Rejected arguments.
      set (empty := VAL_ref (VAL_ref_null T_externref)).
      set (defaults := [empty; empty; empty; word 0; word 0; word 0]).
      set (inside := arguments ++ [ext (treeh tag); ext (treeh claim); empty; word 0; word 0; word 0]).
      eapply G.E.collector_call_void with (code := evaluation_premise)
        (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
        (defaults := defaults) (index := 27%N) (address := 27%N) (arguments := arguments)
        (accepted := false) (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - pose proof (collector_premise_header_executes state store witness tag claim authors reference code
          claimed_lhs claimed_rhs lhs rhs n before page left_equal right_equal Functions Memory Allowed Page
          Witness Fields Tagged Assertion Decoded Ref Left Right) as Header.
        rewrite Rejected in Header.
        eapply G.E.collector_trap_suffix with
          (instructions := to_e_list (firstn 24 (modfunc_body evaluation_premise)))
          (suffix := to_e_list (skipn 24 (modfunc_body evaluation_premise))).
        exact Header.
    Qed.

    Section PremiseRejection.
      Variable state : nat.
      Variable store : store_record.
      Variable witness tag claim : TreeName.
      Variable authors reference opcode claimed_lhs claimed_rhs lhs rhs : handle.
      Variable kind : nat.
      (** The store contains the collector's expected functions. *)
      Hypothesis functions : s_funcs store = s_funcs G.E.allocated_store.
      (** The witness contains exactly the supplied Tag and assertion Objects. *)
      Hypothesis witness_fields : get_tree_raw witness = [treeh tag; treeh claim].
      (** The Tag contains the authors field followed by the assertion reference. *)
      Hypothesis tag_fields : get_tree_raw tag = [authors; reference].
      (** The supplied claim authoring Tree is marked as a Tag. *)
      Hypothesis tagged : tree_tag tag = true.
      (** The supplied assertion contains the code and both requested claim endpoints. *)
      Hypothesis claim_fields : get_tree_raw claim = [opcode; claimed_lhs; claimed_rhs].
      (** The ready privileged comparison identifies the supplied assertion with the Tag's
          assertion Ref. *)
      Hypothesis authenticated : observed_same state reference (Data (Ref (TreeRef claim))).

      Let arguments := [VAL_num (VAL_int64 (wasm_nat64 kind)); ext (treeh witness); ext lhs; ext rhs].
      Let empty := VAL_ref (VAL_ref_null T_externref).
      Let defaults := [empty; empty; empty; word 0; word 0; word 0].
      Let inside := arguments ++ [ext (treeh tag); ext (treeh claim); empty; word 0; word 0; word 0].

      (** The premise helper traps when the claim code is not an unsigned eight-byte Blob. *)
      Lemma collector_premise_rejects_opcode_format locals :
        (forall blob, opcode = blobh blob ->
          (Z.of_nat (List.length (get_blob_data blob)) < 2 ^ 64)%Z /\
          List.length (get_blob_data blob) <> 8) ->
        reduce_trans
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list arguments ++ [AI_basic (BI_call 27%N)])
          (state + 2, store, Build_frame locals G.E.collector_instance, [AI_trap]).
      Proof.
        intro Invalid.
        eapply G.E.collector_call_void with (code := evaluation_premise)
          (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
          (defaults := defaults) (index := 27%N) (address := 27%N) (arguments := arguments)
          (accepted := false) (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
        - rewrite functions; reflexivity.
        - pose proof (collector_premise_input_executes state store witness tag claim authors reference opcode
            claimed_lhs claimed_rhs lhs rhs kind functions witness_fields tag_fields tagged claim_fields authenticated) as Input.
          replace (firstn 10 (modfunc_body evaluation_premise)) with
            (firstn 10 (firstn 24 (modfunc_body evaluation_premise))) in Input by reflexivity.
          rewrite evaluation_premise_header in Input; cbn [firstn] in Input.
          replace (to_e_list (modfunc_body evaluation_premise)) with
            (to_e_list (firstn 24 (modfunc_body evaluation_premise)) ++
              to_e_list (skipn 24 (modfunc_body evaluation_premise))) by reflexivity.
          rewrite evaluation_premise_header; execute Input.
          eapply G.E.collector_trap_suffix with
            (instructions := v_to_e_list [ext opcode] ++ [AI_basic (BI_call 23%N)]).
          destruct opcode as [[object|ref]|thunk|encode].
          + destruct object as [blob|tree].
            * destruct (Invalid blob eq_refl) as [Bound Width].
              apply collector_integer_rejects_width; assumption.
            * apply collector_integer_rejects_type; [exact functions|intros; discriminate].
          + apply collector_integer_rejects_type; [exact functions|intros; discriminate].
          + apply collector_integer_rejects_type; [exact functions|intros; discriminate].
          + apply collector_integer_rejects_type; [exact functions|intros; discriminate].
      Qed.

      (** The premise helper traps when the decoded claim code differs from the requested
          kind. *)
      Lemma collector_premise_rejects_opcode_value locals blob number before page :
        opcode = blobh blob -> decode_u64 (get_blob_data blob) = Some number ->
        (Z.of_nat kind < 2 ^ 64)%Z -> number <> kind ->
        s_mems store = [before] ->
        G.E.H.attachment_allowed before (get_blob_data blob) = true ->
        attachment_page (get_blob_data blob) = Some page ->
        reduce_trans
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list arguments ++ [AI_basic (BI_call 27%N)])
          (state + 2, upd_s_mem store [page], Build_frame locals G.E.collector_instance, [AI_trap]).
      Proof.
        intros Opcode Decoded KindBound Different Memory Allowed Page.
        eapply G.E.collector_call_void with (code := evaluation_premise)
          (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
          (defaults := defaults) (index := 27%N) (address := 27%N) (arguments := arguments)
          (accepted := false) (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
        - rewrite functions; reflexivity.
        - pose proof (collector_premise_input_executes state store witness tag claim authors reference opcode
            claimed_lhs claimed_rhs lhs rhs kind functions witness_fields tag_fields tagged claim_fields authenticated) as Input.
          replace (firstn 10 (modfunc_body evaluation_premise)) with
            (firstn 10 (firstn 24 (modfunc_body evaluation_premise))) in Input by reflexivity.
          rewrite evaluation_premise_header in Input; cbn [firstn] in Input.
          replace (to_e_list (modfunc_body evaluation_premise)) with
            (to_e_list (firstn 24 (modfunc_body evaluation_premise)) ++
              to_e_list (skipn 24 (modfunc_body evaluation_premise))) by reflexivity.
          rewrite evaluation_premise_header; execute Input; rewrite Opcode.
          execute (collector_integer_executes (state + 2) store inside blob number before page
            functions Memory Allowed Page Decoded).
          execute_under [VAL_num (VAL_int64 (wasm_nat64 number))]
            (local_get_run (state + 2) (upd_s_mem store [page]) inside 0%N
              (VAL_num (VAL_int64 (wasm_nat64 kind))) eq_refl).
          assert (Mismatch : app_relop (Relop_i ROI_eq) (VAL_int64 (wasm_nat64 number))
            (VAL_int64 (wasm_nat64 kind)) = false).
          { change (Wasm_int.Int64.eq (wasm_nat64 number) (wasm_nat64 kind) = false).
            unfold Wasm_int.Int64.eq.
            rewrite (wasm_nat64_unsigned number (decode_u64_bound _ _ Decoded)), (wasm_nat64_unsigned kind KindBound).
            destruct (Coqlib.zeq (Z.of_nat number) (Z.of_nat kind)); [exfalso; apply Different; lia|reflexivity]. }
          assert (Compared : reduce_trans
            (state + 2, upd_s_mem store [page], Build_frame inside G.E.collector_instance,
              [v_to_e (VAL_num (VAL_int64 (wasm_nat64 number)));
               v_to_e (VAL_num (VAL_int64 (wasm_nat64 kind))); AI_basic (BI_relop T_i64 (Relop_i ROI_eq))])
            (state + 2, upd_s_mem store [page], Build_frame inside G.E.collector_instance,
              v_to_e_list [VAL_num (VAL_int32 (wasm_bool false))])).
          { rewrite <- Mismatch; apply rt_step, r_simple; eapply rs_relop with
              (v1 := VAL_int64 (wasm_nat64 number)) (v2 := VAL_int64 (wasm_nat64 kind))
              (t := T_i64) (op := Relop_i ROI_eq); reflexivity. }
          execute Compared.
          eapply G.E.collector_trap_suffix with
            (instructions := [v_to_e (VAL_num (VAL_int32 (wasm_bool false))); AI_basic (BI_call 18%N)]).
          apply G.E.collector_require_executes with (condition := false).
          change (lookup_N (s_funcs store) 18%N = lookup_N (s_funcs G.E.allocated_store) 18%N).
          rewrite functions; reflexivity.
      Qed.
    End PremiseRejection.

    Section AuthorCopy.
      Variable author_base : store_record.
      Variable previous : list handle.
      Variable author_input : TreeName.
      Variable author_type : table_type.
      Variable other_tables : list tableinst.
      Variable author_prefix : list value.
      (** The existing author prefix leaves space within the native unsigned 32-bit counter
          bound. *)
      Hypothesis prefix_length : List.length author_prefix = 7.
      (** The starting author table contains the encoded existing author prefix. *)
      Hypothesis prefix_input : lookup_N author_prefix 6%N = Some (ext (treeh author_input)).
      (** The author-copy store retains the collector's expected functions. *)
      Hypothesis author_functions : s_funcs author_base = s_funcs G.E.allocated_store.
      (** The input author list fits in the unsigned 32-bit table-index range. *)
      Hypothesis author_bound :
        (Z.of_nat (List.length previous + List.length (get_tree_raw author_input)) < 2 ^ 32)%Z.

      Definition author_table index := Build_tableinst author_type
        (G.E.H.Tables.copied_entries previous (get_tree_raw author_input) index).
      Definition author_store index := upd_s_table author_base (author_table index :: other_tables).
      Definition author_locals index := author_prefix ++
        [word index; word (List.length previous); word (List.length (get_tree_raw author_input))].
      Definition author_frame index := Build_frame (author_locals index) G.E.collector_instance.

      (** The author-copy frame holds the input authors, current index, previous length,
          and input length in their specified locals. *)
      Lemma author_locals_lookup index :
        lookup_N (author_locals index) 6%N = Some (ext (treeh author_input)) /\
        lookup_N (author_locals index) 7%N = Some (word index) /\
        lookup_N (author_locals index) 8%N = Some (word (List.length previous)) /\
        lookup_N (author_locals index) 9%N = Some (word (List.length (get_tree_raw author_input))).
      Proof.
        unfold author_locals, lookup_N; repeat split.
        - rewrite List.nth_error_app1 by (rewrite prefix_length; cbn; lia); exact prefix_input.
        - rewrite List.nth_error_app2 by (rewrite prefix_length; cbn; lia); rewrite prefix_length; reflexivity.
        - rewrite List.nth_error_app2 by (rewrite prefix_length; cbn; lia); rewrite prefix_length; reflexivity.
        - rewrite List.nth_error_app2 by (rewrite prefix_length; cbn; lia); rewrite prefix_length; reflexivity.
      Qed.

      (** Incrementing the author-copy counter advances the modeled local frame by one. *)
      Lemma author_counter_update index :
        seq.set_nth (word (S index)) (author_locals index) 7 (word (S index)) = author_locals (S index).
      Proof.
        unfold author_locals; rewrite <- prefix_length.
        apply G.E.H.Tables.set_nth_after_prefix.
      Qed.

      (** A valid author-copy index fits in an unsigned 32-bit integer. *)
      Lemma author_index_bound index : index <= List.length (get_tree_raw author_input) ->
        (Z.of_nat index < 2 ^ 32)%Z.
      Proof. intro Bound; pose proof author_bound as Total; rewrite Nat2Z.inj_add in Total; lia. Qed.

      (** The author-copy loop test exits exactly at the input length and otherwise
          continues. *)
      Lemma author_test_executes state index : index <= List.length (get_tree_raw author_input) ->
        reduce_trans
          (state, author_store index, author_frame index, to_e_list (firstn 4 evaluation_author_loop))
          (state, author_store index, author_frame index,
            if Nat.eqb index (List.length (get_tree_raw author_input))
            then [AI_basic (BI_br 1%N)] else []).
      Proof.
        intro Bound; destruct (author_locals_lookup index) as [Input [Counter [Offset Length]]].
        apply counter_test_run; [apply author_index_bound; exact Bound|
          apply author_index_bound; lia|exact Counter|exact Length].
      Qed.

      (** Writing the next input author into the destination table advances the copy
          invariant by one. *)
      Lemma author_write index value : nth_error (get_tree_raw author_input) index = Some value ->
        stab_update (author_store index) G.E.collector_instance 0%N
          (N.of_nat (List.length previous + index)) (G.E.H.Tables.externref value) =
        Some (author_store (S index)).
      Proof.
        intro Entry.
        assert (Bound : index < List.length (get_tree_raw author_input)).
        { apply (proj1 (List.nth_error_Some (get_tree_raw author_input) index)); rewrite Entry; discriminate. }
        unfold stab_update; rewrite Nat2N.id.
        change ((if N.ltb (N.of_nat (List.length previous + index))
          (N.of_nat (List.length (G.E.H.Tables.copied_entries previous (get_tree_raw author_input) index)))
          then Some (upd_s_table (author_store index)
            (Build_tableinst author_type
              (seq.set_nth (G.E.H.Tables.externref value)
                (G.E.H.Tables.copied_entries previous (get_tree_raw author_input) index)
                (List.length previous + index) (G.E.H.Tables.externref value)) :: other_tables))
          else None) = Some (author_store (S index))).
        rewrite (G.E.H.Tables.copied_entries_length previous (get_tree_raw author_input) index ltac:(lia)).
        assert (Fits : N.ltb (N.of_nat (List.length previous + index))
          (N.of_nat (List.length previous + List.length (get_tree_raw author_input))) = true)
          by (apply N.ltb_lt; lia).
        rewrite Fits, (G.E.H.Tables.copied_entries_write _ _ _ _ Entry); reflexivity.
      Qed.
      (** The native offset addition computes the destination index as the prefix length
          plus the copy index. *)
      Lemma author_offset_add state index : index <= List.length (get_tree_raw author_input) ->
        reduce_trans
          (state, author_store index, author_frame index,
            [v_to_e (word (List.length previous)); v_to_e (word index);
             AI_basic (BI_binop T_i32 (Binop_i BOI_add))])
          (state, author_store index, author_frame index,
            v_to_e_list [word (List.length previous + index)]).
      Proof.
        intro Bound.
        assert (Previous : (Z.of_nat (List.length previous) < 2 ^ 32)%Z).
        { pose proof author_bound as Total; rewrite Nat2Z.inj_add in Total; lia. }
        assert (Sum : Wasm_int.Int32.add (wasm_nat (List.length previous)) (wasm_nat index) =
          wasm_nat (List.length previous + index)).
        { rewrite Wasm_int.Int32.add_unsigned, (wasm_nat_unsigned _ Previous),
            (wasm_nat_unsigned _ (author_index_bound _ Bound)).
          unfold wasm_nat; rewrite Nat2Z.inj_add; reflexivity. }
        apply rt_step, r_simple, rs_binop_success; [reflexivity|].
        change (Some (VAL_int32 (Wasm_int.Int32.add (wasm_nat (List.length previous)) (wasm_nat index))) =
          Some (VAL_int32 (wasm_nat (List.length previous + index)))).
        rewrite Sum; reflexivity.
      Qed.

      (** One author-copy loop iteration writes the next author and advances to the next
          loop state. *)
      Lemma author_iteration state index : index < List.length (get_tree_raw author_input) ->
        reduce_trans (state, author_store index, author_frame index, to_e_list evaluation_author_loop)
          (state, author_store (S index), author_frame (S index), [AI_basic (BI_br 0%N)]).
      Proof.
        intro Bound; destruct (author_locals_lookup index) as [Input [Counter [Offset Length]]].
        assert (WordBound : (Z.of_nat index < 2 ^ 32)%Z) by (apply author_index_bound; lia).
        assert (SumBound : (Z.of_nat (List.length previous + index) < 2 ^ 32)%Z).
        { pose proof author_bound as Total; rewrite Nat2Z.inj_add in Total |- *; lia. }
        destruct (nth_error (get_tree_raw author_input) index) as [child|] eqn:Child;
          [|apply List.nth_error_None in Child; lia].
        assert (Functions : s_funcs (author_store index) = s_funcs G.E.allocated_store) by exact author_functions.
        pose proof (author_test_executes state index ltac:(lia)) as Test.
        assert (Continue : Nat.eqb index (List.length (get_tree_raw author_input)) = false)
          by (apply Nat.eqb_neq; lia).
        rewrite Continue in Test; cbn [evaluation_author_loop firstn] in Test.
        unfold evaluation_author_loop.
        execute Test.
        execute (local_get_run state (author_store index) (author_locals index) 8%N
          (word (List.length previous)) Offset).
        execute_under [word (List.length previous)]
          (local_get_run state (author_store index) (author_locals index) 7%N (word index) Counter).
        execute (author_offset_add state index ltac:(lia)).
        execute_under [word (List.length previous + index)]
          (local_get_run state (author_store index) (author_locals index) 6%N (ext (treeh author_input)) Input).
        execute_under [word (List.length previous + index); ext (treeh author_input)]
          (local_get_run state (author_store index) (author_locals index) 7%N (word index) Counter).
        execute_under [word (List.length previous + index)]
          (child_run state (author_store index) (author_locals index) author_input index child Functions WordBound Child).
        assert (Write : reduce_trans
          (state, author_store index, author_frame index,
            v_to_e_list [word (List.length previous + index); ext child] ++ [AI_basic (BI_table_set 0%N)])
          (state, author_store (S index), author_frame index, [])).
        { apply rt_step, r_table_set_success.
          rewrite (wasm_nat_to_N _ SumBound); apply author_write; exact Child. }
        execute Write.
        execute (local_get_run state (author_store (S index)) (author_locals index) 7%N (word index) Counter).
        assert (Increment : reduce_trans
          (state, author_store (S index), author_frame index,
            [v_to_e (word index); v_to_e (word 1); AI_basic (BI_binop T_i32 (Binop_i BOI_add))])
          (state, author_store (S index), author_frame index, v_to_e_list [word (S index)])).
        { apply rt_step, r_simple, wasm_nat_increment_step; exact WordBound. }
        execute Increment.
        pose proof (local_set_run state (author_store (S index)) (author_locals index) 7%N
          (word (S index)) ltac:(unfold author_locals; rewrite List.length_app, prefix_length; cbn; lia)) as SetCounter.
        change (reduce_trans
          (state, author_store (S index), author_frame index, [v_to_e (word (S index)); AI_basic (BI_local_set 7%N)])
          (state, author_store (S index),
            Build_frame (seq.set_nth (word (S index)) (author_locals index) 7 (word (S index)))
              G.E.collector_instance, [])) in SetCounter.
        rewrite author_counter_update in SetCounter.
        execute SetCounter; apply rt_refl.
      Qed.

      (** The full author-copy loop appends every input author after the existing author
          prefix. *)
      Lemma collector_authors_copied state :
        reduce_trans
          (state, author_store 0, author_frame 0, to_e_list (skipn 39 (modfunc_body evaluation_premise)))
          (state, author_store (List.length (get_tree_raw author_input)),
            author_frame (List.length (get_tree_raw author_input)), []) /\
        tableinst_elem (author_table (List.length (get_tree_raw author_input))) =
          map G.E.H.Tables.externref (previous ++ get_tree_raw author_input).
      Proof.
        split; [|apply G.E.H.Tables.copied_entries_complete].
        rewrite evaluation_author_loop_body.
        eapply G.E.collector_loop_executes with (count := List.length (get_tree_raw author_input))
          (states := fun _ => state) (stores := author_store) (frames := author_frame).
        - intros index Bound; apply author_iteration; exact Bound.
        - exists (to_e_list (skipn 4 evaluation_author_loop)).
          pose proof (author_test_executes state (List.length (get_tree_raw author_input)) (Nat.le_refl _)) as Test.
          rewrite Nat.eqb_refl in Test.
          change (reduce_trans
            (state, author_store (List.length (get_tree_raw author_input)),
              author_frame (List.length (get_tree_raw author_input)),
              to_e_list (firstn 4 evaluation_author_loop) ++ to_e_list (skipn 4 evaluation_author_loop))
            (state, author_store (List.length (get_tree_raw author_input)),
              author_frame (List.length (get_tree_raw author_input)),
              [AI_basic (BI_br 1%N)] ++ to_e_list (skipn 4 evaluation_author_loop))).
          eapply G.E.collector_runs_context with (prefix := [])
            (suffix := to_e_list (skipn 4 evaluation_author_loop))
            (instructions := to_e_list (firstn 4 evaluation_author_loop))
            (instructions' := [AI_basic (BI_br 1%N)]); exact Test.
      Qed.
    End AuthorCopy.

    (** Growing the bounded author table supplies exactly the initial state required for
        copying the new authors. *)
    Lemma collector_grows_authors store previous authors minimum other_tables :
      s_tables store =
        Build_tableinst (Build_table_type (Build_limits minimum None) T_externref)
          (map G.E.H.Tables.externref previous) :: other_tables ->
      (Z.of_nat (List.length previous + List.length (get_tree_raw authors)) < 2 ^ 32)%Z ->
      stab_grow store G.E.collector_instance 0%N
        (N.of_nat (List.length (get_tree_raw authors))) (VAL_ref_null T_externref) =
      Some (author_store store previous authors
        (Build_table_type (Build_limits
          (N.of_nat (List.length previous + List.length (get_tree_raw authors))) None) T_externref)
        other_tables 0, List.length previous).
    Proof.
      intros Tables Bound; unfold stab_grow.
      rewrite (proj1 G.E.collector_resource_indices), Tables.
      cbn -[growtable author_store tab_size].
      rewrite (G.E.H.Tables.grow_copied_entries previous (get_tree_raw authors) minimum
        ltac:(unfold u32_bound; cbn in Bound; lia)).
      unfold tab_size; cbn; rewrite List.length_map; reflexivity.
    Qed.

    (** The premise helper's author setup grows the author table and initializes the
        copy-loop locals. *)
    Lemma collector_author_setup_executes state store previous authors author_type other_tables
        n witness lhs rhs tag claim :
      s_funcs store = s_funcs G.E.allocated_store ->
      nth_error (get_tree_raw tag) 0 = Some (treeh authors) ->
      (Z.of_nat (List.length previous + List.length (get_tree_raw authors)) < 2 ^ 32)%Z ->
      (Z.of_nat (List.length previous) < 2 ^ 32 - 1)%Z ->
      stab_grow store G.E.collector_instance 0%N
        (N.of_nat (List.length (get_tree_raw authors))) (VAL_ref_null T_externref) =
        Some (author_store store previous authors author_type other_tables 0, List.length previous) ->
      let parameters := [VAL_num (VAL_int64 (wasm_nat64 n)); ext (treeh witness); ext lhs; ext rhs] in
      let empty := VAL_ref (VAL_ref_null T_externref) in
      reduce_trans
        (state, store, Build_frame (parameters ++ [ext (treeh tag); ext (treeh claim); empty;
          word 0; word 0; word 0]) G.E.collector_instance,
          to_e_list (firstn 15 (skipn 24 (modfunc_body evaluation_premise))))
        (state, author_store store previous authors author_type other_tables 0,
          author_frame previous authors (parameters ++ [ext (treeh tag); ext (treeh claim); ext (treeh authors)]) 0,
          []).
    Proof.
      intros Functions Author Bound OffsetBound Growing parameters empty.
      set (count := List.length (get_tree_raw authors)).
      assert (CountBound : (Z.of_nat count < 2 ^ 32)%Z).
      { unfold count; rewrite Nat2Z.inj_add in Bound; lia. }
      assert (PreviousBound : (Z.of_nat (List.length previous) < 2 ^ 32)%Z) by lia.
      set (locals0 := parameters ++ [ext (treeh tag); ext (treeh claim); empty; word 0; word 0; word 0]).
      set (locals1 := parameters ++ [ext (treeh tag); ext (treeh claim); ext (treeh authors); word 0; word 0; word 0]).
      set (locals2 := parameters ++ [ext (treeh tag); ext (treeh claim); ext (treeh authors); word 0; word 0; word count]).
      set (locals3 := parameters ++ [ext (treeh tag); ext (treeh claim); ext (treeh authors);
        word 0; word (List.length previous); word count]).
      set (grown := author_store store previous authors author_type other_tables 0).
      rewrite evaluation_author_setup_body.
      execute (local_get_run state store locals0 4%N (ext (treeh tag)) eq_refl).
      execute (child_run state store locals0 tag 0 (treeh authors) Functions
        ltac:(vm_compute; congruence) Author).
      execute (local_set_run state store locals0 6%N (ext (treeh authors)) ltac:(cbn; lia)).
      execute (local_get_run state store locals1 6%N (ext (treeh authors)) eq_refl).
      execute (G.collector_size_executes combination handle_bytes state store locals1 authors Functions CountBound).
      execute (local_set_run state store locals1 9%N (word count) ltac:(cbn; lia)).
      execute_under [empty] (local_get_run state store locals2 9%N (word count) eq_refl).
      assert (Grow : reduce_trans
        (state, store, Build_frame locals2 G.E.collector_instance,
          v_to_e_list [empty; word count] ++ [AI_basic (BI_table_grow 0%N)])
        (state, grown, Build_frame locals2 G.E.collector_instance,
          v_to_e_list [word (List.length previous)])).
      { apply rt_step, r_table_grow_success.
        rewrite (wasm_nat_to_N _ CountBound); exact Growing. }
      execute Grow.
      execute (local_set_run state grown locals2 8%N (word (List.length previous)) ltac:(cbn; lia)).
      execute (local_get_run state grown locals3 8%N (word (List.length previous)) eq_refl).
      assert (NotFailed : app_relop (Relop_i ROI_ne) (VAL_int32 (wasm_nat (List.length previous)))
        (VAL_int32 (Wasm_int.int_of_Z i32m (-1)%Z)) = true).
      { change (negb (Wasm_int.Int32.eq (wasm_nat (List.length previous))
          (Wasm_int.Int32.repr (-1))) = true).
        unfold Wasm_int.Int32.eq; rewrite (wasm_nat_unsigned _ PreviousBound).
        change (negb (if Coqlib.zeq (Z.of_nat (List.length previous)) 4294967295 then true else false) = true).
        destruct (Coqlib.zeq (Z.of_nat (List.length previous)) 4294967295);
          [cbn in OffsetBound; lia|reflexivity]. }
      assert (Checked : reduce_trans
        (state, grown, Build_frame locals3 G.E.collector_instance,
          [v_to_e (word (List.length previous));
           v_to_e (VAL_num (VAL_int32 (Wasm_int.int_of_Z i32m (-1)%Z)));
           AI_basic (BI_relop T_i32 (Relop_i ROI_ne))])
        (state, grown, Build_frame locals3 G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int32 (wasm_bool true))])).
      { rewrite <- NotFailed; apply rt_step, r_simple; eapply rs_relop with
          (v1 := VAL_int32 (wasm_nat (List.length previous)))
          (v2 := VAL_int32 (Wasm_int.int_of_Z i32m (-1)%Z))
          (t := T_i32) (op := Relop_i ROI_ne); reflexivity. }
      execute Checked.
      execute (G.E.collector_require_executes combination true handle_bytes state grown locals3 true
        ltac:(change (lookup_N (s_funcs store) 18%N = lookup_N (s_funcs G.E.allocated_store) 18%N);
          rewrite Functions; reflexivity)).
      apply rt_refl.
    Qed.

    (** A ready premise body authenticates its claim, validates its code and endpoints, and
        appends its authors. *)
    Lemma collector_premise_body_executes state store witness tag claim authors reference code
        claimed_lhs claimed_rhs lhs rhs n before page previous minimum other_tables :
      s_funcs store = s_funcs G.E.allocated_store ->
      s_mems store = [before] ->
      G.E.H.attachment_allowed before (get_blob_data code) = true ->
      attachment_page (get_blob_data code) = Some page ->
      get_tree_raw witness = [treeh tag; treeh claim] ->
      get_tree_raw tag = [treeh authors; reference] -> tree_tag tag = true ->
      get_tree_raw claim = [blobh code; claimed_lhs; claimed_rhs] ->
      decode_u64 (get_blob_data code) = Some n ->
      observed_same state reference (Data (Ref (TreeRef claim))) ->
      observed_same (state + 2) claimed_lhs lhs ->
      observed_same (state + 4) claimed_rhs rhs ->
      (Z.of_nat (List.length previous + List.length (get_tree_raw authors)) < 2 ^ 32)%Z ->
      (Z.of_nat (List.length previous) < 2 ^ 32 - 1)%Z ->
      s_tables store =
        Build_tableinst (Build_table_type (Build_limits minimum None) T_externref)
          (map G.E.H.Tables.externref previous) :: other_tables ->
      let author_type := Build_table_type
        (Build_limits (N.of_nat (List.length previous + List.length (get_tree_raw authors))) None) T_externref in
      let parameters := [VAL_num (VAL_int64 (wasm_nat64 n)); ext (treeh witness); ext lhs; ext rhs] in
      let empty := VAL_ref (VAL_ref_null T_externref) in
      reduce_trans
        (state, store, Build_frame (parameters ++ [empty; empty; empty; word 0; word 0; word 0])
          G.E.collector_instance, to_e_list (modfunc_body evaluation_premise))
        (state + 6,
          author_store (upd_s_mem store [page]) previous authors author_type other_tables
            (List.length (get_tree_raw authors)),
          author_frame previous authors (parameters ++ [ext (treeh tag); ext (treeh claim); ext (treeh authors)])
            (List.length (get_tree_raw authors)), []).
    Proof.
      intros Functions Memory Allowed Page Witness Fields Tagged Assertion Number Ref Left Right
        Bound OffsetBound Tables author_type parameters empty.
      set (prefix := parameters ++ [ext (treeh tag); ext (treeh claim); ext (treeh authors)]).
      set (locals := parameters ++ [ext (treeh tag); ext (treeh claim); empty; word 0; word 0; word 0]).
      set (loaded := upd_s_mem store [page]).
      assert (LoadedFunctions : s_funcs loaded = s_funcs G.E.allocated_store) by exact Functions.
      assert (Growing : stab_grow loaded G.E.collector_instance 0%N
        (N.of_nat (List.length (get_tree_raw authors))) (VAL_ref_null T_externref) =
        Some (author_store loaded previous authors author_type other_tables 0, List.length previous)).
      { apply collector_grows_authors with (minimum := minimum); [exact Tables|exact Bound]. }
      eapply rt_trans with (y := (state + 6, loaded, Build_frame locals G.E.collector_instance,
        to_e_list (skipn 24 (modfunc_body evaluation_premise)))).
      - eapply G.E.collector_runs_context with (prefix := [])
          (suffix := to_e_list (skipn 24 (modfunc_body evaluation_premise)))
          (instructions := to_e_list (firstn 24 (modfunc_body evaluation_premise))) (instructions' := []).
        eapply collector_premise_header_executes with (left_equal := true) (right_equal := true); eassumption.
      - eapply rt_trans with (y := (state + 6,
          author_store loaded previous authors author_type other_tables 0,
          author_frame previous authors prefix 0, to_e_list (skipn 39 (modfunc_body evaluation_premise)))).
        + eapply G.E.collector_runs_context with (prefix := [])
            (suffix := to_e_list (skipn 39 (modfunc_body evaluation_premise)))
            (instructions := to_e_list (firstn 15 (skipn 24 (modfunc_body evaluation_premise)))) (instructions' := []).
          eapply collector_author_setup_executes; try eassumption.
          rewrite Fields; reflexivity.
        + exact (proj1 (collector_authors_copied loaded previous authors author_type other_tables prefix
            eq_refl eq_refl LoadedFunctions Bound (state + 6))).
    Qed.

    (** A ready premise call authenticates its claim, validates its code and endpoints, and
        appends its authors. *)
    Lemma collector_premise_executes state store locals witness tag claim authors reference code
        claimed_lhs claimed_rhs lhs rhs n before page previous minimum other_tables :
      s_funcs store = s_funcs G.E.allocated_store ->
      s_mems store = [before] ->
      G.E.H.attachment_allowed before (get_blob_data code) = true ->
      attachment_page (get_blob_data code) = Some page ->
      get_tree_raw witness = [treeh tag; treeh claim] ->
      get_tree_raw tag = [treeh authors; reference] -> tree_tag tag = true ->
      get_tree_raw claim = [blobh code; claimed_lhs; claimed_rhs] ->
      decode_u64 (get_blob_data code) = Some n ->
      observed_same state reference (Data (Ref (TreeRef claim))) ->
      observed_same (state + 2) claimed_lhs lhs ->
      observed_same (state + 4) claimed_rhs rhs ->
      (Z.of_nat (List.length previous + List.length (get_tree_raw authors)) < 2 ^ 32)%Z ->
      (Z.of_nat (List.length previous) < 2 ^ 32 - 1)%Z ->
      s_tables store =
        Build_tableinst (Build_table_type (Build_limits minimum None) T_externref)
          (map G.E.H.Tables.externref previous) :: other_tables ->
      let author_type := Build_table_type
        (Build_limits (N.of_nat (List.length previous + List.length (get_tree_raw authors))) None) T_externref in
      let parameters := [VAL_num (VAL_int64 (wasm_nat64 n)); ext (treeh witness); ext lhs; ext rhs] in
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list parameters ++ [AI_basic (BI_call 27%N)])
        (state + 6,
          author_store (upd_s_mem store [page]) previous authors author_type other_tables
            (List.length (get_tree_raw authors)),
          Build_frame locals G.E.collector_instance, []).
    Proof.
      intros Functions Memory Allowed Page Witness Fields Tagged Assertion Number Ref Left Right
        Bound OffsetBound Tables author_type parameters.
      set (empty := VAL_ref (VAL_ref_null T_externref)).
      eapply G.E.collector_call_void with (code := evaluation_premise)
        (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
        (defaults := [empty; empty; empty; word 0; word 0; word 0])
        (index := 27%N) (address := 27%N) (arguments := parameters) (accepted := true)
        (final_frame := author_frame previous authors
          (parameters ++ [ext (treeh tag); ext (treeh claim); ext (treeh authors)])
          (List.length (get_tree_raw authors))); try reflexivity.
      - rewrite Functions; reflexivity.
      - eapply collector_premise_body_executes with (minimum := minimum); eassumption.
    Qed.

    (** Premise validation traps when the authors field is not a Tree of supported size. *)
    Lemma collector_premise_rejects_author_container state store locals witness tag claim authors reference code
        claimed_lhs claimed_rhs lhs rhs n before page :
      s_funcs store = s_funcs G.E.allocated_store ->
      s_mems store = [before] ->
      G.E.H.attachment_allowed before (get_blob_data code) = true ->
      attachment_page (get_blob_data code) = Some page ->
      get_tree_raw witness = [treeh tag; treeh claim] ->
      get_tree_raw tag = [authors; reference] -> tree_tag tag = true ->
      get_tree_raw claim = [blobh code; claimed_lhs; claimed_rhs] ->
      decode_u64 (get_blob_data code) = Some n ->
      observed_same state reference (Data (Ref (TreeRef claim))) ->
      observed_same (state + 2) claimed_lhs lhs ->
      observed_same (state + 4) claimed_rhs rhs ->
      (forall tree, authors = treeh tree ->
        (2 ^ 32 <= Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 64)%Z) ->
      let arguments := [VAL_num (VAL_int64 (wasm_nat64 n)); ext (treeh witness); ext lhs; ext rhs] in
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list arguments ++ [AI_basic (BI_call 27%N)])
        (state + 6, upd_s_mem store [page], Build_frame locals G.E.collector_instance, [AI_trap]).
    Proof.
      intros Functions Memory Allowed Page Witness Fields Tagged Assertion Decoded Ref Left Right Invalid arguments.
      set (empty := VAL_ref (VAL_ref_null T_externref)).
      set (defaults := [empty; empty; empty; word 0; word 0; word 0]).
      set (locals0 := arguments ++ [ext (treeh tag); ext (treeh claim); empty; word 0; word 0; word 0]).
      set (locals1 := arguments ++ [ext (treeh tag); ext (treeh claim); ext authors; word 0; word 0; word 0]).
      set (loaded := upd_s_mem store [page]).
      assert (LoadedFunctions : s_funcs loaded = s_funcs G.E.allocated_store) by exact Functions.
      assert (Authors : nth_error (get_tree_raw tag) 0 = Some authors) by now rewrite Fields.
      eapply G.E.collector_call_void with (code := evaluation_premise)
        (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
        (defaults := defaults) (index := 27%N) (address := 27%N) (arguments := arguments)
        (accepted := false) (final_frame := Build_frame locals1 G.E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - pose proof (collector_premise_header_executes state store witness tag claim authors reference code
          claimed_lhs claimed_rhs lhs rhs n before page true true Functions Memory Allowed Page Witness Fields Tagged Assertion
          Decoded Ref Left Right) as Header.
        rewrite evaluation_premise_header in Header; cbn [firstn] in Header.
        replace (to_e_list (modfunc_body evaluation_premise)) with
          (to_e_list (firstn 24 (modfunc_body evaluation_premise)) ++
            to_e_list (skipn 24 (modfunc_body evaluation_premise))) by reflexivity.
        rewrite evaluation_premise_header; execute Header.
        execute (local_get_run (state + 6) loaded locals0 4%N (ext (treeh tag)) eq_refl).
        execute (child_run (state + 6) loaded locals0 tag 0 authors LoadedFunctions
          ltac:(vm_compute; congruence) Authors).
        execute (local_set_run (state + 6) loaded locals0 6%N (ext authors) ltac:(cbn; lia)).
        execute (local_get_run (state + 6) loaded locals1 6%N (ext authors) eq_refl).
        eapply G.E.collector_trap_suffix with
          (instructions := v_to_e_list [ext authors] ++ [AI_basic (BI_call 21%N)]).
        apply G.collector_size_rejects_shape; assumption.
    Qed.

    (** A witness whose reference-byte check succeeds decodes to its stated abstract claim. *)
    Lemma collector_claim_decodes state witness tag claim authors reference code lhs rhs kind n
        reference_bytes claim_bytes :
      get_tree_raw witness = [treeh tag; treeh claim] ->
      get_tree_raw tag = [treeh authors; reference] -> tree_tag tag = true ->
      get_tree_raw claim = [blobh code; lhs; rhs] ->
      to_nat (get_blob_data code) = Some n -> Claims.Code.statement_kind n = Some kind ->
      handle_bytes state reference = Some reference_bytes ->
      handle_bytes (Datatypes.S state) (Data (Ref (TreeRef claim))) = Some claim_bytes ->
      G.E.H.Tables.T.O.compare_handles (blobh reference_bytes) (blobh claim_bytes) = Some true ->
      Claims.decode (treeh witness) = Some (kind, authors, lhs, rhs).
    Proof.
      intros Witness Fields Tagged Assertion Number Kind Reference ClaimBytes Compared.
      pose proof (G.collector_object_comparison_sound handle_bytes C.bytes_identify state reference
        (TreeObj claim) reference_bytes claim_bytes Reference ClaimBytes Compared) as Authenticated.
      cbn [G.E.H.W.H.data_reference] in Authenticated; subst reference.
      unfold Claims.decode; cbn [treeh]; rewrite Witness.
      cbn -[Claims.API.handle_eqb to_nat Claims.Code.statement_kind]; rewrite Tagged, Fields.
      cbn -[Claims.API.handle_eqb to_nat Claims.Code.statement_kind].
      rewrite (proj2 (Claims.API.handle_eqb_spec _ _) eq_refl).
      rewrite Assertion.
      cbn -[to_nat Claims.Code.statement_kind]; rewrite Number.
      cbn -[Claims.Code.statement_kind]; rewrite Kind; reflexivity.
    Qed.
    (** A ready authenticated header with matching code and endpoints decodes to the
        requested abstract claim. *)
    Lemma collector_premise_header_decodes state witness tag claim authors reference code
        claimed_lhs claimed_rhs lhs rhs kind n :
      get_tree_raw witness = [treeh tag; treeh claim] ->
      get_tree_raw tag = [treeh authors; reference] -> tree_tag tag = true ->
      get_tree_raw claim = [blobh code; claimed_lhs; claimed_rhs] ->
      decode_u64 (get_blob_data code) = Some n -> Claims.Code.statement_kind n = Some kind ->
      observed_same state reference (Data (Ref (TreeRef claim))) ->
      observed_same (state + 2) claimed_lhs lhs ->
      observed_same (state + 4) claimed_rhs rhs ->
      Claims.decode (treeh witness) = Some (kind, authors, lhs, rhs).
    Proof.
      intros Witness Fields Tagged Assertion Number Kind Reference Left Right.
      apply observed_same_identifies in Left; apply observed_same_identifies in Right.
      subst claimed_lhs claimed_rhs.
      destruct Reference as [reference_bytes [claim_bytes [Reference [ClaimBytes Compared]]]].
      eapply collector_claim_decodes; eassumption.
    Qed.
    Definition premise_ready state store witness kind lhs rhs state' store' :=
      exists bundle tag claim authors reference code claimed_lhs claimed_rhs before page previous minimum other_tables,
        witness = treeh bundle /\
        s_funcs store = s_funcs G.E.allocated_store /\ s_mems store = [before] /\
        G.E.H.attachment_allowed before (get_blob_data code) = true /\
        attachment_page (get_blob_data code) = Some page /\
        get_tree_raw bundle = [treeh tag; treeh claim] /\
        get_tree_raw tag = [treeh authors; reference] /\ tree_tag tag = true /\
        get_tree_raw claim = [blobh code; claimed_lhs; claimed_rhs] /\
        decode_u64 (get_blob_data code) = Some kind /\
        observed_same state reference (Data (Ref (TreeRef claim))) /\
        observed_same (state + 2) claimed_lhs lhs /\
        observed_same (state + 4) claimed_rhs rhs /\
        (Z.of_nat (List.length previous + List.length (get_tree_raw authors)) < 2 ^ 32)%Z /\
        (Z.of_nat (List.length previous) < 2 ^ 32 - 1)%Z /\
        s_tables store =
          Build_tableinst (Build_table_type (Build_limits minimum None) T_externref)
            (map G.E.H.Tables.externref previous) :: other_tables /\
        state' = state + 6 /\
        store' = author_store (upd_s_mem store [page]) previous authors
          (Build_table_type (Build_limits
            (N.of_nat (List.length previous + List.length (get_tree_raw authors))) None) T_externref)
          other_tables (List.length (get_tree_raw authors)).

    (** The readiness predicate supplies a successful native execution of premise
        validation. *)
    Lemma collector_ready_premise_executes state store witness kind lhs rhs state' store' locals :
      premise_ready state store witness kind lhs rhs state' store' ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 kind)); ext witness; ext lhs; ext rhs] ++
            [AI_basic (BI_call 27%N)])
        (state', store', Build_frame locals G.E.collector_instance, []).
    Proof.
      intros [bundle [tag [claim [authors [reference [code [claimed_lhs [claimed_rhs
        [before [page [previous [minimum [other_tables Ready]]]]]]]]]]]]].
      destruct Ready as [Witness [Functions [Memory [Allowed [Page [Bundle [Fields [Tagged [Assertion
        [Number [Ref [Left [Right [Bound [Offset [Tables [State Store]]]]]]]]]]]]]]]]].
      subst witness state' store'.
      eapply collector_premise_executes; eassumption.
    Qed.

    (** A ready premise decodes as the requested statement kind and endpoints with some
        author list. *)
    Lemma collector_ready_premise_decodes state store witness kind lhs rhs state' store' claim_kind :
      premise_ready state store witness kind lhs rhs state' store' ->
      Claims.Code.statement_kind kind = Some claim_kind ->
      exists authors, Claims.decode witness = Some (claim_kind, authors, lhs, rhs).
    Proof.
      intros [bundle [tag [claim [authors [reference [code [claimed_lhs [claimed_rhs
        [before [page [previous [minimum [other_tables Ready]]]]]]]]]]]]] Kind.
      destruct Ready as [Witness [Functions [Memory [Allowed [Page [Bundle [Fields [Tagged [Assertion
        [Number [Ref [Left [Right [Bound [Offset [Tables [State Store]]]]]]]]]]]]]]]]].
      subst witness; exists authors.
      eapply collector_premise_header_decodes; eassumption.
    Qed.

    (** Validating a ready premise preserves attachment capacity and appends exactly its
        author dependencies to the table. *)
    Lemma collector_ready_premise_resources state store witness kind lhs rhs state' store'
        claim_kind previous minimum other_tables :
      premise_ready state store witness kind lhs rhs state' store' ->
      Claims.Code.statement_kind kind = Some claim_kind ->
      s_tables store = Build_tableinst (Build_table_type (Build_limits minimum None) T_externref)
        (map G.E.H.Tables.externref previous) :: other_tables ->
      exists page next_minimum,
        s_mems store' = [page] /\
        (forall bytes, List.length bytes = 8 -> G.E.H.attachment_allowed page bytes = true) /\
        s_tables store' = Build_tableinst (Build_table_type (Build_limits next_minimum None) T_externref)
          (map G.E.H.Tables.externref (previous ++ X.dependencies [witness])) :: other_tables.
    Proof.
      intros Ready Kind InitialTables.
      destruct Ready as [bundle [tag [claim [authors [reference [code [claimed_lhs [claimed_rhs
        [before [page [prior [initial_minimum [others Ready]]]]]]]]]]]]].
      destruct Ready as [Witness [Functions [Memory [Allowed [Page [Bundle [Fields [Tagged [Assertion
        [Number [Ref [Left [Right [Bound [Offset [Tables [State Store]]]]]]]]]]]]]]]]].
      assert (Entries : map G.E.H.Tables.externref previous = map G.E.H.Tables.externref prior) by congruence.
      assert (Tail : others = other_tables) by congruence.
      apply (f_equal G.E.H.Tables.read_externrefs) in Entries.
      rewrite !G.E.H.Tables.read_externrefs_encode in Entries; inversion Entries; subst prior others.
      assert (Decoded : X.Wire.decode witness = Some (claim_kind, authors, lhs, rhs)).
      { rewrite Witness; eapply collector_premise_header_decodes; eassumption. }
      subst store'.
      exists page, (N.of_nat (List.length previous + List.length (get_tree_raw authors))).
      split; [reflexivity|]; split.
      - intros bytes Width; destruct (attachment_page_shape _ _ Page) as [PageType Length].
        destruct page as [limits data]; cbn in PageType.
        unfold G.E.H.attachment_allowed.
        unfold operations.mem_length in Length |- *; cbn [meminst_type meminst_data] in Length |- *.
        rewrite PageType, Length, Width; reflexivity.
      - unfold author_store, author_table; cbn.
        rewrite G.E.H.Tables.copied_entries_complete.
        unfold X.dependencies, X.authors_of; rewrite Decoded; cbn; rewrite List.app_nil_r; reflexivity.
    Qed.

    Definition claim_ready state witness claim :=
      exists bundle tag authors reference,
        witness = treeh bundle /\ get_tree_raw bundle = [treeh tag; treeh claim] /\
        get_tree_raw tag = [authors; reference] /\ tree_tag tag = true /\
        List.length (get_tree_raw claim) = 3 /\
        observed_same state reference (Data (Ref (TreeRef claim))).

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

    (** With bounded witness structure and available handle bytes, claim authentication
        traps or returns a ready authenticated claim. *)
    Lemma collector_claim_execution state store witness :
      s_funcs store = s_funcs G.E.allocated_store ->
      lengths_bounded 2 witness ->
      (forall index value, exists bytes, handle_bytes index value = Some bytes) ->
      exists state' returned,
        (forall locals, reduce_trans
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [ext witness] ++ [AI_basic (BI_call 26%N)])
          (state', store, Build_frame locals G.E.collector_instance,
            match returned with Some claim => v_to_e_list [ext (treeh claim)] | None => [AI_trap] end)) /\
        (forall claim, returned = Some claim -> state' = state + 2 /\ claim_ready state witness claim).
    Proof.
      intros Functions Bounds Bytes.
      destruct witness as [[[blob|bundle]|reference]|thunk|encoded];
        try solve [exists state, None; split;
          [intro locals; apply collector_claim_rejects_container; [exact Functions|intros tree Same; discriminate]
          |intros claim Returned; discriminate]].
      pose proof (proj1 Bounds) as RootLength.
      destruct (get_tree_raw bundle) as [|coupon [|assertion [|extra rest]]] eqn:Envelope;
        try solve [exists state, None; split;
          [intro locals; apply collector_claim_rejects_container; [exact Functions|];
            intros tree Same; inversion Same; subst tree; split;
              [rewrite Envelope; exact RootLength|rewrite Envelope; cbn; lia]
          |intros claim Returned; discriminate]].
      pose proof (lengths_bounded_child 1 bundle coupon Bounds ltac:(rewrite Envelope; cbn; auto)) as CouponBound.
      pose proof (lengths_bounded_child 1 bundle assertion Bounds ltac:(rewrite Envelope; cbn; auto)) as AssertionBound.
      destruct coupon as [[[blob|tag]|reference]|thunk|encoded];
        try solve [exists state, None; split;
          [intro locals; eapply collector_claim_rejects_untagged; [exact Functions|exact Envelope|reflexivity]
          |intros claim Returned; discriminate]].
      destruct (tree_tag tag) eqn:Tagged; [|exists state, None; split;
        [intro locals; eapply collector_claim_rejects_untagged; [exact Functions|exact Envelope|exact Tagged]
        |intros claim Returned; discriminate]].
      destruct (get_tree_raw tag) as [|authors [|reference [|extra rest]]] eqn:Fields;
        try solve [exists state, None; split;
          [intro locals; eapply collector_claim_rejects_inner_shape; [exact Functions|exact Envelope|exact Tagged|];
            left; split; [exact (proj1 CouponBound)|rewrite Fields; cbn; lia]
          |intros claim Returned; discriminate]].
      assert (Arity : List.length (get_tree_raw tag) = 2) by (rewrite Fields; reflexivity).
      destruct assertion as [[[blob|claim]|ref]|thunk|encoded];
        try solve [exists state, None; split;
          [intro locals; eapply collector_claim_rejects_inner_shape; [exact Functions|exact Envelope|exact Tagged|];
            right; split; [exact Arity|intros tree Same; discriminate]
          |intros claim Returned; discriminate]].
      destruct (Nat.eq_dec (List.length (get_tree_raw claim)) 3) as [ClaimArity|Invalid];
        [|exists state, None; split;
          [intro locals; eapply collector_claim_rejects_inner_shape; [exact Functions|exact Envelope|exact Tagged|];
            right; split; [exact Arity|]; intros tree Same; inversion Same; subst tree; split; [exact (proj1 AssertionBound)|exact Invalid]
          |intros actual Returned; discriminate]].
      destruct (observed_comparison_total state reference (Data (Ref (TreeRef claim))) Bytes) as [equal Compared].
      exists (state + 2), (if equal then Some claim else None); split.
      - intro locals; destruct Compared as [reference_bytes [claim_bytes [Reference [ClaimBytes Equal]]]].
        replace (state + 2) with (Datatypes.S (Datatypes.S state)) by lia.
        destruct equal;
          [eapply collector_claim_executes with (equal := true)
          |eapply collector_claim_executes with (equal := false)]; eassumption.
      - destruct equal; intros actual Returned; [inversion Returned; subst actual|discriminate].
        split; [reflexivity|].
        exists bundle, tag, authors, reference; repeat split; assumption || reflexivity.
    Qed.

    (** With the stated host and resource bounds, premise validation traps or produces a
        ready premise and its resulting store. *)
    Lemma collector_premise_execution state store witness kind claim_kind lhs rhs
        before previous minimum other_tables :
      s_funcs store = s_funcs G.E.allocated_store ->
      lengths_bounded 3 witness ->
      (forall index value, exists bytes, handle_bytes index value = Some bytes) ->
      s_mems store = [before] ->
      (forall bytes, List.length bytes = 8 -> G.E.H.attachment_allowed before bytes = true) ->
      s_tables store = Build_tableinst (Build_table_type (Build_limits minimum None) T_externref)
        (map G.E.H.Tables.externref previous) :: other_tables ->
      (Z.of_nat (List.length previous) < 2 ^ 32 - 1)%Z ->
      (Z.of_nat kind < 2 ^ 64)%Z -> Claims.Code.statement_kind kind = Some claim_kind ->
      (forall authors left right, Claims.decode witness = Some (claim_kind, authors, left, right) ->
        (Z.of_nat (List.length previous + List.length (get_tree_raw authors)) < 2 ^ 32)%Z) ->
      exists state' store' (accepted : bool),
        (forall locals, reduce_trans
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 kind)); ext witness; ext lhs; ext rhs] ++
              [AI_basic (BI_call 27%N)])
          (state', store', Build_frame locals G.E.collector_instance, if accepted then [] else [AI_trap])) /\
        (accepted = true -> premise_ready state store witness kind lhs rhs state' store').
    Proof.
      intros Functions Bounds Bytes Memory Allowed Tables Offset KindBound Kind Budget.
      set (arguments := [VAL_num (VAL_int64 (wasm_nat64 kind)); ext witness; ext lhs; ext rhs]).
      set (empty := VAL_ref (VAL_ref_null T_externref)).
      set (defaults := [empty; empty; empty; word 0; word 0; word 0]).
      set (inside := arguments ++ defaults).
      destruct (collector_claim_execution state store witness Functions
        (lengths_bounded_weaken 2 witness Bounds) Bytes)
        as [claim_state [returned [ClaimRun Authenticated]]].
      destruct returned as [claim|].
      - destruct (Authenticated claim eq_refl) as [ClaimState Ready].
        destruct Ready as [bundle [tag [authors [reference
          [Witness [Envelope [Fields [Tagged [ClaimArity Ref]]]]]]]]]; subst witness.
        destruct (get_tree_raw claim) as [|opcode [|claimed_lhs [|claimed_rhs tail]]] eqn:Assertion;
          cbn in ClaimArity; try discriminate.
        destruct tail; cbn in ClaimArity; [|discriminate].
        destruct opcode as [[[code|tree]|ref]|thunk|encoded];
          try solve [exists (state + 2), store, false; split;
            [intro locals; eapply collector_premise_rejects_opcode_format; try eassumption; intros blob Same; discriminate
            |intro Accepted; discriminate]].
        assert (CodeLength : (Z.of_nat (List.length (get_blob_data code)) < 2 ^ 64)%Z).
        { exact (lengths_bounded_child 1 claim (blobh code)
            (lengths_bounded_child 2 bundle (treeh claim) Bounds ltac:(rewrite Envelope; cbn; auto))
            ltac:(rewrite Assertion; cbn; auto)). }
        destruct (Nat.eq_dec (List.length (get_blob_data code)) 8) as [Width|Invalid].
        2: { exists (state + 2), store, false; split.
          - intro locals; eapply collector_premise_rejects_opcode_format; try eassumption.
            intros blob Same; inversion Same; subst blob; auto.
          - intro Accepted; discriminate. }
        assert (Decoded : exists number, decode_u64 (get_blob_data code) = Some number).
        { change (exists number,
            (if Nat.eqb (List.length (get_blob_data code)) 8
             then Some (Z.to_nat (Memdata.int_of_bytes (get_blob_data code))) else None) = Some number).
          rewrite Width; eexists; reflexivity. }
        destruct Decoded as [number Decoded].
        destruct (attachment_page_exists (get_blob_data code)) as [page Page].
        specialize (Allowed (get_blob_data code) Width).
        destruct (Nat.eq_dec number kind) as [Same|Different].
        2: { exists (state + 2), (upd_s_mem store [page]), false; split.
          - intro locals; eapply collector_premise_rejects_opcode_value; try eassumption; reflexivity.
          - intro Accepted; discriminate. }
        subst number.
        destruct (observed_comparison_total (state + 2) claimed_lhs lhs Bytes) as [left_equal Left].
        destruct left_equal.
        2: { exists (state + 4), (upd_s_mem store [page]), false; split.
          - intro locals; eapply collector_premise_rejects_endpoints with (left_equal := false) (right_equal := false);
              try eassumption; reflexivity || exact I.
          - intro Accepted; discriminate. }
        destruct (observed_comparison_total (state + 4) claimed_rhs rhs Bytes) as [right_equal Right].
        destruct right_equal.
        2: { exists (state + 6), (upd_s_mem store [page]), false; split.
          - intro locals; eapply collector_premise_rejects_endpoints with (left_equal := true) (right_equal := false);
              try eassumption; reflexivity.
          - intro Accepted; discriminate. }
        destruct authors as [[[blob|authors]|ref]|thunk|encoded];
          try solve [exists (state + 6), (upd_s_mem store [page]), false; split;
            [intro locals; eapply collector_premise_rejects_author_container; try eassumption; intros tree Same; discriminate
            |intro Accepted; discriminate]].
        assert (Witness : Claims.decode (treeh bundle) = Some (claim_kind, authors, lhs, rhs)).
        { eapply collector_premise_header_decodes; eassumption. }
        specialize (Budget authors lhs rhs Witness).
        set (author_type := Build_table_type
          (Build_limits (N.of_nat (List.length previous + List.length (get_tree_raw authors))) None) T_externref).
        exists (state + 6), (author_store (upd_s_mem store [page]) previous authors author_type other_tables
          (List.length (get_tree_raw authors))), true; split.
        + intro locals; eapply collector_premise_executes; eassumption.
        + intro Accepted; exists bundle, tag, claim, authors, reference, code, claimed_lhs, claimed_rhs,
            before, page, previous, minimum, other_tables; repeat split; assumption || reflexivity.
      - exists claim_state, store, false; split; [|intro Accepted; discriminate].
        intro locals; eapply G.E.collector_call_void with (code := evaluation_premise)
          (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
          (defaults := defaults) (index := 27%N) (address := 27%N) (arguments := arguments)
          (accepted := false) (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
        + rewrite Functions; reflexivity.
        + replace (to_e_list (modfunc_body evaluation_premise)) with
            ([AI_basic (BI_local_get 1%N); AI_basic (BI_call 26%N)] ++
              to_e_list (skipn 2 (modfunc_body evaluation_premise))) by reflexivity.
          execute (local_get_run state store inside 1%N (ext witness) eq_refl).
          eapply G.E.collector_trap_suffix with
            (instructions := v_to_e_list [ext witness] ++ [AI_basic (BI_call 26%N)]).
          exact (ClaimRun inside).
    Qed.

  End Execution.
End WasmWitnesses.
