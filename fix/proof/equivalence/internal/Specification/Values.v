  Lemma value_flag : forall value, is_value value = true <-> value_handle value.
  Proof.
    assert (Trees : forall tree, tree_value tree = true <-> value_tree tree).
    { intro tree; induction tree as [tree IH] using (well_founded_induction wfp_tree_child).
      assert (Child : forall value, In value (get_tree_raw tree) -> (is_value value = true <-> value_handle value)).
      { intros value Present; destruct value as [[[b|child]|reference]|thunk|encoded]; cbn [is_value].
        - split; [intros _; constructor|intros _; reflexivity].
        - split.
          + intro Flag; constructor; apply (proj1 (IH child Present)); exact Flag.
          + intro Value; inversion Value; apply (proj2 (IH child Present)); assumption.
        - split; [intros _; constructor|intros _; reflexivity].
        - split; [intros _; constructor|intros _; reflexivity].
        - split; [discriminate|intro Value; inversion Value]. }
      rewrite tree_value_contents, forallb_forall; split.
      - intro Flag; constructor; apply Forall_forall; intros value Present.
        apply (proj1 (Child value Present)); apply Flag; exact Present.
      - intro Value; inversion Value; subst; intros value Present.
        apply (proj2 (Child value Present)); eapply Forall_forall; eassumption. }
    intro value; destruct value as [[[b|tree]|reference]|thunk|encoded]; cbn [is_value].
    - split; [intros _; constructor|intros _; reflexivity].
    - split.
      + intro Flag; constructor; apply (proj1 (Trees tree)); exact Flag.
      + intro Value; inversion Value; apply (proj2 (Trees tree)); assumption.
    - split; [intros _; constructor|intros _; reflexivity].
    - split; [intros _; constructor|intros _; reflexivity].
    - split; [discriminate|intro Value; inversion Value].
  Qed.
