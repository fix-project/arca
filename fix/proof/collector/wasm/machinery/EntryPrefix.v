Module WasmEntryPrefix.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module D := WasmDispatchSupport.
  Import D D.V D.V.V D.V.V.Support.
  Module Fields := WasmEntryFields.
  Module Authority := WasmAuthorityValidation.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Lemma collector_entry_prefix state store root metadata author opcode lhs rhs witnesses code :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.within_capacity state -> G.E.H.resolve state root = Some (treeh combination) ->
    get_tree_raw combination = [metadata;author;G.E.H.H.HBlobObj opcode;lhs;rhs;treeh witnesses] ->
    decode_u64 (get_blob_data opcode) = Some code -> lengths_bounded 1 (treeh witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + (5 + 7 * List.length (get_tree_raw witnesses)) <= G.E.H.capacity ->
    checked (state,store,Build_frame (Fields.entry_locals root 0 0 0) G.E.collector_instance,
      to_e_list (modfunc_body evaluation_entry))
      (fun final => exists next updated left_slot right_slot operation_slot,
        final = (next,updated,Build_frame (Fields.entry_locals root left_slot right_slot operation_slot) G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Fields.tree_request code lhs rhs)))] ++ to_e_list Fields.entry_tail) /\
        scope state next (5 + 7 * List.length (get_tree_raw witnesses)) /\ G.E.H.within_capacity next /\
        G.E.H.resolve next (wasm_nat left_slot) = Some lhs /\ G.E.H.resolve next (wasm_nat right_slot) = Some rhs /\
        G.E.H.resolve next (wasm_nat operation_slot) = Some (G.E.H.H.HBlobObj opcode) /\
        s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Forall (Authority.authorized author) (get_tree_raw witnesses)).
  Proof.
    intros Functions Typed Capacity Within Root Combination Decoded WitnessBound Bounds Bytes Room.
    pose (initial := Fields.entry_locals root 0 0 0).
    assert (Arity : List.length (get_tree_raw combination) = 6) by (rewrite Combination; reflexivity).
    pose proof (arity_run state store initial root combination 6 Functions Root Arity ltac:(vm_compute; congruence)) as ArityRun.
    assert (Author : nth_error (get_tree_raw combination) 1 = Some author) by (rewrite Combination; reflexivity).
    assert (Witnesses : nth_error (get_tree_raw combination) 5 = Some (treeh witnesses)) by (rewrite Combination; reflexivity).
    pose proof (Authority.collector_authority_validation state store initial root author witnesses Functions Bytes Root
      Author Witnesses WitnessBound Bounds ltac:(lia)) as Policy.
    lazymatch type of Policy with Authority.A.V.V.checked ?source ?post => change (checked source post) in Policy end.
    lazymatch type of Policy with checked ?source ?post =>
      pose proof (checked_capacity source post Within Policy) as PolicyWithin end.
    unfold Authority.A.V.V.Support.word, Authority.A.V.V.Support.descriptor in PolicyWithin.
    rewrite evaluation_entry_body; unfold Fields.entry_locals, Fields.V.Support.word, Fields.V.Support.descriptor.
    checked_pure; checked_advance ArityRun; checked_pure.
    checked_use PolicyWithin.
    intros final [[validated [Final [PolicyScope Authorized]]] ValidatedWithin]; subst final; normalize_checked.
    pose proof (proj2 PolicyScope) as Count0.
    destruct (Fields.collector_entry_fields validated store root metadata author opcode lhs rhs witnesses code
      Functions Typed Capacity (proj1 PolicyScope _ _ Root) Combination Decoded ltac:(lia))
      as [next [updated [left_slot [right_slot [operation_slot
        [Prepare [FieldsScope [LeftLive [RightLive [OperationLive [SameFunctions [NextTyped NextCapacity]]]]]]]]]]]].
    checked_advance Prepare.
    eapply checked_result; [apply rt_refl|].
    exists next,updated,left_slot,right_slot,operation_slot; split; [reflexivity|].
    split.
    - replace (5+7*List.length (get_tree_raw witnesses)) with
        ((2+7*List.length (get_tree_raw witnesses))+3) by lia.
      eapply scope_trans; eassumption.
    - split; [exact (G.E.collector_runs_capacity combination true true handle_bytes _ _ Prepare ValidatedWithin)|].
      split; [exact LeftLive|]; split; [exact RightLive|]; split; [exact OperationLive|].
      split; [rewrite SameFunctions; exact Functions|]; auto.
  Qed.
End WasmEntryPrefix.
