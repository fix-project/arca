  (** Lowering Data produces a Ref whose lifted Object is the original Data's lifted
      Object. *)
  Lemma lower_reference d : exists reference,
    lower (Data d) = Data (Ref reference) /\
    lift (Data (Ref reference)) = lift (Data d).
  Proof. destruct d as [[b|t]|[b|t]]; eexists; split; reflexivity. Qed.
