  (** Slicing indistinguishable Tree Objects gives indistinguishable range-selection
      results or shared failure. *)
  Lemma indistinguishable_tree_range t1 t2 a b :
    indistinguishable (HTreeObj t1) (HTreeObj t2) ->
    rel_opt indistinguishable (Sl.select_range (HTreeObj t1) a b)
      (Sl.select_range (HTreeObj t2) a b).
  Proof.
    intro Related; apply indistinguishable_trees in Related as [_ Children].
    pose proof (Sl.list_slice_X indistinguishable _ _ a b Children) as Slices.
    unfold HTreeObj, Sl.select_range, Sl.range_ref, Sl.tree_slice.
    destruct (Sl.list_slice (get_tree_raw t1) a b) as [xs|],
      (Sl.list_slice (get_tree_raw t2) a b) as [ys|];
      cbn [rel_opt omap] in *; try contradiction; [|exact I].
    apply (indistinguishable_lower_data (Object (TreeObj (create_tree xs)))
      (Object (TreeObj (create_tree ys)))).
    apply indistinguishable_trees; rewrite !tree_tag_create_tree, !get_tree_raw_create_tree; auto.
  Qed.
