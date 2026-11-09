(** Models Blob attachment to Wasm memory and proves byte contents, typing,
    and store extension under explicit attachment constraints. *)

From mathcomp Require Import -(notations) ssrnat eqtype seq.
From Wasm Require Import datatypes operations properties typing type_preservation.
From FixProof.Support Require Import BlobInteger.
From Stdlib Require Import List Arith NArith ZArith Lia.
Delimit Scope N_scope with N.
Import ListNotations.
Local Transparent Archi.big_endian.

Section Attachment.
  Context `{hfc : host_function_class} `{mem : BlockUpdateMemory}.

  Definition attachment_byte (bytes : list bytes.byte) offset :=
    List.nth (N.to_nat offset) bytes Integers.Byte.zero.

  Definition attachment_page bytes :=
    match mem_update_gen 0%N page_size (attachment_byte bytes) (mem_make page_size) with
    | Some data => Some (Build_meminst (Build_limits 1%N None) data)
    | None => None end.

  (** A Blob attachment page can always be constructed in the memory model. *)
  Lemma attachment_page_exists bytes : exists page, attachment_page bytes = Some page.
  Proof.
    unfold attachment_page.
    pose proof (mem_update_gen_ib (n := 0%N) (len := page_size)
      (gen := attachment_byte bytes) (m := mem_make page_size)) as Exists.
    rewrite mem_make_length in Exists.
    specialize (Exists ltac:(vm_compute; congruence)).
    destruct (mem_update_gen 0%N page_size (attachment_byte bytes) (mem_make page_size));
      [eexists; reflexivity|contradiction].
  Qed.

  (** A constructed attachment has one page of memory and the required memory limits. *)
  Lemma attachment_page_shape bytes page : attachment_page bytes = Some page ->
    meminst_type page = Build_limits 1%N None /\ mem_length page = page_size.
  Proof.
    unfold attachment_page.
    destruct (mem_update_gen 0%N page_size (attachment_byte bytes) (mem_make page_size))
      as [data|] eqn:Write; [|discriminate].
    intro Page; inversion Page; subst; split; [reflexivity|].
    change (memory.mem_length data = page_size).
    rewrite <- (mem_update_gen_length Write), mem_make_length; reflexivity.
  Qed.

  (** Reading an in-page byte from an attachment gives the corresponding Blob byte or zero
      padding. *)
  Lemma attachment_page_lookup bytes page offset :
    attachment_page bytes = Some page -> (offset < page_size)%N ->
    mem_lookup offset (meminst_data page) = Some (attachment_byte bytes offset).
  Proof.
    unfold attachment_page.
    destruct (mem_update_gen 0%N page_size (attachment_byte bytes) (mem_make page_size))
      as [data|] eqn:Write; [|discriminate].
    intros Page Bound; inversion Page; subst; cbn.
    exact (mem_update_gen_lookup Write Bound).
  Qed.

  (** A Blob fitting in one page can be read back unchanged from its attachment. *)
  Lemma attachment_page_read bytes page :
    attachment_page bytes = Some page -> (N.of_nat (List.length bytes) <= page_size)%N ->
    read_bytes page 0%N (List.length bytes) = Some bytes.
  Proof.
    intros Page Bound; unfold read_bytes; apply those_spec.
    - rewrite List.length_map, size_iota; reflexivity.
    - intros index byte Byte.
      assert (Peano.lt index (List.length bytes)) as Index.
      { apply (proj1 (List.nth_error_Some bytes index)).
        intro Empty; rewrite Byte in Empty; discriminate. }
      rewrite List.nth_error_map.
      assert (List.nth_error (iota 0 (List.length bytes)) index = Some index) as Offset.
      { apply iota_lookup; exact (ssrbool.introT ssrnat.ltP Index). }
      rewrite Offset.
      change (Some (mem_lookup (N.of_nat index) (meminst_data page)) = Some (Some byte)).
      rewrite (attachment_page_lookup _ _ _ Page) by
        (eapply N.lt_le_trans with (m := N.of_nat (List.length bytes));
          [lia|exact Bound]).
      unfold attachment_byte; rewrite Nat2N.id.
      rewrite (List.nth_error_nth bytes index Integers.Byte.zero Byte); reflexivity.
  Qed.

  (** A constructed attachment page has the declared Wasm memory type. *)
  Lemma attachment_page_typing bytes page s :
    attachment_page bytes = Some page ->
    meminst_typing s page = Some (Build_limits 1%N None).
  Proof.
    intro Page; destruct (attachment_page_shape _ _ Page) as [Limits Length].
    destruct page as [limits data]; cbn in Limits; subst limits.
    change ((if eq_op (mem_length (Build_meminst (Build_limits 1%N None) data)) page_size
      then Some (Build_limits 1%N None) else None) = Some (Build_limits 1%N None)).
    rewrite Length; reflexivity.
  Qed.

  (** Replacing an allowed memory with the attachment page satisfies memory extension. *)
  Lemma attachment_page_extension bytes page before :
    attachment_page bytes = Some page ->
    lim_max (meminst_type before) = None ->
    (lim_min (meminst_type before) <= 1)%N ->
    (mem_length before <= page_size)%N ->
    mem_extension before page = true.
  Proof.
    intros Page Maximum Minimum Length.
    destruct (attachment_page_shape _ _ Page) as [Limits Size].
    destruct page as [limits data]; cbn in Limits; subst limits.
    change (memory.mem_length data = page_size) in Size.
    change (((N.leb (lim_min (meminst_type before)) 1 &&
      eq_op (lim_max (meminst_type before)) None) &&
      N.leb (mem_length before) (memory.mem_length data))%bool = true).
    rewrite Maximum, Size; cbn -[N.leb]; rewrite Bool.andb_true_r.
    apply Bool.andb_true_iff; split; apply N.leb_le; assumption.
  Qed.

  (** Replacing the allowed single memory with a Blob page satisfies store extension. *)
  Lemma attachment_store_extension bytes page s before :
    s_mems s = [before] -> attachment_page bytes = Some page ->
    lim_max (meminst_type before) = None ->
    (lim_min (meminst_type before) <= 1)%N ->
    (mem_length before <= page_size)%N ->
    store_extension s (upd_s_mem s [page]) = true.
  Proof.
    intros Memory Page Maximum Minimum Length.
    change ((((((component_extension func_extension (s_funcs s) (s_funcs s) &&
      component_extension table_extension (s_tables s) (s_tables s)) &&
      component_extension mem_extension (s_mems s) [page]) &&
      component_extension global_extension (s_globals s) (s_globals s)) &&
      component_extension elem_extension (s_elems s) (s_elems s)) &&
      component_extension data_extension (s_datas s) (s_datas s))%bool = true).
    rewrite (component_extension_same_refl (s_funcs s) func_extension_refl),
      (component_extension_same_refl (s_tables s) table_extension_refl),
      (component_extension_same_refl (s_globals s) global_extension_refl),
      (component_extension_same_refl (s_elems s) elem_extension_refl),
      (component_extension_same_refl (s_datas s) data_extension_refl), Memory.
    repeat rewrite Bool.andb_true_r; cbn -[component_extension].
    change ((mem_extension before page && true)%bool = true).
    rewrite Bool.andb_true_r; eapply attachment_page_extension; eassumption.
  Qed.

  (** A Blob attachment satisfying store extension preserves store typing.

      Store extension preserves typing of the unchanged components; attachment supplies a
      well-typed replacement memory. *)
  Lemma attachment_store_typing bytes page s :
    attachment_page bytes = Some page ->
    store_extension s (upd_s_mem s [page]) = true ->
    store_typing s -> store_typing (upd_s_mem s [page]).
  Proof.
    intros Page Extension Typed.
    destruct s as [functions tables memories globals elements data].
    destruct Typed as [Functions [Tables [Memories [Globals [Elements Data]]]]].
    change (store_typing (Build_store_record functions tables [page] globals elements data)).
    cbn [store_typing].
    repeat split; try solve [eauto using store_extension_funcs_typing,
      store_extension_tables_typing, store_extension_globals_typing,
      store_extension_elems_typing, store_extension_datas_typing].
    constructor; [|constructor].
    exists (Build_limits 1%N None); eapply attachment_page_typing; exact Page.
  Qed.

  (** An attached eight-byte Blob can be loaded and deserialized to the same unsigned i64
      value. *)
  Lemma attachment_page_u64 bytes page value :
    attachment_page bytes = Some page -> decode_u64 bytes = Some value ->
    load page 0%N 0%N 8%N = Some bytes /\
    wasm_deserialise bytes T_i64 =
      VAL_int64 (Wasm_int.Int64.repr (Z.of_nat value)).
  Proof.
    intros Page Decoded; split; [|apply wasm_deserialise_u64; exact Decoded].
    pose proof (decode_u64_width _ _ Decoded) as Width.
    pose proof (attachment_page_read _ _ Page ltac:(rewrite Width; vm_compute; congruence)) as Read.
    destruct (attachment_page_shape _ _ Page) as [_ Length].
    unfold load; rewrite Length; cbn; rewrite Width in Read; exact Read.
  Qed.
  Lemma blob_copy_shape before destination bytes after :
    write_bytes_meminst before destination bytes = Some after ->
    meminst_type after = meminst_type before /\ mem_length after = mem_length before.
  Proof.
    unfold write_bytes_meminst, write_bytes, write_bytes_gen.
    destruct (mem_update_gen destination (N.of_nat (List.length bytes))
      (fun offset => match lookup_N bytes offset with Some byte => byte | None => Integers.Byte.zero end)
      (meminst_data before)) as [data|] eqn:Write; [|discriminate].
    intro Copied; inversion Copied; subst; split; [reflexivity|].
    change (memory.mem_length data = memory.mem_length (meminst_data before)).
    symmetry; eapply mem_update_gen_length; exact Write.
  Qed.

  Lemma blob_copy_typing before destination bytes after old_store new_store typ :
    write_bytes_meminst before destination bytes = Some after ->
    meminst_typing old_store before = Some typ ->
    meminst_typing new_store after = Some typ.
  Proof.
    intros Copied Typed; destruct (blob_copy_shape _ _ _ _ Copied) as [TypeOf Length].
    unfold meminst_typing in *.
    destruct before as [before_type before_data], after as [after_type after_data]; cbn in TypeOf.
    subst after_type; cbn -[mem_length] in *.
    rewrite Length; exact Typed.
  Qed.

  Lemma blob_copy_extension before destination bytes after :
    write_bytes_meminst before destination bytes = Some after -> mem_extension before after = true.
  Proof.
    intro Copied; destruct (blob_copy_shape _ _ _ _ Copied) as [TypeOf Length].
    unfold mem_extension, limits_extension; rewrite TypeOf, Length.
    rewrite !N.leb_refl, eqtype.eq_refl; reflexivity.
  Qed.

  Lemma blob_copy_store_extension store before destination bytes after :
    s_mems store = [before] -> write_bytes_meminst before destination bytes = Some after ->
    store_extension store (upd_s_mem store [after]) = true.
  Proof.
    intros Memory Copied.
    change ((((((component_extension func_extension (s_funcs store) (s_funcs store) &&
      component_extension table_extension (s_tables store) (s_tables store)) &&
      component_extension mem_extension (s_mems store) [after]) &&
      component_extension global_extension (s_globals store) (s_globals store)) &&
      component_extension elem_extension (s_elems store) (s_elems store)) &&
      component_extension data_extension (s_datas store) (s_datas store))%bool = true).
    rewrite (component_extension_same_refl (s_funcs store) func_extension_refl),
      (component_extension_same_refl (s_tables store) table_extension_refl),
      (component_extension_same_refl (s_globals store) global_extension_refl),
      (component_extension_same_refl (s_elems store) elem_extension_refl),
      (component_extension_same_refl (s_datas store) data_extension_refl), Memory.
    repeat rewrite Bool.andb_true_r; cbn -[component_extension].
    change ((mem_extension before after && true)%bool = true).
    rewrite Bool.andb_true_r; eapply blob_copy_extension; exact Copied.
  Qed.

  Lemma blob_copy_store_typing store before destination bytes after :
    s_mems store = [before] -> write_bytes_meminst before destination bytes = Some after ->
    store_typing store -> store_typing (upd_s_mem store [after]).
  Proof.
    intros Memory Copied Typed.
    pose proof (blob_copy_store_extension _ _ _ _ _ Memory Copied) as Extension.
    destruct store as [functions tables memories globals elements data].
    cbn in Memory; subst memories.
    destruct Typed as [Functions [Tables [Memories [Globals [Elements Data]]]]].
    inversion Memories as [|unused empty Head Tail]; subst.
    destruct Head as [typ BeforeTyped].
    cbn [upd_s_mem store_typing]; repeat split; try solve [eauto using store_extension_funcs_typing,
      store_extension_tables_typing, store_extension_globals_typing,
      store_extension_elems_typing, store_extension_datas_typing].
    constructor; [|constructor].
    exists typ; eapply blob_copy_typing; eassumption.
  Qed.

  Lemma blob_copy_exists before destination bytes :
    (destination + N.of_nat (List.length bytes) <= mem_length before)%N ->
    exists after, write_bytes_meminst before destination bytes = Some after.
  Proof.
    intro Bound; unfold write_bytes_meminst, write_bytes, write_bytes_gen.
    pose proof (mem_update_gen_ib (n := destination) (len := N.of_nat (List.length bytes))
      (gen := fun offset => match lookup_N bytes offset with
        Some byte => byte | None => Integers.Byte.zero end)
      (m := meminst_data before) Bound) as Exists.
    destruct (mem_update_gen destination (N.of_nat (List.length bytes))
      (fun offset => match lookup_N bytes offset with Some byte => byte | None => Integers.Byte.zero end)
      (meminst_data before)); [eexists; reflexivity|contradiction].
  Qed.

  Lemma blob_copy_read before destination bytes after :
    write_bytes_meminst before destination bytes = Some after ->
    read_bytes after destination (List.length bytes) = Some bytes.
  Proof.
    unfold write_bytes_meminst, write_bytes, write_bytes_gen.
    destruct (mem_update_gen destination (N.of_nat (List.length bytes))
      (fun offset => match lookup_N bytes offset with Some byte => byte | None => Integers.Byte.zero end)
      (meminst_data before)) as [data|] eqn:Write; [|discriminate].
    intro Copied; inversion Copied; subst; unfold read_bytes; apply those_spec.
    - rewrite List.length_map, size_iota; reflexivity.
    - intros index byte Byte.
      assert (Index : Peano.lt index (List.length bytes)).
      { apply (proj1 (List.nth_error_Some bytes index)); rewrite Byte; discriminate. }
      rewrite List.nth_error_map.
      assert (Offset : List.nth_error (iota 0 (List.length bytes)) index = Some index).
      { apply iota_lookup; exact (ssrbool.introT ssrnat.ltP Index). }
      rewrite Offset.
      change (Some (mem_lookup (destination + N.of_nat index)%N data) = Some (Some byte)).
      rewrite (mem_update_gen_lookup Write) by lia.
      unfold lookup_N; rewrite Nat2N.id, Byte; reflexivity.
  Qed.

  Lemma blob_copy_preserves_other_bytes before destination bytes after offset :
    write_bytes_meminst before destination bytes = Some after ->
    ((offset < destination)%N \/ (destination + N.of_nat (List.length bytes) <= offset)%N) ->
    mem_lookup offset (meminst_data after) = mem_lookup offset (meminst_data before).
  Proof.
    unfold write_bytes_meminst, write_bytes, write_bytes_gen.
    destruct (mem_update_gen destination (N.of_nat (List.length bytes))
      (fun position => match lookup_N bytes position with Some byte => byte | None => Integers.Byte.zero end)
      (meminst_data before)) as [data|] eqn:Write; [|discriminate].
    intros Copied Outside; inversion Copied; subst; cbn.
    destruct Outside as [Before|After].
    - eapply mem_update_gen_lookup_lt; eassumption.
    - eapply mem_update_gen_lookup_ge; [exact Write|lia].
  Qed.

End Attachment.
