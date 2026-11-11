Module WasmExport.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Import WasmEntryExecution WasmCollect WasmExchange WasmExchange.Native WasmExchange.Native.Dispatcher
    WasmExchange.Native.Dispatcher.D WasmExchange.Native.Dispatcher.D.V
    WasmExchange.Native.Dispatcher.D.V.V WasmExchange.Native.Dispatcher.D.V.V.Support.
  Module TreeExecution := WasmTreeEntryExecution.
  Module Policy := WasmAuthorityPolicy.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Theorem exported_exchange_refinement state store locals root metadata author opcode lhs rhs witnesses code selected rule :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.within_capacity state -> G.E.H.resolve state root = Some (treeh combination) ->
    get_tree_raw combination = [metadata;author;G.E.H.H.HBlobObj opcode;lhs;rhs;treeh witnesses] ->
    decode_u64 (get_blob_data opcode) = Some code ->
    Fields.tree_request code lhs rhs = false -> X.select_exchange code lhs = Some selected -> X.decode_exchange selected = Some rule ->
    lengths_bounded 1 lhs -> lengths_bounded 1 rhs -> lengths_bounded 1 (treeh witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + (64 * (List.length (get_tree_raw witnesses) + 1)) <= G.E.H.capacity ->
    checked (state,store,Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor root] ++ [AI_basic (BI_call 43%N)])
      (fun final => exists next updated index number kind,
        final = (next,updated,Build_frame locals G.E.collector_instance,v_to_e_list [word index]) /\
        G.E.H.resolve next (wasm_nat index) = Some (treeh (Output.issued_tag author lhs rhs number)) /\
        G.E.H.resolve state (wasm_nat index) = None /\ scope state next (64 * (List.length (get_tree_raw witnesses) + 1)) /\
        s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Forall (Prefix.Authority.authorized author) (get_tree_raw witnesses) /\
        Claims.Code.statement_kind number = Some kind /\ X.check rule (get_tree_raw witnesses) lhs rhs = Some kind /\
        X.issue author kind lhs rhs (get_tree_raw witnesses) (treeh (create_tree
          [treeh (Output.issued_tag author lhs rhs number);treeh (Output.issued_claim lhs rhs number)]))).
  Proof.
    intros Functions Typed Capacity Within Root Combination Decoded Ordinary Selected Rule
      LeftBound RightBound WitnessBound Bounds Bytes Room.
    eapply checked_weaken.
    - eapply WasmEntryExecution.exported_exchange_execution; eassumption.
    - intros final [next [updated [index [number [kind
        [Final [Present [Fresh [Scoped [NextFunctions [NextTyped [NextCapacity [Authorized [Kind [Checked Encoded]]]]]]]]]]]]]]].
      exists next,updated,index,number,kind; split; [exact Final|].
      split; [exact Present|]; split; [exact Fresh|]; split; [exact Scoped|]; split; [exact NextFunctions|].
      split; [exact NextTyped|]; split; [exact NextCapacity|]; split; [exact Authorized|]; split; [exact Kind|]; split; [exact Checked|].
      split; [apply Policy.authorized_policy; exact Authorized|exact Encoded].
  Qed.

  Theorem exported_tree_exchange_sound trusted state store locals root metadata author opcode original fields witnesses :
    trusted author -> Forall (X.stable_conditional trusted) (get_tree_raw witnesses) ->
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.within_capacity state -> G.E.H.resolve state root = Some (treeh combination) ->
    get_tree_raw combination = [metadata;author;G.E.H.H.HBlobObj opcode;treeh original;treeh fields;treeh witnesses] ->
    decode_u64 (get_blob_data opcode) = Some 5 -> tree_tag original = true -> tree_tag fields = false ->
    lengths_bounded 1 (treeh original) -> lengths_bounded 1 (treeh fields) -> lengths_bounded 1 (treeh witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + (64 * (List.length (get_tree_raw witnesses) + 1)) <= G.E.H.capacity ->
    let result := treeh (create_tree_tagged true (get_tree_raw fields)) in
    checked (state,store,Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor root] ++ [AI_basic (BI_call 43%N)])
      (fun final => exists next updated index,
        final = (next,updated,Build_frame locals G.E.collector_instance,v_to_e_list [word index]) /\
        G.E.H.resolve next (wasm_nat index) = Some (treeh (TreeExecution.Output.issued_tag author (treeh original) result 1)) /\
        G.E.H.resolve state (wasm_nat index) = None /\ scope state next (64 * (List.length (get_tree_raw witnesses) + 1)) /\
        s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Forall (TreeExecution.Prefix.Authority.authorized author) (get_tree_raw witnesses) /\
        X.D.coupon_eval (treeh original) result /\
        X.issue author Eval (treeh original) result (get_tree_raw witnesses) (treeh (create_tree
          [treeh (TreeExecution.Output.issued_tag author (treeh original) result 1);treeh (TreeExecution.Output.issued_claim (treeh original) result 1)]))).
  Proof.
    intros Trusted Conditional Functions Typed Capacity Within Root Combination Decoded Tagged Untagged
      OriginalBound FieldsBound WitnessBound Bounds Bytes Room; cbn zeta.
    eapply checked_weaken.
    - eapply TreeExecution.exported_tree_exchange_execution; eassumption.
    - intros final [next [updated [index
        [Final [Present [Fresh [Scoped [NextFunctions [NextTyped [NextCapacity [Authorized [Tree Encoded]]]]]]]]]]]].
      exists next,updated,index; split; [exact Final|].
      split; [exact Present|]; split; [exact Fresh|]; split; [exact Scoped|]; split; [exact NextFunctions|].
      split; [exact NextTyped|]; split; [exact NextCapacity|]; split; [exact Authorized|].
      pose proof (Policy.authorized_policy _ _ Authorized) as Allowed.
      split.
      + exact (X.reconstruction_validated_sound trusted author (get_tree_raw witnesses) (treeh original)
          (treeh (create_tree_tagged true (get_tree_raw fields))) Trusted Conditional Allowed Tree).
      + split; assumption.
  Qed.

  Lemma initial_descriptor_budget witnesses :
    WasmAuthorityGuard.witnesses_bounded witnesses ->
    G.E.H.D.live_count (G.E.H.slots (G.E.H.initial (treeh combination))) +
      64 * (List.length (get_tree_raw witnesses) + 1) <= G.E.H.capacity.
  Proof.
    intro Bound.
    change (1 + 64 * (List.length (get_tree_raw witnesses) + 1) <= G.E.H.capacity).
    apply Nat2Z.inj_le.
    rewrite Nat2Z.inj_add, Nat2Z.inj_mul, Nat2Z.inj_add, G.E.H.capacity_value.
    change ((1 + 64 * (Z.of_nat (List.length (get_tree_raw witnesses)) + 1)) <= 134217728)%Z.
    unfold WasmAuthorityGuard.witnesses_bounded, WasmAuthorityGuard.witness_limit in Bound; lia.
  Qed.
End WasmExport.
