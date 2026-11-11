  Definition entry_tail := skipn 28 (modfunc_body evaluation_entry).

  Lemma collector_entry_fields state store root metadata author opcode lhs rhs witnesses code :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state root = Some (G.E.H.H.HTreeObj combination) ->
    get_tree_raw combination = [metadata;author;G.E.H.H.HBlobObj opcode;lhs;rhs;G.E.H.H.HTreeObj witnesses] ->
    decode_u64 (get_blob_data opcode) = Some code ->
    G.E.H.D.live_count (G.E.H.slots state) + 3 <= G.E.H.capacity ->
    exists next updated left_slot right_slot operation_slot,
      reduce_trans
        (state,store,Build_frame (entry_locals root 0 0 0) G.E.collector_instance,
          to_e_list (skipn 5 (modfunc_body evaluation_entry)))
        (next,updated,Build_frame (entry_locals root left_slot right_slot operation_slot) G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int32 (wasm_bool (tree_request code lhs rhs)))] ++ to_e_list entry_tail) /\
      scope state next 3 /\
      G.E.H.resolve next (wasm_nat left_slot) = Some lhs /\ G.E.H.resolve next (wasm_nat right_slot) = Some rhs /\
      G.E.H.resolve next (wasm_nat operation_slot) = Some (G.E.H.H.HBlobObj opcode) /\
      s_funcs updated = s_funcs store /\ store_typing updated /\ Resources.memory_capacity updated.
  Proof.
    intros Functions Typed Capacity Root Fields Decoded Room.
    pose (initial := entry_locals root 0 0 0).
    destruct (child_run state store initial root combination 3 lhs Functions Root
      ltac:(rewrite Fields; reflexivity) ltac:(lia) ltac:(lia))
      as [first [left_slot [LeftRun [LeftLive [LeftFresh [Scope1 Obs1]]]]]].
    pose proof (proj2 Scope1) as Count1.
    pose (locals1 := entry_locals root left_slot 0 0).
    assert (Root1 : G.E.H.resolve first root = Some (G.E.H.H.HTreeObj combination)) by (apply (proj1 Scope1); exact Root).
    destruct (child_run first store locals1 root combination 4 rhs Functions Root1
      ltac:(rewrite Fields; reflexivity) ltac:(lia) ltac:(lia))
      as [second [right_slot [RightRun [RightLive [RightFresh [Scope2 Obs2]]]]]].
    pose proof (proj2 Scope2) as Count2.
    pose (locals2 := entry_locals root left_slot right_slot 0).
    assert (Root2 : G.E.H.resolve second root = Some (G.E.H.H.HTreeObj combination)) by (apply (proj1 Scope2); exact Root1).
    destruct (child_run second store locals2 root combination 2 (G.E.H.H.HBlobObj opcode) Functions Root2
      ltac:(rewrite Fields; reflexivity) ltac:(lia) ltac:(lia))
      as [next [operation_slot [OperationRun [OperationLive [OperationFresh [Scope3 Obs3]]]]]].
    pose (inside := entry_locals root left_slot right_slot operation_slot).
    assert (Left3 : G.E.H.resolve next (wasm_nat left_slot) = Some lhs)
      by (apply (proj1 Scope3), (proj1 Scope2); exact LeftLive).
    assert (Right3 : G.E.H.resolve next (wasm_nat right_slot) = Some rhs)
      by (apply (proj1 Scope3); exact RightLive).
    destruct (Resources.integer_available next store inside (wasm_nat operation_slot) opcode code
      Functions Typed Capacity OperationLive Decoded) as [updated [IntegerRun [SameFunctions [NextTyped NextCapacity]]]].
    assert (FunctionsUpdated : s_funcs updated = s_funcs G.E.allocated_store) by (rewrite SameFunctions; exact Functions).
    exists next,updated,left_slot,right_slot,operation_slot; split.
    - { rewrite evaluation_entry_body; cbn [skipn to_e_list seq.map List.app].
      native_pure; execute LeftRun; native_pure.
      native_pure; execute RightRun; native_pure.
      native_pure; execute OperationRun; native_pure.
      native_pure; execute IntegerRun.
      pose proof (native_relop next updated inside (VAL_int64 (wasm_nat64 code)) (VAL_int64 (wasm_nat64 5))
        T_i64 (Relop_i ROI_eq) eq_refl) as Compare.
      rewrite (wasm_nat64_equal code 5 (decode_u64_bound _ _ Decoded) ltac:(vm_compute; reflexivity)) in Compare.
      execute Compare.
      unfold tree_request.
      destruct (Nat.eqb code 5) eqn:Test; destruct (G.E.H.Api.is_tag lhs) eqn:LeftTag; destruct (G.E.H.Api.is_tag rhs) eqn:RightTag.
      all: cbn [andb negb].
      all: native_pure.
      all: pose proof (tag_run next updated inside (wasm_nat left_slot) lhs _ FunctionsUpdated Left3 LeftTag) as Left;
        first [execute_under [word 1] Left|execute_under [word 0] Left].
      all: do 2 native_pure.
      all: pose proof (tag_run next updated inside (wasm_nat right_slot) rhs _ FunctionsUpdated Right3 RightTag) as Right;
        first [execute_under [word 1] Right|execute_under [word 0] Right].
      all: do 2 native_pure; apply rt_refl.
      }
    - split.
      + replace 3 with (1+(1+1)) by lia; eapply scope_trans; [exact Scope1|].
        eapply scope_trans; eassumption.
      + auto 8.
  Qed.
