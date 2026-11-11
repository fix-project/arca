  (** Every issued witness has a claim justified by the abstract coupon rules. *)
  Fixpoint issued_sound issuing_author witness (Derivation : issued issuing_author witness) {struct Derivation} : valid witness.
  Proof.
    (* Every possible source of a certificate must be justified. The two primitive
        sources require the actual computation facts in their constructors. For an
        exchange, the earlier issuance derivations justify its premises, and checker
        soundness justifies the new fact. *)
    destruct Derivation as [tree result Value Applied|tree result Selected|
      rule coupons lhs rhs kind output Inputs Checked Encoded].
    - eapply (encoded_valid Apply (HTreeObj (create_tree [])) (HTreeObj tree) result);
        [|reflexivity].
      apply CouponApply; assumption.
    - eapply (encoded_valid Select (HTreeObj (create_tree [])) (HTreeObj tree) result);
        [|reflexivity].
      apply CouponSelect; exact Selected.
    - destruct Encoded as [_ Encoded].
      eapply encoded_valid; [|exact Encoded].
      eapply check_sound; [|exact Checked].
      refine ((fix premises xs (Proof : Forall (issued issuing_author) xs) {struct Proof} :
        Forall valid xs := _) _ Inputs).
      destruct Proof as [|head tail Head Tail].
      + constructor.
      + constructor; [exact (issued_sound issuing_author head Head)|exact (premises tail Tail)].
  Qed.
