Module WasmReconstruction.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Support := WasmSupport.
  Import Support.
  Module Resources := WasmResources.
  Module Trees := WasmTreeLoop.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Ltac keep := first [assumption | match goal with
    | Scoped : scope ?before ?after ?extra |- G.E.H.resolve ?after ?index = Some ?value =>
      apply (proj1 Scoped); keep
    end].

  Lemma collector_reconstruction state store locals root original fields witnesses author body :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state root = Some (treeh combination) -> List.length (get_tree_raw combination) = 6 ->
    nth_error (get_tree_raw combination) 3 = Some (treeh original) ->
    nth_error (get_tree_raw combination) 4 = Some (treeh fields) ->
    nth_error (get_tree_raw combination) 5 = Some (treeh witnesses) ->
    tree_tag original = true -> get_tree_raw fields = author :: body ->
    List.length (get_tree_raw fields) = List.length (get_tree_raw original) ->
    List.length (get_tree_raw witnesses) = List.length (get_tree_raw original) ->
    native_size_bound (List.length (get_tree_raw original)) ->
    (forall current index, retains state current -> index < List.length (get_tree_raw original) ->
      Trees.iteration_ready current original (create_tree_tagged true (get_tree_raw fields)) witnesses 1 index) ->
    G.E.H.D.live_count (G.E.H.slots state) + (4 + 10 * List.length (get_tree_raw original)) <= G.E.H.capacity ->
    exists next updated output,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [descriptor root] ++ [AI_basic (BI_call 41%N)])
        (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [word output]) /\
      G.E.H.resolve next (wasm_nat output) = Some (treeh (create_tree_tagged true (get_tree_raw fields))) /\
      G.E.H.resolve state (wasm_nat output) = None /\ scope state next (4 + 10 * List.length (get_tree_raw original)) /\
      s_funcs updated = s_funcs store /\ store_typing updated /\ Resources.memory_capacity updated.
  Proof.
    intros Functions Typed Capacity Root Arity Original Fields Witnesses Tagged Nonempty FieldCount WitnessCount Bound Ready Room.
    set (tag := create_tree_tagged true (get_tree_raw fields)).
    set (initial := [descriptor root; word 0; word 0; word 0; word 0]).
    destruct (child_run state store initial root combination 3 (treeh original) Functions Root Original ltac:(lia) ltac:(lia))
      as [first [original_slot [OriginalRun [OriginalLive [OriginalFresh [Scope1 Obs1]]]]]].
    pose proof (proj2 Scope1) as Count1.
    set (locals1 := [descriptor root; word original_slot; word 0; word 0; word 0]).
    assert (Root1 : G.E.H.resolve first root = Some (treeh combination)) by keep.
    destruct (child_run first store locals1 root combination 4 (treeh fields) Functions Root1 Fields ltac:(lia) ltac:(lia))
      as [second [fields_slot [FieldsRun [FieldsLive [FieldsFresh [Scope2 Obs2]]]]]].
    pose proof (proj2 Scope2) as Count2.
    set (locals2 := [descriptor root; word original_slot; word fields_slot; word 0; word 0]).
    assert (Root2 : G.E.H.resolve second root = Some (treeh combination)) by keep.
    destruct (child_run second store locals2 root combination 5 (treeh witnesses) Functions Root2 Witnesses ltac:(lia) ltac:(lia))
      as [third [witnesses_slot [WitnessesRun [WitnessesLive [WitnessesFresh [Scope3 Obs3]]]]]].
    pose proof (proj2 Scope3) as Count3.
    set (locals3 := [descriptor root; word original_slot; word fields_slot; word witnesses_slot; word 0]).
    assert (Fields3 : G.E.H.resolve third (wasm_nat fields_slot) = Some (treeh fields)) by keep.
    assert (TagValues : G.E.H.host_values combination G.E.collector_instance true true handle_bytes third store
      shell_create_tag_unchecked [word fields_slot] = G.E.H.allocate third (treeh tag)).
    { unfold G.E.H.host_values, G.E.H.transform, word; rewrite Fields3.
      cbn [G.E.H.H.obind G.E.H.unchecked_tag G.E.H.H.HTreeObj].
      change (G.E.H.H.obind
        (match get_tree_raw fields with [] => None | a :: rest => Some (treeh (create_tree_tagged true (a :: rest))) end)
        (G.E.H.allocate third) = G.E.H.allocate third (treeh tag)).
      unfold tag; rewrite Nonempty; reflexivity. }
    destruct (new_handle_run third store locals3 18%N shell_create_tag_unchecked [word fields_slot] (treeh tag)
      Functions eq_refl eq_refl eq_refl ltac:(discriminate) TagValues ltac:(lia))
      as [fourth [tag_slot [TagRun [TagLive [TagFresh [Scope4 Obs4]]]]]].
    set (inside := [descriptor root; word original_slot; word fields_slot; word witnesses_slot; word tag_slot]).
    assert (Scope03 : scope state third 3).
    { replace 3 with (1+(1+1)) by lia; eapply scope_trans; [exact Scope1|]; eapply scope_trans; eassumption. }
    assert (Scope04 : scope state fourth 4).
    { replace 4 with (3+1) by lia; eapply scope_trans; eassumption. }
    pose proof (proj2 Scope04) as Count4.
    assert (Original4 : G.E.H.resolve fourth (wasm_nat original_slot) = Some (treeh original)) by keep.
    assert (Witnesses4 : G.E.H.resolve fourth (wasm_nat witnesses_slot) = Some (treeh witnesses)) by keep.
    destruct (Trees.native_tree_rule_executes fourth store inside (wasm_nat original_slot) (wasm_nat tag_slot)
      (wasm_nat witnesses_slot) original tag witnesses 1 Functions Typed Capacity Original4 TagLive Witnesses4
      ltac:(unfold tag; rewrite get_tree_raw_create_tree_tagged; exact FieldCount) WitnessCount
      ltac:(unfold tag; rewrite tree_tag_create_tree_tagged; exact Tagged) Bound)
      as [fifth [updated [TreeRun [Scope5 [Obs5 [Functions5 [Typed5 Capacity5]]]]]]].
    - intros current index Scoped Index; apply Ready; [|exact Index].
      eapply retains_trans; [exact (proj1 Scope04)|exact Scoped].
    - lia.
    - unfold Trees.Support.word, Trees.Support.descriptor in TreeRun.
      assert (Scoped : scope state fifth (4+10*List.length (get_tree_raw original)))
        by (eapply scope_trans; eassumption).
      assert (TagNext : G.E.H.resolve fifth (wasm_nat tag_slot) = Some (treeh tag)) by keep.
      assert (TagFresh0 : G.E.H.resolve state (wasm_nat tag_slot) = None)
        by (eapply fresh_before; [exact (proj1 Scope03)|exact TagFresh]).
      exists fifth, updated, tag_slot; split.
      2: { split; [exact TagNext|]; split; [exact TagFresh0|]; split; [exact Scoped|auto]. }
      eapply G.E.collector_call with (code := native_reconstruct)
        (parameters := [T_num T_i32]) (results := [T_num T_i32]) (defaults := repeat (word 0) 4)
        (address := 41%N) (arguments := [descriptor root]) (values := [word tag_slot]) (accepted := true)
        (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
      + rewrite Functions; reflexivity.
      + change (reduce_trans (state, store, Build_frame initial G.E.collector_instance,
          to_e_list (modfunc_body native_reconstruct))
          (fifth, updated, Build_frame inside G.E.collector_instance, v_to_e_list [word tag_slot])).
        rewrite native_reconstruct_body.
        native_pure; execute (arity_run state store initial root combination 6 Functions Root Arity ltac:(vm_compute; congruence)).
        native_pure; execute OriginalRun; native_pure.
        native_pure; execute (tag_run first store locals1 (wasm_nat original_slot) (treeh original) true Functions OriginalLive Tagged).
        native_pure.
        native_pure; execute FieldsRun; native_pure.
        native_pure; execute WitnessesRun; native_pure.
        native_pure; execute TagRun; native_pure.
        do 3 native_pure; execute TreeRun.
        native_pure; apply rt_refl.
  Qed.
End WasmReconstruction.
