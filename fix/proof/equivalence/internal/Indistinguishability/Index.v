  (** Lifting Data preserves indistinguishability. *)
  Lemma indistinguishable_lift_data d1 d2 :
    indistinguishable (Data d1) (Data d2) ->
    indistinguishable (Data (lift_data d1)) (Data (lift_data d2)).
  Proof.
    intros [Equivalent Observed]; split.
    - exact (equiv_to_lift _ _ Equivalent).
    - exact (observations_lift _ _ Observed).
  Qed.

  (** Lowering Data preserves indistinguishability. *)
  Lemma indistinguishable_lower_data d1 d2 :
    indistinguishable (Data d1) (Data d2) ->
    indistinguishable (Data (lower_data d1)) (Data (lower_data d2)).
  Proof.
    intros [Equivalent Observed]; split.
    - exact (equiv_to_lower _ _ Equivalent).
    - exact (observations_lower _ _ Observed).
  Qed.

  (** Equal-content Blob Objects give indistinguishable range-selection results or shared
      failure. *)
  Lemma indistinguishable_blob_range b1 b2 a b :
    get_blob_data b1 = get_blob_data b2 ->
    rel_opt indistinguishable (Sl.select_range (HBlobObj b1) a b)
      (Sl.select_range (HBlobObj b2) a b).
  Proof.
    intro Contents; unfold HBlobObj, Sl.select_range, Sl.range_ref, Sl.blob_slice; rewrite Contents.
    destruct (Sl.list_slice (get_blob_data b2) a b); cbn [omap rel_opt];
      [apply indistinguishable_refl|exact I].
  Qed.

  (** Indexing indistinguishable Tree Objects gives indistinguishable results or shared
      failure. *)
  Lemma indistinguishable_tree_index t1 t2 i :
    indistinguishable (HTreeObj t1) (HTreeObj t2) ->
    rel_opt indistinguishable (Sl.select_index (HTreeObj t1) i)
      (Sl.select_index (HTreeObj t2) i).
  Proof.
    intro Related; apply indistinguishable_trees in Related as [_ Children].
    apply Forall2_nth_error; exact Children.
  Qed.
