  Lemma retains_refl state : retains state state.
  Proof. intros index value Present; exact Present. Qed.

  Lemma retains_trans before middle after : retains before middle -> retains middle after -> retains before after.
  Proof. intros First Second index value Present; apply Second, First, Present. Qed.

  Lemma scope_refl state : scope state state 0.
  Proof. split; [apply retains_refl|lia]. Qed.

  Lemma scope_trans before middle after first second :
    scope before middle first -> scope middle after second -> scope before after (first + second).
  Proof.
    intros [First Count1] [Second Count2]; split.
    - eapply retains_trans; eassumption.
    - lia.
  Qed.

  Lemma scope_weaken before after first second :
    scope before after first -> first <= second -> scope before after second.
  Proof. intros [Retained Count] Bound; split; [exact Retained|lia]. Qed.

  Lemma fresh_before before after index : retains before after -> G.E.H.resolve after index = None ->
    G.E.H.resolve before index = None.
  Proof.
    intros Retained Fresh; destruct (G.E.H.resolve before index) as [value|] eqn:Present; [|reflexivity].
    specialize (Retained index value Present); rewrite Fresh in Retained; discriminate.
  Qed.

  Lemma allocate_scope before value after values : G.E.H.allocate before value = Some (after, values) ->
    exists index, values = [word index] /\ index < G.E.H.capacity /\
      G.E.H.resolve before (wasm_nat index) = None /\
      G.E.H.resolve after (wasm_nat index) = Some value /\ scope before after 1.
  Proof.
    intro Allocated; destruct (G.E.H.allocate_resolves _ _ _ _ Allocated)
      as [index [Values [Bound [Fresh [Present [Count Observations]]]]]].
    exists index; repeat split; try exact Values; try exact Bound; try exact Present.
    - unfold G.E.H.resolve; rewrite G.E.H.word_roundtrip by exact Bound; exact Fresh.
    - intros other previous Previous; eapply G.E.H.allocate_retains; eassumption.
    - lia.
  Qed.

  Lemma allocate_available state value : G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    exists next index, G.E.H.allocate state value = Some (next, [word index]) /\
      G.E.H.resolve next (wasm_nat index) = Some value /\ scope state next 1.
  Proof.
    intro Room; destruct (G.E.H.allocate_available state value Room) as [next [values Allocated]].
    destruct (allocate_scope _ _ _ _ Allocated) as [index [Values [_ [_ [Present Scoped]]]]].
    subst values; exists next, index; auto.
  Qed.
