(** Interprets Fix Shell imports as WasmCert host operations and proves
    that their results preserve the required typing and store-extension laws. *)

From Wasm Require Import datatypes operations typing host properties.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Shell Require Import Operations Externref ABI Memory Tables.
From Stdlib Require Import List String Arith NArith ZArith Bool.
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
    Variable handle_bytes : nat -> handle -> option BlobName.

    Definition host_values state (store : store_record) operation arguments :=
      match operation, arguments with
      | shell_get_type, [VAL_ref (VAL_ref_extern address)] =>
        word_result (value_type (to_handle address))
      | shell_get_data_type, [VAL_ref (VAL_ref_extern address)] =>
        obind (data_type (to_handle address)) word_result
      | shell_get_encode_type, [VAL_ref (VAL_ref_extern address)] =>
        obind (encode_type (to_handle address)) word_result
      | shell_is_tag, [VAL_ref (VAL_ref_extern address)] => W.bool_result (W.Api.is_tag (to_handle address))
      | shell_handle_bytes, [VAL_ref (VAL_ref_extern address)] =>
        if privileged then W.handle_result (omap HBlobObj (handle_bytes state (to_handle address))) else None
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
      | shell_create_tag, [VAL_num (VAL_int32 index); VAL_num (VAL_int32 depth)] =>
        W.handle_result (omap HTreeObj (Tables.create_tag_from_table combination store instance
          (Wasm_int.N_of_uint i32m index) (W.nat_of_i32 depth)))
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
        else match host_values state store operation arguments with
          | Some values => (store, result_values values) | None => (store, result_trap) end
      else (store, result_trap).

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
      - destruct (host_values state store operation arguments); intro Result; inversion Result; subst;
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
      - destruct (host_values state store operation arguments); intro Result; inversion Result; subst; auto.
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
      - destruct (host_values state store operation arguments) as [values|] eqn:Values;
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
      rewrite Values; reflexivity.
    Qed.

    (** Privileged handle inspection returns the Blob provided by the handle-byte
        interface. *)
    Lemma handle_bytes_result state store value bytes :
      privileged = true -> handle_bytes state value = Some bytes ->
      host_result state store (shell_signature shell_handle_bytes)
        shell_handle_bytes [W.extern_value value] =
      (store, result_values [W.extern_value (HBlobObj bytes)]).
    Proof.
      intros Privileged Observed; unfold host_result.
      destruct (function_type_eq_dec _ _); [|contradiction].
      destruct (shell_function_eq_dec _ _); [discriminate|].
      unfold host_values, W.extern_value.
      rewrite to_handle_to_externref, Privileged, Observed; reflexivity.
    Qed.

    (** The Shell comparison import returns the modeled comparison result for Blob Objects. *)
    Lemma compare_blobs_result state store lhs rhs equal :
      Tables.T.O.compare_handles (HBlobObj lhs) (HBlobObj rhs) = Some equal ->
      host_result state store (shell_signature shell_equals) shell_equals
        [W.extern_value (HBlobObj lhs); W.extern_value (HBlobObj rhs)] =
      (store, result_values [VAL_num (VAL_int32 (wasm_bool equal))]).
    Proof.
      intro Compared; unfold host_result.
      destruct (function_type_eq_dec _ _); [|contradiction].
      destruct (shell_function_eq_dec _ _); [discriminate|].
      unfold host_values, W.extern_value; rewrite !to_handle_to_externref, Compared.
      destruct equal; reflexivity.
    Qed.

    Definition next_state (state : nat) operation := match operation with
      | shell_handle_bytes => Datatypes.S state | _ => state end.
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
