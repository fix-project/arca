  Theorem execute_unfold_proof e : execute e =
    omap (match e with Strict _ => lift | Shallow _ => lower end)
         (force (encode_to_thunk e)).
  Proof.
    apply option_ext_some; intro r; rewrite omap_some; split.
    - intro Ex; apply execute_some in Ex; destruct Ex as [n Ex].
      rewrite execute_with_fuel_force in Ex; apply omap_some in Ex.
      destruct Ex as [v [F ->]]; exists v; split; [apply force_unique; exists n; exact F|reflexivity].
    - intros [v [F ->]]; apply force_some in F; destruct F as [n F].
      apply execute_unique; exists n; rewrite execute_with_fuel_force, F; reflexivity.
  Qed.
