  Lemma collector_output_encodes lhs rhs author number kind :
    Claims.Code.statement_kind number = Some kind ->
    X.Wire.encode kind author lhs rhs = Some (treeh (create_tree
      [treeh (issued_tag author lhs rhs number);treeh (issued_claim lhs rhs number)])).
  Proof. apply collector_output_encodes_proof. Qed.
