Module WasmExchange.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Native := WasmNativeDispatch.
  Import Native Native.Dispatcher Native.Dispatcher.D Native.Dispatcher.D.V Native.Dispatcher.D.V.V Native.Dispatcher.D.V.V.Support.
  Module Resolution := WasmResolution.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.

  Theorem collector_family_validation state store locals code selected rule lhs_index rhs_index witnesses_index lhs rhs witnesses :
    (Z.of_nat code < 2 ^ 64)%Z -> X.select_exchange code lhs = Some selected -> X.decode_exchange selected = Some rule ->
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.within_capacity state -> G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state witnesses_index = Some (G.E.H.H.HTreeObj witnesses) ->
    lengths_bounded 1 lhs -> lengths_bounded 1 rhs -> lengths_bounded 1 (G.E.H.H.HTreeObj witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + (32 * (List.length (get_tree_raw witnesses) + 1)) <= G.E.H.capacity ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list (Base.rule_arguments code lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 45%N)])
      (fun final => exists next updated result kind,
        final = (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [Base.result_word result]) /\
        scope state next (32 * (List.length (get_tree_raw witnesses) + 1)) /\ s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Claims.Code.statement_kind result = Some kind /\ X.check rule (get_tree_raw witnesses) lhs rhs = Some kind).
  Proof.
    intros Code Selected Decoded Functions Typed Capacity Within LhsLive RhsLive WitnessesLive
      LeftBound RightBound WitnessBound WitnessBounds Bytes Room.
    pose (arguments := Base.rule_arguments code lhs_index rhs_index witnesses_index).
    eapply checked_call with (code := native_family)
      (parameters := [T_num T_i64;T_num T_i32;T_num T_i32;T_num T_i32]) (results := [T_num T_i64])
      (defaults := []) (address := 45%N) (arguments := arguments) (dummy := [Base.result_word 0]); try reflexivity.
    - rewrite Functions; reflexivity.
    - rewrite native_family_body.
      do 2 dispatch_pure.
      pose proof (Resolution.resolve_validation code lhs selected state store arguments lhs_index Code Selected Functions LhsLive) as Resolve.
      lazymatch type of Resolve with Resolution.V.checked ?source ?post => change (checked source post) in Resolve end.
      unfold Resolution.V.Support.descriptor in Resolve.
      checked_use Resolve.
      intros final Final; subst final; normalize_checked.
      do 3 dispatch_pure.
      pose proof (collector_dispatch_validation state store arguments selected rule lhs_index rhs_index witnesses_index lhs rhs witnesses
        Decoded Functions Typed Capacity Within LhsLive RhsLive WitnessesLive LeftBound RightBound WitnessBound WitnessBounds Bytes Room) as Dispatch.
      lazymatch type of Dispatch with checked ?source ?post =>
        eapply checked_weaken with (before := post); [exact Dispatch|] end.
      intros final [next [updated [result [kind [Final [Scoped [NextFunctions [NextTyped [NextCapacity [Kind Checked]]]]]]]]]];
        subst final.
      exists [Base.result_word result]; split; [left; reflexivity|]; split; [reflexivity|].
      exists next, updated, result, kind; split; [reflexivity|]; auto 10.
  Qed.
End WasmExchange.
