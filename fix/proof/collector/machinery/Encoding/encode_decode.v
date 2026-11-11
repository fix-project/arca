  Theorem encode_decode_proof kind author lhs rhs encoded :
    encode kind author lhs rhs = Some encoded ->
    decode encoded = Some (kind, author, lhs, rhs).
  Proof.
    unfold encode; destruct (Code.statement_code kind) as [code|] eqn:Code;
      cbn [omap]; intro Encoded; [inversion Encoded; subst encoded|discriminate].
    unfold witness, decode; repeat progress
      (cbn -[API.handle_eqb to_nat from_nat Code.statement_kind]; autorewrite with coupon_claims).
    rewrite to_nat_from_nat.
    - cbn [obind]; rewrite (Code.statement_code_roundtrip _ _ Code); reflexivity.
    - apply (Z.lt_trans _ 7);
        [pose proof (Code.statement_code_bound _ _ Code); lia|vm_compute; reflexivity].
  Qed.
