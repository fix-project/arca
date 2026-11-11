  Lemma execute_to_equiv encoded result : execute encoded = Some result -> computational_equiv (Encode encoded) result.
  Proof.
    intro Executed; split; [apply EC.R_to_equiv, REvalStep; exact Executed|].
    rewrite eval_encode, Executed; cbn [obind]; destruct (eval result); cbn [rel_opt];
      [apply indistinguishable_refl|exact I].
  Qed.

  Lemma equivalent_force first second : computational_equiv (Thunk first) (Thunk second) ->
    rel_opt indistinguishable (omap lower (force first)) (omap lower (force second)).
  Proof. intro Related; apply indistinguishable_force, equivalent_thunks; exact Related. Qed.

  Lemma equivalent_think_thunk first second : think first = Some (Thunk second) ->
    computational_equiv (Thunk first) (Thunk second).
  Proof.
    intro Thought; apply indistinguishable_equiv; split.
    - apply EC.R_to_equiv, RThinkSingleStepThunk; exact Thought.
    - apply represented_observations, RepresentBehavior.
      assert (Same : force first = force second) by (rewrite (force_unfold first), Thought; reflexivity).
      rewrite Same; destruct (force second); cbn [omap]; constructor; try apply representation_refl.
  Qed.

  Lemma equivalent_think_strict first second : think first = Some (Encode (Strict second)) ->
    computational_equiv (Thunk first) (Thunk second).
  Proof.
    intro Thought; apply indistinguishable_equiv; split.
    - apply EC.R_to_equiv, RThunkSingleStepEncodeStrict; exact Thought.
    - apply represented_observations, RepresentBehavior.
      assert (Same : force first = force second) by (rewrite (force_unfold first), Thought; reflexivity).
      rewrite Same; destruct (force second); cbn [omap]; constructor; try apply representation_refl.
  Qed.

  Lemma equivalent_think_shallow first second : think first = Some (Encode (Shallow second)) ->
    computational_equiv (Thunk first) (Thunk second).
  Proof.
    intro Thought; apply indistinguishable_equiv; split.
    - apply EC.R_to_equiv, RThunkSingleStepEncodeShallow; exact Thought.
    - apply represented_observations, RepresentBehavior.
      assert (Same : force first = force second) by (rewrite (force_unfold first), Thought; reflexivity).
      rewrite Same; destruct (force second); cbn [omap]; constructor; try apply representation_refl.
  Qed.

  Lemma force_result_direct thunk result : force thunk = Some result -> force_result (Thunk thunk) result.
  Proof.
    intro Forced; destruct (force_data _ _ Forced) as [data ->]; exists data; split; [exact Forced|apply indistinguishable_refl].
  Qed.

  Lemma force_result_equiv first second result : computational_equiv first second ->
    force_result first result -> force_result second result.
  Proof.
    intros Related Forced; destruct first as [data|thunk|encoded], result as [data'|thunk'|encoded'];
      cbn [force_result] in Forced; try contradiction.
    destruct (proj1 (EC.equiv_preserve_thunk _ _ (proj1 Related)) (ex_intro _ thunk eq_refl)) as [other ->].
    destruct Forced as [actual [Forced Claim]].
    pose proof (equivalent_force _ _ Related) as Correspondence.
    rewrite Forced in Correspondence; destruct (force other) as [output|] eqn:Other;
      cbn [omap rel_opt] in Correspondence; [|contradiction].
    destruct (force_data _ _ Other) as [next ->]; exists next; split; [exact Other|].
    eapply indistinguishable_trans; eassumption.
  Qed.

  Lemma force_result_strict thunk result : force_result (Thunk thunk) result ->
    computational_equiv (Encode (Strict thunk)) (lift result).
  Proof.
    destruct result as [data|other|encoded]; cbn [force_result]; try contradiction.
    intros [actual [Forced Claim]].
    assert (Executed : execute (Strict thunk) = Some (lift (Data actual))).
    { rewrite execute_unfold; cbn [encode_to_thunk]; rewrite Forced; reflexivity. }
    eapply equiv_trans; [apply execute_to_equiv; exact Executed|].
    apply equiv_sym, indistinguishable_equiv, indistinguishable_lift_lower; exact Claim.
  Qed.

  Lemma force_result_shallow thunk result : force_result (Thunk thunk) result ->
    computational_equiv (Encode (Shallow thunk)) (lower result).
  Proof.
    destruct result as [data|other|encoded]; cbn [force_result]; try contradiction.
    intros [actual [Forced Claim]].
    assert (Executed : execute (Shallow thunk) = Some (lower (Data actual))).
    { rewrite execute_unfold; cbn [encode_to_thunk]; rewrite Forced; reflexivity. }
    eapply equiv_trans; [apply execute_to_equiv; exact Executed|].
    apply equiv_sym, indistinguishable_equiv; exact Claim.
  Qed.

  Lemma force_results_equiv first second left right :
    force_result first left -> force_result second right ->
    computational_equiv (lower left) (lower right) -> computational_equiv first second.
  Proof.
    intros First Second Related.
    destruct first as [d|first|e], second as [d'|second|e'], left as [left|th|enc], right as [right|th'|enc'];
      cbn [force_result] in First, Second; try contradiction.
    destruct First as [actual [First Claim1]], Second as [other [Second Claim2]].
    assert (Outputs : indistinguishable (lower (Data actual)) (lower (Data other))).
    { eapply indistinguishable_trans; [apply indistinguishable_sym; exact Claim1|].
      eapply indistinguishable_trans; [|exact Claim2].
      apply (proj1 (equiv_values (lower (Data left)) (lower (Data right)) ltac:(rewrite lower_data_reference; constructor)
        ltac:(rewrite lower_data_reference; constructor))); exact Related. }
    apply indistinguishable_equiv; split.
    - eapply EC.force_some_to_equiv; [exact First|exact Second|].
      exact (proj1 (indistinguishable_lift_lower _ _ Outputs)).
    - apply represented_observations, RepresentBehavior.
      rewrite First, Second; cbn [omap]; constructor; apply indistinguishable_representation; exact Outputs.
  Qed.
