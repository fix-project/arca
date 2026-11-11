  Theorem reconstruction_validated_sound_proof trusted collector coupons lhs rhs :
    trusted collector -> Forall (stable_conditional trusted) coupons ->
    reconstruction_authority collector coupons = true ->
    tree Eval coupons lhs rhs = Some Eval -> D.coupon_eval lhs rhs.
  Proof.
    intros Trusted Conditional Allowed Checked.
    change (relation_of Eval lhs rhs).
    eapply tree_sound with (input := Eval) (kind := Eval);
      [intros; apply CouponEvalTreeObj; assumption| |exact Checked].
    eapply reconstruction_evidence_valid; eassumption.
  Qed.
