Module WasmCollectPrefix.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Base := WasmRules.
  Import Base Base.Support.
  Module Resources := WasmResources.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.

  Definition collect_locals root lhs rhs witnesses operation code :=
    [descriptor root;word lhs;word rhs;word operation;word witnesses;
      VAL_num (VAL_int64 (wasm_nat64 code))].

  Definition collect_tail := skipn 21 (modfunc_body evaluation_apply).

  Lemma collector_collect_prefix state store root metadata author opcode lhs rhs witnesses code :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state root = Some (G.E.H.H.HTreeObj combination) ->
    get_tree_raw combination = [metadata;author;G.E.H.H.HBlobObj opcode;lhs;rhs;G.E.H.H.HTreeObj witnesses] ->
    decode_u64 (get_blob_data opcode) = Some code ->
    G.E.H.D.live_count (G.E.H.slots state) + 4 <= G.E.H.capacity ->
    exists next updated left_slot right_slot operation_slot witnesses_slot,
      reduce_trans
        (state,store,Build_frame ([descriptor root] ++
          [word 0;word 0;word 0;word 0;VAL_num (VAL_int64 (wasm_nat64 0))]) G.E.collector_instance,
          to_e_list (modfunc_body evaluation_apply))
        (next,updated,Build_frame (collect_locals root left_slot right_slot witnesses_slot operation_slot 0) G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 code))] ++ to_e_list collect_tail) /\
      scope state next 4 /\
      G.E.H.resolve next (wasm_nat left_slot) = Some lhs /\
      G.E.H.resolve next (wasm_nat right_slot) = Some rhs /\
      G.E.H.resolve next (wasm_nat operation_slot) = Some (G.E.H.H.HBlobObj opcode) /\
      G.E.H.resolve next (wasm_nat witnesses_slot) = Some (G.E.H.H.HTreeObj witnesses) /\
      s_funcs updated = s_funcs store /\ store_typing updated /\ Resources.memory_capacity updated.
  Proof.
    intros Functions Typed Capacity Root Fields Decoded Room.
    pose (initial := collect_locals root 0 0 0 0 0).
    destruct (child_run state store initial root combination 3 lhs Functions Root
      ltac:(rewrite Fields; reflexivity) ltac:(lia) ltac:(lia))
      as [first [left_slot [LeftRun [LeftLive [LeftFresh [Scope1 Obs1]]]]]].
    pose proof (proj2 Scope1) as Count1.
    pose (locals1 := collect_locals root left_slot 0 0 0 0).
    assert (Root1 : G.E.H.resolve first root = Some (G.E.H.H.HTreeObj combination)) by (apply (proj1 Scope1); exact Root).
    destruct (child_run first store locals1 root combination 4 rhs Functions Root1
      ltac:(rewrite Fields; reflexivity) ltac:(lia) ltac:(lia))
      as [second [right_slot [RightRun [RightLive [RightFresh [Scope2 Obs2]]]]]].
    pose proof (proj2 Scope2) as Count2.
    pose (locals2 := collect_locals root left_slot right_slot 0 0 0).
    assert (Root2 : G.E.H.resolve second root = Some (G.E.H.H.HTreeObj combination)) by (apply (proj1 Scope2); exact Root1).
    destruct (child_run second store locals2 root combination 2 (G.E.H.H.HBlobObj opcode) Functions Root2
      ltac:(rewrite Fields; reflexivity) ltac:(lia) ltac:(lia))
      as [third [operation_slot [OperationRun [OperationLive [OperationFresh [Scope3 Obs3]]]]]].
    pose proof (proj2 Scope3) as Count3.
    pose (locals3 := collect_locals root left_slot right_slot 0 operation_slot 0).
    assert (Root3 : G.E.H.resolve third root = Some (G.E.H.H.HTreeObj combination)) by (apply (proj1 Scope3); exact Root2).
    destruct (child_run third store locals3 root combination 5 (G.E.H.H.HTreeObj witnesses) Functions Root3
      ltac:(rewrite Fields; reflexivity) ltac:(lia) ltac:(lia))
      as [next [witnesses_slot [WitnessRun [WitnessLive [WitnessFresh [Scope4 Obs4]]]]]].
    pose (inside := collect_locals root left_slot right_slot witnesses_slot operation_slot 0).
    assert (Operation4 : G.E.H.resolve next (wasm_nat operation_slot) = Some (G.E.H.H.HBlobObj opcode))
      by (apply (proj1 Scope4); exact OperationLive).
    destruct (Resources.integer_available next store inside (wasm_nat operation_slot) opcode code
      Functions Typed Capacity Operation4 Decoded)
      as [updated [IntegerRun [SameFunctions [NextTyped NextCapacity]]]].
    exists next,updated,left_slot,right_slot,operation_slot,witnesses_slot; split.
    - rewrite evaluation_apply_body.
      native_pure.
      execute (arity_run state store initial root combination 6 Functions Root
        ltac:(rewrite Fields; reflexivity) ltac:(unfold native_size_bound, native_table_limit; cbn; lia)).
      native_pure; execute LeftRun; native_pure.
      native_pure; execute RightRun; native_pure.
      native_pure; execute OperationRun; native_pure.
      native_pure; execute WitnessRun; native_pure.
      native_pure; execute IntegerRun; apply rt_refl.
    - split.
      + replace 4 with (1+(1+(1+1))) by lia.
        eapply scope_trans; [exact Scope1|]; eapply scope_trans; [exact Scope2|].
        eapply scope_trans; eassumption.
      + split; [apply (proj1 Scope4), (proj1 Scope3), (proj1 Scope2); exact LeftLive|].
        split; [apply (proj1 Scope4), (proj1 Scope3); exact RightLive|].
        split; [exact Operation4|]; split; [exact WitnessLive|]; auto.
  Qed.
End WasmCollectPrefix.
