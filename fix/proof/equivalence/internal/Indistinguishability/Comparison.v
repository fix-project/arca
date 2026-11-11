  (** Handle comparison returns true exactly for Comparable, indistinguishable operands. *)
  Lemma compare_handles_equal left right : compare_handles left right = Some true <->
    is_comparable left = true /\ is_comparable right = true /\
    indistinguishable left right.
  Proof.
    unfold compare_handles.
    destruct (is_comparable left && is_comparable right) eqn:Comparable.
    - apply andb_true_iff in Comparable as [Left Right].
      destruct (excluded_middle_informative _); intuition discriminate.
    - split; [discriminate|intros [Left [Right _]]].
      rewrite Left, Right in Comparable; discriminate.
  Qed.

  (** Handle comparison is defined exactly when both operands are Comparable. *)
  Lemma compare_handles_defined left right :
    (exists result, compare_handles left right = Some result) <->
    is_comparable left = true /\ is_comparable right = true.
  Proof.
    unfold compare_handles.
    destruct (is_comparable left && is_comparable right) eqn:Comparable.
    - apply andb_true_iff in Comparable; split; [auto|intros _; eexists; reflexivity].
    - split; [intros [? Impossible]; discriminate|intros [Left Right]].
      rewrite Left, Right in Comparable; discriminate.
  Qed.
