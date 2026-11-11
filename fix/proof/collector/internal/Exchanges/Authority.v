  Lemma authenticated_author collector witness kind author lhs rhs :
    authenticated_by collector witness = true ->
    Wire.decode witness = Some (kind, author, lhs, rhs) ->
    author = collector \/ author = kernel_author.
  Proof.
    intros Allowed Decoded; unfold authenticated_by in Allowed.
    rewrite (Wire.decode_author _ _ _ _ _ Decoded) in Allowed.
    apply orb_true_iff in Allowed as [Same|Kernel];
      apply API.handle_eqb_spec in Same || apply API.handle_eqb_spec in Kernel; auto.
  Qed.

  Lemma conditional_evidence_valid trusted collector coupons :
    stable_requirement trusted collector -> Forall (stable_conditional trusted) coupons ->
    reconstruction_authority collector coupons = true -> Forall valid coupons.
  Proof.
    intros Trusted Conditional Allowed; rewrite Forall_forall in Conditional |- *.
    unfold reconstruction_authority in Allowed; rewrite forallb_forall in Allowed.
    intros witness Present kind author lhs rhs Decoded.
    apply (Conditional _ Present _ _ _ _ Decoded).
    destruct (authenticated_author _ _ _ _ _ _ (Allowed _ Present) Decoded) as [Same|Kernel].
    - subst author; exact Trusted.
    - subst author; intros _; left; reflexivity.
  Qed.

  Lemma reconstruction_evidence_valid trusted collector coupons :
    trusted collector -> Forall (stable_conditional trusted) coupons ->
    reconstruction_authority collector coupons = true -> Forall valid coupons.
  Proof.
    intros Trusted Conditional Allowed; eapply conditional_evidence_valid; eauto.
    intros _; right; exact Trusted.
  Qed.
