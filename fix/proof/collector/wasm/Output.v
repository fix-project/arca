(** Proves native output construction with the selected issuing identity. *)

From FixProof.Collector.wasm Require Import Backend Support.
Import ListNotations.
Local Open Scope list_scope.

Module WasmOutput (C : COLLECTOR_CONTEXT).
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

  Section Execution.
    Section OutputConstruction.
      Variable base : store_record.
      Variable output_type assertion_type : table_type.
      Variable value result issuing_author : handle.
      Variable kind : nat.
      (** The output statement code fits in an unsigned 64-bit integer. *)
      Hypothesis kind_bound : (Z.of_nat kind < 2 ^ 64)%Z.
      (** The starting store contains exactly the collector's expected functions. *)
      Hypothesis base_functions : s_funcs base = s_funcs G.E.allocated_store.
      (** The output table stores externrefs. *)
      Hypothesis output_element : tt_elem_type output_type = T_externref.
      (** The assertion table stores externrefs. *)
      Hypothesis assertion_element : tt_elem_type assertion_type = T_externref.
      (** The combination's callable field selects the intended issuing author. *)
      Hypothesis selected_author : G.E.H.Tables.T.A.author_at 1 (treeh combination) = Some issuing_author.

      Hypothesis selected_author_view :
        stab_elem base G.E.collector_instance 2%N 1%N = Some (G.E.H.Tables.externref issuing_author).

      Let code := blobh (create_blob (from_nat kind)).
      Let claim_fields := [code; value; result].
      Let claim := create_tree claim_fields.
      Let reference := Data (Ref (TreeRef claim)).
      Let output_fields := [issuing_author; reference].
      Let tag := create_tree_tagged true [issuing_author; reference].

      Definition coupon_output_store assertion_index output_index := upd_s_table base
        ([Build_tableinst output_type (G.E.H.Tables.copied_entries [] output_fields output_index);
         Build_tableinst assertion_type (G.E.H.Tables.copied_entries [] claim_fields assertion_index)] ++
          skipn 2 (s_tables base)).

      (** The native output helper returns the Ref-backed certificate Tag. *)
      Lemma collector_output_body_executes state :
        reduce_trans
          (state, coupon_output_store 0 0,
            Build_frame [ext (treeh combination); ext value; ext result; VAL_num (VAL_int64 (wasm_nat64 kind))] G.E.collector_instance,
            to_e_list (modfunc_body native_issue))
          (state, coupon_output_store 3 2,
            Build_frame [ext (treeh combination); ext value; ext reference; VAL_num (VAL_int64 (wasm_nat64 kind))] G.E.collector_instance,
            if is_comparable issuing_author then v_to_e_list [ext (treeh tag)] else [AI_trap]).
      Proof.
        set (before := [ext (treeh combination); ext value; ext result; VAL_num (VAL_int64 (wasm_nat64 kind))]).
        set (after := [ext (treeh combination); ext value; ext reference; VAL_num (VAL_int64 (wasm_nat64 kind))]).
        assert (Functions : forall i j, s_funcs (coupon_output_store i j) = s_funcs G.E.allocated_store)
          by (intros; exact base_functions).
        assert (WriteCode : reduce_trans
          (state, coupon_output_store 0 0, Build_frame before G.E.collector_instance,
            v_to_e_list [word 0; ext code] ++ [AI_basic (BI_table_set 1%N)])
          (state, coupon_output_store 1 0, Build_frame before G.E.collector_instance, [])).
        { apply rt_step, r_table_set_success; reflexivity. }
        assert (WriteValue : reduce_trans
          (state, coupon_output_store 1 0, Build_frame before G.E.collector_instance,
            v_to_e_list [word 1; ext value] ++ [AI_basic (BI_table_set 1%N)])
          (state, coupon_output_store 2 0, Build_frame before G.E.collector_instance, [])).
        { apply rt_step, r_table_set_success; reflexivity. }
        assert (WriteResult : reduce_trans
          (state, coupon_output_store 2 0, Build_frame before G.E.collector_instance,
            v_to_e_list [word 2; ext result] ++ [AI_basic (BI_table_set 1%N)])
          (state, coupon_output_store 3 0, Build_frame before G.E.collector_instance, [])).
        { apply rt_step, r_table_set_success; reflexivity. }
        rewrite evaluation_output_body.
        execute_under [word 0] (local_get_run state (coupon_output_store 0 0) before 3%N
          (VAL_num (VAL_int64 (wasm_nat64 kind))) eq_refl).
        execute_under [word 0] (collector_create_integer_executes state (coupon_output_store 0 0) before kind
          (Functions 0 0) kind_bound).
        execute WriteCode.
        execute_under [word 1] (local_get_run state (coupon_output_store 1 0) before 1%N (ext value) eq_refl).
        execute WriteValue.
        execute_under [word 2] (local_get_run state (coupon_output_store 2 0) before 2%N (ext result) eq_refl).
        execute WriteResult.
        assert (ClaimTree : reduce_trans
          (state, coupon_output_store 3 0, Build_frame before G.E.collector_instance,
            v_to_e_list [word 1; word 3] ++ [AI_basic (BI_call 10%N)])
          (state, coupon_output_store 3 0, Build_frame before G.E.collector_instance, v_to_e_list [ext (treeh claim)])).
        { eapply collector_create_tree_executes with (index := wasm_nat 1) (values := claim_fields)
            (table := Build_tableinst assertion_type (G.E.H.Tables.copied_entries [] claim_fields 3)) (unused := []);
            [exact (Functions 3 0)|reflexivity|exact assertion_element|reflexivity|vm_compute; congruence]. }
        execute ClaimTree.
        assert (RefRun : reduce_trans
          (state, coupon_output_store 3 0, Build_frame before G.E.collector_instance,
            v_to_e_list [ext (treeh claim)] ++ [AI_basic (BI_call 12%N)])
          (state, coupon_output_store 3 0, Build_frame before G.E.collector_instance, v_to_e_list [ext reference])).
        { eapply G.E.collector_import_executes with (operation := shell_create_ref)
            (arguments := [ext (treeh claim)]) (result := result_values [ext reference]); try reflexivity.
          - rewrite (Functions 3 0); reflexivity.
          - apply G.E.H.host_values_result; unfold G.E.H.host_values, ext.
            rewrite to_handle_to_externref; reflexivity. }
        execute RefRun.
        execute (local_set_run state (coupon_output_store 3 0) before 2%N (ext reference) ltac:(cbn; lia)).
        assert (SelectedView : stab_elem (coupon_output_store 3 0) G.E.collector_instance
            2%N 1%N = Some (G.E.H.Tables.externref issuing_author)).
        { rewrite <- selected_author_view.
          change (match lookup_N (skipn 2 (s_tables base)) 0%N with
            | Some table => lookup_N (tableinst_elem table) 1%N | None => None end =
            match lookup_N (s_tables base) 2%N with
            | Some table => lookup_N (tableinst_elem table) 1%N | None => None end).
          destruct (s_tables base) as [|first [|second rest]]; reflexivity. }
        execute_under [word 0] (table_get_run state (coupon_output_store 3 0) after
          2%N (wasm_nat 1) (G.E.H.Tables.externref issuing_author) SelectedView).
        assert (WriteAuthor : reduce_trans
          (state, coupon_output_store 3 0, Build_frame after G.E.collector_instance,
            v_to_e_list [word 0; ext issuing_author] ++ [AI_basic (BI_table_set 0%N)])
          (state, coupon_output_store 3 1, Build_frame after G.E.collector_instance, [])).
        { apply rt_step, r_table_set_success; reflexivity. }
        execute WriteAuthor.
        execute_under [word 1] (local_get_run state (coupon_output_store 3 1) after 2%N (ext reference) eq_refl).
        assert (WriteReference : reduce_trans
          (state, coupon_output_store 3 1, Build_frame after G.E.collector_instance,
            v_to_e_list [word 1; ext reference] ++ [AI_basic (BI_table_set 0%N)])
          (state, coupon_output_store 3 2, Build_frame after G.E.collector_instance, [])).
        { apply rt_step, r_table_set_success; reflexivity. }
        execute WriteReference.
        eapply collector_create_tag_executes with (index := wasm_nat 0)
          (table := Build_tableinst output_type (G.E.H.Tables.copied_entries [] output_fields 2))
          (body := [reference]) (issuing_author := issuing_author);
          [exact (Functions 3 2)|reflexivity|exact output_element|reflexivity|exact selected_author].
      Qed.

      Theorem collector_output_encodes claim_kind :
        Claims.Code.statement_kind kind = Some claim_kind ->
        X.Wire.encode claim_kind issuing_author value result =
          Some (treeh (create_tree [treeh tag; treeh claim])).
      Proof.
        intro Kind; unfold X.Wire.encode.
        change (X.Wire.Code.statement_kind kind = Some claim_kind) in Kind.
        rewrite (X.Wire.Code.statement_kind_roundtrip _ _ Kind).
        cbn [X.Wire.H.omap].
        cbv beta iota zeta delta [X.Wire.witness tag claim claim_fields reference code]; reflexivity.
      Qed.
    End OutputConstruction.

    Definition issued_claim value result kind :=
      create_tree [blobh (create_blob (from_nat kind)); value; result].
    Definition issued_tag issuing_author value result kind :=
      create_tree_tagged true
        [issuing_author; Data (Ref (TreeRef (issued_claim value result kind)))].

    Lemma collector_issue_executes base output_type assertion_type
        value result issuing_author kind state caller :
      (Z.of_nat kind < 2 ^ 64)%Z -> s_funcs base = s_funcs G.E.allocated_store ->
      tt_elem_type output_type = T_externref -> tt_elem_type assertion_type = T_externref ->
      G.E.H.Tables.T.A.author_at 1 (treeh combination) = Some issuing_author ->
      stab_elem base G.E.collector_instance 2%N 1%N = Some (G.E.H.Tables.externref issuing_author) ->
      reduce_trans
        (state, coupon_output_store base output_type assertion_type value result issuing_author kind 0 0,
          Build_frame caller G.E.collector_instance,
          v_to_e_list [ext (treeh combination); ext value; ext result; VAL_num (VAL_int64 (wasm_nat64 kind))] ++
            [AI_basic (BI_call 41%N)])
        (state, coupon_output_store base output_type assertion_type value result issuing_author kind 3 2,
          Build_frame caller G.E.collector_instance,
          if is_comparable issuing_author then
            v_to_e_list [ext (treeh (issued_tag issuing_author value result kind))] else [AI_trap]).
    Proof.
      intros Kind Functions OutputElement AssertionElement Author AuthorView.
      set (arguments := [ext (treeh combination); ext value; ext result; VAL_num (VAL_int64 (wasm_nat64 kind))]).
      eapply G.E.collector_call with (code := native_issue)
        (parameters := [T_ref T_externref; T_ref T_externref; T_ref T_externref; T_num T_i64])
        (results := [T_ref T_externref]) (address := 41%N) (defaults := []) (arguments := arguments)
        (values := [ext (treeh (issued_tag issuing_author value result kind))]) (accepted := is_comparable issuing_author)
        (final_frame := Build_frame
          [ext (treeh combination); ext value; ext (Data (Ref (TreeRef (issued_claim value result kind))));
           VAL_num (VAL_int64 (wasm_nat64 kind))] G.E.collector_instance); try reflexivity.
      - change (lookup_N (s_funcs base) 41%N = Some (FC_func_native
          (Tf [T_ref T_externref; T_ref T_externref; T_ref T_externref; T_num T_i64] [T_ref T_externref])
          G.E.collector_instance native_issue)); rewrite Functions; reflexivity.
      - apply collector_output_body_executes; assumption.
    Qed.

    Lemma collector_output_executes base output_type assertion_type
        value result issuing_author kind state :
      (Z.of_nat kind < 2 ^ 64)%Z -> s_funcs base = s_funcs G.E.allocated_store ->
      tt_elem_type output_type = T_externref -> tt_elem_type assertion_type = T_externref ->
      G.E.H.Tables.T.A.author_at 1 (treeh combination) = Some issuing_author ->
      stab_elem base G.E.collector_instance 2%N 1%N = Some (G.E.H.Tables.externref issuing_author) ->
      reduce_trans
        (state, coupon_output_store base output_type assertion_type value result issuing_author kind 0 0,
          Build_frame [ext (treeh combination); ext value; ext result; VAL_num (VAL_int64 (wasm_nat64 kind))]
            G.E.collector_instance, to_e_list (skipn 18 (modfunc_body evaluation_apply)))
        (state, coupon_output_store base output_type assertion_type value result issuing_author kind 3 2,
          Build_frame [ext (treeh combination); ext value; ext result; VAL_num (VAL_int64 (wasm_nat64 kind))]
            G.E.collector_instance, if is_comparable issuing_author then
            v_to_e_list [ext (treeh (issued_tag issuing_author value result kind))] else [AI_trap]).
    Proof.
      intros Kind Functions OutputElement AssertionElement Author AuthorView.
      cbn [evaluation_apply modfunc_body mod_funcs evaluation_module nth skipn to_e_list seq.map].
      do 4 native_pure.
      apply collector_issue_executes; assumption.
    Qed.
  End Execution.
End WasmOutput.
