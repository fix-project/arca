  Lemma observed_result_iff first second :
    observed_result first second <-> rel_opt same_observations first second.
  Proof.
    destruct first, second; cbn [rel_opt]; split; intro Related;
      try solve [inversion Related|constructor; assumption|constructor|exact I].
    inversion Related; assumption.
  Qed.

  Lemma eval_tree_children tree :
    eval (HTreeObj tree) =
    omap (fun children => HTreeObj (create_tree_tagged (tree_tag tree) children))
      (eval_list_using eval (get_tree_raw tree)).
  Proof.
    apply option_ext_some; intro result; rewrite omap_some; split.
    - intro Evaluated.
      destruct (eval_tree_to_eval_entry _ _ Evaluated) as [children [Result [Entries _]]].
      exists children; split; [apply Forall2_eval_list; exact Entries|exact Result].
    - intros [children [Entries ->]]; apply evaluation_tree.
      apply eval_list_Forall2; exact Entries.
  Qed.

  Lemma observed_eval_lists xs ys :
    Forall2 same_observations xs ys ->
    (forall x y, In x xs -> same_observations x y -> observed_result (eval x) (eval y)) ->
    rel_opt (Forall2 same_observations) (eval_list_using eval xs) (eval_list_using eval ys).
  Proof.
    intro Children; induction Children; intro Steps; cbn [eval_list_using]; [constructor|].
    pose proof (Steps x y (or_introl eq_refl) H) as Head.
    apply observed_result_iff in Head.
    assert (Tail : rel_opt (Forall2 same_observations) (eval_list_using eval l) (eval_list_using eval l')).
    { apply IHChildren; intros a b Present Same; apply Steps; [right; exact Present|exact Same]. }
    destruct (eval x), (eval y); cbn [rel_opt] in Head; try contradiction;
      cbn [obind]; [|exact I].
    destruct (eval_list_using eval l), (eval_list_using eval l'); cbn [rel_opt] in Tail;
      try contradiction; cbn [omap rel_opt]; [constructor; assumption|exact I].
  Qed.

  Lemma observed_children_comparability xs ys : Forall2 same_observations xs ys ->
    forallb is_comparable xs = forallb is_comparable ys.
  Proof. induction 1; cbn; [reflexivity|rewrite (observations_comparable _ _ H), IHForall2; reflexivity]. Qed.

  Lemma observations_eval_tree : forall first second,
    same_observations (HTreeObj first) (HTreeObj second) ->
    observed_result (eval (HTreeObj first)) (eval (HTreeObj second)).
  Proof.
    intro first; induction first as [first IH] using (well_founded_induction wfp_tree_child).
    intros second Same; inversion Same; subst.
    rewrite !eval_tree_children.
    assert (Children : Forall2 same_observations (get_tree_raw first) (get_tree_raw second))
      by (apply observed_children_Forall2; assumption).
    assert (Steps : forall x y, In x (get_tree_raw first) -> same_observations x y ->
      observed_result (eval x) (eval y)).
    { intros x y Present Related; destruct Related.
      - rewrite !eval_blob; constructor; constructor; assumption.
      - unfold HBlobRef; rewrite !eval_ref; constructor; constructor; assumption.
      - apply IH; [exact Present|constructor; assumption].
      - unfold HTreeRef; rewrite !eval_ref; constructor; constructor; assumption.
      - rewrite !eval_thunk; constructor; constructor; assumption.
      - apply observations_encode_eval; constructor; assumption.
      - apply observations_encode_eval; constructor; assumption. }
    pose proof (observed_eval_lists _ _ Children Steps) as Evaluated.
    destruct (eval_list_using eval (get_tree_raw first)), (eval_list_using eval (get_tree_raw second));
      cbn [rel_opt] in Evaluated; try contradiction; cbn [omap]; [|constructor].
    constructor; constructor.
    - rewrite !tree_tag_create_tree_tagged; assumption.
    - rewrite !tree_comparable_create_tree_tagged; apply observed_children_comparability; exact Evaluated.
    - rewrite !get_tree_raw_create_tree_tagged; apply observed_children_Forall2; exact Evaluated.
  Qed.

  Lemma observations_eval left right : same_observations left right ->
    observed_result (eval left) (eval right).
  Proof.
    intro Related; destruct Related.
    - rewrite !eval_blob; constructor; constructor; assumption.
    - unfold HBlobRef; rewrite !eval_ref; constructor; constructor; assumption.
    - apply observations_eval_tree; constructor; assumption.
    - unfold HTreeRef; rewrite !eval_ref; constructor; constructor; assumption.
    - rewrite !eval_thunk; constructor; constructor; assumption.
    - apply observations_encode_eval; constructor; assumption.
    - apply observations_encode_eval; constructor; assumption.
  Qed.
