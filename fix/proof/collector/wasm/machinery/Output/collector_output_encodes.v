  Lemma collector_output_encodes_proof lhs rhs author number kind :
    Claims.Code.statement_kind number = Some kind ->
    X.Wire.encode kind author lhs rhs = Some (treeh (create_tree
      [treeh (issued_tag author lhs rhs number);treeh (issued_claim lhs rhs number)])).
  Proof.
    intro Kind; unfold X.Wire.encode.
    change (X.Wire.Code.statement_kind number = Some kind) in Kind.
    rewrite (X.Wire.Code.statement_kind_roundtrip _ _ Kind).
    cbn [X.Wire.H.omap].
    cbv beta iota zeta delta [X.Wire.witness issued_tag issued_claim]; reflexivity.
  Qed.
