Module WasmEntryExecution.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Import WasmCollect WasmExchange WasmExchange.Native WasmExchange.Native.Dispatcher WasmExchange.Native.Dispatcher.D
    WasmExchange.Native.Dispatcher.D.V WasmExchange.Native.Dispatcher.D.V.V WasmExchange.Native.Dispatcher.D.V.V.Support.
  Module Prefix := WasmEntryPrefix.
  Module Fields := Prefix.Fields.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Ltac entry_pure := unfold Fields.entry_locals, Fields.V.Support.word, Fields.V.Support.descriptor; dispatch_pure.

  Theorem exported_exchange_execution state store locals root metadata author opcode lhs rhs witnesses code selected rule :
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
        X.Wire.encode kind author lhs rhs = Some (treeh (create_tree
          [treeh (Output.issued_tag author lhs rhs number);treeh (Output.issued_claim lhs rhs number)]))).
  Proof.
    intros Functions Typed Capacity Within Root Combination Decoded Ordinary Selected Rule
      LeftBound RightBound WitnessBound Bounds Bytes Room.
    eapply checked_call with (code := evaluation_entry) (parameters := [T_num T_i32]) (results := [T_num T_i32])
      (defaults := repeat (word 0) 3) (address := 43%N) (arguments := [descriptor root]) (dummy := [word 0]); try reflexivity.
    - rewrite Functions; reflexivity.
    - pose proof (Prefix.collector_entry_prefix state store root metadata author opcode lhs rhs witnesses code
        Functions Typed Capacity Within Root Combination Decoded WitnessBound Bounds Bytes ltac:(lia)) as Prepare.
      lazymatch type of Prepare with Prefix.D.V.V.checked ?source ?post => change (checked source post) in Prepare end.
      unfold Fields.entry_locals, Fields.V.Support.word, Fields.V.Support.descriptor in Prepare.
      checked_use Prepare.
      intros final [prepared [copied [left_slot [right_slot [operation_slot
        [Final [PrefixScope [PreparedWithin [LeftLive [RightLive [OperationLive [CopiedFunctions [CopiedTyped [CopiedCapacity Authorized]]]]]]]]]]]]]];
        subst final; normalize_checked.
      pose proof (proj2 PrefixScope) as Count.
      pose (inside := Fields.entry_locals root left_slot right_slot operation_slot).
      unfold Fields.entry_tail; rewrite evaluation_entry_body; cbn [skipn to_e_list seq.map List.app].
      rewrite Ordinary; do 2 entry_pure.
      pose proof (collector_collect_validation prepared copied inside root metadata author opcode lhs rhs witnesses code selected rule
        CopiedFunctions CopiedTyped CopiedCapacity PreparedWithin (proj1 PrefixScope _ _ Root)
        Combination Decoded Selected Rule LeftBound RightBound WitnessBound Bounds Bytes ltac:(lia)) as Collected.
      checked_use Collected.
      intros final [next [updated [index [number [kind
        [Final [Present [Fresh [OutputScope [NextFunctions [NextTyped [NextCapacity [Kind [Checked Encoded]]]]]]]]]]]]]];
        subst final; normalize_checked.
      eapply checked_result; [apply rt_refl|].
      exists [word index]; split; [left; reflexivity|]; split; [reflexivity|].
      exists next,updated,index,number,kind; split; [reflexivity|]; split; [exact Present|]; split.
      + eapply fresh_before; [exact (proj1 PrefixScope)|exact Fresh].
      + split.
        * eapply scope_weaken; [eapply scope_trans; eassumption|lia].
        * split; [exact NextFunctions|]; split; [exact NextTyped|]; split; [exact NextCapacity|];
            split; [exact Authorized|]; split; [exact Kind|]; split; [exact Checked|exact Encoded].
  Qed.
End WasmEntryExecution.
