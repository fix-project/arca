Module WasmValidation.
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

    Fixpoint lengths_bounded depth value := match depth with
      | 0 => True
      | Datatypes.S remaining => match value with
        | Data (Object (BlobObj blob)) => (Z.of_nat (List.length (get_blob_data blob)) < 2 ^ 64)%Z
        | Data (Object (TreeObj t)) =>
          (Z.of_nat (List.length (get_tree_raw t)) < 2 ^ 64)%Z /\
          Forall (lengths_bounded remaining) (get_tree_raw t)
        | _ => True end
      end.

    (** A bounded-length property at one more level implies the property at the shallower
        level. *)
    Lemma lengths_bounded_weaken depth value :
      lengths_bounded (Datatypes.S depth) value -> lengths_bounded depth value.
    Proof.
      revert value; induction depth as [|depth IH]; intro value; [intros; exact I|].
      destruct value as [[[blob|tree]|reference]|thunk|encoded]; cbn [lengths_bounded]; try tauto.
      intros [Length Children]; split; [exact Length|].
      eapply Forall_impl; [|exact Children]; intros child Bound; apply IH; exact Bound.
    Qed.

    (** A child of a bounded Tree satisfies the bound with one less level of depth. *)
    Lemma lengths_bounded_child depth tree child :
      lengths_bounded (Datatypes.S depth) (treeh tree) ->
      In child (get_tree_raw tree) -> lengths_bounded depth child.
    Proof.
      intros [_ Children] Member; rewrite Forall_forall in Children; apply Children; exact Member.
    Qed.

  Lemma checked_size state store locals index input :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some input ->
    lengths_bounded 1 input ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor index] ++ [AI_basic (BI_call 25%N)])
      (fun final => exists node, input = treeh node /\
        native_size_bound (List.length (get_tree_raw node)) /\
        final = (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [word (List.length (get_tree_raw node))])).
  Proof.
    intros Functions Live Bounds.
    destruct input as [[[blob|node]|reference]|thunk|encoded];
      try solve [eapply checked_trap;
        let input := lazymatch type of Live with G.E.H.resolve _ _ = Some ?value => constr:(value) end in
        exact (G.collector_size_rejects_non_tree combination handle_bytes (address := index)
          state store locals input Functions ltac:(intros; discriminate) Live)].
    destruct Bounds as [Wide _].
    destruct (Z_lt_ge_dec (Z.of_nat (List.length (get_tree_raw node)))
      (Z.of_N native_table_limit + 1)%Z) as [Bound|Large].
    - eapply checked_result.
      + exact (G.collector_size_executes combination handle_bytes (address := index)
          state store locals node Functions Bound Live).
      + exists node; auto.
    - eapply checked_trap; eapply (G.collector_size_rejects_large combination handle_bytes (address := index)) with (tree := node);
        [exact Functions|lia|exact Live].
  Qed.

  Lemma checked_arity state store locals index input expected :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some input ->
    lengths_bounded 1 input -> (Z.of_nat expected < 2 ^ 32)%Z ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor index; word expected] ++ [AI_basic (BI_call 26%N)])
      (fun final => final = (state, store, Build_frame locals G.E.collector_instance, []) /\
        exists node, input = treeh node /\ List.length (get_tree_raw node) = expected).
  Proof.
    intros Functions Live Bounds Expected.
    destruct input as [[[blob|node]|reference]|thunk|encoded];
      try solve [eapply checked_trap;
        let input := lazymatch type of Live with G.E.H.resolve _ _ = Some ?value => constr:(value) end in
        exact (G.collector_arity_rejects_non_tree combination handle_bytes (address := index)
          state store locals input (wasm_nat expected) Functions ltac:(intros; discriminate) Live)].
    destruct Bounds as [Wide _].
    destruct (Z_lt_ge_dec (Z.of_nat (List.length (get_tree_raw node)))
      (Z.of_N native_table_limit + 1)%Z) as [Bound|Large].
    - destruct (Nat.eq_dec (List.length (get_tree_raw node)) expected) as [Length|Different].
      + eapply checked_result; [eapply arity_run; try eassumption; rewrite <- Length; exact Bound|].
        split; [reflexivity|exists node; auto].
      + eapply checked_trap; eapply (G.collector_arity_rejects_length combination handle_bytes (address := index)) with (tree := node); eassumption.
    - eapply checked_trap; eapply (G.collector_arity_rejects_large combination handle_bytes (address := index)) with (tree := node);
        [exact Functions|lia|exact Live].
  Qed.

  Lemma object_reject_comparison state store locals reference_index object_index reference object :
    s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state reference_index = Some reference ->
    G.E.H.resolve state object_index = Some (Data (Object object)) ->
    observed_comparison state reference (Data (Ref (G.E.H.H.data_reference (Object object)))) false ->
    G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    exists next, reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [descriptor reference_index; descriptor object_index] ++ [AI_basic (BI_call 29%N)])
      (next, store, Build_frame locals G.E.collector_instance, [AI_trap]).
  Proof.
    intros Functions Reference ObjectLive Observed Room.
    set (value := Data (Ref (G.E.H.H.data_reference (Object object)))).
    destruct (G.E.H.allocate_available state value Room) as [allocated [values Allocated]].
    destruct (allocate_scope _ _ _ _ Allocated) as [copy [Values [_ [Fresh [Present Scoped]]]]].
    subst values.
    assert (ReferenceNext : G.E.H.resolve allocated reference_index = Some reference).
    { eapply G.E.H.allocate_retains; eassumption. }
    assert (ObservedNext : observed_comparison allocated reference value false).
    { eapply observed_comparison_observations; [eapply allocate_observations; exact Allocated|exact Observed]. }
    set (initial := [descriptor reference_index; descriptor object_index]).
    set (inside := [descriptor reference_index; descriptor object_index]).
    exists (compared allocated).
    eapply G.E.collector_call_void with (code := native_object)
      (parameters := [T_num T_i32; T_num T_i32]) (defaults := [])
      (address := 29%N) (arguments := [descriptor reference_index; descriptor object_index])
      (accepted := false) (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
    - rewrite Functions; reflexivity.
    - change (reduce_trans (state, store, Build_frame initial G.E.collector_instance,
        to_e_list (modfunc_body native_object)) (compared allocated, store, Build_frame inside G.E.collector_instance, [AI_trap])).
      rewrite native_object_body.
      execute (native_type_query state store initial 1%N object_index (Data (Object object)) Functions eq_refl ObjectLive).
      do 4 native_pure.
      assert (Created : G.E.H.host_values combination G.E.collector_instance true true handle_bytes
        state store shell_create_ref [descriptor object_index] = Some (allocated, [word copy])).
      { unfold G.E.H.host_values, G.E.H.transform, descriptor.
        rewrite ObjectLive; cbn [G.E.H.H.obind G.E.H.reference]; exact Allocated. }
      execute_under [descriptor reference_index] (native_value_import state store allocated initial 11%N shell_create_ref
        [descriptor object_index] [word copy] Functions eq_refl eq_refl eq_refl ltac:(discriminate) Created).
      exact (same_run allocated store inside reference_index (wasm_nat copy) reference value false
        Functions ReferenceNext Present ObservedNext).
  Qed.

  Lemma checked_object state store locals reference_index object_index reference input :
    s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state reference_index = Some reference -> G.E.H.resolve state object_index = Some input ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor reference_index; descriptor object_index] ++ [AI_basic (BI_call 29%N)])
      (fun final => exists next object,
        final = (next, store, Build_frame locals G.E.collector_instance, []) /\
        scope state next 1 /\ G.E.H.observations next = G.E.H.observations state + 2 /\
        input = Data (Object object) /\ reference = Data (Ref (G.E.H.H.data_reference (Object object)))).
  Proof.
    intros Functions Reference Live Bytes Room.
    destruct input as [[object|reference2]|thunk|encoded].
    { destruct (observed_comparison_total state reference
        (Data (Ref (G.E.H.H.data_reference (Object object)))) Bytes) as [equal Compared].
      destruct equal.
      + destruct (object_run state store locals reference_index object_index reference object
          Functions Reference Live Compared Room) as [next [Run [Scoped Observations]]].
        eapply checked_result; [exact Run|]; exists next, object.
        repeat split; try reflexivity; try exact (proj1 Scoped); try exact (proj2 Scoped); try exact Observations.
        eapply observed_same_identifies; exact Compared.
      + destruct (object_reject_comparison state store locals reference_index object_index reference object
          Functions Reference Live Compared Room) as [next Run].
        eapply checked_trap; exact Run. }
    all: eapply checked_trap; eapply G.E.collector_call_void with (code := native_object)
        (parameters := [T_num T_i32; T_num T_i32]) (defaults := [])
        (address := 29%N) (arguments := [descriptor reference_index; descriptor object_index])
        (accepted := false)
        (final_frame := Build_frame [descriptor reference_index; descriptor object_index] G.E.collector_instance);
        try reflexivity.
      all: try solve [rewrite Functions; reflexivity].
      all: rewrite native_object_body;
        eapply G.E.collector_trap_suffix with
          (instructions := to_e_list [BI_local_get 1%N; BI_call 0%N; BI_testop T_i32 TO_eqz; BI_call 22%N]);
        exact (G.collector_type_guard_executes combination handle_bytes (address := object_index)
          state store [descriptor reference_index; descriptor object_index] 1%N _ Functions eq_refl Live).
  Qed.

  Lemma checked_data state store locals index input :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some input ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor index] ++ [AI_basic (BI_call 34%N)])
      (fun final => final = (state, store, Build_frame locals G.E.collector_instance, []) /\
        exists datum, input = Data datum).
  Proof.
    intros Functions Live; destruct input as [datum|thunk|encoded].
    { eapply checked_result; [eapply Acceptance.native_data_executes; eassumption|].
      split; [reflexivity|exists datum; reflexivity]. }
    all: eapply checked_trap; eapply G.E.collector_call_void with (code := native_data)
        (parameters := [T_num T_i32]) (defaults := []) (address := 34%N)
        (arguments := [descriptor index]) (accepted := false)
        (final_frame := Build_frame [descriptor index] G.E.collector_instance); try reflexivity.
      all: try solve [rewrite Functions; reflexivity].
      all: unfold native_data; cbn [modfunc_body mod_funcs evaluation_module nth];
        execute (native_type_query state store [descriptor index] 0%N index _ Functions eq_refl Live);
        do 2 native_pure; apply rt_refl.
  Qed.
End WasmValidation.
