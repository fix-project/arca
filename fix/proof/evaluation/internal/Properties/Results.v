  (** Successful forcing returns Data. *)
  Lemma force_data th r : force th = Some r -> exists d, r = Data d.
  Proof. rewrite force_some; intros [n F]; eapply force_with_fuel_to_data; exact F. Qed.
  (** Successful Encode execution returns Data. *)
  Lemma execute_data e h : execute e = Some h -> exists d, h = Data d.
  Proof.
    rewrite execute_unfold; destruct e; cbn; intro Exec; apply omap_some in Exec;
      destruct Exec as [v [Force ->]]; destruct (force_data _ _ Force) as [d ->];
      [exists (lift_data d)|exists (lower_data d)]; reflexivity.
  Qed.
  (** The strict version of a successful Encode returns the lifted result. *)
  Lemma execute_to_lift e h : execute e = Some h ->
    execute (Strict (encode_to_thunk e)) = Some (lift h).
  Proof.
    destruct e as [th|th]; intro Exec; rewrite execute_unfold in Exec;
      apply omap_some in Exec; destruct Exec as [r [Force ->]];
      cbn [encode_to_thunk] in Force;
      destruct (force_data _ _ Force) as [d ->];
      rewrite execute_unfold; cbn [encode_to_thunk]; rewrite Force;
      destruct d as [[b|t]|[b|t]]; reflexivity.
  Qed.
  (** Successful evaluation never returns an exposed Encode. *)
  Lemma eval_not_encode h r : eval h = Some r -> not_encode r.
  Proof.
    rewrite eval_some; intros [n Ev]; apply eval_with_fuel_not_encode in Ev;
      destruct Ev as [[d ->]|[th ->]]; exact I.
  Qed.
