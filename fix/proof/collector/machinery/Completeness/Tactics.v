  #[local] Hint Rewrite API.handle_eqb_refl : coupon.
  Ltac simplify_check := repeat progress (cbn -[API.handle_eqb]; autorewrite with coupon).
  Ltac exchange exchange_rule premises :=
    eapply issue_checked with (rule := exchange_rule) (coupons := premises);
      [solve [auto using Forall_cons, Forall_nil]| |reflexivity];
    cbn [check unary binary];
    repeat match goal with
    | Read : read ?kind ?witness = _ |- context [read ?kind ?witness] => rewrite Read
    end;
    cbv beta iota zeta delta [substitute_eval substitute_force think_data force_encode think_step
      X.think_application X.think_selection];
    simplify_check; reflexivity.
