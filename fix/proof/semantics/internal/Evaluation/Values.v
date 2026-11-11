  (** A Value is not an Encode. *)
  Lemma value_handle_not_encode h : value_handle h -> ~(exists e, h = Encode e).
  Proof. intros V [e H]; inversion V; subst; discriminate. Qed.

  (** A Comparable Tree is already a Value Tree. *)
  Lemma comparable_tree_value t : tree_comparable t = true -> value_tree t.
  Proof.
    revert t; refine (well_founded_induction wfp_tree_child
      (fun t => tree_comparable t = true -> value_tree t) _).
    intros t IH Comparable; constructor.
    rewrite tree_comparable_contents, forallb_forall in Comparable.
    apply Forall_forall; intros h Member.
    pose proof (Comparable h Member) as Child.
    destruct h as [[[b|child]|reference]|thunk|encode]; cbn [is_comparable] in Child;
      try discriminate; constructor.
    apply IH; assumption.
  Qed.

  (** Every Comparable handle is already a Value. *)
  Lemma comparable_handle_value h : is_comparable h = true -> value_handle h.
  Proof.
    destruct h as [[[b|t]|reference]|thunk|encode]; cbn [is_comparable];
      intro Comparable; try discriminate; constructor.
    apply comparable_tree_value; exact Comparable.
  Qed.

  (** If entry evaluation returns Values, a successfully evaluated list contains only
      Values. *)
  Lemma eval_list_using_output f xs ys :
    (forall h r, f h = Some r -> value_handle r) ->
    eval_list_using f xs = Some ys -> Forall value_handle ys.
  Proof.
    intros Safe Ev; apply eval_list_Forall2 in Ev.
    induction Ev; constructor; eauto.
  Qed.
  (** Successful finite-fuel evaluation returns a Value. *)
  Lemma eval_with_fuel_to_value_handle n h r :
    eval_with_fuel n h = Some r -> value_handle r.
  Proof.
    revert h r; induction n as [|n IH]; intros h r Ev;
      destruct h as [d|th|e]; try (destruct d as [[b|t]|ref]).
    all: try solve [cbn [eval_with_fuel with_fuel next_evaluators eval_fn] in Ev;
      inversion Ev; subst; constructor].
    - change (omap HTreeObj (omap (create_tree_tagged (tree_tag t)) (eval_list_using (eval_with_fuel n) (get_tree_raw t))) = Some r) in Ev.
      destruct (eval_list_using (eval_with_fuel n) (get_tree_raw t)) as [ys|] eqn:List; simpl in Ev; try discriminate.
      inversion Ev; subst; apply tree_obj_handle, value_tree_intro.
      rewrite get_tree_raw_create_tree_tagged.
      eapply eval_list_using_output; [exact IH|exact List].
    - change (obind (execute_with_fuel n e) (eval_with_fuel n) = Some r) in Ev.
      destruct (execute_with_fuel n e) eqn:Ex; simpl in Ev; try discriminate.
      eapply IH; exact Ev.
  Qed.
