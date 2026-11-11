  (** For a result-unique fuelled function, removing fuel succeeds exactly when some finite
      fuel succeeds. *)
  Lemma unfuel_some {A B} (f : nat -> A -> option B)
      (unique : forall x r1 r2, (exists n, f n x = Some r1) -> (exists n, f n x = Some r2) -> r1 = r2)
      x r : unfuel f x = Some r <-> exists n, f n x = Some r.
  Proof.
    (* The unfuelled evaluator chooses a successful result if one exists. Result
        uniqueness makes that choice immaterial: every finite successful run supplies the
        same answer, so removing the bound cannot introduce an additional result. *)
    unfold unfuel; destruct (excluded_middle_informative _) as [H|H].
    - destruct (constructive_indefinite_description _ H) as [chosen C]; simpl.
      split; [intro R; inversion R; subst; exact C|].
      intro R; f_equal; apply (unique x chosen r C R).
    - split; [discriminate|intro R; exfalso; apply H; exists r; exact R].
  Qed.
