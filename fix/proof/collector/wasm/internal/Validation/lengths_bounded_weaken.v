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
