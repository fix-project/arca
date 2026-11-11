Module WasmCollect.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Import WasmExchange WasmExchange.Native WasmExchange.Native.Dispatcher WasmExchange.Native.Dispatcher.D WasmExchange.Native.Dispatcher.D.V WasmExchange.Native.Dispatcher.D.V.V WasmExchange.Native.Dispatcher.D.V.V.Support.
  Module Prefix := WasmCollectPrefix.
  Module Issuance := WasmIssuance.
  Module Output := WasmOutput.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Ltac collect_pure := unfold Prefix.collect_locals, Prefix.Base.Support.word, Prefix.Base.Support.descriptor; dispatch_pure.

  Theorem collector_collect_validation state store locals root metadata author opcode lhs rhs witnesses code selected rule :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.within_capacity state -> G.E.H.resolve state root = Some (treeh combination) ->
    get_tree_raw combination = [metadata;author;G.E.H.H.HBlobObj opcode;lhs;rhs;treeh witnesses] ->
    decode_u64 (get_blob_data opcode) = Some code ->
    X.select_exchange code lhs = Some selected -> X.decode_exchange selected = Some rule ->
    lengths_bounded 1 lhs -> lengths_bounded 1 rhs -> lengths_bounded 1 (treeh witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + (10 + 32 * (List.length (get_tree_raw witnesses) + 1)) <= G.E.H.capacity ->
    checked (state,store,Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor root] ++ [AI_basic (BI_call 39%N)])
      (fun final => exists next updated index number kind,
        final = (next,updated,Build_frame locals G.E.collector_instance,v_to_e_list [word index]) /\
        G.E.H.resolve next (wasm_nat index) = Some (treeh (Output.issued_tag author lhs rhs number)) /\
        G.E.H.resolve state (wasm_nat index) = None /\ scope state next (10 + 32 * (List.length (get_tree_raw witnesses) + 1)) /\
        s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Claims.Code.statement_kind number = Some kind /\ X.check rule (get_tree_raw witnesses) lhs rhs = Some kind /\
        X.Wire.encode kind author lhs rhs = Some (treeh (create_tree
          [treeh (Output.issued_tag author lhs rhs number);treeh (Output.issued_claim lhs rhs number)]))).
  Proof.
    intros Functions Typed Capacity Within Root Fields Decoded Selected Rule LeftBound RightBound WitnessBound WitnessBounds Bytes Room.
    destruct (Prefix.collector_collect_prefix state store root metadata author opcode lhs rhs witnesses code
      Functions Typed Capacity Root Fields Decoded ltac:(lia))
      as [prepared [copied [left_slot [right_slot [operation_slot [witnesses_slot
        [Prepare [Scoped [LeftLive [RightLive [OperationLive [WitnessLive [CopiedFunctions [CopiedTyped CopiedCapacity]]]]]]]]]]]]]].
    pose proof (proj2 Scoped) as Count4.
    assert (PreparedWithin : G.E.H.within_capacity prepared).
    { exact (G.E.collector_runs_capacity combination true true handle_bytes _ _ Prepare Within). }
    assert (FunctionsCopied : s_funcs copied = s_funcs G.E.allocated_store) by (rewrite CopiedFunctions; exact Functions).
    pose (inside := Prefix.collect_locals root left_slot right_slot witnesses_slot operation_slot 0).
    eapply checked_call with (code := evaluation_apply)
      (parameters := [T_num T_i32]) (results := [T_num T_i32])
      (defaults := [word 0;word 0;word 0;word 0;VAL_num (VAL_int64 (wasm_nat64 0))])
      (address := 39%N) (arguments := [descriptor root]) (dummy := [word 0]); try reflexivity.
    - rewrite Functions; reflexivity.
    - eapply checked_progress; [exact Prepare|].
      unfold Prefix.collect_tail; rewrite evaluation_apply_body; cbn [skipn to_e_list seq.map List.app].
      do 3 collect_pure.
      pose proof (collector_family_validation prepared copied inside code selected rule
        (wasm_nat left_slot) (wasm_nat right_slot) (wasm_nat witnesses_slot) lhs rhs witnesses
        (decode_u64_bound _ _ Decoded) Selected Rule FunctionsCopied CopiedTyped CopiedCapacity PreparedWithin
        LeftLive RightLive WitnessLive LeftBound RightBound WitnessBound WitnessBounds Bytes ltac:(lia)) as Exchange.
      unfold Base.rule_arguments, Base.Support.descriptor, Base.Support.word in Exchange.
      checked_use Exchange.
      intros final [dispatched [updated [number [kind
        [Final [DispatchScope [UpdatedFunctions [UpdatedTyped [UpdatedCapacity [Kind Checked]]]]]]]]]]; subst final; normalize_checked.
      pose proof (proj2 DispatchScope) as CountDispatch.
      pose (arguments := Prefix.collect_locals root left_slot right_slot witnesses_slot operation_slot number).
      do 5 collect_pure.
      assert (RootPrepared : G.E.H.resolve prepared root = Some (treeh combination)) by (apply (proj1 Scoped); exact Root).
      assert (Author : nth_error (get_tree_raw combination) 1 = Some author) by (rewrite Fields; reflexivity).
      pose proof (Issuance.collector_issue_validation dispatched updated arguments root (wasm_nat left_slot) (wasm_nat right_slot)
        (wasm_nat64 number) lhs rhs author UpdatedFunctions UpdatedTyped UpdatedCapacity
        (proj1 DispatchScope _ _ RootPrepared) (proj1 DispatchScope _ _ LeftLive) (proj1 DispatchScope _ _ RightLive)
        Author ltac:(lia)) as Issue.
      cbn zeta in Issue.
      lazymatch type of Issue with Issuance.V.V.checked ?source ?post => change (checked source post) in Issue end.
      unfold Issuance.V.V.Support.descriptor, Issuance.V.V.Support.word in Issue.
      checked_use Issue.
      intros final [issued [result_store [output_slot [Final [OutputLive [OutputFresh [IssueScope [ResultFunctions [ResultTyped ResultCapacity]]]]]]]]];
        subst final; normalize_checked.
      assert (OutputNext : G.E.H.resolve issued (wasm_nat output_slot) = Some
        (treeh (Output.issued_tag author lhs rhs number))).
      { unfold Output.issued_tag; rewrite <- Output.issued_claim_serialised.
        - exact OutputLive.
        - pose proof (Claims.Code.statement_code_bound _ _ (Claims.Code.statement_kind_roundtrip _ _ Kind)); cbn; lia. }
      assert (FinalScope : scope state issued (10+32*(List.length (get_tree_raw witnesses)+1))).
      { replace (10+32*(List.length (get_tree_raw witnesses)+1)) with
          (4+(32*(List.length (get_tree_raw witnesses)+1)+6)) by lia.
        eapply scope_trans; [exact Scoped|]; eapply scope_trans; eassumption. }
      eapply checked_result; [apply rt_refl|].
      exists [word output_slot]; split; [left; reflexivity|]; split; [reflexivity|].
      exists issued,result_store,output_slot,number,kind; split; [reflexivity|].
      split; [exact OutputNext|]; split.
      + eapply fresh_before; [eapply retains_trans; [exact (proj1 Scoped)|exact (proj1 DispatchScope)]|exact OutputFresh].
      + split; [exact FinalScope|]; split; [rewrite ResultFunctions; exact UpdatedFunctions|].
        split; [exact ResultTyped|]; split; [exact ResultCapacity|]; split; [exact Kind|]; split; [exact Checked|].
        apply Output.collector_output_encodes; exact Kind.
  Qed.
End WasmCollect.
