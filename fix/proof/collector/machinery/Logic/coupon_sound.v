  Theorem coupon_sound_proof :
    (forall h r, coupon_force h r -> force_result h r) /\
    (forall h r, coupon_apply h r -> exists t,
      h = HTreeObj t /\ value_handle h /\ A.apply_tree t = Some r) /\
    (forall h r, coupon_select h r -> exists t, h = HTreeObj t /\ Sl.select t = Some r) /\
    (forall h r, coupon_think h r -> think_result h r) /\
    (forall h r, coupon_eval h r -> eval_result h r) /\
    (forall h1 h2, coupon_equiv h1 h2 -> computational_equiv h1 h2).
  Proof.
    (* The mutually recursive argument justifies each rule from its premises, including
        substitution of equivalent representatives. The combined theorem exposes those
        separate judgements without introducing a new logical rule. *)
    split; [exact coupon_force_sound|].
    split; [exact coupon_apply_sound|].
    split; [exact coupon_select_sound|].
    split; [exact coupon_think_sound|].
    split; [exact coupon_eval_sound|exact coupon_equiv_sound].
  Qed.
