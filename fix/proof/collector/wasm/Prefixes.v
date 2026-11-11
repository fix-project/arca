Module WasmPrefixes.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module D := WasmDispatchSupport.
  Import D D.V D.V.V D.V.V.Support.
  Module A := WasmAuthentication.
  Module Strict := WasmStrictValidation.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Lemma checked_arity_label state store locals root witnesses expected :
    s_funcs store = s_funcs G.E.allocated_store -> lookup_N locals 3%N = Some (descriptor root) ->
    G.E.H.resolve state root = Some (treeh witnesses) -> lengths_bounded 1 (treeh witnesses) ->
    (Z.of_nat expected < 2 ^ 32)%Z ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      [AI_label 0 [] (to_e_list [BI_local_get 3%N; BI_const_num (VAL_int32 (wasm_nat expected)); BI_call 26%N])])
      (fun final => final = (state, store, Build_frame locals G.E.collector_instance, []) /\
        List.length (get_tree_raw witnesses) = expected).
  Proof.
    intros Functions Local Live Bounds Bound.
    apply Strict.checked_void_label.
    lazymatch goal with |- Strict.V.V.checked ?initial ?post => change (checked initial post) end.
    checked_advance (G.E.collector_local_get combination true true handle_bytes state store locals 3%N (descriptor root) Local).
    checked_use (checked_arity state store locals root (treeh witnesses) expected Functions Live Bounds Bound).
    intros final [Final [node [Node Length]]]; subst final; inversion Node; subst node; normalize_checked.
    eapply checked_result; [apply rt_refl|]; split; [reflexivity|]; split; [reflexivity|exact Length].
  Qed.

  Lemma unary_prefix_validation state store code lhs rhs root witnesses :
    In code Base.unary_codes -> s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state root = Some (treeh witnesses) -> lengths_bounded 1 (treeh witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 5 <= G.E.H.capacity ->
    checked
      (state, store, Build_frame (Base.initial_locals code lhs rhs root) G.E.collector_instance,
        to_e_list (firstn 48 Base.rule_body))
      (fun final => exists next output witness claim,
        final = (next, store, Build_frame (Base.single_locals code lhs rhs root output 0) G.E.collector_instance, []) /\
        get_tree_raw witnesses = [witness] /\ A.Witnesses.authenticated_claim witness claim /\
        G.E.H.resolve next (wasm_nat output) = Some (treeh claim) /\
        G.E.H.resolve state (wasm_nat output) = None /\ scope state next 5 /\
        G.E.H.observations next = G.E.H.observations state + 2).
  Proof.
    intros Code Functions Live Bounds WitnessBounds Bytes Room.
    set (initial := Base.initial_locals code lhs rhs root).
    assert (Arity : checked
      (state, store, Build_frame initial G.E.collector_instance,
        [AI_label 0 [] (to_e_list [BI_local_get 3%N; BI_const_num (VAL_int32 (wasm_nat 1)); BI_call 26%N])])
      (fun final => final = (state, store, Build_frame initial G.E.collector_instance, []) /\
        List.length (get_tree_raw witnesses) = 1)).
    { eapply checked_arity_label with (root := root); try assumption; [reflexivity|vm_compute; congruence]. }
    let finish := ltac:(
      repeat checked_pure;
      checked_use Arity;
      let final := fresh "final" in let Final := fresh "Final" in let Length := fresh "Length" in
      intros final [Final Length]; subst final; normalize_checked;
      destruct (get_tree_raw witnesses) as [|witness [|extra rest]] eqn:Fields;
        cbn [List.length] in Length; try discriminate;
      let WitnessBound := fresh "WitnessBound" in
      assert (WitnessBound : lengths_bounded 2 witness) by
        (inversion WitnessBounds; apply lengths_bounded_weaken; assumption);
      checked_pure;
      let Reopen := fresh "Reopen" in
      pose proof (A.checked_witness_claim state store initial root witnesses 0 witness Functions Live
        ltac:(rewrite Fields; reflexivity) ltac:(lia) WitnessBound Bytes Room) as Reopen;
      lazymatch type of Reopen with A.V.V.checked ?initial ?post => change (checked initial post) in Reopen end;
      unfold A.V.V.Support.word, A.V.V.Support.descriptor in Reopen;
      checked_use Reopen;
      let next := fresh "next" in let output := fresh "output" in let claim := fresh "claim" in
      let Auth := fresh "Authenticated" in let Present := fresh "Present" in let Fresh := fresh "Fresh" in
      let Scoped := fresh "Scoped" in let Observations := fresh "Observations" in
      intros final [next [output [claim [Final [Auth [Present [Fresh [Scoped Observations]]]]]]]];
      subst final; normalize_checked;
      checked_pure;
      eapply checked_result; [apply rt_refl|];
      exists next, output, witness, claim; split; [reflexivity|]; split; [reflexivity|]; split; [exact Auth|];
      split; [exact Present|]; split; [exact Fresh|]; auto) in
    unfold Base.rule_body; rewrite native_exchange_body; cbn [nth firstn];
    unfold initial, Base.initial_locals, Base.rule_arguments, Base.result_word, Base.Support.word, Base.Support.descriptor in Arity |- *;
    unfold Base.unary_codes in Code; cbn in Code;
    destruct Code as [Code|[Code|[Code|[Code|[Code|[Code|[Code|[Code|[Code|[Code|[Code|[Code|[Code|[Code|[Code|Impossible]]]]]]]]]]]]]]];
      try contradiction; subst code; finish.
  Qed.

  Lemma binary_prefix_validation state store code lhs rhs root witnesses :
    In code Base.binary_codes -> s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state root = Some (treeh witnesses) -> lengths_bounded 1 (treeh witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 5 <= G.E.H.capacity ->
    checked
      (state, store, Build_frame (Base.initial_locals code lhs rhs root) G.E.collector_instance,
        to_e_list (firstn 48 Base.rule_body))
      (fun final => exists next output witness second_witness claim,
        final = (next, store, Build_frame (Base.single_locals code lhs rhs root output 0) G.E.collector_instance, []) /\
        get_tree_raw witnesses = [witness; second_witness] /\ A.Witnesses.authenticated_claim witness claim /\
        G.E.H.resolve next (wasm_nat output) = Some (treeh claim) /\
        G.E.H.resolve state (wasm_nat output) = None /\ scope state next 5 /\
        G.E.H.observations next = G.E.H.observations state + 2).
  Proof.
    intros Code Functions Live Bounds WitnessBounds Bytes Room.
    set (initial := Base.initial_locals code lhs rhs root).
    assert (Arity : checked
      (state, store, Build_frame initial G.E.collector_instance,
        [AI_label 0 [] (to_e_list [BI_local_get 3%N; BI_const_num (VAL_int32 (wasm_nat 2)); BI_call 26%N])])
      (fun final => final = (state, store, Build_frame initial G.E.collector_instance, []) /\
        List.length (get_tree_raw witnesses) = 2)).
    { eapply checked_arity_label with (root := root); try assumption; [reflexivity|vm_compute; congruence]. }
    let finish := ltac:(
      repeat checked_pure;
      checked_use Arity;
      let final := fresh "final" in let Final := fresh "Final" in let Length := fresh "Length" in
      intros final [Final Length]; subst final; normalize_checked;
      destruct (get_tree_raw witnesses) as [|witness [|second_witness [|extra rest]]] eqn:Fields;
        cbn [List.length] in Length; try discriminate;
      let WitnessBound := fresh "WitnessBound" in
      assert (WitnessBound : lengths_bounded 2 witness) by
        (inversion WitnessBounds; apply lengths_bounded_weaken; assumption);
      checked_pure;
      let Reopen := fresh "Reopen" in
      pose proof (A.checked_witness_claim state store initial root witnesses 0 witness Functions Live
        ltac:(rewrite Fields; reflexivity) ltac:(lia) WitnessBound Bytes Room) as Reopen;
      lazymatch type of Reopen with A.V.V.checked ?initial ?post => change (checked initial post) in Reopen end;
      unfold A.V.V.Support.word, A.V.V.Support.descriptor in Reopen;
      checked_use Reopen;
      let next := fresh "next" in let output := fresh "output" in let claim := fresh "claim" in
      let Auth := fresh "Authenticated" in let Present := fresh "Present" in let Fresh := fresh "Fresh" in
      let Scoped := fresh "Scoped" in let Observations := fresh "Observations" in
      intros final [next [output [claim [Final [Auth [Present [Fresh [Scoped Observations]]]]]]]];
      subst final; normalize_checked;
      checked_pure;
      eapply checked_result; [apply rt_refl|];
      exists next, output, witness, second_witness, claim; split; [reflexivity|]; split; [reflexivity|]; split; [exact Auth|];
      split; [exact Present|]; split; [exact Fresh|]; auto) in
    unfold Base.rule_body; rewrite native_exchange_body; cbn [nth firstn];
    unfold initial, Base.initial_locals, Base.rule_arguments, Base.result_word, Base.Support.word, Base.Support.descriptor in Arity |- *;
    unfold Base.binary_codes in Code; cbn in Code;
    destruct Code as [Code|[Code|[Code|[Code|[Code|Impossible]]]]];
      try contradiction; subst code; finish.
  Qed.
End WasmPrefixes.
