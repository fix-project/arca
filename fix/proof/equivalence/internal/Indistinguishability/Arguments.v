  (** Indistinguishable handles decode to identical selection indices or shared failure. *)
  Lemma indistinguishable_read_index h1 h2 : indistinguishable h1 h2 ->
    Sl.read_index h1 = Sl.read_index h2.
  Proof.
    intros [_ Observed]; destruct Observed;
      cbn [HBlobObj HTreeObj HBlobRef HTreeRef Sl.read_index]; congruence.
  Qed.
