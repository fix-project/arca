  Theorem comparable_equivalence_indistinguishable_proof left right :
    is_comparable left = true -> is_comparable right = true ->
    (computational_equiv left right <-> indistinguishable left right).
  Proof. unfold indistinguishable; intuition auto using comparable_equivalent_observations. Qed.
