Module WasmForceRules.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Base := WasmRules.
  Module Binary := WasmBinaryRules.
  Import Base Base.Support Binary.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation lift := X.D.Context.EC.E.EP.E.lift.
  Definition object_data datum := match datum with Object _ => true | Ref _ => false end.
  Definition strict_rhs_code :=
    [BI_local_get 7%N; BI_call 34%N; BI_local_get 7%N; BI_call 0%N; BI_testop T_i32 TO_eqz;
      BI_if (BT_valtype None) [BI_local_get 2%N; BI_local_get 7%N; BI_call 23%N]
        [BI_local_get 7%N; BI_local_get 2%N; BI_call 29%N]].

  Lemma strict_rhs_prefix state store locals index datum :
    s_funcs store = s_funcs G.E.allocated_store -> lookup_N locals 7%N = Some (descriptor index) ->
    G.E.H.resolve state index = Some (Data datum) ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        to_e_list [BI_local_get 7%N; BI_call 34%N; BI_local_get 7%N; BI_call 0%N; BI_testop T_i32 TO_eqz])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool (object_data datum))) ]).
  Proof.
    intros Functions Local Live.
    execute (G.E.collector_local_get combination true true handle_bytes state store locals 7%N (descriptor index) Local).
    execute (Acceptance.native_data_executes state store locals index datum Functions Live).
    execute (native_type_query state store locals 7%N index (Data datum) Functions Local Live).
    destruct datum; scratch_pure; apply rt_refl.
  Qed.

  Lemma strict_rhs_executes state store locals rhs_index value_index datum :
    s_funcs store = s_funcs G.E.allocated_store ->
    lookup_N locals 2%N = Some (descriptor rhs_index) -> lookup_N locals 7%N = Some (descriptor value_index) ->
    G.E.H.resolve state rhs_index = Some (lift (Data datum)) -> G.E.H.resolve state value_index = Some (Data datum) ->
    observed_same state (Data datum) (Data datum) -> G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    exists next,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance, to_e_list strict_rhs_code)
        (next, store, Build_frame locals G.E.collector_instance, []) /\
      scope state next 1 /\ G.E.H.observations next = G.E.H.observations state + 2.
  Proof.
    intros Functions RhsLocal ValueLocal RhsLive ValueLive Observed Room.
    pose proof (strict_rhs_prefix state store locals value_index datum Functions ValueLocal ValueLive) as Prefix.
    destruct datum as [obj|[blob|tree]].
    - exists (compared state); split.
      2: { split; [eapply scope_weaken; [apply compared_scope|lia]|reflexivity]. }
      unfold strict_rhs_code; execute Prefix.
      apply G.E.collector_if_void with (condition := true) (accepted := true).
      execute (G.E.collector_local_get combination true true handle_bytes state store locals 2%N (descriptor rhs_index) RhsLocal).
      execute_under [descriptor rhs_index]
        (G.E.collector_local_get combination true true handle_bytes state store locals 7%N (descriptor value_index) ValueLocal).
      exact (same_run state store locals rhs_index value_index (Data (Object obj)) (Data (Object obj)) true Functions RhsLive ValueLive Observed).
    - destruct (object_run state store locals value_index rhs_index (Data (Ref (BlobRef blob))) (BlobObj blob)
        Functions ValueLive RhsLive Observed Room) as [next [Run Result]].
      exists next; split; [|exact Result].
      unfold strict_rhs_code; execute Prefix.
      apply G.E.collector_if_void with (condition := false) (accepted := true).
      execute (G.E.collector_local_get combination true true handle_bytes state store locals 7%N (descriptor value_index) ValueLocal).
      execute_under [descriptor value_index]
        (G.E.collector_local_get combination true true handle_bytes state store locals 2%N (descriptor rhs_index) RhsLocal).
      exact Run.
    - destruct (object_run state store locals value_index rhs_index (Data (Ref (TreeRef tree))) (TreeObj tree)
        Functions ValueLive RhsLive Observed Room) as [next [Run Result]].
      exists next; split; [|exact Result].
      unfold strict_rhs_code; execute Prefix.
      apply G.E.collector_if_void with (condition := false) (accepted := true).
      execute (G.E.collector_local_get combination true true handle_bytes state store locals 7%N (descriptor value_index) ValueLocal).
      execute_under [descriptor value_index]
        (G.E.collector_local_get combination true true handle_bytes state store locals 2%N (descriptor rhs_index) RhsLocal).
      exact Run.
  Qed.

End WasmForceRules.
