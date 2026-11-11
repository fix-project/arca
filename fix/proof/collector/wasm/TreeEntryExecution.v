Module WasmTreeEntryExecution.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Reconstruct := WasmReconstructionValidation.
  Import Reconstruct Reconstruct.T Reconstruct.T.A Reconstruct.T.A.V Reconstruct.T.A.V.V Reconstruct.T.A.V.V.Support.
  Module Prefix := WasmEntryPrefix.
  Module Output := WasmOutput.
  Module Issuance := WasmIssuance.
  Module Fields := Prefix.Fields.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.
  Ltac entry_pure := unfold Fields.entry_locals, Fields.V.Support.word, Fields.V.Support.descriptor; checked_pure.

  Theorem exported_tree_exchange_execution state store locals root metadata author opcode original fields witnesses :
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
        G.E.H.resolve next (wasm_nat index) = Some (treeh (Output.issued_tag author (treeh original) result 1)) /\
        G.E.H.resolve state (wasm_nat index) = None /\ scope state next (64 * (List.length (get_tree_raw witnesses) + 1)) /\
        s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Forall (Prefix.Authority.authorized author) (get_tree_raw witnesses) /\
        X.tree Eval (get_tree_raw witnesses) (treeh original) result = Some Eval /\
        X.Wire.encode Eval author (treeh original) result = Some (treeh (create_tree
          [treeh (Output.issued_tag author (treeh original) result 1);treeh (Output.issued_claim (treeh original) result 1)]))).
  Proof.
    intros Functions Typed Capacity Within Root Combination Decoded Tagged Untagged
      OriginalBound FieldsBound WitnessBound Bounds Bytes Room; cbn zeta.
    pose (result := treeh (create_tree_tagged true (get_tree_raw fields))).
    eapply checked_call with (code := evaluation_entry) (parameters := [T_num T_i32]) (results := [T_num T_i32])
      (defaults := repeat (word 0) 3) (address := 43%N) (arguments := [descriptor root]) (dummy := [word 0]); try reflexivity.
    - rewrite Functions; reflexivity.
    - pose proof (Prefix.collector_entry_prefix state store root metadata author opcode (treeh original) (treeh fields) witnesses 5
        Functions Typed Capacity Within Root Combination Decoded WitnessBound Bounds Bytes ltac:(lia)) as Prepare.
      lazymatch type of Prepare with Prefix.D.V.V.checked ?source ?post => change (checked source post) in Prepare end.
      unfold Fields.entry_locals, Fields.V.Support.word, Fields.V.Support.descriptor in Prepare.
      checked_use Prepare.
      intros final [prepared [copied [left_slot [right_slot [operation_slot
        [Final [PrefixScope [PreparedWithin [LeftLive [RightLive [OperationLive [CopiedFunctions [CopiedTyped [CopiedCapacity Authorized]]]]]]]]]]]]]];
        subst final; normalize_checked.
      pose proof (proj2 PrefixScope) as CountPrefix.
      pose (inside := Fields.entry_locals root left_slot right_slot operation_slot).
      assert (Selected : Fields.tree_request 5 (treeh original) (treeh fields) = true).
      { unfold Fields.tree_request; change ((true && tree_tag original && negb (tree_tag fields)) = true).
        rewrite Tagged,Untagged; reflexivity. }
      unfold Fields.entry_tail; rewrite evaluation_entry_body; cbn [skipn to_e_list seq.map List.app].
      rewrite Selected; entry_pure.
      lazymatch goal with
      | |- checked (?world,?memory,?frame,AI_label 0 [] ?body :: ?suffix) ?post =>
        let plug := constr:(LH_rec [] 0 [] (LH_base [] []) suffix) in
        eapply checked_context with (initial := (world,memory,frame,body)) (context := plug)
          (before := fun final => post (in_context 1 plug final));
        [|intros final Post; eapply checked_result; [apply rt_refl|exact Post]]
      end.
      entry_pure.
      assert (RootPrepared : G.E.H.resolve prepared root = Some (treeh combination))
        by (apply (proj1 PrefixScope); exact Root).
      pose proof (collector_reconstruction_validation prepared copied inside root original (treeh fields) witnesses
        CopiedFunctions CopiedTyped CopiedCapacity RootPrepared ltac:(rewrite Combination; reflexivity)
        ltac:(rewrite Combination; reflexivity) ltac:(rewrite Combination; reflexivity) ltac:(rewrite Combination; reflexivity)
        Tagged OriginalBound FieldsBound WitnessBound Bounds Bytes ltac:(lia)) as Reconstruction.
      checked_use Reconstruction.
      intros final [rebuilt [updated [replacement [field_tree [field_author [body
        [Final [FieldTree [Nonempty [ReplacementLive [ReplacementFresh [RebuiltScope [UpdatedFunctions [UpdatedTyped [UpdatedCapacity Tree]]]]]]]]]]]]]]];
        subst final; normalize_checked.
      assert (SameFields : field_tree = fields) by (inversion FieldTree; reflexivity); subst field_tree.
      pose proof (proj2 RebuiltScope) as CountRebuilt.
      entry_pure.
      pose (rebuilt_locals := Fields.entry_locals root left_slot replacement operation_slot).
      do 3 entry_pure.
      assert (RootRebuilt : G.E.H.resolve rebuilt root = Some (treeh combination))
        by (apply (proj1 RebuiltScope); exact RootPrepared).
      assert (OriginalRebuilt : G.E.H.resolve rebuilt (wasm_nat left_slot) = Some (treeh original))
        by (apply (proj1 RebuiltScope); exact LeftLive).
      assert (Author : nth_error (get_tree_raw combination) 1 = Some author) by (rewrite Combination; reflexivity).
      pose proof (Issuance.collector_issue_validation rebuilt updated rebuilt_locals root (wasm_nat left_slot) (wasm_nat replacement)
        (wasm_nat64 1) (treeh original) result author UpdatedFunctions UpdatedTyped UpdatedCapacity RootRebuilt
        OriginalRebuilt ReplacementLive Author ltac:(lia)) as Issue.
      cbn zeta in Issue.
      lazymatch type of Issue with Issuance.V.V.checked ?source ?post => change (checked source post) in Issue end.
      unfold Issuance.V.V.Support.word, Issuance.V.V.Support.descriptor in Issue.
      checked_use Issue.
      intros final [next [result_store [output_slot [Final [OutputLive [OutputFresh [OutputScope
        [ResultFunctions [ResultTyped ResultCapacity]]]]]]]]]; subst final; normalize_checked.
      assert (OutputNext : G.E.H.resolve next (wasm_nat output_slot) = Some
        (treeh (Output.issued_tag author (treeh original) result 1))).
      { unfold Output.issued_tag; rewrite <- Output.issued_claim_serialised.
        - exact OutputLive.
        - vm_compute; congruence. }
      eapply checked_result; [apply rt_refl|].
      exists [word output_slot]; split.
      + right; exists 1, (LH_rec [] 0 [] (LH_base [] []) (to_e_list [BI_local_get 0%N;BI_call 39%N])); reflexivity.
      + split; [reflexivity|].
        exists next,result_store,output_slot; split; [reflexivity|]; split; [exact OutputNext|]; split.
        * eapply fresh_before; [eapply retains_trans; [exact (proj1 PrefixScope)|exact (proj1 RebuiltScope)]|exact OutputFresh].
        * split.
          -- eapply scope_weaken; [eapply scope_trans; [exact PrefixScope|]; eapply scope_trans; eassumption|lia].
          -- split; [rewrite ResultFunctions; exact UpdatedFunctions|]; split; [exact ResultTyped|]; split; [exact ResultCapacity|];
               split; [exact Authorized|]; split; [exact Tree|].
             apply Output.collector_output_encodes; reflexivity.
  Qed.
End WasmTreeEntryExecution.
