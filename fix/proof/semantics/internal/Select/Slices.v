  (** A list slice succeeds exactly for valid half-open bounds and returns that range. *)
  Lemma list_slice_spec {A} (l : list A) a b result :
    list_slice l a b = Some result <->
    a <= b /\ b <= length l /\ result = firstn (b-a) (skipn a l).
  Proof.
    unfold list_slice; destruct ((a <=? b) && (b <=? length l)) eqn:Bounds.
    - apply andb_true_iff in Bounds; destruct Bounds as [Start End].
      apply Nat.leb_le in Start; apply Nat.leb_le in End.
      split; [intro Result; inversion Result; auto|intros [_ [_ ->]]; reflexivity].
    - split; [discriminate|intros [Start [End _]]].
      apply Nat.leb_le in Start; apply Nat.leb_le in End.
      rewrite Start, End in Bounds; discriminate.
  Qed.
