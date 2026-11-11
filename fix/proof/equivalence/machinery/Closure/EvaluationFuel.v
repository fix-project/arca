  (** Every successful evaluation is a justified replacement of its input by its result.
      This connects operational computation to the equivalence logic used by the
      collector. *)
  Theorem eval_with_fuel_equiv n : forall h r, eval_with_fuel n h = Some r -> computational_equiv h r.
  Proof.
    (* Evaluation changes only exposed Tree children and Encodes. Child replacements can
        be lifted through the enclosing Tree; an Encode contributes its explicit execution
        step, followed by the replacement justified for the returned value. *)
    induction n as [|n IH]; intros h r Fuel.
    - destruct h as [[[b|t]|ref]|th|e]; cbn [eval_with_fuel with_fuel eval_fn] in Fuel;
        try discriminate; inversion Fuel; subst; apply equiv_refl.
    - destruct h as [[[b|t]|ref]|th|e].
      + inversion Fuel; subst; apply equiv_refl.
      + change (omap HTreeObj (eval_tree_with_fuel n t) = Some r) in Fuel.
        apply omap_some in Fuel; destruct Fuel as [t' [Tree ->]].
        unfold eval_tree_with_fuel, eval_tree_using in Tree; apply omap_some in Tree.
        destruct Tree as [ys [List ->]].
        change (computational_equiv (HTreeObj t) (HTreeObj (create_tree_tagged (tree_tag t) ys))).
        (* Each recursive computation gives a replacement of one child. The Tree
            congruence argument combines these independent paths without requiring them to
            have matching numbers of steps. *)
        assert (ChildrenEquivalent : Forall2 computational_equiv (get_tree_raw t) ys).
        { apply eval_list_Forall2 in List; induction List; constructor; eauto. }
        rewrite <- (create_tree_get_tree_raw t) at 1.
        exact (equiv_tree_tagged_Forall2 _ _ _ ChildrenEquivalent).
      + inversion Fuel; subst; apply equiv_refl.
      + inversion Fuel; subst; apply equiv_refl.
      + change (obind (execute_with_fuel n e) (eval_with_fuel n) = Some r) in Fuel.
        apply obind_some in Fuel; destruct Fuel as [h' [Exec Ev]].
        (* This edge is supplied by actual execution, not by guessing an equivalent
            result. It is the bridge from the original Encode to the recursive evaluation
            of its result. *)
        assert (ExecutionEquivalent : computational_equiv (Encode e) h').
        { apply R_to_equiv, REvalStep, execute_unique; exists n; exact Exec. }
        pose proof (IH h' r Ev) as ResultEquivalent.
        exact (equiv_trans _ _ _ ExecutionEquivalent ResultEquivalent).
  Qed.
