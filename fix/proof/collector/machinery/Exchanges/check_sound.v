  Theorem check_sound_proof rule coupons lhs rhs kind :
    Forall valid coupons -> check rule coupons lhs rhs = Some kind -> relation_of kind lhs rhs.
  Proof.
    (* The explicit request code chooses the inference rule, and each branch checks only
        that rule's ordered premises and endpoints. The earlier helper lemmas establish
        the rule-shaped implications, so an accepted branch contributes exactly one
        legitimate logical constructor. *)
    intros Valid Checked; destruct rule; cbn [check] in Checked;
      try solve [eapply (tree_sound Equivalent); [intros; apply CouponTreeEq; assumption|exact Valid|exact Checked]];
      try solve [eapply (tree_sound Eval); [intros; apply CouponEvalTreeObj; assumption|exact Valid|exact Checked]];
      try solve [eauto using force_results_sound, leaf_sound, self_sound, identification_sound];
      try solve [eapply unary_sound; [|exact Valid|exact Checked]; intros; cbn [relation_of] in *;
        eauto using mapped_good, force_encode_good, think_step_good, think_data_good];
      try solve [eapply binary_sound; [|exact Valid|exact Checked]; intros; cbn [relation_of] in *;
        eauto using think_application_good, substitute_eval_good, transitive_good,
          substitute_force_good, think_selection_good].
    - eapply unary_sound; [|exact Valid|exact Checked].
      intros a b x y Related Derived; inversion Derived; subst; apply CouponSym; exact Related.
    - eapply unary_sound; [|exact Valid|exact Checked].
      intros a b x y Evaluated Derived; inversion Derived; subst; apply CouponEvalResultEq; exact Evaluated.
  Qed.
