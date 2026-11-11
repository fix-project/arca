  Theorem indistinguishable_select_arguments_proof d1 d2 xs ys :
    indistinguishable (Data d1) (Data d2) -> Forall2 indistinguishable xs ys ->
    rel_opt indistinguishable (Sl.select_arguments (Data d1) xs)
      (Sl.select_arguments (Data d2) ys).
  Proof.
    (* Indistinguishable index operands decode to the same integers. Consequently both
        selections choose the same arity-specific operation and the same bounds; only the
        already related targets can affect the returned contents. *)
    intros Related Indices; destruct Indices as [|i1 i2 xs ys Index Tail]; [exact I|].
    pose proof (indistinguishable_read_index _ _ Index) as First.
    destruct Tail as [|j1 j2 xs ys Second Tail].
    - cbn [Sl.select_arguments]; rewrite First.
      destruct (Sl.read_index i2); cbn [obind];
        [apply indistinguishable_select_index; exact Related|exact I].
    - destruct Tail as [|k1 k2 xs ys Third Tail]; [|exact I].
      pose proof (indistinguishable_read_index _ _ Second) as Last.
      cbn [Sl.select_arguments]; rewrite First, Last.
      destruct (Sl.read_index i2), (Sl.read_index j2); cbn [obind];
        [apply indistinguishable_select_range; exact Related|exact I|exact I|exact I].
  Qed.
