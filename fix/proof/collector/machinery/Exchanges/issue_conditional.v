  Theorem issue_conditional_proof trusted issuing_author rule coupons lhs rhs kind output :
    Forall (stable_conditional trusted) coupons -> check rule coupons lhs rhs = Some kind ->
    issue issuing_author kind lhs rhs coupons output -> stable_conditional trusted output.
  Proof.
    intros Inputs Checked [Allowed Issued] operation author left right Decoded Trusted.
    apply Wire.encode_decode in Issued; rewrite Issued in Decoded;
      inversion Decoded; subst operation author left right.
    eapply check_sound; [|exact Checked].
    eapply conditional_evidence_valid; eassumption.
  Qed.
