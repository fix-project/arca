  Lemma eval_to_value_handle_proof h r : eval h = Some r -> value_handle r.
  Proof. rewrite eval_some; intros [n Ev]; eapply eval_with_fuel_to_value_handle; exact Ev. Qed.
