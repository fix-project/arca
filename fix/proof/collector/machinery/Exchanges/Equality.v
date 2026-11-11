  Ltac check_handle_eq left right :=
    let Same := fresh "Same" in
    destruct (API.handle_eqb left right) eqn:Same; [|discriminate];
    apply API.handle_eqb_spec in Same; subst right.
