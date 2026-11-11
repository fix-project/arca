  Definition compared state := G.E.H.State (G.E.H.slots state) (G.E.H.observations state + 2).

  Lemma compared_scope state : scope state (compared state) 0.
  Proof. split; [intros index value Present; exact Present|cbn; lia]. Qed.

  Definition observed_comparison state lhs rhs equal := exists left right,
    handle_bytes (G.E.H.observations state) lhs = Some left /\
    handle_bytes (Datatypes.S (G.E.H.observations state)) rhs = Some right /\
    G.E.H.T.O.compare_handles (blobh left) (blobh right) = Some equal.
  Definition observed_same state lhs rhs := observed_comparison state lhs rhs true.
