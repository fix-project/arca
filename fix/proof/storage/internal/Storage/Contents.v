  (** An existing Tree is Comparable exactly when every child is Comparable. *)
  Lemma tree_comparable_contents t :
    tree_comparable t = forallb is_comparable (get_tree_raw t).
  Proof.
    rewrite <- (tree_comparable_create_tree_tagged (tree_tag t) (get_tree_raw t)).
    rewrite create_tree_get_tree_raw; reflexivity.
  Qed.
  (** An existing Tree is fully evaluated exactly when every child is fully evaluated. *)
  Lemma tree_value_contents t :
    tree_value t = forallb is_value (get_tree_raw t).
  Proof.
    rewrite <- (tree_value_create_tree_tagged (tree_tag t) (get_tree_raw t)).
    rewrite create_tree_get_tree_raw; reflexivity.
  Qed.
  (** Reading an ordinary newly created Tree recovers its input children. *)
  Lemma get_tree_raw_create_tree xs : get_tree_raw (create_tree xs) = xs.
  Proof. apply get_tree_raw_create_tree_tagged. Qed.
  (** Ordinary Tree construction does not set the Tag marker. *)
  Lemma tree_tag_create_tree xs : tree_tag (create_tree xs) = false.
  Proof. apply tree_tag_create_tree_tagged. Qed.
  (** An unsigned 64-bit integer survives encoding and decoding unchanged. *)
  Lemma to_nat_from_nat n : (Z.of_nat n < 2 ^ 64)%Z ->
    to_nat (from_nat n) = Some n.
  Proof. apply decode_encode_u64. Qed.
