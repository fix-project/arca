  Theorem exported_tree_exchange_sound_proof trusted state store locals root metadata author opcode original fields witnesses :
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
