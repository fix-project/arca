  Lemma indistinguishable_lower first second : indistinguishable (Data first) (Data second) ->
    indistinguishable (lower (Data first)) (lower (Data second)).
  Proof.
    intros [Related Observed]; split.
    - exact (EC.equiv_to_lower _ _ Related).
    - apply observations_lower; exact Observed.
  Qed.

  Lemma indistinguishable_lift first second : indistinguishable (Data first) (Data second) ->
    indistinguishable (lift (Data first)) (lift (Data second)).
  Proof.
    intros [Related Observed]; split.
    - exact (EC.equiv_to_lift _ _ Related).
    - apply observations_lift; exact Observed.
  Qed.

  Lemma indistinguishable_lift_lower first second :
    indistinguishable (lower (Data first)) (lower (Data second)) ->
    indistinguishable (lift (Data first)) (lift (Data second)).
  Proof.
    intro Related.
    assert (Lifted : indistinguishable (lift (lower (Data first))) (lift (lower (Data second)))).
    { rewrite !lower_data_reference in *; apply indistinguishable_lift; exact Related. }
    destruct first as [[b|t]|[b|t]], second as [[b'|t']|[b'|t']]; exact Lifted.
  Qed.

  Lemma equiv_blob_to_ref first second : computational_equiv (HBlobObj first) (HBlobObj second) ->
    computational_equiv (HBlobRef first) (HBlobRef second).
  Proof.
    intro Related; apply indistinguishable_equiv, indistinguishable_blob_reference.
    apply (proj1 (equiv_values _ _ (blob_obj_handle first) (blob_obj_handle second))); exact Related.
  Qed.

  Lemma equiv_tree_to_ref first second : value_handle (HTreeObj first) -> value_handle (HTreeObj second) ->
    computational_equiv (HTreeObj first) (HTreeObj second) -> computational_equiv (HTreeRef first) (HTreeRef second).
  Proof.
    intros First Second Related; apply indistinguishable_equiv, indistinguishable_tree_reference.
    apply (proj1 (equiv_values _ _ First Second)); exact Related.
  Qed.

  Lemma equiv_identification first second : computational_equiv (Data (Ref first)) (Data (Ref second)) ->
    computational_equiv (Thunk (Identification first)) (Thunk (Identification second)).
  Proof. intro Related; apply indistinguishable_equiv, indistinguishable_identification, equivalent_references; exact Related. Qed.

  Lemma equiv_thunk_to_strict_encode first second : computational_equiv (Thunk first) (Thunk second) ->
    computational_equiv (Encode (Strict first)) (Encode (Strict second)).
  Proof. intro Related; apply indistinguishable_equiv, indistinguishable_encode_strict, equivalent_thunks; exact Related. Qed.

  Lemma equiv_thunk_to_shallow_encode first second : computational_equiv (Thunk first) (Thunk second) ->
    computational_equiv (Encode (Shallow first)) (Encode (Shallow second)).
  Proof. intro Related; apply indistinguishable_equiv, indistinguishable_encode_shallow, equivalent_thunks; exact Related. Qed.

  Lemma equivalent_children first second : Forall2 computational_equiv first second ->
    rel_opt (Forall2 indistinguishable) (eval_list_using eval first) (eval_list_using eval second).
  Proof.
    induction 1; cbn [eval_list_using]; [constructor|].
    pose proof (equiv_eval _ _ H) as Head.
    destruct (eval x), (eval y); cbn [rel_opt] in Head; try contradiction; cbn [obind]; [|exact I].
    destruct (eval_list_using eval l), (eval_list_using eval l'); cbn [rel_opt] in IHForall2; try contradiction;
      cbn [omap rel_opt]; [constructor; assumption|exact I].
  Qed.

  Lemma equiv_tree_entries first second : tree_tag first = tree_tag second ->
    Forall2 computational_equiv (get_tree_raw first) (get_tree_raw second) ->
    computational_equiv (HTreeObj first) (HTreeObj second).
  Proof.
    intros Tag Children; split.
    - apply (proj2 (EC.equiv_tree_iff _ _)); split; [exact Tag|].
      eapply (A.Forall2_mono computational_equiv EC.computational_equiv);
        [intros x y Related; exact (proj1 Related)|exact Children].
    - rewrite !eval_tree_children.
      pose proof (equivalent_children _ _ Children) as Evaluated.
      destruct (eval_list_using eval (get_tree_raw first)), (eval_list_using eval (get_tree_raw second));
        cbn [rel_opt] in Evaluated; try contradiction; cbn [omap rel_opt]; [|exact I].
      apply (proj2 (indistinguishable_trees _ _)).
      rewrite !tree_tag_create_tree_tagged, !get_tree_raw_create_tree_tagged; auto.
  Qed.

  Lemma equiv_thunk_constructor kind operation
      (Weak : forall first second, EC.computational_equiv (HTreeObj first) (HTreeObj second) ->
        EC.computational_equiv (Thunk (kind first)) (Thunk (kind second)))
      (Represent : forall first second, representation (HTreeObj first) (HTreeObj second) ->
        representation (Thunk (kind first)) (Thunk (kind second)))
      (Think : forall tree, think (kind tree) = obind (eval_tree tree) operation)
      first second : computational_equiv (HTreeObj first) (HTreeObj second) ->
    computational_equiv (Thunk (kind first)) (Thunk (kind second)).
  Proof.
    intros [Related Evaluated]; apply indistinguishable_equiv; split; [apply Weak; exact Related|].
    apply represented_observations, RepresentBehavior.
    rewrite !eval_tree_handle in Evaluated.
    destruct (eval_tree first) as [left|] eqn:First, (eval_tree second) as [right|] eqn:Second;
      cbn [omap rel_opt] in Evaluated; try contradiction.
    - assert (Left : force (kind first) = force (kind left)).
      { rewrite (force_unfold (kind first)), (force_unfold (kind left)), !Think, First,
          (value_tree_fixed_eval _ (eval_tree_to_value_handle _ _ First)); reflexivity. }
      assert (Right : force (kind second) = force (kind right)).
      { rewrite (force_unfold (kind second)), (force_unfold (kind right)), !Think, Second,
          (value_tree_fixed_eval _ (eval_tree_to_value_handle _ _ Second)); reflexivity. }
      rewrite Left, Right; apply represented_force, Represent, indistinguishable_representation; exact Evaluated.
    - assert (Left : force (kind first) = None) by (rewrite force_unfold, Think, First; reflexivity).
      assert (Right : force (kind second) = None) by (rewrite force_unfold, Think, Second; reflexivity).
      rewrite Left, Right; constructor.
  Qed.

  Lemma equiv_application first second : computational_equiv (HTreeObj first) (HTreeObj second) ->
    computational_equiv (Thunk (Application first)) (Thunk (Application second)).
  Proof.
    eapply equiv_thunk_constructor with (operation := A.apply_tree).
    - apply (EC.tree_thunk_equiv Application (application_thunk_cong R R_properties)).
    - intros; constructor; assumption.
    - exact think_application.
  Qed.

  Lemma equiv_selection first second : computational_equiv (HTreeObj first) (HTreeObj second) ->
    computational_equiv (Thunk (Selection first)) (Thunk (Selection second)).
  Proof.
    eapply equiv_thunk_constructor with (operation := Sl.select).
    - apply (EC.tree_thunk_equiv Selection (selection_thunk_cong R R_properties)).
    - intros; constructor; assumption.
    - exact think_selection.
  Qed.

  Lemma equiv_tree_tagged_Forall2 tag xs ys : Forall2 computational_equiv xs ys ->
    computational_equiv (HTreeObj (create_tree_tagged tag xs)) (HTreeObj (create_tree_tagged tag ys)).
  Proof.
    intro Related; apply equiv_tree_entries; rewrite ?tree_tag_create_tree_tagged, ?get_tree_raw_create_tree_tagged; auto.
  Qed.

  Lemma equiv_blobs first second : get_blob_data first = get_blob_data second ->
    computational_equiv (HBlobObj first) (HBlobObj second).
  Proof.
    intro Bytes; apply indistinguishable_equiv; split;
      [apply EC.R_to_equiv, RBlob; exact Bytes|constructor; exact Bytes].
  Qed.

  Lemma equiv_blob_same_data h1 h2 : computational_equiv h1 h2 -> forall b1, h1 = HBlobObj b1 ->
    (exists b2, h2 = HBlobObj b2 /\ get_blob_data b1 = get_blob_data b2) \/
    (exists th b2, h2 = Encode (Strict th) /\ eval h2 = Some (HBlobObj b2) /\ get_blob_data b1 = get_blob_data b2).
  Proof. intros [Related _]; exact (EC.equiv_blob_same_data _ _ Related). Qed.
