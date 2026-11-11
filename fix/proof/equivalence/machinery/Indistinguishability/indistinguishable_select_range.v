  Theorem indistinguishable_select_range_proof d1 d2 a b :
    indistinguishable (Data d1) (Data d2) ->
    rel_opt indistinguishable (Sl.select_range (Data d1) a b)
      (Sl.select_range (Data d2) a b).
  Proof.
    intros [Equivalent Observed]; pose proof (observations_type _ _ Observed) as Types.
    destruct d1 as [[b1|t1]|[b1|t1]], d2 as [[b2|t2]|[b2|t2]];
      cbn [get_type] in Types; try discriminate.
    - apply indistinguishable_blob_range; inversion Observed; assumption.
    - apply indistinguishable_tree_range; split; assumption.
    - apply indistinguishable_blob_range; inversion Observed; assumption.
    - apply indistinguishable_tree_range.
      apply (indistinguishable_lift_data (Ref (TreeRef t1)) (Ref (TreeRef t2))).
      split; assumption.
  Qed.
