  (** A supported statement kind survives encoding to its numeric code and decoding back. *)
  Lemma statement_code_roundtrip kind code :
    statement_code kind = Some code -> statement_kind code = Some kind.
  Proof. destruct kind; cbn; intro Code; inversion Code; reflexivity. Qed.

  (** A recognized statement code survives decoding and re-encoding unchanged. *)
  Lemma statement_kind_roundtrip code kind :
    statement_kind code = Some kind -> statement_code kind = Some code.
  Proof.
    do 7 (destruct code as [|code]; [cbn; intro Kind; inversion Kind; reflexivity|]); discriminate.
  Qed.

  (** Every supported statement code is less than seven. *)
  Lemma statement_code_bound kind code :
    statement_code kind = Some code -> code < 7.
  Proof. destruct kind; cbn; intro Code; inversion Code; auto with arith. Qed.
