Module WasmInteger.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module V := WasmValidation.
  Import V V.V V.V.Support.
  Module Resources := WasmResources.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.

  Lemma integer_reject_type state store locals index input :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some input ->
    (forall blob, input <> G.E.H.H.HBlobObj blob) ->
    reduce_trans (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor index] ++ [AI_basic (BI_call 27%N)])
      (state, store, Build_frame locals G.E.collector_instance, [AI_trap]).
  Proof.
    intros Functions Live NotBlob; destruct input as [[[blob|node]|reference]|thunk|encoded].
    { exfalso; apply (NotBlob blob); reflexivity. }
    all: eapply G.E.collector_call with (code := evaluation_integer)
      (parameters := [T_num T_i32]) (results := [T_num T_i64]) (defaults := [])
      (address := 27%N) (arguments := [descriptor index]) (values := [VAL_num (VAL_int64 (wasm_nat64 0))])
      (accepted := false) (final_frame := Build_frame [descriptor index] G.E.collector_instance); try reflexivity.
    all: try solve [rewrite Functions; reflexivity].
    all: rewrite evaluation_integer_body;
      execute (native_type_query state store [descriptor index] 0%N index _ Functions eq_refl Live);
      do 2 native_pure.
    all: try solve [eapply G.E.collector_trap_suffix with (instructions := [AI_trap]); apply rt_refl].
    execute (native_data_type_query state store [descriptor index] 0%N index (Object (TreeObj node)) Functions eq_refl Live).
    do 2 native_pure; eapply G.E.collector_trap_suffix with (instructions := [AI_trap]); apply rt_refl.
  Qed.

  Lemma integer_reject_width state store locals index blob :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some (G.E.H.H.HBlobObj blob) ->
    (Z.of_nat (List.length (get_blob_data blob)) < 2 ^ 64)%Z -> List.length (get_blob_data blob) <> 8 ->
    reduce_trans (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor index] ++ [AI_basic (BI_call 27%N)])
      (state, store, Build_frame locals G.E.collector_instance, [AI_trap]).
  Proof.
    intros Functions Live Bound Width.
    eapply G.E.collector_call with (code := evaluation_integer)
      (parameters := [T_num T_i32]) (results := [T_num T_i64]) (defaults := [])
      (address := 27%N) (arguments := [descriptor index]) (values := [VAL_num (VAL_int64 (wasm_nat64 0))])
      (accepted := false) (final_frame := Build_frame [descriptor index] G.E.collector_instance); try reflexivity.
    - rewrite Functions; reflexivity.
    - rewrite evaluation_integer_body.
      execute (native_type_query state store [descriptor index] 0%N index (G.E.H.H.HBlobObj blob) Functions eq_refl Live).
      do 2 native_pure.
      execute (native_data_type_query state store [descriptor index] 0%N index (Object (BlobObj blob)) Functions eq_refl Live).
      do 2 native_pure.
      execute (native_length_query state store [descriptor index] 0%N index (Object (BlobObj blob)) Functions eq_refl Live Bound).
      change (G.E.H.T.O.data_length (Object (BlobObj blob))) with (List.length (get_blob_data blob)).
      pose proof (wasm_nat64_equal (List.length (get_blob_data blob)) 8 Bound ltac:(vm_compute; congruence)) as Compared.
      pose proof (native_relop state store [descriptor index]
        (VAL_int64 (wasm_nat64 (List.length (get_blob_data blob)))) (VAL_int64 (wasm_nat64 8))
        T_i64 (Relop_i ROI_eq) eq_refl) as Compare.
      rewrite Compared in Compare.
      assert (Different : Nat.eqb (List.length (get_blob_data blob)) 8 = false) by (apply Nat.eqb_neq; exact Width).
      rewrite Different in Compare; execute Compare.
      eapply G.E.collector_trap_suffix with
        (instructions := v_to_e_list [VAL_num (VAL_int32 (wasm_bool false))] ++ [AI_basic (BI_call 22%N)]).
      exact (native_require state store [descriptor index] false Functions).
  Qed.

  Lemma checked_integer state store locals index input :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state index = Some input -> lengths_bounded 1 input ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor index] ++ [AI_basic (BI_call 27%N)])
      (fun final => exists next blob number,
        final = (state, next, Build_frame locals G.E.collector_instance, v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 number))]) /\
        input = G.E.H.H.HBlobObj blob /\ decode_u64 (get_blob_data blob) = Some number /\
        s_funcs next = s_funcs store /\ store_typing next /\ Resources.memory_capacity next).
  Proof.
    intros Functions Typed Capacity Live Bounds.
    destruct input as [[[blob|node]|reference]|thunk|encoded];
      try solve [eapply checked_trap; eapply integer_reject_type; [exact Functions|exact Live|intros; discriminate]].
    destruct (Nat.eq_dec (List.length (get_blob_data blob)) 8) as [Width|WrongWidth].
    - assert (Decoded : decode_u64 (get_blob_data blob) = Some (Z.to_nat (Memdata.int_of_bytes (get_blob_data blob)))).
      { change ((if Nat.eqb (List.length (get_blob_data blob)) 8 then
          Some (Z.to_nat (Memdata.int_of_bytes (get_blob_data blob))) else None) = Some (Z.to_nat (Memdata.int_of_bytes (get_blob_data blob)))) .
        rewrite Width; reflexivity. }
      destruct (Resources.integer_available state store locals index blob _ Functions Typed Capacity Live Decoded)
        as [next [Run [NextFunctions [NextTyped NextCapacity]]]].
      eapply checked_result; [exact Run|]; exists next, blob, (Z.to_nat (Memdata.int_of_bytes (get_blob_data blob))).
      repeat split; try reflexivity; assumption.
    - eapply checked_trap; eapply integer_reject_width; eassumption.
  Qed.
End WasmInteger.
