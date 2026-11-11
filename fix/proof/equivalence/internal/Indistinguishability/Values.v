  (** A Tree Object indistinguishable from a Value Tree is also a Value Tree. *)
  Lemma indistinguishable_tree_value t1 t2 :
    indistinguishable (HTreeObj t1) (HTreeObj t2) -> value_tree t1 -> value_tree t2.
  Proof.
    (* Being a Value is determined by exposed structure: a Tree requires Value children,
        while Refs and bare Thunks stop the traversal. Matching observations preserve
        these stopping points, and well-founded Object containment propagates the argument
        through the exposed Tree children. *)
    revert t1 t2; refine (well_founded_induction wfp_tree_child
      (fun t1 => forall t2, indistinguishable (HTreeObj t1) (HTreeObj t2) ->
        value_tree t1 -> value_tree t2) _).
    intros t1 IH t2 Related Value.
    assert (Children : Forall2 indistinguishable (get_tree_raw t1) (get_tree_raw t2))
      by (hauto cases: - use: indistinguishable_trees).
    assert (Values : Forall value_handle (get_tree_raw t1))
      by (sauto inv: value_tree ctrs: - cases: -).
    assert (ChildValues : forall child child', In child (get_tree_raw t1) ->
      indistinguishable child child' -> value_handle child -> value_handle child').
    { intros child child' Member ChildRelated ChildValue.
      pose proof ChildRelated as [_ Observed].
      destruct Observed; inversion ChildValue; subst; constructor.
      eapply IH; [exact Member|exact ChildRelated|assumption]. }
    constructor; clear -Children Values ChildValues.
    revert Values ChildValues; induction Children;
      sauto inv: Forall ctrs: Forall cases: -.
  Qed.

  (** A handle indistinguishable from a Value is also a Value. *)
  Lemma indistinguishable_value h1 h2 :
    indistinguishable h1 h2 -> value_handle h1 -> value_handle h2.
  Proof.
    intros Related Value; pose proof Related as [_ Observed];
      destruct Observed; inversion Value; subst; constructor.
    eapply indistinguishable_tree_value; eassumption.
  Qed.
