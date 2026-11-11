  Theorem exported_exchange_refinement_proof state store locals root metadata author opcode lhs rhs witnesses code selected rule :
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
