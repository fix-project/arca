  (** Finite-fuel Encode execution forces its Thunk, then lifts or lowers the result
      according to strictness. *)
  Lemma execute_with_fuel_force n e : execute_with_fuel n e =
    omap (match e with Strict _ => lift | Shallow _ => lower end)
         (force_with_fuel n (encode_to_thunk e)).
  Proof. destruct n, e; reflexivity. Qed.
