(** Interprets Fix Shell imports as WasmCert host operations and proves
    that their results preserve the required typing and store-extension laws. *)

From Wasm Require Import datatypes operations typing host properties type_preservation.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Shell Require Import Operations Externref ABI Memory Tables.
From Stdlib Require Import List String Arith NArith ZArith Bool Lia.
Import ListNotations.
Local Open Scope string_scope.
Local Open Scope list_scope.

Module ShellHost (S : STORAGE) (P : PROGRAM S) (R : EXTERNREFS S).
  Module W := Externrefs S R.
  Module Tables := ShellTables S P R.
  Import S W.H R.

  Definition word_result value := Some [VAL_num (VAL_int32 (W.i32_of_nat value))].
  Definition length_result value :=
    if Z.ltb (Z.of_nat value) (2 ^ 64)%Z then
      Some [VAL_num (VAL_int64 (Wasm_int.int_of_Z i64m (Z.of_nat value)))] else None.
  (** A length fitting in 64 bits is returned as a Wasm i64. *)
  Lemma length_result_small value : (Z.of_nat value < 2 ^ 64)%Z ->
    length_result value = Some [VAL_num (VAL_int64 (Wasm_int.int_of_Z i64m (Z.of_nat value)))].
  Proof.
    intro Bound; unfold length_result.
    assert (Fits : Z.ltb (Z.of_nat value) (2 ^ 64)%Z = true) by (apply Z.ltb_lt; exact Bound).
    rewrite Fits; reflexivity.
  Qed.
  Definition data_type value := match value with
    | Data (Object (BlobObj _)) | Data (Ref (BlobRef _)) => Some 0
    | Data (Object (TreeObj _)) | Data (Ref (TreeRef _)) => Some 1
    | _ => None end.
  Definition encode_type value := match value with
    | Encode (Strict _) => Some 0 | Encode (Shallow _) => Some 1
    | _ => None end.
  Definition thunk_type value := match value with
    | Thunk (Application _) | Encode (Strict (Application _)) | Encode (Shallow (Application _)) => Some 0
    | Thunk (Selection _) | Encode (Strict (Selection _)) | Encode (Shallow (Selection _)) => Some 1
    | Thunk (Identification _) | Encode (Strict (Identification _)) | Encode (Shallow (Identification _)) => Some 2
    | _ => None end.
  Definition reference value := match value with
    | Data (Object object) => Some (Data (Ref (data_reference (Object object))))
    | _ => None end.
  Definition tree_thunk constructor value := match value with
    | Data (Object (TreeObj node)) | Data (Ref (TreeRef node)) => Some (Thunk (constructor node))
    | _ => None end.

  Section Calls.
    Context `{mem : BlockUpdateMemory}.
    Variable combination : TreeName.
    Variable instance : moduleinst.
    Variable privileged : bool.
    Variable trusted_tagger : bool.
    Variable handle_bytes : nat -> handle -> option BlobName.

    Definition host_values state (store : store_record) operation arguments :=
      match operation, arguments with
      | shell_get_type, [VAL_ref (VAL_ref_extern address)] =>
        word_result (value_type (to_handle address))
      | shell_get_data_type, [VAL_ref (VAL_ref_extern address)] =>
        obind (data_type (to_handle address)) word_result
      | shell_get_encode_type, [VAL_ref (VAL_ref_extern address)] =>
        obind (encode_type (to_handle address)) word_result
      | shell_get_thunk_type, [VAL_ref (VAL_ref_extern address)] =>
        if privileged then obind (thunk_type (to_handle address)) word_result else None
      | shell_is_tag, [VAL_ref (VAL_ref_extern address)] => W.bool_result (W.Api.is_tag (to_handle address))
      | shell_same_handle_bytes, [VAL_ref (VAL_ref_extern lhs); VAL_ref (VAL_ref_extern rhs)] =>
        if privileged then
          obind (handle_bytes state (to_handle lhs)) (fun left =>
            obind (handle_bytes (Datatypes.S state) (to_handle rhs)) (fun right =>
              obind (Tables.T.O.compare_handles (HBlobObj left) (HBlobObj right)) W.bool_result))
        else None
      | shell_equals, [VAL_ref (VAL_ref_extern lhs); VAL_ref (VAL_ref_extern rhs)] =>
        obind (Tables.T.O.compare_handles (to_handle lhs) (to_handle rhs)) W.bool_result
      | shell_len, [VAL_ref (VAL_ref_extern address)] =>
        match to_handle address with
        | Data data => length_result (Tables.T.O.data_length data) | _ => None end
      | shell_get_tree_element, [VAL_ref (VAL_ref_extern address); VAL_num (VAL_int32 index)] =>
        W.handle_result (W.Api.get_tree_data_api (to_handle address) (W.nat_of_i32 index))
      | shell_create_tree, [VAL_num (VAL_int32 index); VAL_num (VAL_int32 length)] =>
        W.handle_result (omap HTreeObj (Tables.create_tree_from_table store instance
          (Wasm_int.N_of_uint i32m index) (W.nat_of_i32 length)))
      | shell_create_blob_i64, [VAL_num (VAL_int64 value)] =>
        Some [W.extern_value (HBlobObj (create_blob (serialise_i64 value)))]
      | shell_create_ref, [VAL_ref (VAL_ref_extern address)] =>
        W.handle_result (reference (to_handle address))
      | shell_create_tag, [VAL_num (VAL_int32 index)] =>
        W.handle_result (omap HTreeObj (Tables.create_tag_from_table combination store instance
          (Wasm_int.N_of_uint i32m index)))
      | shell_create_tag_unchecked, [VAL_ref (VAL_ref_extern address)] =>
        if privileged && trusted_tagger then
          match to_handle address with
          | Data (Object (TreeObj fields)) =>
            match get_tree_raw fields with
            | _ :: _ =>
              Some [W.extern_value (HTreeObj (create_tree_tagged true (get_tree_raw fields)))]
            | _ => None end
          | _ => None end
        else None
      | shell_application, [VAL_ref (VAL_ref_extern address)] =>
        W.handle_result (tree_thunk Application (to_handle address))
      | shell_selection, [VAL_ref (VAL_ref_extern address)] =>
        W.handle_result (tree_thunk Selection (to_handle address))
      | shell_identification, [VAL_ref (VAL_ref_extern address)] =>
        W.handle_result (W.Api.create_identification_thunk_api (to_handle address))
      | shell_strict, [VAL_ref (VAL_ref_extern address)] =>
        W.handle_result (W.Api.create_strict_encode_api (to_handle address))
      | shell_shallow, [VAL_ref (VAL_ref_extern address)] =>
        W.handle_result (W.Api.create_shallow_encode_api (to_handle address))
      | _, _ => None
      end.

    Lemma unchecked_tag_requires_authority state store arguments :
      privileged = false \/ trusted_tagger = false ->
      host_values state store shell_create_tag_unchecked arguments = None.
    Proof.
      unfold host_values; intros Denied; destruct privileged, trusted_tagger;
        try (destruct Denied; discriminate); cbn;
        repeat match goal with
        | |- context [match ?value with _ => _ end] => destruct value
        end; reflexivity.
    Qed.

    Lemma unchecked_tag_preserves_fields state store fields author body :
      privileged = true -> trusted_tagger = true ->
      get_tree_raw fields = author :: body ->
      host_values state store shell_create_tag_unchecked [W.extern_value (HTreeObj fields)] =
        Some [W.extern_value (HTreeObj (create_tree_tagged true (author :: body)))].
    Proof.
      intros Privileged Trusted Fields; unfold host_values, W.extern_value.
      rewrite Privileged, Trusted, to_handle_to_externref; cbn; rewrite Fields; reflexivity.
    Qed.

    Lemma thunk_type_requires_privilege state store address :
      privileged = false ->
      host_values state store shell_get_thunk_type [VAL_ref (VAL_ref_extern address)] = None.
    Proof. intro Denied; unfold host_values; rewrite Denied; reflexivity. Qed.

    (** Successful pure Shell operations return values of their declared Wasm result types. *)
    Lemma host_values_typing state store operation arguments values :
      host_values state store operation arguments = Some values ->
      (match shell_signature operation with Tf _ results => values_typing store values results end) = true.
    Proof.
      intro Result; destruct operation;
        cbv beta iota zeta delta [host_values word_result length_result W.bool_result W.handle_result W.H.omap W.Api.H.omap
          obind omap option_map] in Result;
        repeat (cbv beta iota zeta delta [host_values word_result length_result W.bool_result W.handle_result W.H.omap W.Api.H.omap
            obind omap option_map] in Result;
          match type of Result with context [match ?value with _ => _ end] => destruct value eqn:? end);
        try discriminate; inversion Result; subst; reflexivity.
    Qed.

    Definition component_typed {A B} (typing : A -> option B) values :=
      forallb (fun value => match typing value with Some _ => true | None => false end) values.

    Lemma component_typed_correct {A B} (typing : A -> option B) values :
      component_typed typing values = true ->
      List.Forall (fun value => exists typ, typing value = Some typ) values.
    Proof.
      intro Typed; apply List.Forall_forall; intros value Present.
      unfold component_typed in Typed; rewrite List.forallb_forall in Typed.
      specialize (Typed value Present).
      destruct (typing value) eqn:TypeOf; [eauto|discriminate].
    Qed.

    Lemma component_typed_complete {A B} (typing : A -> option B) values :
      List.Forall (fun value => exists typ, typing value = Some typ) values ->
      component_typed typing values = true.
    Proof.
      intro Typed; unfold component_typed; apply List.forallb_forall.
      intros value Present; rewrite List.Forall_forall in Typed.
      destruct (Typed value Present) as [typ TypeOf]; rewrite TypeOf; reflexivity.
    Qed.

    (** The supported host fragment retains WasmCert's monotone resource limits. *)
    Definition supported_update (store : store_record) tables memories :=
      let next := upd_s_mem (upd_s_table store tables) memories in
      if (store_extension store next &&
          (component_typed (tableinst_typing next) tables &&
           component_typed (meminst_typing next) memories))%bool
      then Some next else None.

    Lemma supported_update_shape store tables memories next :
      supported_update store tables memories = Some next ->
      next = upd_s_mem (upd_s_table store tables) memories.
    Proof.
      unfold supported_update; destruct (_ && _)%bool;
        intro Updated; inversion Updated; reflexivity.
    Qed.

    Lemma supported_update_functions store tables memories next :
      supported_update store tables memories = Some next -> s_funcs next = s_funcs store.
    Proof. intro Updated; rewrite (supported_update_shape _ _ _ _ Updated); reflexivity. Qed.

    Lemma supported_update_correct store tables memories next :
      supported_update store tables memories = Some next ->
      store_extension store next = true /\
      (store_typing store -> store_typing next).
    Proof.
      unfold supported_update.
      destruct (store_extension store (upd_s_mem (upd_s_table store tables) memories) &&
        (component_typed (tableinst_typing (upd_s_mem (upd_s_table store tables) memories)) tables &&
         component_typed (meminst_typing (upd_s_mem (upd_s_table store tables) memories)) memories))%bool
        eqn:Allowed; [|discriminate].
      intro Updated; inversion Updated; subst next.
      apply Bool.andb_true_iff in Allowed as [Extension Components].
      apply Bool.andb_true_iff in Components as [Tables Memories].
      split; [exact Extension|].
      intro Typed; destruct store as [functions oldtables oldmemories globals elements data].
      destruct Typed as [Functions [OldTables [OldMemories [Globals [Elements Data]]]]].
      cbn [upd_s_mem upd_s_table store_typing].
      repeat split.
      - eapply store_extension_funcs_typing; eassumption.
      - apply component_typed_correct; exact Tables.
      - apply component_typed_correct; exact Memories.
      - eapply store_extension_globals_typing; eassumption.
      - eapply store_extension_elems_typing; eassumption.
      - eapply store_extension_datas_typing; eassumption.
    Qed.

    Lemma supported_update_accept store tables memories :
      store_extension store (upd_s_mem (upd_s_table store tables) memories) = true ->
      store_typing (upd_s_mem (upd_s_table store tables) memories) ->
      supported_update store tables memories =
        Some (upd_s_mem (upd_s_table store tables) memories).
    Proof.
      intros Extension Typed.
      change (store_typing (upd_s_mem (upd_s_table store tables) memories)) in Typed.
      destruct store as [functions oldtables oldmemories globals elements data].
      destruct Typed as [Functions [Tables [Memories [Globals [Elements Data]]]]].
      cbn in Tables, Memories.
      unfold supported_update; rewrite Extension.
      rewrite (component_typed_complete _ _ Tables), (component_typed_complete _ _ Memories).
      reflexivity.
    Qed.

    Definition replace_resource {A} (values : list A) index before after :=
      seq.set_nth before values (N.to_nat index) after.

    Lemma supported_table_replacement store address before after typ :
      lookup_N (s_tables store) address = Some before ->
      let tables := replace_resource (s_tables store) address before after in
      let next := upd_s_mem (upd_s_table store tables) (s_mems store) in
      store_typing store -> store_extension store next = true ->
      tableinst_typing next after = Some typ ->
      supported_update store tables (s_mems store) = Some next.
    Proof.
      intros Present tables next Typed Extension After.
      apply supported_update_accept; [exact Extension|].
      destruct store as [functions oldtables memories globals elements data].
      destruct Typed as [Functions [Tables [Memories [Globals [Elements Data]]]]].
      cbn [store_typing upd_s_mem upd_s_table]; repeat split.
      - eapply store_extension_funcs_typing; eassumption.
      - unfold tables, replace_resource; eapply Forall_set.
        + eapply store_extension_tables_typing; eassumption.
        + exists typ; exact After.
        + apply (ssrbool.introT ssrnat.ltP).
          apply (proj1 (List.nth_error_Some _ _)); unfold lookup_N in Present;
            rewrite Present; discriminate.
      - eapply store_extension_mems_typing; eassumption.
      - eapply store_extension_globals_typing; eassumption.
      - eapply store_extension_elems_typing; eassumption.
      - eapply store_extension_datas_typing; eassumption.
    Qed.

    Lemma supported_memory_replacement store address before after typ :
      lookup_N (s_mems store) address = Some before ->
      let memories := replace_resource (s_mems store) address before after in
      let next := upd_s_mem (upd_s_table store (s_tables store)) memories in
      store_typing store -> store_extension store next = true ->
      meminst_typing next after = Some typ ->
      supported_update store (s_tables store) memories = Some next.
    Proof.
      intros Present memories next Typed Extension After.
      apply supported_update_accept; [exact Extension|].
      destruct store as [functions tables oldmemories globals elements data].
      destruct Typed as [Functions [Tables [Memories [Globals [Elements Data]]]]].
      cbn [store_typing upd_s_mem upd_s_table]; repeat split.
      - eapply store_extension_funcs_typing; eassumption.
      - eapply store_extension_tables_typing; eassumption.
      - unfold memories, replace_resource; eapply Forall_set.
        + eapply store_extension_mems_typing; eassumption.
        + exists typ; exact After.
        + apply (ssrbool.introT ssrnat.ltP).
          apply (proj1 (List.nth_error_Some _ _)); unfold lookup_N in Present;
            rewrite Present; discriminate.
      - eapply store_extension_globals_typing; eassumption.
      - eapply store_extension_elems_typing; eassumption.
      - eapply store_extension_datas_typing; eassumption.
    Qed.

    Definition tree_attachment_result (store : store_record) arguments :=
      match arguments with
      | [VAL_ref (VAL_ref_extern address); VAL_num (VAL_int32 index)] =>
        match to_handle address, lookup_N (inst_tables instance) (Wasm_int.N_of_uint i32m index) with
        | Data (Object (TreeObj node)), Some table_address =>
          if N.leb (N.of_nat (List.length (get_tree_raw node))) native_table_limit then
          obind (lookup_N (s_tables store) table_address) (fun before =>
            let values := get_tree_raw node in
            let after := Tables.tree_attachment before values in
            omap (fun next => (next, result_values []))
              (supported_update store
                (replace_resource (s_tables store) table_address before after) (s_mems store)))
          else None
        | _, _ => None end
      | _ => None end.

    Definition blob_read_result (store : store_record) arguments :=
      match arguments with
      | [VAL_ref (VAL_ref_extern address); VAL_num (VAL_int32 offset);
         VAL_num (VAL_int32 destination); VAL_num (VAL_int32 length)] =>
        match to_handle address, lookup_N (inst_mems instance) 0%N with
        | Data (Object (BlobObj blob)), Some memory_address =>
          let bytes := get_blob_data blob in
          if Nat.leb (W.nat_of_i32 offset) (List.length bytes) then
            let copied := firstn (W.nat_of_i32 length) (skipn (W.nat_of_i32 offset) bytes) in
            obind (lookup_N (s_mems store) memory_address) (fun before =>
              obind (write_bytes_meminst before (Wasm_int.N_of_uint i32m destination) copied) (fun after =>
                omap (fun next => (next, result_values
                  [VAL_num (VAL_int32 (W.i32_of_nat (List.length copied)))]))
                  (supported_update store (s_tables store)
                    (replace_resource (s_mems store) memory_address before after))))
          else None
        | _, _ => None end
      | _ => None end.

    Lemma tree_attachment_result_success store node index address before next :
      (N.of_nat (List.length (get_tree_raw node)) <= native_table_limit)%N ->
      lookup_N (inst_tables instance) (Wasm_int.N_of_uint i32m index) = Some address ->
      lookup_N (s_tables store) address = Some before ->
      supported_update store
        (replace_resource (s_tables store) address before
          (Tables.tree_attachment before (get_tree_raw node))) (s_mems store) = Some next ->
      tree_attachment_result store [W.extern_value (HTreeObj node); VAL_num (VAL_int32 index)] =
        Some (next, result_values []).
    Proof.
      intros NativeBound Address Before Supported; unfold tree_attachment_result, W.extern_value.
      rewrite to_handle_to_externref, Address; cbn [HTreeObj].
      assert (Allowed : N.leb (N.of_nat (List.length (get_tree_raw node))) native_table_limit = true)
        by (apply N.leb_le; exact NativeBound).
      rewrite Allowed; cbn [obind].
      rewrite Before.
      change (omap (fun updated => (updated, result_values []))
        (supported_update store
          (replace_resource (s_tables store) address before
            (Tables.tree_attachment before (get_tree_raw node))) (s_mems store)) =
        Some (next, result_values [])).
      rewrite Supported; reflexivity.
    Qed.

    Lemma tree_attachment_result_bound store node index next result :
      tree_attachment_result store [W.extern_value (HTreeObj node); VAL_num (VAL_int32 index)] =
        Some (next, result) ->
      (N.of_nat (List.length (get_tree_raw node)) <= native_table_limit)%N.
    Proof.
      unfold tree_attachment_result, W.extern_value.
      rewrite to_handle_to_externref; cbn [HTreeObj].
      destruct (lookup_N (inst_tables instance) (Wasm_int.N_of_uint i32m index)); [|discriminate].
      destruct (N.leb (N.of_nat (List.length (get_tree_raw node))) native_table_limit)
        eqn:Bound; intro Attached; [apply N.leb_le; exact Bound|discriminate].
    Qed.

    Lemma tree_attachment_result_oversized store node index :
      (native_table_limit < N.of_nat (List.length (get_tree_raw node)))%N ->
      tree_attachment_result store [W.extern_value (HTreeObj node); VAL_num (VAL_int32 index)] = None.
    Proof.
      intro Bound; unfold tree_attachment_result, W.extern_value.
      rewrite to_handle_to_externref; cbn [HTreeObj].
      destruct (lookup_N (inst_tables instance) (Wasm_int.N_of_uint i32m index)); [|reflexivity].
      assert (Denied : N.leb (N.of_nat (List.length (get_tree_raw node))) native_table_limit = false)
        by (apply N.leb_gt; exact Bound).
      rewrite Denied; reflexivity.
    Qed.

    Lemma tree_attachment_store_extension store address before values :
      lookup_N (s_tables store) address = Some before ->
      table_extension before (Tables.tree_attachment before values) = true ->
      store_extension store (upd_s_mem (upd_s_table store
        (replace_resource (s_tables store) address before (Tables.tree_attachment before values)))
        (s_mems store)) = true.
    Proof.
      intros Before Extension.
      change ((((((component_extension func_extension (s_funcs store) (s_funcs store) &&
        component_extension table_extension (s_tables store)
          (replace_resource (s_tables store) address before (Tables.tree_attachment before values))) &&
        component_extension mem_extension (s_mems store) (s_mems store)) &&
        component_extension global_extension (s_globals store) (s_globals store)) &&
        component_extension elem_extension (s_elems store) (s_elems store)) &&
        component_extension data_extension (s_datas store) (s_datas store))%bool = true).
      rewrite (component_extension_same_refl (s_funcs store) func_extension_refl),
        (component_extension_same_refl (s_mems store) mem_extension_refl),
        (component_extension_same_refl (s_globals store) global_extension_refl),
        (component_extension_same_refl (s_elems store) elem_extension_refl),
        (component_extension_same_refl (s_datas store) data_extension_refl).
      repeat rewrite Bool.andb_true_r; cbn.
      unfold replace_resource; apply component_extension_update with (x := before);
        [exact table_extension_refl|exact Before|exact Extension].
    Qed.

    Lemma tree_attachment_bounded store node index address before :
      lookup_N (inst_tables instance) (Wasm_int.N_of_uint i32m index) = Some address ->
      lookup_N (s_tables store) address = Some before -> store_typing store ->
      tt_elem_type (tableinst_type before) = T_externref ->
      lim_max (tt_limits (tableinst_type before)) = None ->
      (lim_min (tt_limits (tableinst_type before)) <= N.of_nat (List.length (get_tree_raw node)))%N ->
      tab_size before <= List.length (get_tree_raw node) ->
      (N.of_nat (List.length (get_tree_raw node)) <= native_table_limit)%N ->
      tree_attachment_result store [W.extern_value (HTreeObj node); VAL_num (VAL_int32 index)] =
        Some (upd_s_mem (upd_s_table store
          (replace_resource (s_tables store) address before
            (Tables.tree_attachment before (get_tree_raw node)))) (s_mems store), result_values []).
    Proof.
      intros Address Before Typed Element Maximum Minimum Size Bound.
      assert (CoreBound : (N.of_nat (List.length (get_tree_raw node)) <= table_limit_bound)%N).
      { eapply N.le_trans; [exact Bound|].
        unfold native_table_limit, table_limit_bound, u32_bound; vm_compute; congruence. }
      apply tree_attachment_result_success with (address := address) (before := before);
        [exact Bound|exact Address|exact Before|].
      eapply supported_table_replacement; [exact Before|exact Typed| |].
      - apply tree_attachment_store_extension; [exact Before|].
        apply Tables.tree_attachment_extension; assumption.
      - apply Tables.tree_attachment_typing; assumption.
    Qed.

    Lemma blob_read_result_success store blob offset destination length address before after next :
      lookup_N (inst_mems instance) 0%N = Some address ->
      lookup_N (s_mems store) address = Some before ->
      W.nat_of_i32 offset <= List.length (get_blob_data blob) ->
      write_bytes_meminst before (Wasm_int.N_of_uint i32m destination)
        (firstn (W.nat_of_i32 length) (skipn (W.nat_of_i32 offset) (get_blob_data blob))) = Some after ->
      supported_update store (s_tables store)
        (replace_resource (s_mems store) address before after) = Some next ->
      blob_read_result store [W.extern_value (HBlobObj blob); VAL_num (VAL_int32 offset);
        VAL_num (VAL_int32 destination); VAL_num (VAL_int32 length)] =
        Some (next, result_values [VAL_num (VAL_int32 (W.i32_of_nat
          (List.length (firstn (W.nat_of_i32 length)
            (skipn (W.nat_of_i32 offset) (get_blob_data blob))))))]).
    Proof.
      intros Address Before Offset Copied Supported; unfold blob_read_result, W.extern_value.
      rewrite to_handle_to_externref, Address.
      assert (Bound : Nat.leb (W.nat_of_i32 offset) (List.length (get_blob_data blob)) = true)
        by (apply Nat.leb_le; exact Offset).
      cbn [HBlobObj].
      rewrite Bound; cbn [obind]; rewrite Before; cbn [obind].
      rewrite Copied; cbn [obind]; rewrite Supported; reflexivity.
    Qed.

    Lemma blob_read_zero_bounded store blob before after :
      inst_mems instance = [0%N] -> s_mems store = [before] -> store_typing store ->
      List.length (get_blob_data blob) = 8 ->
      write_bytes_meminst before 0%N (get_blob_data blob) = Some after ->
      blob_read_result store [W.extern_value (HBlobObj blob);
        VAL_num (VAL_int32 (W.i32_of_nat 0)); VAL_num (VAL_int32 (W.i32_of_nat 0));
        VAL_num (VAL_int32 (W.i32_of_nat 8))] =
        Some (upd_s_mem store [after], result_values [VAL_num (VAL_int32 (W.i32_of_nat 8))]).
    Proof.
      intros Instance Memory Typed Width Copied.
      assert (Prefix : firstn 8 (get_blob_data blob) = get_blob_data blob).
      { apply firstn_all2; lia. }
      assert (Supported : supported_update store (s_tables store)
        (replace_resource (s_mems store) 0%N before after) = Some (upd_s_mem store [after])).
      { unfold replace_resource; rewrite Memory.
        change (supported_update store (s_tables store) [after] = Some (upd_s_mem store [after])).
        apply supported_update_accept.
        - change (store_extension store (upd_s_mem store [after]) = true).
          eapply blob_copy_store_extension; eassumption.
        - change (store_typing (upd_s_mem store [after])).
          eapply blob_copy_store_typing; eassumption. }
      pose proof (blob_read_result_success store blob (W.i32_of_nat 0) (W.i32_of_nat 0)
        (W.i32_of_nat 8) 0%N before after (upd_s_mem store [after])) as Read.
      change (lookup_N (inst_mems instance) 0%N = Some 0%N ->
        lookup_N (s_mems store) 0%N = Some before ->
        0 <= List.length (get_blob_data blob) ->
        write_bytes_meminst before 0%N (firstn 8 (get_blob_data blob)) = Some after ->
        supported_update store (s_tables store) (replace_resource (s_mems store) 0%N before after) =
          Some (upd_s_mem store [after]) ->
        blob_read_result store [W.extern_value (HBlobObj blob);
          VAL_num (VAL_int32 (W.i32_of_nat 0)); VAL_num (VAL_int32 (W.i32_of_nat 0));
          VAL_num (VAL_int32 (W.i32_of_nat 8))] =
          Some (upd_s_mem store [after], result_values
            [VAL_num (VAL_int32 (W.i32_of_nat (List.length (firstn 8 (get_blob_data blob)))))])) in Read.
      rewrite Prefix in Read.
      specialize (Read ltac:(rewrite Instance; reflexivity) ltac:(rewrite Memory; reflexivity)
        ltac:(lia) Copied Supported).
      rewrite Width in Read; exact Read.
    Qed.

    Lemma tree_attachment_result_correct store arguments next result :
      tree_attachment_result store arguments = Some (next, result) ->
      store_extension store next = true /\
      (store_typing store -> store_typing next) /\ result = result_values [].
    Proof.
      unfold tree_attachment_result; intro Attached.
      repeat (cbv beta iota zeta delta [obind omap] in Attached;
        match type of Attached with context [match ?value with _ => _ end] => destruct value eqn:? end);
        try discriminate; inversion Attached; subst.
      split; [eapply supported_update_correct; eassumption|].
      split; [eapply supported_update_correct; eassumption|reflexivity].
    Qed.

    Lemma blob_read_result_correct store arguments next result :
      blob_read_result store arguments = Some (next, result) ->
      store_extension store next = true /\
      (store_typing store -> store_typing next) /\
      exists count, result = result_values [VAL_num (VAL_int32 count)].
    Proof.
      unfold blob_read_result; intro Read.
      repeat (cbv beta iota zeta delta [obind omap] in Read;
        match type of Read with context [match ?value with _ => _ end] => destruct value eqn:? end);
        try discriminate; inversion Read; subst.
      split; [eapply supported_update_correct; eassumption|].
      split; [eapply supported_update_correct; eassumption|eauto].
    Qed.

    Lemma tree_attachment_result_shape store node index next :
      tree_attachment_result store [W.extern_value (HTreeObj node); VAL_num (VAL_int32 index)] =
        Some (next, result_values []) ->
      exists address before,
        lookup_N (inst_tables instance) (Wasm_int.N_of_uint i32m index) = Some address /\
        lookup_N (s_tables store) address = Some before /\
        next = upd_s_mem (upd_s_table store
          (replace_resource (s_tables store) address before
            (Tables.tree_attachment before (get_tree_raw node)))) (s_mems store).
    Proof.
      unfold tree_attachment_result, W.extern_value.
      rewrite to_handle_to_externref; cbn [HTreeObj].
      destruct (lookup_N (inst_tables instance) (Wasm_int.N_of_uint i32m index))
        as [address|] eqn:Address; [|discriminate].
      destruct (N.leb (N.of_nat (List.length (get_tree_raw node))) native_table_limit)
        eqn:NativeBound; [|discriminate].
      cbn [obind].
      destruct (lookup_N (s_tables store) address) as [before|] eqn:Before; [|discriminate].
      change (omap (fun updated => (updated, result_values []))
        (supported_update store
          (replace_resource (s_tables store) address before
            (Tables.tree_attachment before (get_tree_raw node))) (s_mems store)) =
        Some (next, result_values []) ->
        exists address' before',
          Some address = Some address' /\
          lookup_N (s_tables store) address' = Some before' /\
          next = upd_s_mem (upd_s_table store
            (replace_resource (s_tables store) address' before'
              (Tables.tree_attachment before' (get_tree_raw node)))) (s_mems store)).
      destruct (supported_update store
        (replace_resource (s_tables store) address before
          (Tables.tree_attachment before (get_tree_raw node))) (s_mems store))
        as [updated|] eqn:Supported; [|discriminate].
      cbn [omap]; intro Attached; inversion Attached; subst updated.
      exists address, before; repeat split; try assumption.
      eapply supported_update_shape; exact Supported.
    Qed.

    Lemma tree_attachment_result_other_view store node index next other target address :
      tree_attachment_result store [W.extern_value (HTreeObj node); VAL_num (VAL_int32 index)] =
        Some (next, result_values []) ->
      lookup_N (inst_tables instance) (Wasm_int.N_of_uint i32m index) = Some target ->
      lookup_N (inst_tables instance) other = Some address -> target <> address ->
      stab next instance other = stab store instance other.
    Proof.
      intros Attached Target Address Different.
      destruct (tree_attachment_result_shape _ _ _ _ Attached)
        as [target' [before [Target' [Before Next]]]].
      rewrite Target in Target'; inversion Target'; subst target'.
      rewrite Next; unfold stab; rewrite Address.
      change (lookup_N
        (replace_resource (s_tables store) target before
          (Tables.tree_attachment before (get_tree_raw node))) address =
        lookup_N (s_tables store) address).
      unfold lookup_N, replace_resource; rewrite nth_error_set_neq; [reflexivity|lia|].
      apply (ssrbool.introT ssrnat.ltP).
      apply (proj1 (List.nth_error_Some _ _)); unfold lookup_N in Before;
        rewrite Before; discriminate.
    Qed.

    Lemma tree_attachment_result_other_table store node index next other target address offset :
      tree_attachment_result store [W.extern_value (HTreeObj node); VAL_num (VAL_int32 index)] =
        Some (next, result_values []) ->
      lookup_N (inst_tables instance) (Wasm_int.N_of_uint i32m index) = Some target ->
      lookup_N (inst_tables instance) other = Some address -> target <> address ->
      stab_elem next instance other offset = stab_elem store instance other offset.
    Proof.
      intros Attached Target Address Different.
      destruct (tree_attachment_result_shape _ _ _ _ Attached)
        as [target' [before [Target' [Before Next]]]].
      rewrite Target in Target'; inversion Target'; subst target'.
      rewrite Next; unfold stab_elem; rewrite Address.
      change (match lookup_N
        (replace_resource (s_tables store) target before
          (Tables.tree_attachment before (get_tree_raw node))) address with
        | Some table => lookup_N (tableinst_elem table) offset | None => None end =
        match lookup_N (s_tables store) address with
        | Some table => lookup_N (tableinst_elem table) offset | None => None end).
      unfold lookup_N, replace_resource; rewrite nth_error_set_neq; [reflexivity|lia|].
      apply (ssrbool.introT ssrnat.ltP).
      apply (proj1 (List.nth_error_Some _ _)); unfold lookup_N in Before;
        rewrite Before; discriminate.
    Qed.

    Lemma tree_attachment_result_elements store node index next offset value :
      tree_attachment_result store [W.extern_value (HTreeObj node); VAL_num (VAL_int32 index)] =
        Some (next, result_values []) ->
      nth_error (get_tree_raw node) offset = Some value ->
      stab_elem next instance (Wasm_int.N_of_uint i32m index) (N.of_nat offset) =
        Some (Tables.externref value).
    Proof.
      unfold tree_attachment_result, W.extern_value.
      rewrite to_handle_to_externref; cbn [HTreeObj].
      destruct (lookup_N (inst_tables instance) (Wasm_int.N_of_uint i32m index))
        as [address|] eqn:Address; [|discriminate].
      destruct (N.leb (N.of_nat (List.length (get_tree_raw node))) native_table_limit)
        eqn:NativeBound; [|discriminate].
      cbn [obind].
      destruct (lookup_N (s_tables store) address) as [before|] eqn:Before; [|discriminate].
      change (omap (fun updated => (updated, result_values []))
        (supported_update store
          (replace_resource (s_tables store) address before
            (Tables.tree_attachment before (get_tree_raw node))) (s_mems store)) =
        Some (next, result_values []) ->
        nth_error (get_tree_raw node) offset = Some value ->
        stab_elem next instance (Wasm_int.N_of_uint i32m index) (N.of_nat offset) =
          Some (Tables.externref value)).
      destruct (supported_update store
        (replace_resource (s_tables store) address before
          (Tables.tree_attachment before (get_tree_raw node))) (s_mems store))
        as [updated|] eqn:Supported; [|discriminate].
      cbn [omap]; intros Attached Entry; inversion Attached; subst updated.
      rewrite (supported_update_shape _ _ _ _ Supported).
      unfold stab_elem; rewrite Address.
      change (match lookup_N
        (replace_resource (s_tables store) address before
          (Tables.tree_attachment before (get_tree_raw node))) address with
        | Some table => lookup_N (tableinst_elem table) (N.of_nat offset)
        | None => None end = Some (Tables.externref value)).
      unfold lookup_N, replace_resource; rewrite nth_error_set_eq.
      change (nth_error (map Tables.externref (get_tree_raw node))
        (N.to_nat (N.of_nat offset)) = Some (Tables.externref value)).
      rewrite Nat2N.id, List.nth_error_map, Entry; reflexivity.
    Qed.

    Lemma tree_attachment_result_functions store arguments next result :
      tree_attachment_result store arguments = Some (next, result) ->
      s_funcs next = s_funcs store.
    Proof.
      unfold tree_attachment_result; intro Attached.
      repeat (cbv beta iota zeta delta [obind omap] in Attached;
        match type of Attached with context [match ?value with _ => _ end] => destruct value eqn:? end);
        try discriminate; inversion Attached; subst.
      eapply supported_update_functions; eassumption.
    Qed.

    Lemma blob_read_result_functions store arguments next result :
      blob_read_result store arguments = Some (next, result) ->
      s_funcs next = s_funcs store.
    Proof.
      unfold blob_read_result; intro Read.
      repeat (cbv beta iota zeta delta [obind omap] in Read;
        match type of Read with context [match ?value with _ => _ end] => destruct value eqn:? end);
        try discriminate; inversion Read; subst.
      eapply supported_update_functions; eassumption.
    Qed.

    Lemma blob_read_result_tables store arguments next result :
      blob_read_result store arguments = Some (next, result) ->
      s_tables next = s_tables store.
    Proof.
      unfold blob_read_result; intro Read.
      repeat (cbv beta iota zeta delta [obind omap] in Read;
        match type of Read with context [match ?value with _ => _ end] => destruct value eqn:? end);
        try discriminate; inversion Read; subst.
      match goal with
      | Updated : supported_update _ _ _ = Some _ |- _ =>
        rewrite (supported_update_shape _ _ _ _ Updated); reflexivity
      end.
    Qed.

    Lemma blob_read_result_memory_shape store arguments next result :
      blob_read_result store arguments = Some (next, result) ->
      exists address before after,
        lookup_N (inst_mems instance) 0%N = Some address /\
        lookup_N (s_mems store) address = Some before /\
        meminst_type after = meminst_type before /\
        mem_length after = mem_length before /\
        s_mems next = replace_resource (s_mems store) address before after.
    Proof.
      unfold blob_read_result; intro Read.
      repeat (cbv beta iota zeta delta [obind omap] in Read;
        match type of Read with context [match ?value with _ => _ end] => destruct value eqn:? end);
        try discriminate; inversion Read; subst.
      match goal with
      | Copied : write_bytes_meminst ?before ?destination ?bytes = Some ?after,
        Updated : supported_update _ _ _ = Some _ |- _ =>
          destruct (blob_copy_shape _ _ _ _ Copied) as [TypeOf Length];
          eexists; exists before, after; repeat split; try eassumption;
          rewrite (supported_update_shape _ _ _ _ Updated); reflexivity
      end.
    Qed.

    Definition attachment_allowed before (bytes : list bytes.byte) :=
      match lim_max (meminst_type before) with
      | None => ((N.leb (lim_min (meminst_type before)) 1 &&
          N.leb (mem_length before) page_size) &&
          (Nat.ltb 0 (List.length bytes) && N.leb (N.of_nat (List.length bytes)) page_size))%bool
      | Some _ => false end.

    (** Permitted Blob attachment satisfies the memory limits and page-size bounds required
        by the host model. *)
    Lemma attachment_allowed_bounds before bytes : attachment_allowed before bytes = true ->
      lim_max (meminst_type before) = None /\
      (lim_min (meminst_type before) <= 1)%N /\ (mem_length before <= page_size)%N.
    Proof.
      unfold attachment_allowed; destruct (lim_max (meminst_type before)) eqn:Maximum;
        [discriminate|].
      intro Allowed; apply Bool.andb_true_iff in Allowed as [Bounds _].
      apply Bool.andb_true_iff in Bounds as [Minimum Length].
      split; [reflexivity|]; split; apply N.leb_le; assumption.
    Qed.

    Definition attachment_result (store : store_record) arguments :=
      match arguments with
      | [VAL_ref (VAL_ref_extern address); VAL_num (VAL_int32 index)] =>
        match W.nat_of_i32 index, to_handle address, inst_mems instance, s_mems store with
        | 0, Data (Object (BlobObj blob)), [0%N], [before] =>
          if attachment_allowed before (get_blob_data blob)
          then omap (fun page => (upd_s_mem store [page], result_values []))
            (attachment_page (get_blob_data blob)) else None
        | _, _, _, _ => None end
      | _ => None end.

    (** Successful attachment replaces the single memory by the Blob page and returns no
        Wasm values. *)
    Lemma attachment_result_shape store arguments next result :
      attachment_result store arguments = Some (next, result) ->
      exists before bytes page,
        s_mems store = [before] /\ attachment_allowed before bytes = true /\
        attachment_page bytes = Some page /\ next = upd_s_mem store [page] /\
        result = result_values [].
    Proof.
      unfold attachment_result; intro Attached.
      repeat (cbv beta iota zeta delta [omap] in Attached;
        match type of Attached with context [match ?value with _ => _ end] => destruct value eqn:? end);
        try discriminate; inversion Attached; subst; eauto 10.
    Qed.

    (** Successful attachment extends the store, preserves typing, and returns no values. *)
    Lemma attachment_result_correct store arguments next result :
      attachment_result store arguments = Some (next, result) ->
      store_extension store next = true /\
      (store_typing store -> store_typing next) /\ result = result_values [].
    Proof.
      intro Attached; destruct (attachment_result_shape _ _ _ _ Attached)
        as [before [bytes [page [Memory [Allowed [Page [-> ->]]]]]]].
      destruct (attachment_allowed_bounds _ _ Allowed) as [Maximum [Minimum Length]].
      assert (store_extension store (upd_s_mem store [page]) = true) as Extension by
        (eapply attachment_store_extension; eassumption).
      split; [exact Extension|]; split; [|reflexivity].
      intro Typed; eapply attachment_store_typing; eassumption.
    Qed.

    Definition host_result state store signature operation arguments :=
      if function_type_eq_dec signature (shell_signature operation) then
        if shell_function_eq_dec operation shell_attach_blob then
          match attachment_result store arguments with
          | Some outcome => outcome | None => (store, result_trap) end
        else if shell_function_eq_dec operation shell_attach_tree then
          match tree_attachment_result store arguments with
          | Some outcome => outcome | None => (store, result_trap) end
        else if shell_function_eq_dec operation shell_read_blob then
          match blob_read_result store arguments with
          | Some outcome => outcome | None => (store, result_trap) end
        else match host_values state store operation arguments with
          | Some values => (store, result_values values) | None => (store, result_trap) end
      else (store, result_trap).

    Lemma tree_attachment_host_result state store arguments next result :
      tree_attachment_result store arguments = Some (next, result) ->
      host_result state store (shell_signature shell_attach_tree) shell_attach_tree arguments =
        (next, result).
    Proof.
      intro Attached; unfold host_result.
      destruct (function_type_eq_dec _ _); [|contradiction].
      destruct (shell_function_eq_dec _ _); [discriminate|].
      destruct (shell_function_eq_dec _ _); [rewrite Attached; reflexivity|contradiction].
    Qed.

    Lemma blob_read_host_result state store arguments next result :
      blob_read_result store arguments = Some (next, result) ->
      host_result state store (shell_signature shell_read_blob) shell_read_blob arguments =
        (next, result).
    Proof.
      intro Read; unfold host_result.
      destruct (function_type_eq_dec _ _); [|contradiction].
      destruct (shell_function_eq_dec _ _); [discriminate|].
      destruct (shell_function_eq_dec _ _); [discriminate|].
      destruct (shell_function_eq_dec _ _); [rewrite Read; reflexivity|contradiction].
    Qed.

    (** Every modeled Shell call preserves Wasm store extension. *)
    Lemma host_result_extension state store signature operation arguments next result :
      host_result state store signature operation arguments = (next, result) ->
      store_extension store next = true.
    Proof.
      unfold host_result.
      destruct (function_type_eq_dec signature (shell_signature operation));
        [|intro Result; inversion Result; subst; apply store_extension_same].
      destruct (shell_function_eq_dec operation shell_attach_blob).
      - destruct (attachment_result store arguments) as [[updated outcome]|] eqn:Attached;
          intro Result; inversion Result; subst.
        + exact (proj1 (attachment_result_correct _ _ _ _ Attached)).
        + apply store_extension_same.
      - destruct (shell_function_eq_dec operation shell_attach_tree).
        + destruct (tree_attachment_result store arguments) as [[updated outcome]|] eqn:Attached;
            intro Result; inversion Result; subst.
          * exact (proj1 (tree_attachment_result_correct _ _ _ _ Attached)).
          * apply store_extension_same.
        + destruct (shell_function_eq_dec operation shell_read_blob).
          * destruct (blob_read_result store arguments) as [[updated outcome]|] eqn:Read;
              intro Result; inversion Result; subst.
            -- exact (proj1 (blob_read_result_correct _ _ _ _ Read)).
            -- apply store_extension_same.
          * destruct (host_values state store operation arguments); intro Result; inversion Result; subst;
              apply store_extension_same.
    Qed.

    (** Every modeled Shell call preserves a well-typed Wasm store. *)
    Lemma host_result_typing state store signature operation arguments next result :
      host_result state store signature operation arguments = (next, result) ->
      store_typing store -> store_typing next.
    Proof.
      unfold host_result.
      destruct (function_type_eq_dec signature (shell_signature operation));
        [|intro Result; inversion Result; subst; auto].
      destruct (shell_function_eq_dec operation shell_attach_blob).
      - destruct (attachment_result store arguments) as [[updated outcome]|] eqn:Attached;
          intro Result; inversion Result; subst.
        + exact (proj1 (proj2 (attachment_result_correct _ _ _ _ Attached))).
        + auto.
      - destruct (shell_function_eq_dec operation shell_attach_tree).
        + destruct (tree_attachment_result store arguments) as [[updated outcome]|] eqn:Attached;
            intro Result; inversion Result; subst.
          * exact (proj1 (proj2 (tree_attachment_result_correct _ _ _ _ Attached))).
          * auto.
        + destruct (shell_function_eq_dec operation shell_read_blob).
          * destruct (blob_read_result store arguments) as [[updated outcome]|] eqn:Read;
              intro Result; inversion Result; subst.
            -- exact (proj1 (proj2 (blob_read_result_correct _ _ _ _ Read))).
            -- auto.
          * destruct (host_values state store operation arguments); intro Result; inversion Result; subst; auto.
    Qed.

    (** Every modeled Shell call returns values matching the declared result types or a
        permitted trap. *)
    Lemma host_result_types state store parameters results operation arguments next result :
      host_result state store (Tf parameters results) operation arguments = (next, result) ->
      result_types_agree next results result = true.
    Proof.
      unfold host_result.
      destruct (function_type_eq_dec (Tf parameters results) (shell_signature operation))
        as [Signature|Signature]; [|intro Result; inversion Result; subst; reflexivity].
      destruct (shell_function_eq_dec operation shell_attach_blob) as [->|Other].
      - destruct (attachment_result store arguments) as [[updated outcome]|] eqn:Attached;
          intro Result; inversion Result; subst; [|reflexivity].
        rewrite (proj2 (proj2 (attachment_result_correct _ _ _ _ Attached))).
        cbn [shell_signature] in Signature; inversion Signature; reflexivity.
      - destruct (shell_function_eq_dec operation shell_attach_tree) as [->|OtherTree].
        + destruct (tree_attachment_result store arguments) as [[updated outcome]|] eqn:Attached;
            intro Result; inversion Result; subst; [|reflexivity].
          rewrite (proj2 (proj2 (tree_attachment_result_correct _ _ _ _ Attached))).
          cbn [shell_signature] in Signature; inversion Signature; reflexivity.
        + destruct (shell_function_eq_dec operation shell_read_blob) as [->|OtherRead].
          * destruct (blob_read_result store arguments) as [[updated outcome]|] eqn:Read;
              intro Result; inversion Result; subst; [|reflexivity].
            destruct (proj2 (proj2 (blob_read_result_correct _ _ _ _ Read))) as [count ->].
            cbn [shell_signature] in Signature; inversion Signature; reflexivity.
          * destruct (host_values state store operation arguments) as [values|] eqn:Values;
              intro Result; inversion Result; subst; [|reflexivity].
            pose proof (host_values_typing _ _ _ _ _ Values) as Typed.
            rewrite <- Signature in Typed; exact Typed.
    Qed.

    (** A successful pure Shell call returns its computed values without changing the
        store. *)
    Lemma host_values_result state store operation arguments values :
      host_values state store operation arguments = Some values ->
      host_result state store (shell_signature operation) operation arguments =
        (store, result_values values).
    Proof.
      intro Values; unfold host_result.
      destruct (function_type_eq_dec _ _); [|contradiction].
      destruct (shell_function_eq_dec operation shell_attach_blob) as [->|Other];
        [discriminate|].
      destruct (shell_function_eq_dec operation shell_attach_tree) as [->|OtherTree];
        [discriminate|].
      destruct (shell_function_eq_dec operation shell_read_blob) as [->|OtherRead];
        [discriminate|].
      rewrite Values; reflexivity.
    Qed.

    Lemma host_values_trap state store operation arguments :
      operation <> shell_attach_blob -> operation <> shell_attach_tree -> operation <> shell_read_blob ->
      host_values state store operation arguments = None ->
      host_result state store (shell_signature operation) operation arguments =
        (store, result_trap).
    Proof.
      intros Other OtherTree OtherRead Values; unfold host_result.
      destruct (function_type_eq_dec _ _); [|contradiction].
      destruct (shell_function_eq_dec operation shell_attach_blob); [contradiction|].
      destruct (shell_function_eq_dec operation shell_attach_tree); [contradiction|].
      destruct (shell_function_eq_dec operation shell_read_blob); [contradiction|].
      rewrite Values; reflexivity.
    Qed.

    (** Privileged byte comparison checks exact handle encodings. *)
    Lemma same_handle_bytes_result state store lhs rhs left right equal :
      privileged = true ->
      handle_bytes state lhs = Some left ->
      handle_bytes (Datatypes.S state) rhs = Some right ->
      Tables.T.O.compare_handles (HBlobObj left) (HBlobObj right) = Some equal ->
      host_result state store (shell_signature shell_same_handle_bytes)
        shell_same_handle_bytes [W.extern_value lhs; W.extern_value rhs] =
      (store, result_values [VAL_num (VAL_int32 (wasm_bool equal))]).
    Proof.
      intros Privileged Left Right Compared; apply host_values_result.
      unfold host_values, W.extern_value.
      rewrite !to_handle_to_externref, Privileged, Left, Right.
      cbn [obind].
      rewrite Compared.
      destruct equal; reflexivity.
    Qed.

    (** The Shell comparison import returns the modeled comparison result for Blob Objects. *)
    Lemma compare_blobs_result state store lhs rhs equal :
      Tables.T.O.compare_handles (HBlobObj lhs) (HBlobObj rhs) = Some equal ->
      host_result state store (shell_signature shell_equals) shell_equals
        [W.extern_value (HBlobObj lhs); W.extern_value (HBlobObj rhs)] =
      (store, result_values [VAL_num (VAL_int32 (wasm_bool equal))]).
    Proof.
      intro Compared; apply host_values_result.
      unfold host_values, W.extern_value; rewrite !to_handle_to_externref, Compared.
      destruct equal; reflexivity.
    Qed.

    Definition next_state (state : nat) operation := match operation with
      | shell_same_handle_bytes => Datatypes.S (Datatypes.S state) | _ => state end.
    Definition host_application_impl state store signature operation arguments state' output :=
      state' = next_state state operation /\
      output = Some (host_result state store signature operation arguments).
    #[export] Instance shell_host : @host shell_host_functions mem.
    Proof.
      refine {| host_state := nat; host_application := host_application_impl |}.
      - intros state store signature operation arguments state' next result [_ Applied].
        injection Applied as Result; symmetry in Result.
        eapply host_result_extension; exact Result.
      - intros state store signature operation arguments state' next result [_ Applied] Typed.
        injection Applied as Result; symmetry in Result.
        eapply host_result_typing; eassumption.
      - intros state parameters results store operation arguments state' next result Typed [_ Applied].
        injection Applied as Result; symmetry in Result.
        eapply host_result_types; exact Result.
    Defined.
  End Calls.
End ShellHost.
