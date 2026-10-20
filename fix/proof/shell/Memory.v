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
End Attachment.
