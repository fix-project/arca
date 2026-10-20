(** Proves that native output construction issues the asserted claim with
    the accumulated premise authors and the collector issuing identity. *)

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
  Local Notation local_get_run := (G.E.collector_local_get combination true handle_bytes).

  Section Execution.
    Section OutputConstruction.
      Variable base : store_record.
      Variable author_values : list handle.
      Variable author_type output_type assertion_type : table_type.
      Variable value result issuing_author : handle.
      Variable kind : nat.
      (** The output statement code fits in an unsigned 64-bit integer. *)
      Hypothesis kind_bound : (Z.of_nat kind < 2 ^ 64)%Z.
      (** The starting store contains exactly the collector's expected functions. *)
      Hypothesis base_functions : s_funcs base = s_funcs G.E.allocated_store.
      (** The author table stores externrefs. *)
      Hypothesis author_element : tt_elem_type author_type = T_externref.
      (** The output table stores externrefs. *)
      Hypothesis output_element : tt_elem_type output_type = T_externref.
      (** The assertion table stores externrefs. *)
      Hypothesis assertion_element : tt_elem_type assertion_type = T_externref.
      (** The accumulated author list fits in the unsigned 32-bit table-index range. *)
      Hypothesis author_bound : (Z.of_nat (List.length author_values) < 2 ^ 32)%Z.
      (** The combination's callable field selects the intended issuing author. *)
      Hypothesis selected_author : G.E.H.Tables.T.A.author_at 1 (treeh combination) = Some issuing_author.

      Let code := blobh (create_blob (from_nat kind)).
      Let claim_fields := [code; value; result].
      Let claim := create_tree claim_fields.
      Let reference := Data (Ref (TreeRef claim)).
      Let authors := create_tree author_values.
      Let output_fields := [treeh authors; reference].
      Let tag := create_tree_tagged true [treeh (G.E.H.Tables.T.add_author authors issuing_author); reference].

      Definition coupon_output_store assertion_index output_index := upd_s_table base
        [Build_tableinst author_type (map G.E.H.Tables.externref author_values);
         Build_tableinst output_type (G.E.H.Tables.copied_entries [] output_fields output_index);
         Build_tableinst assertion_type (G.E.H.Tables.copied_entries [] claim_fields assertion_index)].

      (** The native output suffix constructs and returns the Ref-backed certificate Tag
          using the accumulated authors. *)
      Lemma collector_output_executes state :
        reduce_trans
          (state, coupon_output_store 0 0,
            Build_frame [ext (treeh combination); ext value; ext result; VAL_num (VAL_int64 (wasm_nat64 kind))] G.E.collector_instance,
            to_e_list (skipn 22 (modfunc_body evaluation_apply)))
          (state, coupon_output_store 3 2,
            Build_frame [ext (treeh combination); ext value; ext reference; VAL_num (VAL_int64 (wasm_nat64 kind))] G.E.collector_instance,
            v_to_e_list [ext (treeh tag)]).
      Proof.
        set (before := [ext (treeh combination); ext value; ext result; VAL_num (VAL_int64 (wasm_nat64 kind))]).
        set (after := [ext (treeh combination); ext value; ext reference; VAL_num (VAL_int64 (wasm_nat64 kind))]).
        assert (Functions : forall i j, s_funcs (coupon_output_store i j) = s_funcs G.E.allocated_store)
          by (intros; exact base_functions).
        assert (WriteCode : reduce_trans
          (state, coupon_output_store 0 0, Build_frame before G.E.collector_instance,
            v_to_e_list [word 0; ext code] ++ [AI_basic (BI_table_set 2%N)])
          (state, coupon_output_store 1 0, Build_frame before G.E.collector_instance, [])).
        { apply rt_step, r_table_set_success; reflexivity. }
        assert (WriteValue : reduce_trans
          (state, coupon_output_store 1 0, Build_frame before G.E.collector_instance,
            v_to_e_list [word 1; ext value] ++ [AI_basic (BI_table_set 2%N)])
          (state, coupon_output_store 2 0, Build_frame before G.E.collector_instance, [])).
        { apply rt_step, r_table_set_success; reflexivity. }
        assert (WriteResult : reduce_trans
          (state, coupon_output_store 2 0, Build_frame before G.E.collector_instance,
            v_to_e_list [word 2; ext result] ++ [AI_basic (BI_table_set 2%N)])
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
            v_to_e_list [word 2; word 3] ++ [AI_basic (BI_call 9%N)])
          (state, coupon_output_store 3 0, Build_frame before G.E.collector_instance, v_to_e_list [ext (treeh claim)])).
        { eapply collector_create_tree_executes with (index := wasm_nat 2) (values := claim_fields)
            (table := Build_tableinst assertion_type (G.E.H.Tables.copied_entries [] claim_fields 3)) (unused := []);
            [exact (Functions 3 0)|reflexivity|exact assertion_element|reflexivity|vm_compute; congruence]. }
        execute ClaimTree.
        assert (RefRun : reduce_trans
          (state, coupon_output_store 3 0, Build_frame before G.E.collector_instance,
            v_to_e_list [ext (treeh claim)] ++ [AI_basic (BI_call 11%N)])
          (state, coupon_output_store 3 0, Build_frame before G.E.collector_instance, v_to_e_list [ext reference])).
        { eapply G.E.collector_import_executes with (operation := shell_create_ref)
            (arguments := [ext (treeh claim)]) (result := result_values [ext reference]); try reflexivity.
          - rewrite (Functions 3 0); reflexivity.
          - apply G.E.H.host_values_result; unfold G.E.H.host_values, ext.
            rewrite to_handle_to_externref; reflexivity. }
        execute RefRun.
        execute (local_set_run state (coupon_output_store 3 0) before 2%N (ext reference) ltac:(cbn; lia)).
        assert (Size : reduce_trans
          (state, coupon_output_store 3 0, Build_frame after G.E.collector_instance, [AI_basic (BI_table_size 0%N)])
          (state, coupon_output_store 3 0, Build_frame after G.E.collector_instance,
            v_to_e_list [word (List.length author_values)])).
        { apply rt_step; eapply r_table_size with
            (tab := Build_tableinst author_type (map G.E.H.Tables.externref author_values))
            (sz := List.length author_values); [reflexivity|].
          unfold tab_size; cbn; rewrite List.length_map; reflexivity. }
        execute_under [word 0; word 0] Size.
        assert (AuthorsTree : reduce_trans
          (state, coupon_output_store 3 0, Build_frame after G.E.collector_instance,
            v_to_e_list [word 0; word (List.length author_values)] ++ [AI_basic (BI_call 9%N)])
          (state, coupon_output_store 3 0, Build_frame after G.E.collector_instance, v_to_e_list [ext (treeh authors)])).
        { eapply collector_create_tree_executes with (index := wasm_nat 0) (values := author_values)
            (table := Build_tableinst author_type (map G.E.H.Tables.externref author_values)) (unused := []);
            [exact (Functions 3 0)|reflexivity|exact author_element|rewrite List.app_nil_r; reflexivity|exact author_bound]. }
        execute_under [word 0] AuthorsTree.
        assert (WriteAuthors : reduce_trans
          (state, coupon_output_store 3 0, Build_frame after G.E.collector_instance,
            v_to_e_list [word 0; ext (treeh authors)] ++ [AI_basic (BI_table_set 1%N)])
          (state, coupon_output_store 3 1, Build_frame after G.E.collector_instance, [])).
        { apply rt_step, r_table_set_success; reflexivity. }
        execute WriteAuthors.
        execute_under [word 1] (local_get_run state (coupon_output_store 3 1) after 2%N (ext reference) eq_refl).
        assert (WriteReference : reduce_trans
          (state, coupon_output_store 3 1, Build_frame after G.E.collector_instance,
            v_to_e_list [word 1; ext reference] ++ [AI_basic (BI_table_set 1%N)])
          (state, coupon_output_store 3 2, Build_frame after G.E.collector_instance, [])).
        { apply rt_step, r_table_set_success; reflexivity. }
        execute WriteReference.
        eapply collector_create_tag_executes with (index := wasm_nat 1) (depth := 1)
          (table := Build_tableinst output_type (G.E.H.Tables.copied_entries [] output_fields 2))
          (authors := authors) (body := [reference]) (issuing_author := issuing_author);
          [exact (Functions 3 2)|reflexivity|exact output_element|reflexivity|
           vm_compute; congruence|exact selected_author].
      Qed.

      (** The native output Tag and assertion witness are exactly the result of the
          abstract certificate issuer. *)
      Theorem collector_output_issues claim_kind used :
        Claims.Code.statement_kind kind = Some claim_kind ->
        author_values = X.dependencies used ->
        X.issue issuing_author claim_kind value result used =
          Some (treeh (create_tree [treeh tag; treeh claim])).
      Proof.
        intros Kind Authorities; unfold X.issue, X.Wire.encode.
        change (X.Wire.Code.statement_kind kind = Some claim_kind) in Kind.
        rewrite (X.Wire.Code.statement_kind_roundtrip _ _ Kind).
        cbn [X.Wire.H.omap]; subst author_values.
        cbv beta iota zeta delta [X.Wire.witness X.output_authors tag claim claim_fields authors reference code]; reflexivity.
      Qed.

    End OutputConstruction.
  End Execution.
End WasmOutput.
