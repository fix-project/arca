Module WasmResources.
Import ListNotations.
Local Open Scope list_scope.
Local Transparent Archi.big_endian.
  Import C S.
  Module Support := WasmSupport.
  Import Support.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.

  Definition memory_capacity store := exists page,
    s_mems store = [page] /\ meminst_type page = Build_limits 1%N None /\ mem_length page = page_size.

  Lemma initial_descriptor_capacity :
    G.E.H.within_capacity (G.E.H.initial (G.E.H.H.HTreeObj combination)).
  Proof. apply G.E.H.initial_capacity. Qed.

  Lemma execution_descriptor_capacity state store frame instructions next updated following remaining :
    G.E.H.within_capacity state ->
    reduce_trans (state, store, frame, instructions) (next, updated, following, remaining) ->
    G.E.H.within_capacity next.
  Proof.
    intros Within Run; exact (G.E.collector_runs_capacity combination true true handle_bytes
      (state, store, frame, instructions) (next, updated, following, remaining) Run Within).
  Qed.

  Lemma blob_read_memory_capacity state store arguments next result :
    G.E.H.blob_read_result G.E.collector_instance state store arguments = Some (next, result) ->
    memory_capacity store -> memory_capacity next.
  Proof.
    intros Read [page [Memory [TypeOf Length]]].
    destruct (G.E.H.blob_read_result_memory_shape _ _ _ _ _ _ Read)
      as [address [before [after [Address [Before [AfterType [AfterLength Memories]]]]]]].
    rewrite (proj2 G.E.collector_resource_indices) in Address.
    cbn [lookup_N] in Address; inversion Address; subst address.
    unfold lookup_N in Before; rewrite Memory in Before; cbn in Before;
      inversion Before; subst before.
    exists after; split.
    - rewrite Memories, Memory; reflexivity.
    - rewrite AfterType, AfterLength; auto.
  Qed.

  Lemma integer_available state store locals index blob number :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> memory_capacity store ->
    G.E.H.resolve state index = Some (G.E.H.H.HBlobObj blob) ->
    decode_u64 (get_blob_data blob) = Some number ->
    exists next,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [descriptor index] ++ [AI_basic (BI_call 27%N)])
        (state, next, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 number))]) /\
      s_funcs next = s_funcs store /\ store_typing next /\ memory_capacity next.
  Proof.
    intros Functions Typed Capacity Live Decoded.
    destruct Capacity as [before [Memory [TypeOf Length]]].
    pose proof (decode_u64_width _ _ Decoded) as Width.
    destruct (blob_copy_exists before 0%N (get_blob_data blob)) as [after Copied].
    { rewrite Width, Length; vm_compute; congruence. }
    set (next := upd_s_mem store [after]).
    assert (Read : G.E.H.blob_read_result G.E.collector_instance state store
      [descriptor index; word 0; word 0; word 8] = Some (next, [word 8])).
    { eapply G.E.H.blob_read_zero_bounded; try eassumption; reflexivity. }
    exists next; split.
    - eapply collector_integer_executes with (page := after); try eassumption; try reflexivity.
      pose proof (blob_copy_read _ _ _ _ Copied) as Loaded; rewrite Width in Loaded.
      destruct (blob_copy_shape _ _ _ _ Copied) as [_ SameLength].
      unfold load; rewrite SameLength, Length; cbn; exact Loaded.
    - split; [reflexivity|]; split.
      + eapply blob_copy_store_typing; eassumption.
      + eapply blob_read_memory_capacity; [exact Read|exists before; auto].
  Qed.

  Lemma serialise_i32_width index : List.length (serialise_i32 index) = 4.
  Proof.
    unfold serialise_i32, Memdata.encode_int, Memdata.rev_if_be, Archi.big_endian.
    apply bytes_of_int_length.
  Qed.

  Lemma deserialise_i32_roundtrip index : wasm_deserialise (serialise_i32 index) T_i32 = VAL_int32 index.
  Proof.
    unfold wasm_deserialise, serialise_i32, Memdata.decode_int, Memdata.encode_int,
      Memdata.rev_if_be, Archi.big_endian.
    rewrite int_of_bytes_roundtrip by (exact (Wasm_int.Int32.unsigned_range index)).
    rewrite Wasm_int.Int32.repr_unsigned; reflexivity.
  Qed.

  Definition staged memory pointer indices := forall offset index,
    nth_error indices offset = Some index ->
    read_bytes memory (pointer + 4 * N.of_nat offset)%N 4 = Some (serialise_i32 index).

  Lemma read_staged_handles memory world pointer indices values :
    staged memory pointer indices ->
    Forall2 (fun index value => G.E.H.resolve world index = Some value) indices values ->
    G.E.H.read_handles memory (G.E.H.slots world) pointer (List.length indices) = Some values.
  Proof.
    intros Staged Live; revert pointer Staged.
    induction Live as [|index value indices values Present Live IH]; intros pointer Staged; [reflexivity|].
    cbn [List.length G.E.H.read_handles].
    specialize (Staged 0 index eq_refl) as Read.
    rewrite N.mul_0_r, N.add_0_r in Read.
    rewrite Read; cbn [G.E.H.H.obind]; rewrite deserialise_i32_roundtrip.
    fold (G.E.H.resolve world index); rewrite Present.
    cbn [G.E.H.H.obind]; rewrite IH; [reflexivity|].
    intros offset slot Field.
    specialize (Staged (Datatypes.S offset) slot Field).
    replace (pointer + 4 + 4 * N.of_nat offset)%N with (pointer + 4 * N.of_nat (Datatypes.S offset))%N by lia.
    exact Staged.
  Qed.

  Lemma iota_member_bound start count index :
    In index (seq.iota start count) -> start <= index < start + count.
  Proof.
    revert start; induction count as [|count IH]; intros start Member; [contradiction|].
    cbn [seq.iota] in Member; destruct Member as [Equal|Member]; [subst; lia|].
    specialize (IH _ Member); lia.
  Qed.

  Lemma copy_preserves_read before destination bytes after pointer count :
    write_bytes_meminst before destination bytes = Some after ->
    ((pointer + N.of_nat count <= destination)%N \/
      (destination + N.of_nat (List.length bytes) <= pointer)%N) ->
    read_bytes after pointer count = read_bytes before pointer count.
  Proof.
    intros Copied Apart; unfold read_bytes; f_equal; apply map_ext_in; intros offset Member.
    apply blob_copy_preserves_other_bytes with (destination := destination) (bytes := bytes); [exact Copied|].
    pose proof (iota_member_bound 0 count offset Member) as Bound.
    destruct Apart; [left|right]; lia.
  Qed.

  Lemma staged_append before after pointer indices index :
    staged before pointer indices ->
    write_bytes_meminst before (pointer + 4 * N.of_nat (List.length indices))%N (serialise_i32 index) = Some after ->
    staged after pointer (indices ++ [index]).
  Proof.
    intros Staged Copied offset slot Field.
    destruct (Nat.lt_ge_cases offset (List.length indices)) as [Earlier|Later].
    - rewrite nth_error_app1 in Field by exact Earlier.
      rewrite (copy_preserves_read _ _ _ _ _ _ Copied); [apply Staged; exact Field|].
      left; lia.
    - assert (Bound : offset < List.length (indices ++ [index])).
      { apply (proj1 (List.nth_error_Some _ _)); rewrite Field; discriminate. }
      rewrite List.length_app in Bound; cbn in Bound.
      assert (Equal : offset = List.length indices) by lia; subst offset.
      rewrite nth_error_app2, Nat.sub_diag in Field by lia; cbn in Field; inversion Field; subst slot.
      pose proof (blob_copy_read _ _ _ _ Copied) as Read; rewrite serialise_i32_width in Read; exact Read.
  Qed.

  Lemma tree_from_staged state store memory pointer indices values :
    s_mems store = [memory] -> staged memory (N.of_nat pointer) indices ->
    Forall2 (fun index value => G.E.H.resolve state index = Some value) indices values ->
    (Z.of_nat pointer < 2 ^ 32)%Z -> List.length indices < G.E.H.capacity ->
    G.E.H.tree_from_memory G.E.collector_instance state store (wasm_nat pointer) (wasm_nat (List.length indices)) =
      G.E.H.allocate state (G.E.H.H.HTreeObj (create_tree values)).
  Proof.
    intros Memory Staged Live Pointer Bound.
    unfold G.E.H.tree_from_memory.
    rewrite G.E.H.word_roundtrip by exact Bound.
    assert (Fits : Nat.leb (List.length indices) G.E.H.capacity = true) by (apply Nat.leb_le; lia).
    rewrite Fits; unfold G.E.H.memory_zero.
    rewrite (proj2 G.E.collector_resource_indices); cbn [lookup_N G.E.H.H.obind].
    rewrite Memory; cbn [lookup_N G.E.H.H.obind].
    rewrite wasm_nat_to_N by exact Pointer.
    change (G.E.H.H.obind
      (G.E.H.read_handles memory (G.E.H.slots state) (N.of_nat pointer) (List.length indices))
      (fun fields => G.E.H.allocate state (G.E.H.H.HTreeObj (create_tree fields))) =
      G.E.H.allocate state (G.E.H.H.HTreeObj (create_tree values))).
    rewrite (read_staged_handles _ _ _ _ _ Staged Live); reflexivity.
  Qed.

  Lemma store_descriptor_run state store locals before after pointer index :
    s_mems store = [before] -> (Z.of_nat pointer < 2 ^ 32)%Z ->
    (N.of_nat pointer + 4 <= mem_length before)%N ->
    write_bytes_meminst before (N.of_nat pointer) (serialise_i32 index) = Some after ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [word pointer; descriptor index] ++
          [AI_basic (BI_store T_i32 None (Build_memarg 0%N 2%N))])
      (state, upd_s_mem store [after], Build_frame locals G.E.collector_instance, []).
  Proof.
    intros Memory Pointer Bound Copied.
    apply rt_step; apply r_store_success.
    unfold smem_store; rewrite (proj2 G.E.collector_resource_indices).
    cbn [lookup_N]; rewrite Memory; cbn [lookup_N].
    rewrite wasm_nat_to_N by exact Pointer.
    change (match operations.store before (N.of_nat pointer) 0%N (serialise_i32 index) 4 with
      | Some page => Some (upd_s_mem store [page]) | None => None end = Some (upd_s_mem store [after])).
    unfold operations.store.
    assert (Fits : N.leb (N.of_nat pointer + 0 + N.of_nat 4) (mem_length before) = true)
      by (apply N.leb_le; cbn; lia).
    rewrite Fits; unfold bytes.bytes_takefill; rewrite serialise_i32_width.
    cbn [Nat.ltb]; rewrite firstn_all2 by (rewrite serialise_i32_width; lia).
    rewrite N.add_0_r.
    change (match write_bytes_meminst before (N.of_nat pointer) (serialise_i32 index) with
      | Some page => Some (upd_s_mem store [page]) | None => None end = Some (upd_s_mem store [after])).
    rewrite Copied; reflexivity.
  Qed.

  Lemma store_descriptor_available state store locals before pointer index :
    s_mems store = [before] -> store_typing store ->
    meminst_type before = Build_limits 1%N None -> mem_length before = page_size ->
    (Z.of_nat pointer < 2 ^ 32)%Z -> (N.of_nat pointer + 4 <= page_size)%N ->
    exists after,
      write_bytes_meminst before (N.of_nat pointer) (serialise_i32 index) = Some after /\
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [word pointer; descriptor index] ++
            [AI_basic (BI_store T_i32 None (Build_memarg 0%N 2%N))])
        (state, upd_s_mem store [after], Build_frame locals G.E.collector_instance, []) /\
      store_typing (upd_s_mem store [after]) /\
      meminst_type after = Build_limits 1%N None /\ mem_length after = page_size.
  Proof.
    intros Memory Typed TypeOf Length Pointer Bound.
    destruct (blob_copy_exists before (N.of_nat pointer) (serialise_i32 index)) as [after Copied].
    { rewrite serialise_i32_width, Length; exact Bound. }
    exists after; split; [exact Copied|]; split.
    - eapply store_descriptor_run; eassumption || (rewrite Length; exact Bound).
    - split.
      + eapply blob_copy_store_typing; eassumption.
      + destruct (blob_copy_shape _ _ _ _ Copied) as [SameType SameLength]; rewrite SameType, SameLength; auto.
  Qed.
End WasmResources.
