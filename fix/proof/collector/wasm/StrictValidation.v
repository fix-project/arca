Module WasmStrictValidation.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module V := WasmValidation.
  Import V V.V V.V.Support.
  Module Force := WasmForceRules.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation local_get_run := (G.E.collector_local_get combination true true handle_bytes).
  Local Notation lift := X.D.EC.E.EP.E.lift.

  Lemma checked_void_label state store frame body post :
    checked (state, store, frame, body)
      (fun final => let '(_, _, _, instructions) := final in instructions = [] /\ post final) ->
    checked (state, store, frame, [AI_label 0 [] body]) post.
  Proof.
    intro Body.
    assert (Shape : in_context 1 (LH_rec [] 0 [] (LH_base [] []) []) (state, store, frame, body) =
      (state, store, frame, [AI_label 0 [] body])).
    { cbn [in_context lfill]; rewrite !seq.cat0s, !seq.cats0; reflexivity. }
    rewrite <- Shape; eapply checked_context with (initial := (state, store, frame, body))
      (context := LH_rec [] 0 [] (LH_base [] []) []) (before := fun final =>
        let '(_, _, _, instructions) := final in instructions = [] /\ post final); [exact Body|].
    intros [[[state' store'] frame'] instructions] [Empty Post]; subst instructions.
    normalize_checked.
    eapply checked_result with (final := (state', store', frame', []));
      [apply rt_step, r_simple, rs_label_const; reflexivity|exact Post].
  Qed.

  Lemma checked_strict_result state store locals input_index result_index input result :
    s_funcs store = s_funcs G.E.allocated_store ->
    lookup_N locals 7%N = Some (descriptor input_index) -> lookup_N locals 2%N = Some (descriptor result_index) ->
    G.E.H.resolve state input_index = Some input -> G.E.H.resolve state result_index = Some result ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    checked (state, store, Build_frame locals G.E.collector_instance, to_e_list Force.strict_rhs_code)
      (fun final => exists next datum,
        final = (next, store, Build_frame locals G.E.collector_instance, []) /\ scope state next 1 /\
        G.E.H.observations next = G.E.H.observations state + 2 /\ input = Data datum /\ result = lift input).
  Proof.
    intros Functions Input Result InputLive ResultLive Bytes Room.
    unfold Force.strict_rhs_code.
    checked_advance (local_get_run state store locals 7%N (descriptor input_index) Input).
    checked_use (checked_data state store locals input_index input Functions InputLive).
    intros final [Final [datum Data]]; subst final input; normalize_checked.
    checked_advance (native_type_query state store locals 7%N input_index (Data datum) Functions Input InputLive).
    destruct datum as [object|reference]; do 2 checked_pure; apply checked_void_label.
    - checked_advance (local_get_run state store locals 2%N (descriptor result_index) Result).
      checked_advance_under [descriptor result_index]
        (local_get_run state store locals 7%N (descriptor input_index) Input).
      checked_use (checked_same state store locals result_index input_index result (Data (Object object))
        Functions ResultLive InputLive Bytes).
      intros final [Final Same]; subst final result; normalize_checked.
      eapply checked_result; [apply rt_refl|]; split; [reflexivity|].
      exists (compared state), (Object object); split; [reflexivity|]; split; [eapply scope_weaken; [apply compared_scope|lia]|];
        split; [reflexivity|]; split; reflexivity.
    - checked_advance (local_get_run state store locals 7%N (descriptor input_index) Input).
      checked_advance_under [descriptor input_index]
        (local_get_run state store locals 2%N (descriptor result_index) Result).
      checked_use (checked_object state store locals input_index result_index (Data (Ref reference)) result
        Functions InputLive ResultLive Bytes Room).
      intros final [next [object [Final [Scoped [Observations [Object Reference]]]]]];
        subst final result; normalize_checked.
      eapply checked_result; [apply rt_refl|]; split; [reflexivity|].
      exists next, (Ref reference); split; [reflexivity|]; split; [exact Scoped|]; split; [exact Observations|];
        split; [reflexivity|].
      inversion Reference; subst reference; destruct object; reflexivity.
  Qed.
End WasmStrictValidation.
