  Corollary coupon_blob_same_data_proof b1 b2 : coupon_equiv (HBlobObj b1) (HBlobObj b2) -> get_blob_data b1 = get_blob_data b2.
  Proof.
    intro CouponEq; destruct (equiv_blob_same_data _ _ (coupon_equiv_sound _ _ CouponEq) b1 Logic.eq_refl) as
      [[b [E' Same]]|[th [b [E' Rest]]]]; [inversion E'; subst; exact Same|discriminate].
  Qed.
