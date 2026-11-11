Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module V := WasmChecked.
  Import V V.Support.
  Module Acceptance := WasmAcceptance.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.

  Lemma native_import_traps state store locals index operation arguments :
    s_funcs store = s_funcs G.E.allocated_store ->
    lookup_N (s_funcs G.E.allocated_store) index = Some (FC_func_host (shell_signature operation) operation) ->
    lookup_N (inst_funcs G.E.collector_instance) index = Some index ->
    (match shell_signature operation with Tf parameters _ => List.length arguments = List.length parameters end) ->
    operation <> shell_read_blob ->
    G.E.H.host_values combination G.E.collector_instance true true handle_bytes state store operation arguments = None ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance, v_to_e_list arguments ++ [AI_basic (BI_call index)])
      (state, store, Build_frame locals G.E.collector_instance, [AI_trap]).
  Proof.
    intros Functions Closure Index Arity Other Failed.
    eapply G.E.collector_import_executes with (operation := operation) (address := index) (result := result_trap);
      try eassumption.
    - rewrite Functions; exact Closure.
    - unfold G.E.H.host_result, G.E.H.candidate.
      destruct (function_type_eq_dec (shell_signature operation) (shell_signature operation)); [|contradiction].
      destruct (shell_function_eq_dec operation shell_read_blob); [contradiction|].
      rewrite Failed; reflexivity.
  Qed.

  Lemma checked_new_handle state store locals index operation arguments value :
    s_funcs store = s_funcs G.E.allocated_store ->
    lookup_N (s_funcs G.E.allocated_store) index = Some (FC_func_host (shell_signature operation) operation) ->
    lookup_N (inst_funcs G.E.collector_instance) index = Some index ->
    (match shell_signature operation with Tf parameters _ => List.length arguments = List.length parameters end) ->
    operation <> shell_read_blob ->
    G.E.H.host_values combination G.E.collector_instance true true handle_bytes state store operation arguments =
      G.E.H.allocate state value ->
    checked
      (state, store, Build_frame locals G.E.collector_instance, v_to_e_list arguments ++ [AI_basic (BI_call index)])
      (fun final => exists next output,
        final = (next, store, Build_frame locals G.E.collector_instance, v_to_e_list [word output]) /\
        G.E.H.resolve next (wasm_nat output) = Some value /\ G.E.H.resolve state (wasm_nat output) = None /\
        scope state next 1 /\ G.E.H.observations next = G.E.H.observations state).
  Proof.
    intros Functions Closure Index Arity Other Values.
    destruct (G.E.H.allocate state value) as [[next outputs]|] eqn:Allocated.
    - destruct (allocate_scope _ _ _ _ Allocated) as [output [Outputs [Bound [Fresh [Live Scoped]]]]].
      subst outputs; eapply checked_result.
      + exact (native_value_import state store next locals index operation arguments [word output]
          Functions Closure Index Arity Other ltac:(rewrite Values; reflexivity)).
      + exists next, output; split; [reflexivity|]; split; [exact Live|]; split; [exact Fresh|]; split; [exact Scoped|].
        exact (allocate_observations _ _ _ _ Allocated).
    - eapply checked_trap.
      exact (native_import_traps state store locals index operation arguments Functions Closure Index Arity Other
        ltac:(rewrite Values; reflexivity)).
  Qed.

  Lemma checked_same state store locals lhs_index rhs_index lhs rhs :
    s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    checked
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [descriptor lhs_index; descriptor rhs_index] ++ [AI_basic (BI_call 23%N)])
      (fun final => final = (compared state, store, Build_frame locals G.E.collector_instance, []) /\ lhs = rhs).
  Proof.
    intros Functions Left Right Bytes.
    destruct (observed_comparison_total state lhs rhs Bytes) as [equal Compared].
    destruct equal.
    - eapply checked_result; [exact (same_run state store locals lhs_index rhs_index lhs rhs true Functions Left Right Compared)|].
      split; [reflexivity|eapply observed_same_identifies; exact Compared].
    - eapply checked_trap; exact (same_run state store locals lhs_index rhs_index lhs rhs false Functions Left Right Compared).
  Qed.

  Lemma checked_child state store locals index offset input :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some input ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor index; descriptor offset] ++ [AI_basic (BI_call 7%N)])
      (fun final => exists next output value,
        final = (next, store, Build_frame locals G.E.collector_instance, v_to_e_list [word output]) /\
        G.E.H.Api.get_tree_data_api input (G.E.H.nat_of_i32 offset) = Some value /\
        G.E.H.resolve next (wasm_nat output) = Some value /\ G.E.H.resolve state (wasm_nat output) = None /\
        scope state next 1 /\ G.E.H.observations next = G.E.H.observations state).
  Proof.
    intros Functions Live.
    assert (Values : G.E.H.host_values combination G.E.collector_instance true true handle_bytes
        state store shell_get_tree_element [descriptor index; descriptor offset] =
      match G.E.H.Api.get_tree_data_api input (G.E.H.nat_of_i32 offset) with
      | Some value => G.E.H.allocate state value | None => None end).
    { unfold G.E.H.host_values, G.E.H.transform, descriptor; rewrite Live; reflexivity. }
    destruct (G.E.H.Api.get_tree_data_api input (G.E.H.nat_of_i32 offset)) as [value|] eqn:Child.
    - eapply checked_weaken.
      + exact (checked_new_handle state store locals 7%N shell_get_tree_element
          [descriptor index; descriptor offset] value Functions eq_refl eq_refl eq_refl ltac:(discriminate) Values).
      + intros final [next [output [Final [Present [Fresh [Scoped Observations]]]]]].
        exists next, output, value; split; [exact Final|]; split; [reflexivity|];
          split; [exact Present|]; split; [exact Fresh|]; auto.
    - eapply checked_trap; exact (native_import_traps state store locals 7%N shell_get_tree_element
        [descriptor index; descriptor offset] Functions eq_refl eq_refl eq_refl ltac:(discriminate) Values).
  Qed.

  Local Notation treeh := G.E.H.H.HTreeObj.

  Lemma checked_tree state store locals index input :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some input ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor index] ++ [AI_basic (BI_call 24%N)])
      (fun final => final = (state, store, Build_frame locals G.E.collector_instance, []) /\
        exists node, input = treeh node).
  Proof.
    intros Functions Live.
    pose proof (G.collector_tree_executes combination handle_bytes (address := index)
      state store locals input Functions Live) as Run.
    destruct input as [[[blob|node]|reference]|thunk|encoded];
      try solve [eapply checked_trap; exact Run].
    eapply checked_result; [exact Run|]; split; [reflexivity|exists node; reflexivity].
  Qed.
