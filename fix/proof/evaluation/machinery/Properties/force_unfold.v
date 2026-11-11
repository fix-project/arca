  Theorem force_unfold_proof th : force th = obind (think th) (fun h =>
    match h with
    | Data _ => Some h
    | Thunk th' => force th'
    | Encode e => force (encode_to_thunk e)
    end).
  Proof.
    apply option_ext_some; intro r; rewrite obind_some; split.
    - intro F; apply force_some in F; destruct F as [n F].
      destruct n as [|n]; [discriminate|].
      change (obind (think_with_fuel n th) (fun h =>
        match h with
        | Data _ => Some h
        | Thunk th' | Encode (Strict th') | Encode (Shallow th') => force_with_fuel n th'
        end) = Some r) in F.
      apply obind_some in F; destruct F as [h [T F]].
      exists h; split; [apply think_unique; exists n; exact T|].
      destruct h as [d|th'|[th'|th']]; [exact F| | |];
        apply force_unique; exists n; exact F.
    - intros [h [T F]]; apply think_some in T; destruct T as [n T].
      destruct h as [d|th'|[th'|th']].
      + inversion F; subst; apply force_unique; exists (S n).
        change (obind (think_with_fuel n th) (force_after n) = Some (Data d)); rewrite T; reflexivity.
      + apply force_some in F; destruct F as [m F].
        apply force_unique; exists (S (n+m)).
        change (obind (think_with_fuel (n+m) th) (force_after (n+m)) = Some r).
        rewrite (think_padding n m th _ T); simpl.
        rewrite Nat.add_comm; apply force_padding; exact F.
      + apply force_some in F; destruct F as [m F].
        apply force_unique; exists (S (n+m)).
        change (obind (think_with_fuel (n+m) th) (force_after (n+m)) = Some r).
        rewrite (think_padding n m th _ T); simpl.
        rewrite Nat.add_comm; apply force_padding; exact F.
      + apply force_some in F; destruct F as [m F].
        apply force_unique; exists (S (n+m)).
        change (obind (think_with_fuel (n+m) th) (force_after (n+m)) = Some r).
        rewrite (think_padding n m th _ T); simpl.
        rewrite Nat.add_comm; apply force_padding; exact F.
  Qed.
