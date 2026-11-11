  Theorem indistinguishable_values_proof h1 h2 : indistinguishable h1 h2 ->
    (value_handle h1 <-> value_handle h2).
  Proof. intuition eauto using indistinguishable_value, indistinguishable_sym. Qed.
