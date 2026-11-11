  (** Reading a successfully encoded certificate recovers its claim endpoints. *)
  Lemma read_encoded kind authors lhs rhs output :
    Wire.encode kind authors lhs rhs = Some output -> read kind output = Some (lhs, rhs).
  Proof.
    intro Encoded; apply Wire.encode_decode in Encoded; unfold read; rewrite Encoded.
    destruct (API.statement_kind_eq_dec kind kind); [reflexivity|contradiction].
  Qed.

  Lemma issued_authorized issuing_author witness :
    issued issuing_author witness -> authenticated_by issuing_author witness = true.
  Proof.
    intro Issued; destruct Issued as [tree result Value Applied|tree result Selected|
      rule coupons lhs rhs kind output Inputs Checked [Allowed Encoded]].
    - unfold authenticated_by.
      rewrite (Wire.decode_author _ _ _ _ _
        (Wire.encode_decode Apply (HTreeObj (create_tree [])) (HTreeObj tree) result _ eq_refl)).
      change (API.handle_eqb kernel_author issuing_author || API.handle_eqb kernel_author kernel_author = true).
      rewrite API.handle_eqb_refl, orb_true_r; reflexivity.
    - unfold authenticated_by.
      rewrite (Wire.decode_author _ _ _ _ _
        (Wire.encode_decode Select (HTreeObj (create_tree [])) (HTreeObj tree) result _ eq_refl)).
      change (API.handle_eqb kernel_author issuing_author || API.handle_eqb kernel_author kernel_author = true).
      rewrite API.handle_eqb_refl, orb_true_r; reflexivity.
    - unfold authenticated_by; rewrite (Wire.decode_author _ _ _ _ _
        (Wire.encode_decode _ _ _ _ _ Encoded)).
      rewrite API.handle_eqb_refl; reflexivity.
  Qed.

  (** A successful public exchange with previously issued premises can issue a certificate
      for its conclusion. *)
  Lemma issue_checked issuing_author rule coupons lhs rhs kind code :
    Forall (issued issuing_author) coupons -> check rule coupons lhs rhs = Some kind ->
    Wire.Code.statement_code kind = Some code -> certifies issuing_author kind lhs rhs.
  Proof.
    (* An abstract inference is useful to the scheduler only if it can be represented by
        an actual issued witness. The supported conclusion code supplies that
        representation, while the previous witnesses remain the ordered premises recorded
        in its issuance derivation. *)
    intros Inputs Checked Code.
    assert (OutputExists : exists output, Wire.encode kind issuing_author lhs rhs = Some output).
    { unfold Wire.encode; rewrite Code; eexists; reflexivity. }
    destruct OutputExists as [output Encoded]; exists output; split.
    - eapply IssuedExchange; [exact Inputs|exact Checked|].
      split; [|exact Encoded].
      unfold reconstruction_authority; apply forallb_forall.
      intros witness Present; eapply issued_authorized.
      eapply Forall_forall; eassumption.
    - eapply read_encoded; exact Encoded.
  Qed.

  (** Encoding a true claim produces a valid certificate witness. *)
  Lemma encoded_valid kind authors lhs rhs output :
    relation_of kind lhs rhs -> Wire.encode kind authors lhs rhs = Some output -> valid output.
  Proof.
    hauto cases: - use: Wire.encode_decode unfold!: valid.
  Qed.
