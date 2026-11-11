  Theorem indistinguishable_select_index_proof d1 d2 i :
    indistinguishable (Data d1) (Data d2) ->
    rel_opt indistinguishable (Sl.select_index (Data d1) i)
      (Sl.select_index (Data d2) i).
  Proof.
    (* A selection may expose either an immediate Tree child or a one-byte Blob range.
        The observational relation supplies the exact child correspondence or byte
        equality needed for those observations, independently of the names of the
        containing Objects. *)
    intros [Equivalent Observed]; pose proof (observations_type _ _ Observed) as Types.
    destruct d1 as [[b1|t1]|[b1|t1]], d2 as [[b2|t2]|[b2|t2]];
      cbn [get_type] in Types; try discriminate.
    - apply indistinguishable_blob_range; inversion Observed; assumption.
    - apply indistinguishable_tree_index; split; assumption.
    - apply indistinguishable_blob_range; inversion Observed; assumption.
    - apply indistinguishable_tree_index.
      apply (indistinguishable_lift_data (Ref (TreeRef t1)) (Ref (TreeRef t2))).
      split; assumption.
  Qed.
