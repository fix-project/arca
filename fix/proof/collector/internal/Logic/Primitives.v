  (** An Apply derivation describes one application of a Value combination and its exact
      result. *)
  Lemma coupon_apply_sound h r : coupon_apply h r -> exists t,
    h = HTreeObj t /\ value_handle h /\ A.apply_tree t = Some r.
  Proof. destruct 1; eauto. Qed.
  (** A Select derivation describes selection from its operand Tree and its exact result. *)
  Lemma coupon_select_sound h r : coupon_select h r -> exists t, h = HTreeObj t /\ Sl.select t = Some r.
  Proof. intro H; inversion H; subst; eauto. Qed.
