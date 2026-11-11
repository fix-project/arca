  Theorem compare_handles_congruence_proof left left' right right' :
    indistinguishable left left' -> indistinguishable right right' ->
    compare_handles left right = compare_handles left' right'.
  Proof.
    (* There are two potential ways to distinguish replacement operands: whether
        comparison is available and whether it returns true. Observational agreement
        preserves Comparability, while symmetry and transitivity preserve the truth of the
        indistinguishability predicate used by the comparison. *)
    intros Left Right.
    assert (SamePredicate : indistinguishable left right <-> indistinguishable left' right')
      by (split; eauto 6 using indistinguishable_sym, indistinguishable_trans).
    assert (SameLeft : is_comparable left = is_comparable left')
      by (hauto cases: - use: indistinguishable_comparable).
    assert (SameRight : is_comparable right = is_comparable right')
      by (hauto cases: - use: indistinguishable_comparable).
    sauto unfold!: compare_handles.
  Qed.
