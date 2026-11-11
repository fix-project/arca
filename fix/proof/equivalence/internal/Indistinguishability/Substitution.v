  Lemma shallow_eval_force thunk : eval (Encode (Shallow thunk)) = omap lower (force thunk).
  Proof.
    rewrite eval_encode, execute_unfold; cbn [encode_to_thunk].
    destruct (force thunk) as [result|] eqn:Forced; cbn [omap obind]; [|reflexivity].
    destruct (force_data _ _ Forced) as [data ->].
    destruct data as [[b|t]|[b|t]]; cbn [lower lower_data]; apply eval_ref.
  Qed.

  Lemma represented_observations : forall left right, representation left right -> same_observations left right.
  Proof.
    refine (cofix Handles left right (Related : representation left right) : same_observations left right := _
      with Children xs ys (Related : represented_children xs ys) : observed_children xs ys := _
      with Results first second (Related : rel_opt representation first second) : observed_result first second := _ for Handles).
    { pose proof Related as Whole; destruct Related;
        try solve [constructor; eauto using Handles, Children; congruence].
      all: apply ObserveThunk; apply Results, represented_eval;
        [apply RepresentStrict|apply RepresentShallow]; exact Whole.
    }
    { destruct Related; constructor; eauto. }
    { destruct first, second; cbn [rel_opt] in Related; try contradiction; constructor; eauto. }
  Qed.

  Lemma observations_represented : forall left right, same_observations left right -> representation left right.
  Proof.
    refine (cofix Handles left right (Related : same_observations left right) : representation left right := _
      with Children xs ys (Related : observed_children xs ys) : represented_children xs ys := _
      with Results first second (Related : observed_result first second) : represented_results first second := _ for Handles).
    - pose proof Related as Whole; destruct Related.
      + constructor; assumption.
      + constructor; assumption.
      + constructor; [assumption|assumption|exact (observations_value _ _ Whole)|apply Children; assumption].
      + constructor; apply Handles; assumption.
      + apply RepresentBehavior; apply Results; rewrite <- !shallow_eval_force; assumption.
      + constructor; apply Handles; assumption.
      + constructor; apply Handles; assumption.
    - destruct Related; constructor; eauto.
    - destruct Related; constructor; eauto.
  Qed.

  Lemma indistinguishable_representation left right : indistinguishable left right -> representation left right.
  Proof. intros [_ Observed]; apply observations_represented; exact Observed. Qed.

  Lemma indistinguishable_typed left right : indistinguishable left right -> same_typed_handle left right.
  Proof. intro Related; apply representation_typed, indistinguishable_representation; exact Related. Qed.

  Lemma indistinguishable_application first second : indistinguishable (HTreeObj first) (HTreeObj second) ->
    indistinguishable (Thunk (Application first)) (Thunk (Application second)).
  Proof.
    intro Related; split.
    - apply (tree_thunk_equiv Application (application_thunk_cong R R_properties)); exact (proj1 Related).
    - apply represented_observations, RepresentApplication, indistinguishable_representation; exact Related.
  Qed.

  Lemma indistinguishable_selection first second : indistinguishable (HTreeObj first) (HTreeObj second) ->
    indistinguishable (Thunk (Selection first)) (Thunk (Selection second)).
  Proof.
    intro Related; split.
    - apply (tree_thunk_equiv Selection (selection_thunk_cong R R_properties)); exact (proj1 Related).
    - apply represented_observations, RepresentSelection, indistinguishable_representation; exact Related.
  Qed.

  Lemma indistinguishable_identification first second : indistinguishable (Data (Ref first)) (Data (Ref second)) ->
    indistinguishable (Thunk (Identification first)) (Thunk (Identification second)).
  Proof.
    intros [Related Observed]; split.
    - apply (think_to_equiv _ _ Related (Identification first) (Identification second) (Ref first) (Ref second)).
      + rewrite think_identification; reflexivity.
      + rewrite think_identification; reflexivity.
      + split; left; apply RSelf.
    - apply represented_observations, RepresentIdentification, observations_represented; exact Observed.
  Qed.

  Lemma indistinguishable_blob_reference first second : indistinguishable (HBlobObj first) (HBlobObj second) ->
    indistinguishable (HBlobRef first) (HBlobRef second).
  Proof.
    intros [Related Observed]; split; [apply equiv_blob_to_ref; exact Related|inversion Observed; constructor; assumption].
  Qed.

  Lemma indistinguishable_tree_reference first second : indistinguishable (HTreeObj first) (HTreeObj second) ->
    indistinguishable (HTreeRef first) (HTreeRef second).
  Proof. intros [Related Observed]; split; [apply equiv_tree_to_ref; exact Related|constructor; exact Observed]. Qed.

  Lemma indistinguishable_reference_blob first second : indistinguishable (HBlobRef first) (HBlobRef second) ->
    indistinguishable (HBlobObj first) (HBlobObj second).
  Proof.
    intros [Related Observed]; split; [apply equiv_ref_to_blob; exact Related|inversion Observed; constructor; assumption].
  Qed.

  Lemma indistinguishable_reference_tree first second : indistinguishable (HTreeRef first) (HTreeRef second) ->
    indistinguishable (HTreeObj first) (HTreeObj second).
  Proof. intros [Related Observed]; split; [apply equiv_ref_to_tree; exact Related|inversion Observed; assumption]. Qed.

  Lemma indistinguishable_laws : congruence indistinguishable.
  Proof.
    constructor; intros.
    - destruct H as [_ Observed]; inversion Observed; assumption.
    - apply (proj1 (indistinguishable_trees _ _)) in H; exact (proj2 H).
    - apply (proj1 (indistinguishable_trees _ _)) in H; exact (proj1 H).
    - apply (proj2 (indistinguishable_trees _ _)); rewrite !tree_tag_create_tree_tagged, !get_tree_raw_create_tree_tagged; auto.
    - apply indistinguishable_blob_reference; assumption.
    - apply indistinguishable_tree_reference; assumption.
    - apply indistinguishable_reference_blob; assumption.
    - apply indistinguishable_reference_tree; assumption.
    - apply indistinguishable_encode_strict; assumption.
    - apply indistinguishable_encode_shallow; assumption.
    - apply indistinguishable_application; assumption.
    - apply indistinguishable_selection; assumption.
    - apply indistinguishable_identification; assumption.
    - pose proof (indistinguishable_representation _ _ H) as Shape; inversion Shape; subst; eexists; reflexivity.
    - pose proof (indistinguishable_representation _ _ H) as Shape; inversion Shape; subst; eexists; reflexivity.
    - pose proof (indistinguishable_representation _ _ H) as Shape; inversion Shape; subst; eexists; reflexivity.
    - pose proof (indistinguishable_representation _ _ H) as Shape; inversion Shape; subst; eexists; reflexivity.
    - pose proof (indistinguishable_representation _ _ H) as Shape; inversion Shape; subst; split; intros [th Equal]; try discriminate; eauto.
    - apply indistinguishable_refl.
  Qed.

  Lemma indistinguishable_apply first second : indistinguishable (HTreeObj first) (HTreeObj second) ->
    rel_opt indistinguishable (A.apply_tree first) (A.apply_tree second).
  Proof.
    intro Related; exact (proj1 (A.apply_tree_X indistinguishable indistinguishable_laws _ _
      (conj Related (indistinguishable_typed _ _ Related)))).
  Qed.

  Lemma indistinguishable_program program first second : A.rel_state indistinguishable first second ->
    rel_opt indistinguishable (A.exec program first) (A.exec program second).
  Proof.
    intro Related.
    assert (Typed : A.rel_state same_typed_handle first second).
    { destruct Related as [Handles Bytes]; split; [|exact Bytes].
      eapply A.Forall2_mono; [intros x y Same; apply indistinguishable_typed; exact Same|exact Handles]. }
    exact (proj1 (A.exec_X indistinguishable indistinguishable_laws program _ _ (conj Related Typed))).
  Qed.

  Lemma indistinguishable_force first second : indistinguishable (Thunk first) (Thunk second) ->
    rel_opt indistinguishable (omap lower (force first)) (omap lower (force second)).
  Proof.
    intros [Related Observed].
    pose proof (equiv_eval _ _ (equiv_thunk_to_shallow_encode _ _ Related)) as Weak.
    change (rel_opt EC.computational_equiv (eval (Encode (Shallow first))) (eval (Encode (Shallow second)))) in Weak.
    rewrite !shallow_eval_force in Weak.
    pose proof (represented_force _ _ (observations_represented _ _ Observed)) as Strong.
    apply represented_results_iff in Strong.
    destruct (omap lower (force first)), (omap lower (force second)); cbn [rel_opt] in *; try contradiction;
      [split; [exact Weak|apply represented_observations; exact Strong]|exact I].
  Qed.
