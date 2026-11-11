  Lemma decode_author supplied kind issuing_author lhs rhs :
    decode supplied = Some (kind, issuing_author, lhs, rhs) ->
    author supplied = Some issuing_author.
  Proof.
    intro Decoded; pose proof (decode_authenticates _ _ _ _ _ Decoded)
      as [bundle [tag [claim [code [n [Supplied [Bundle [Tag [Tagged [Claim _]]]]]]]]]].
    subst supplied; unfold author; cbn [HTreeObj]; rewrite Bundle.
    cbn [HTreeObj]; rewrite Tagged, Tag, Claim.
    rewrite API.handle_eqb_refl; reflexivity.
  Qed.

  (** Successful decoding determines the Tag as its author followed by the assertion
      Ref. *)
  Lemma decode_tag_shape tag claim kind author lhs rhs :
    decode (HTreeObj (create_tree [HTreeObj tag; HTreeObj claim])) =
      Some (kind, author, lhs, rhs) ->
    tag = create_tree_tagged true [author; HTreeRef claim].
  Proof.
    intro Decoded.
    hauto cases: - db: coupon_claims
      use: (decode_authenticates _ _ _ _ _ Decoded), (create_tree_get_tree_raw tag)
      unfold!: HTreeObj.
  Qed.

  (** Replacing a decoded Tag's author preserves its authenticated claim and endpoints. *)
  Lemma decode_replace_author tag claim kind author new_author lhs rhs :
    decode (HTreeObj (create_tree [HTreeObj tag; HTreeObj claim])) =
      Some (kind, author, lhs, rhs) ->
    decode (HTreeObj (create_tree
      [HTreeObj (create_tree_tagged true [new_author; HTreeRef claim]); HTreeObj claim])) =
      Some (kind, new_author, lhs, rhs).
  Proof.
    (* Claim authentication depends on the Ref of the assertion Object, not on the author
        representation. Changing that author can alter trust requirements while
        leaving the asserted operation and endpoints fixed, which is the separation used
        by authority preservation. *)
    intro Decoded.
    pose proof (decode_authenticates _ _ _ _ _ Decoded) as Authenticated.
    hauto cases: - db: coupon_claims unfold!: decode, HTreeObj, obind, omap.
  Qed.
