  (** Every successful finite forcing derivation returns Data. *)
  Lemma forces_to_data th r : forces_to th r -> exists d, r = Data d.
  Proof. intro Force; apply force_data with th; apply force_unique; exact Force. Qed.

  (** Every successful finite evaluation derivation returns Data or a bare Thunk. *)
  Lemma evals_to_not_encode h r : evals_to h r ->
    (exists d, r = Data d) \/ (exists th, r = Thunk th).
  Proof.
    intro Eval; pose proof (eval_not_encode h r (eval_unique h r Eval)) as Shape.
    destruct r; [left; eauto|right; eauto|contradiction].
  Qed.

  (** Forcing to a Blob Object also evaluates the corresponding strict Encode to that Blob. *)
  Lemma forces_to_implies_evals_to th b : forces_to th (HBlobObj b) ->
    evals_to (Encode (Strict th)) (HBlobObj b).
  Proof.
    intro Force; apply eval_some; rewrite eval_encode, execute_unfold.
    cbn [encode_to_thunk]; rewrite (force_unique th _ Force).
    change (eval (HBlobObj b) = Some (HBlobObj b)); apply eval_blob.
  Qed.

  (** Independent child evaluations assemble into evaluation of the containing Tree. The
      common Tag marker records that evaluating fields does not change which kind of Tree
      is being evaluated. *)
  Lemma evaluation_tree tree children :
    Forall2 (fun h result => eval h = Some result) (get_tree_raw tree) children ->
    eval (HTreeObj tree) = Some (HTreeObj (create_tree_tagged (tree_tag tree) children)).
  Proof.
    intro Entries; apply eval_unique.
    apply (A.Forall2_mono _ evals_to) in Entries;
      [|intros h result Evaluated; apply eval_some; exact Evaluated].
    destruct (evals_to_tree_to _ _ Entries) as [n Evaluated].
    exists (S n).
    change (omap HTreeObj (omap (create_tree_tagged (tree_tag tree))
      (eval_list_with_fuel n (get_tree_raw tree))) =
        Some (HTreeObj (create_tree_tagged (tree_tag tree) children))).
    rewrite Evaluated; reflexivity.
  Qed.

  (** Pointwise evaluation of children gives evaluation of their ordinary Trees. *)
  Lemma eval_entry_to_eval_tree xs ys : Forall2 evals_to xs ys ->
    eval (HTreeObj (create_tree xs)) = Some (HTreeObj (create_tree ys)).
  Proof.
    intro Entries; pose proof (evaluation_tree (create_tree xs) ys) as Evaluate.
    rewrite get_tree_raw_create_tree, tree_tag_create_tree in Evaluate.
    apply Evaluate; eapply A.Forall2_mono; [|exact Entries].
    intros h result Evaluated; now apply eval_unique.
  Qed.

  (** A successful Tree evaluation consists of successful child evaluations and preserves the
      Tag marker.

      Guarded Isabelle [the (eval x)] uses become explicit output witnesses. The Forall2
      conclusion specifies every output, including duplicates. *)
  Lemma eval_tree_to_eval_entry t r : eval (HTreeObj t) = Some r ->
    exists ys, r = HTreeObj (create_tree_tagged (tree_tag t) ys) /\
      Forall2 (fun x y => eval x = Some y) (get_tree_raw t) ys /\
      Forall (fun x => exists y, eval x = Some y) (get_tree_raw t).
  Proof.
    intro Eval; rewrite eval_tree_handle in Eval.
    apply omap_some in Eval as [t' [Tree ->]]; apply eval_tree_some in Tree as [n Tree].
    change (omap (create_tree_tagged (tree_tag t)) (eval_list_with_fuel n (get_tree_raw t)) = Some t') in Tree.
    apply omap_some in Tree as [ys [Entries ->]].
    apply eval_list_Forall2 in Entries.
    assert (Outputs : Forall2 (fun x y => eval x = Some y) (get_tree_raw t) ys).
    { eapply A.Forall2_mono; [|exact Entries].
      intros x y Entry; apply eval_unique; exists n; exact Entry. }
    exists ys; split; [reflexivity|]; split; [exact Outputs|].
    clear Entries; induction Outputs; constructor; eauto.
  Qed.

  (** One Tree evaluates to another exactly when their markers agree and their children
      evaluate pointwise. *)
  Lemma eval_tree_iff t u :
    eval (HTreeObj t) = Some (HTreeObj u) <->
    tree_tag t = tree_tag u /\
    Forall2 (fun h r => eval h = Some r) (get_tree_raw t) (get_tree_raw u).
  Proof.
    split.
    - intro Evaluated.
      destruct (eval_tree_to_eval_entry _ _ Evaluated) as (children & Output & Entries & _).
      injection Output as ->.
      rewrite tree_tag_create_tree_tagged, get_tree_raw_create_tree_tagged; auto.
    - intros (Tags & Entries).
      pose proof (evaluation_tree t (get_tree_raw u) Entries) as Evaluated.
      now rewrite Tags, create_tree_get_tree_raw in Evaluated.
  Qed.

  (** Evaluating a Tree/Object-and-Ref pair evaluates only the Object field and preserves
      the outer marker. *)
  Lemma eval_tree_ref_pair tagged t reference evaluated :
    eval (HTreeObj (create_tree_tagged tagged [HTreeObj t; HTreeRef reference])) =
      Some (HTreeObj evaluated) ->
    exists u, eval (HTreeObj t) = Some (HTreeObj u) /\
      evaluated = create_tree_tagged tagged [HTreeObj u; HTreeRef reference].
  Proof.
    intro Evaluated.
    sauto inv: Forall2 ctrs: - cases: -
      use: eval_tree_iff, (eval_tree_to_eval_entry t),
        (eval_ref (TreeRef reference)), (create_tree_get_tree_raw evaluated),
        get_tree_raw_create_tree_tagged, tree_tag_create_tree_tagged
      unfold!: HTreeObj, HTreeRef.
  Qed.

  Lemma eval_handle_ref_pair tagged author reference evaluated :
    eval (HTreeObj (create_tree_tagged tagged [author; HTreeRef reference])) =
      Some (HTreeObj evaluated) ->
    exists evaluated_author, eval author = Some evaluated_author /\
      evaluated = create_tree_tagged tagged [evaluated_author; HTreeRef reference].
  Proof.
    intro Evaluated.
    sauto inv: Forall2 ctrs: - cases: -
      use: eval_tree_iff, (eval_ref (TreeRef reference)),
        (create_tree_get_tree_raw evaluated),
        get_tree_raw_create_tree_tagged, tree_tag_create_tree_tagged
      unfold!: HTreeObj, HTreeRef.
  Qed.

  (** Evaluating a Value Tree leaves it unchanged. *)
  Lemma value_tree_eval_to_itself t : value_tree t -> eval (HTreeObj t) = Some (HTreeObj t).
  Proof.
    (* Object containment is well founded, so the claim can be established from children
        to parents. Every child of a Value Tree is itself a Value and hence unchanged.
        Reconstructing the unchanged child list with the original marker gives the
        original Tree name. *)
    revert t; refine (well_founded_induction wfp_tree_child
      (fun t => value_tree t -> eval (HTreeObj t) = Some (HTreeObj t)) _).
    intros t IH Value; inversion Value as [tree Values]; subst.
    (* These child equalities justify using the original children as the whole evaluation
        result. The remaining step is the storage law identifying a Tree with
        reconstruction from its marker and children. *)
    assert (ChildrenUnchanged : Forall2 (fun child result => eval child = Some result)
      (get_tree_raw t) (get_tree_raw t)).
    (* The containment induction hypothesis applies to each Object-Tree child. Non-Tree
        Values are already leaves of evaluation, so no computation is introduced at those
        positions. *)
    { assert (EachChildUnchanged : forall child, In child (get_tree_raw t) ->
          eval child = Some child).
      { intros child Member.
        assert (ChildValue : value_handle child) by (eapply Forall_forall; eassumption).
        sauto inv: value_handle ctrs: - cases: - use: eval_blob, eval_ref, eval_thunk, IH. }
      apply Forall_forall in EachChildUnchanged.
      clear -EachChildUnchanged; induction EachChildUnchanged; constructor; assumption. }
    pose proof (evaluation_tree t (get_tree_raw t) ChildrenUnchanged) as TreeUnchanged.
    rewrite create_tree_get_tree_raw in TreeUnchanged; exact TreeUnchanged.
  Qed.
