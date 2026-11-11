  Theorem reconstruct_tag_sound_proof trusted collector coupons original fields result :
    trusted collector -> Forall (stable_conditional trusted) coupons ->
    reconstruct_tag collector coupons original fields = Some result ->
    D.coupon_eval (HTreeObj original) (HTreeObj result).
  Proof.
    intros Trusted Conditional Reconstructed.
    unfold reconstruct_tag in Reconstructed.
    destruct (get_tree_raw fields) as [|first rest] eqn:Fields; try discriminate.
    match type of Reconstructed with
    | (if ?condition then _ else _) = _ => destruct condition eqn:Checked
    end; [|discriminate].
    inversion Reconstructed; subst result.
    apply andb_true_iff in Checked as [Prefix Children].
    apply andb_true_iff in Prefix as [Tagged Allowed].
    apply CouponEvalTreeObj.
    - rewrite tree_tag_create_tree_tagged; exact Tagged.
    - rewrite get_tree_raw_create_tree_tagged.
      eapply children_sound with (kind := Eval); [|exact Children].
      eapply reconstruction_evidence_valid; eassumption.
  Qed.
