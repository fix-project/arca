Module WasmIssuePrefix.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Support := WasmSupport.
  Import Support.
  Module Resources := WasmResources.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Ltac keep := first [assumption | match goal with
    | Scoped : scope ?before ?after ?extra |- G.E.H.resolve ?after ?index = Some ?value =>
      apply (proj1 Scoped); keep
    end].

  Definition issue_tail fields := v_to_e_list [word fields] ++ [AI_basic (BI_call 12%N)].

  Lemma collector_issue_prefix state store combination_index lhs_index rhs_index number lhs rhs author :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state combination_index = Some (treeh combination) ->
    G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    nth_error (get_tree_raw combination) 1 = Some author ->
    G.E.H.D.live_count (G.E.H.slots state) + 5 <= G.E.H.capacity ->
    let code := G.E.H.H.HBlobObj (create_blob (serialise_i64 number)) in
    let claim := create_tree [code; lhs; rhs] in
    let fields := create_tree [author; G.E.H.H.HTreeRef claim] in
    let arguments := [descriptor combination_index; descriptor lhs_index; descriptor rhs_index; VAL_num (VAL_int64 number)] in
    exists next updated ref_slot fields_slot,
      reduce_trans
        (state, store, Build_frame (arguments ++ [word 0]) G.E.collector_instance,
          to_e_list (modfunc_body native_issue))
        (next, updated, Build_frame (arguments ++ [word ref_slot]) G.E.collector_instance, issue_tail fields_slot) /\
      scope state next 5 /\ G.E.H.observations next = G.E.H.observations state /\
      s_funcs updated = s_funcs store /\ store_typing updated /\ Resources.memory_capacity updated /\
      G.E.H.resolve next (wasm_nat fields_slot) = Some (treeh fields).
  Proof.
    intros Functions Typed Capacity CombinationLive LhsLive RhsLive Author Room; cbn zeta.
    set (code := G.E.H.H.HBlobObj (create_blob (serialise_i64 number))).
    set (claim := create_tree [code; lhs; rhs]).
    set (reference := G.E.H.H.HTreeRef claim).
    set (fields := create_tree [author; reference]).
    set (output := treeh (create_tree_tagged true [author; reference])).
    set (arguments := [descriptor combination_index; descriptor lhs_index; descriptor rhs_index; VAL_num (VAL_int64 number)]).
    set (initial := arguments ++ [word 0]).
    destruct Capacity as [memory0 [Memory0 [Type0 Length0]]].
    destruct (new_handle_run state store initial 10%N shell_create_blob_i64 [VAL_num (VAL_int64 number)] code
      Functions eq_refl eq_refl eq_refl ltac:(discriminate) eq_refl ltac:(lia))
      as [first [code_slot [CodeRun [CodeLive [CodeFresh [Scope1 Obs1]]]]]].
    pose proof (proj2 Scope1) as Count1.
    set (locals1 := initial).
    destruct (Resources.store_descriptor_available first store locals1 memory0 32 (wasm_nat code_slot)
      Memory0 Typed Type0 Length0 ltac:(lia) ltac:(vm_compute; congruence))
      as [memory1 [CopyCode [StoreCode [Typed1 [Type1 Length1]]]]].
    set (store1 := upd_s_mem store [memory1]).
    destruct (Resources.store_descriptor_available first store1 locals1 memory1 36 lhs_index
      eq_refl Typed1 Type1 Length1 ltac:(lia) ltac:(vm_compute; congruence))
      as [memory2 [CopyLeft [StoreLeft [Typed2 [Type2 Length2]]]]].
    set (store2 := upd_s_mem store1 [memory2]).
    destruct (Resources.store_descriptor_available first store2 locals1 memory2 40 rhs_index
      eq_refl Typed2 Type2 Length2 ltac:(lia) ltac:(vm_compute; congruence))
      as [memory3 [CopyRight [StoreRight [Typed3 [Type3 Length3]]]]].
    set (store3 := upd_s_mem store2 [memory3]).
    assert (Functions3 : s_funcs store3 = s_funcs G.E.allocated_store) by exact Functions.
    assert (Staged1 : Resources.staged memory1 32%N [wasm_nat code_slot]).
    { apply (Resources.staged_append memory0 memory1 32%N [] (wasm_nat code_slot));
        [intros offset slot Field; destruct offset; discriminate|exact CopyCode]. }
    assert (Staged2 : Resources.staged memory2 32%N [wasm_nat code_slot; lhs_index]).
    { apply (Resources.staged_append memory1 memory2 32%N [wasm_nat code_slot] lhs_index Staged1 CopyLeft). }
    assert (Staged3 : Resources.staged memory3 32%N [wasm_nat code_slot; lhs_index; rhs_index]).
    { apply (Resources.staged_append memory2 memory3 32%N [wasm_nat code_slot; lhs_index] rhs_index Staged2 CopyRight). }
    assert (TreeValues : G.E.H.host_values combination G.E.collector_instance true true handle_bytes first store3
      shell_create_tree [word 32; word 3] = G.E.H.allocate first (treeh claim)).
    { unfold G.E.H.host_values; apply Resources.tree_from_staged with (memory := memory3)
        (indices := [wasm_nat code_slot; lhs_index; rhs_index]); try reflexivity; try exact Staged3.
      - repeat constructor; keep.
      - apply Nat2Z.inj_lt; rewrite G.E.H.capacity_value; cbn; lia. }
    destruct (new_handle_run first store3 locals1 9%N shell_create_tree [word 32; word 3] (treeh claim)
      Functions3 eq_refl eq_refl eq_refl ltac:(discriminate) TreeValues ltac:(lia))
      as [second [claim_slot [TreeRun [ClaimLive [ClaimFresh [Scope2 Obs2]]]]]].
    pose proof (proj2 Scope2) as Count2.
    set (locals2 := initial).
    assert (RefValues : G.E.H.host_values combination G.E.collector_instance true true handle_bytes second store3
      shell_create_ref [word claim_slot] = G.E.H.allocate second reference).
    { unfold G.E.H.host_values, G.E.H.transform, word; rewrite ClaimLive; reflexivity. }
    destruct (new_handle_run second store3 locals2 11%N shell_create_ref [word claim_slot] reference
      Functions3 eq_refl eq_refl eq_refl ltac:(discriminate) RefValues ltac:(lia))
      as [third [ref_slot [RefRun [RefLive [RefFresh [Scope3 Obs3]]]]]].
    set (locals3 := arguments ++ [word ref_slot]).
    assert (Scope03 : scope state third 3).
    { replace 3 with (1+(1+1)) by lia; eapply scope_trans; [exact Scope1|]; eapply scope_trans; eassumption. }
    pose (fifth := third).
    assert (Scope5 : scope state fifth 3) by exact Scope03.
    assert (Ref5 : G.E.H.resolve fifth (wasm_nat ref_slot) = Some reference) by exact RefLive.
    assert (Combination5 : G.E.H.resolve fifth combination_index = Some (treeh combination)) by keep.
    pose proof (proj2 Scope5) as Count5.
    destruct (child_run fifth store3 locals3 combination_index combination 1 author Functions3 Combination5 Author ltac:(lia) ltac:(lia))
      as [sixth [author_slot [AuthorRun [AuthorLive [AuthorFresh [Scope6 Obs6]]]]]].
    pose proof (proj2 Scope6) as Count6.
    set (locals6 := locals3).
    destruct (Resources.store_descriptor_available sixth store3 locals6 memory3 16 (wasm_nat author_slot)
      eq_refl Typed3 Type3 Length3 ltac:(lia) ltac:(vm_compute; congruence))
      as [memory4 [CopyAuthor [StoreAuthor [Typed4 [Type4 Length4]]]]].
    set (store4 := upd_s_mem store3 [memory4]).
    destruct (Resources.store_descriptor_available sixth store4 locals6 memory4 20 (wasm_nat ref_slot)
      eq_refl Typed4 Type4 Length4 ltac:(lia) ltac:(vm_compute; congruence))
      as [memory5 [CopyRef [StoreRef [Typed5 [Type5 Length5]]]]].
    set (store5 := upd_s_mem store4 [memory5]).
    assert (Functions5 : s_funcs store5 = s_funcs G.E.allocated_store) by exact Functions.
    assert (Staged4 : Resources.staged memory4 16%N [wasm_nat author_slot]).
    { apply (Resources.staged_append memory3 memory4 16%N [] (wasm_nat author_slot));
        [intros offset slot Field; destruct offset; discriminate|exact CopyAuthor]. }
    assert (Staged5 : Resources.staged memory5 16%N [wasm_nat author_slot; wasm_nat ref_slot]).
    { apply (Resources.staged_append memory4 memory5 16%N [wasm_nat author_slot] (wasm_nat ref_slot) Staged4 CopyRef). }
    assert (FieldsValues : G.E.H.host_values combination G.E.collector_instance true true handle_bytes sixth store5
      shell_create_tree [word 16; word 2] = G.E.H.allocate sixth (treeh fields)).
    { unfold G.E.H.host_values; apply Resources.tree_from_staged with (memory := memory5)
        (indices := [wasm_nat author_slot; wasm_nat ref_slot]); try reflexivity; try exact Staged5.
      - repeat constructor; keep.
      - apply Nat2Z.inj_lt; rewrite G.E.H.capacity_value; cbn; lia. }
    destruct (new_handle_run sixth store5 locals6 9%N shell_create_tree [word 16; word 2] (treeh fields)
      Functions5 eq_refl eq_refl eq_refl ltac:(discriminate) FieldsValues ltac:(lia))
      as [seventh [fields_slot [FieldsRun [FieldsLive [FieldsFresh [Scope7 Obs7]]]]]].
    pose proof (proj2 Scope7) as Count7.
    set (locals7 := locals3).
    assert (Scoped : scope state seventh 5).
    { replace 5 with (3+(1+1)) by lia; eapply scope_trans; [exact Scope5|].
      eapply scope_trans; eassumption. }
    exists seventh, store5, ref_slot, fields_slot; split.
    - change (reduce_trans
        (state, store, Build_frame initial G.E.collector_instance, to_e_list (modfunc_body native_issue))
        (seventh, store5, Build_frame locals7 G.E.collector_instance, issue_tail fields_slot)).
      rewrite evaluation_output_body.
      native_pure; execute_under [word 32] CodeRun; execute StoreCode.
      native_pure; execute StoreLeft.
      native_pure; execute StoreRight.
      execute TreeRun; execute RefRun; native_pure.
      native_pure; execute_under [word 16] AuthorRun; execute StoreAuthor.
      native_pure; execute StoreRef.
      execute FieldsRun; unfold issue_tail; apply rt_refl.
    - split; [exact Scoped|]; split.
      + rewrite Obs7, Obs6; change (G.E.H.observations third = G.E.H.observations state); rewrite Obs3, Obs2, Obs1; reflexivity.
      + split; [reflexivity|]; split; [exact Typed5|]; split; [exists memory5; auto|exact FieldsLive].
  Qed.
End WasmIssuePrefix.
