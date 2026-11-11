Module WasmReconstructionValidation.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module T := WasmTreeValidation.
  Import T T.A T.A.V T.A.V.V T.A.V.V.Support.
  Module Base := WasmRules.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Lemma unchecked_tag_spec value output : G.E.H.unchecked_tag true true value = Some output ->
    exists fields author body, value = treeh fields /\ get_tree_raw fields = author :: body /\
      output = treeh (create_tree_tagged true (get_tree_raw fields)).
  Proof.
    destruct value as [data|thunk|encode]; try discriminate.
    destruct data as [object|reference]; try discriminate.
    destruct object as [blob|fields]; try discriminate.
    unfold G.E.H.unchecked_tag.
    destruct (get_tree_raw fields) as [|author body] eqn:Fields; try discriminate.
    intro Equal; inversion Equal; subst output; exists fields,author,body.
    split; [reflexivity|]; split; [exact Fields|rewrite Fields; reflexivity].
  Qed.

  Lemma checked_unchecked_tag state store locals index value :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some value ->
    checked (state,store,Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor index] ++ [AI_basic (BI_call 18%N)])
      (fun final => exists next output result,
        final = (next,store,Build_frame locals G.E.collector_instance,v_to_e_list [word output]) /\
        G.E.H.unchecked_tag true true value = Some result /\ G.E.H.resolve next (wasm_nat output) = Some result /\
        G.E.H.resolve state (wasm_nat output) = None /\ scope state next 1).
  Proof.
    intros Functions Live.
    assert (Values : G.E.H.host_values combination G.E.collector_instance true true handle_bytes state store
      shell_create_tag_unchecked [descriptor index] = G.E.H.H.obind (G.E.H.unchecked_tag true true value) (G.E.H.allocate state)).
    { unfold G.E.H.host_values, G.E.H.transform, descriptor; rewrite Live; reflexivity. }
    destruct (G.E.H.unchecked_tag true true value) as [result|] eqn:Tag.
    - cbn [G.E.H.H.obind] in Values.
      pose proof (checked_new_handle state store locals 18%N shell_create_tag_unchecked [descriptor index] result
        Functions eq_refl eq_refl eq_refl ltac:(discriminate) Values) as Allocate.
      eapply checked_weaken; [exact Allocate|].
      intros final [next [output [Final [Present [Fresh [Scoped Obs]]]]]].
      exists next,output,result; auto 8.
    - cbn [G.E.H.H.obind] in Values.
      eapply checked_trap; exact (native_import_traps state store locals 18%N shell_create_tag_unchecked [descriptor index]
        Functions eq_refl eq_refl eq_refl ltac:(discriminate) Values).
  Qed.

  Theorem collector_reconstruction_validation state store locals root original fields witnesses :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state root = Some (treeh combination) -> List.length (get_tree_raw combination) = 6 ->
    nth_error (get_tree_raw combination) 3 = Some (treeh original) ->
    nth_error (get_tree_raw combination) 4 = Some fields ->
    nth_error (get_tree_raw combination) 5 = Some (treeh witnesses) -> tree_tag original = true ->
    lengths_bounded 1 (treeh original) -> lengths_bounded 1 fields -> lengths_bounded 1 (treeh witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + (4 + 10 * List.length (get_tree_raw witnesses)) <= G.E.H.capacity ->
    checked (state,store,Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor root] ++ [AI_basic (BI_call 41%N)])
      (fun final => exists next updated output field_tree author body,
        final = (next,updated,Build_frame locals G.E.collector_instance,v_to_e_list [word output]) /\
        fields = treeh field_tree /\ get_tree_raw field_tree = author :: body /\
        G.E.H.resolve next (wasm_nat output) = Some (treeh (create_tree_tagged true (get_tree_raw field_tree))) /\
        G.E.H.resolve state (wasm_nat output) = None /\ scope state next (4 + 10 * List.length (get_tree_raw witnesses)) /\
        s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        X.tree Eval (get_tree_raw witnesses) (treeh original)
          (treeh (create_tree_tagged true (get_tree_raw field_tree))) = Some Eval).
  Proof.
    intros Functions Typed Capacity Root Arity Original Fields Witnesses Tagged OriginalBound FieldsBound WitnessBound Bounds Bytes Room.
    pose (initial := [descriptor root;word 0;word 0;word 0;word 0]).
    destruct (child_run state store initial root combination 3 (treeh original) Functions Root Original ltac:(lia) ltac:(lia))
      as [first [original_slot [OriginalRun [OriginalLive [OriginalFresh [Scope1 Obs1]]]]]].
    pose proof (proj2 Scope1) as Count1.
    pose (locals1 := [descriptor root;word original_slot;word 0;word 0;word 0]).
    assert (Root1 : G.E.H.resolve first root = Some (treeh combination)) by (apply (proj1 Scope1); exact Root).
    destruct (child_run first store locals1 root combination 4 fields Functions Root1 Fields ltac:(lia) ltac:(lia))
      as [second [fields_slot [FieldsRun [FieldsLive [FieldsFresh [Scope2 Obs2]]]]]].
    pose proof (proj2 Scope2) as Count2.
    pose (locals2 := [descriptor root;word original_slot;word fields_slot;word 0;word 0]).
    assert (Root2 : G.E.H.resolve second root = Some (treeh combination)) by (apply (proj1 Scope2); exact Root1).
    destruct (child_run second store locals2 root combination 5 (treeh witnesses) Functions Root2 Witnesses ltac:(lia) ltac:(lia))
      as [third [witnesses_slot [WitnessRun [WitnessLive [WitnessFresh [Scope3 Obs3]]]]]].
    pose proof (proj2 Scope3) as Count3.
    pose (locals3 := [descriptor root;word original_slot;word fields_slot;word witnesses_slot;word 0]).
    eapply checked_call with (code := native_reconstruct) (parameters := [T_num T_i32]) (results := [T_num T_i32])
      (defaults := repeat (word 0) 4) (address := 41%N) (arguments := [descriptor root]) (dummy := [word 0]); try reflexivity.
    - rewrite Functions; reflexivity.
    - lazymatch goal with |- checked _ ?post =>
        change (checked (state,store,Build_frame initial G.E.collector_instance,to_e_list (modfunc_body native_reconstruct)) post) end.
      rewrite native_reconstruct_body.
      checked_pure; checked_advance (arity_run state store initial root combination 6 Functions Root Arity ltac:(vm_compute; congruence)).
      checked_pure; checked_advance OriginalRun; checked_pure.
      checked_pure; checked_advance (tag_run first store locals1 (wasm_nat original_slot) (treeh original) true Functions OriginalLive Tagged).
      checked_pure.
      checked_pure; checked_advance FieldsRun; checked_pure.
      checked_pure; checked_advance WitnessRun; checked_pure.
      checked_pure.
      checked_use (checked_unchecked_tag third store locals3 (wasm_nat fields_slot) fields Functions (proj1 Scope3 _ _ FieldsLive)).
      intros final [fourth [tag_slot [result [Final [Tag [TagLive [TagFresh Scope4]]]]]]]; subst final; normalize_checked.
      destruct (unchecked_tag_spec _ _ Tag) as [field_tree [author [body [FieldTree [Nonempty Result]]]]]; subst fields result.
      pose proof (proj2 Scope4) as Count4.
      pose (tag := create_tree_tagged true (get_tree_raw field_tree)).
      pose (inside := [descriptor root;word original_slot;word fields_slot;word witnesses_slot;word tag_slot]).
      assert (Original4 : G.E.H.resolve fourth (wasm_nat original_slot) = Some (treeh original))
        by (apply (proj1 Scope4), (proj1 Scope3), (proj1 Scope2); exact OriginalLive).
      assert (Witness4 : G.E.H.resolve fourth (wasm_nat witnesses_slot) = Some (treeh witnesses))
        by (apply (proj1 Scope4); exact WitnessLive).
      assert (TagBound : lengths_bounded 1 (treeh tag)).
      { unfold lengths_bounded in *; cbn in FieldsBound |- *; unfold tag; rewrite get_tree_raw_create_tree_tagged; exact FieldsBound. }
      checked_pure; do 3 checked_pure.
      checked_use (checked_tree_rule fourth store inside 1 Eval (wasm_nat original_slot) (wasm_nat tag_slot)
        (wasm_nat witnesses_slot) (treeh original) (treeh tag) witnesses Functions Typed Capacity Original4 TagLive Witness4
        OriginalBound TagBound WitnessBound Bounds eq_refl Bytes ltac:(lia)).
      intros final [fifth [updated [Final [Scope5 [UpdatedFunctions [UpdatedTyped [UpdatedCapacity Tree]]]]]]];
        subst final; normalize_checked.
      assert (Scoped : scope state third 3).
      { replace 3 with (1+(1+1)) by lia; eapply scope_trans; [exact Scope1|]; eapply scope_trans; eassumption. }
      assert (FinalScope : scope state fifth (4+10*List.length (get_tree_raw witnesses))).
      { replace (4+10*List.length (get_tree_raw witnesses)) with
          (3+(1+10*List.length (get_tree_raw witnesses))) by lia.
        eapply scope_trans; [exact Scoped|]; eapply scope_trans; eassumption. }
      assert (TagNext : G.E.H.resolve fifth (wasm_nat tag_slot) = Some (treeh tag))
        by (apply (proj1 Scope5); exact TagLive).
      checked_pure.
      eapply checked_result; [apply rt_refl|].
      exists [word tag_slot]; split; [left; reflexivity|]; split; [reflexivity|].
      exists fifth,updated,tag_slot,field_tree,author,body; split; [reflexivity|].
      split; [reflexivity|]; split; [exact Nonempty|]; split; [exact TagNext|]; split.
      + eapply fresh_before; [exact (proj1 Scoped)|exact TagFresh].
      + split; [exact FinalScope|]; split; [rewrite UpdatedFunctions; exact Functions|]; auto.
  Qed.
End WasmReconstructionValidation.
