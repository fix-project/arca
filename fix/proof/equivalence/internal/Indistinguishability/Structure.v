  (** Blob Objects are indistinguishable exactly when their bytes are identical. *)
  Lemma indistinguishable_blobs b1 b2 :
    indistinguishable (HBlobObj b1) (HBlobObj b2) <-> get_blob_data b1 = get_blob_data b2.
  Proof.
    split.
    - intros [_ Observed]; inversion Observed; assumption.
    - intro Contents; split; [apply R_to_equiv, RBlob|constructor]; exact Contents.
  Qed.

  (** Constructing strict Encodes preserves indistinguishability of bare Thunks. *)
  Lemma indistinguishable_encode_strict th1 th2 :
    indistinguishable (Thunk th1) (Thunk th2) ->
    indistinguishable (Encode (Strict th1)) (Encode (Strict th2)).
  Proof.
    intros [Equivalent Observed]; split.
    - apply equiv_thunk_to_strict_encode; exact Equivalent.
    - apply observations_encode_strict; exact Observed.
  Qed.

  (** Constructing shallow Encodes preserves indistinguishability of bare Thunks. *)
  Lemma indistinguishable_encode_shallow th1 th2 :
    indistinguishable (Thunk th1) (Thunk th2) ->
    indistinguishable (Encode (Shallow th1)) (Encode (Shallow th2)).
  Proof.
    intros [Equivalent Observed]; split.
    - apply equiv_thunk_to_shallow_encode; exact Equivalent.
    - apply observations_encode_shallow; exact Observed.
  Qed.
  (** The recursive observation relation on children is exactly the pointwise observation
      relation. *)
  Lemma observed_children_Forall2 xs ys :
    observed_children xs ys <-> Forall2 same_observations xs ys.
  Proof.
    split.
    - revert ys; induction xs; intros ys Related; inversion Related; subst;
        constructor; auto.
    - induction 1; constructor; assumption.
  Qed.

  (** Pointwise indistinguishable child lists agree on whether every child is Comparable. *)
  Lemma indistinguishable_children_comparable xs ys :
    Forall2 indistinguishable xs ys ->
    forallb is_comparable xs = forallb is_comparable ys.
  Proof.
    induction 1; simpl; [reflexivity|].
    rewrite (indistinguishable_comparable _ _ H), IHForall2; reflexivity.
  Qed.

  (** Tree Objects are indistinguishable exactly when their Tag markers agree and their
      children are pointwise indistinguishable. *)
  Theorem indistinguishable_trees t1 t2 :
    indistinguishable (HTreeObj t1) (HTreeObj t2) <->
    tree_tag t1 = tree_tag t2 /\
    Forall2 indistinguishable (get_tree_raw t1) (get_tree_raw t2).
  Proof.
    (* The two parts of indistinguishability must use the same child pairing. Child
        equivalence builds the computational relation; child observation agreement builds
        the observation relation. Their Comparability observations also determine the
        enclosing Tree's Comparability metadata. *)
    split.
    - intros [Equivalent Observed].
      assert (Structure : tree_tag t1 = tree_tag t2 /\
        Forall2 computational_equiv (get_tree_raw t1) (get_tree_raw t2))
        by (hauto cases: - use: equiv_tree_iff).
      assert (Observations : Forall2 same_observations (get_tree_raw t1) (get_tree_raw t2))
        by (sauto inv: same_observations ctrs: - cases: - use: observed_children_Forall2).
      hauto cases: - use: A.Forall2_and.
    - intros [Tags Children].
      assert (Equivalent : Forall2 computational_equiv (get_tree_raw t1) (get_tree_raw t2))
        by (hauto cases: - use: (A.Forall2_mono indistinguishable computational_equiv)
          unfold!: indistinguishable).
      assert (Observed : observed_children (get_tree_raw t1) (get_tree_raw t2))
        by (hauto cases: - use: observed_children_Forall2,
          (A.Forall2_mono indistinguishable same_observations) unfold!: indistinguishable).
      assert (Comparable : tree_comparable t1 = tree_comparable t2)
        by (hauto cases: - use: indistinguishable_children_comparable, tree_comparable_contents).
      hauto ctrs: same_observations cases: - use: equiv_tree_iff.
  Qed.

  (** For Comparable handles, computational equivalence already supplies agreement of
      observations. This is the key fact connecting the logical relation to handle
      comparison. *)
  Lemma comparable_equivalent_observations : forall left right,
    is_comparable left = true -> is_comparable right = true ->
    computational_equiv left right -> same_observations left right.
  Proof.
    (* Comparable endpoints have Data forms, so their equivalence cannot hide an exposed
        computation at an endpoint. Remove any intermediate Encodes from the equivalence
        path: Blob contents and Tree structure are then invariant. Related Refs reduce the
        same argument to their underlying Objects, and the recursive observation relation
        accepts these repeated comparisons. *)
    cofix IH; intros left right Left Right Related.
    destruct left as [[[b|t]|[b|t]]|th|[th|th]];
      cbn [is_comparable] in Left; try discriminate.
    (* A Blob endpoint forces the other non-Encode endpoint to be a Blob of equal
        content. There is consequently no byte observation left to distinguish. *)
    - assert (MatchingBlob : exists other,
        right = HBlobObj other /\ get_blob_data b = get_blob_data other).
      { destruct (equiv_blob_same_data _ _ Related b Logic.eq_refl) as
          [Blob|[th [other [-> _]]]]; [exact Blob|discriminate]. }
      destruct MatchingBlob as [other [-> SameContents]].
      exact (ObserveBlob _ _ SameContents).
    (* A Tree endpoint forces the other non-Encode endpoint to supply the same kind of
        structure. This permits recursive comparison of corresponding children. *)
    - assert (MatchingTree : exists other, right = HTreeObj other).
      { destruct (equiv_preserve_tree_or_encode _ _ Related t Logic.eq_refl) as
          [Tree|[th [other [-> _]]]]; [exact Tree|discriminate]. }
      destruct MatchingTree as [other ->].
      change (tree_comparable other = true) in Right.
      pose proof (equiv_tree_tag _ _ Related) as SameTag.
      pose proof (equiv_tree_children _ _ Related) as ChildrenEquivalent.
      assert (SameComparability : tree_comparable t = tree_comparable other) by congruence.
      constructor; [exact SameTag|exact SameComparability|].
      rewrite tree_comparable_contents, forallb_forall in Left, Right.
      revert Left Right ChildrenEquivalent; generalize (get_tree_raw t), (get_tree_raw other).
      cofix Entries; intros xs ys Left Right ChildrenEquivalent.
      destruct ChildrenEquivalent; constructor.
      (* The metadata condition descends to this child, so the recursive observation
          argument retains the premise it needs. *)
      + assert (HeadComparable : is_comparable x = true) by (apply Left; left; reflexivity).
        (* The same premise is needed on the other child; computational equivalence alone
            would not guarantee Comparability. *)
        assert (OtherHeadComparable : is_comparable y = true) by (apply Right; left; reflexivity).
        exact (IH x y HeadComparable OtherHeadComparable H).
      + apply Entries; [intros child Member; apply Left; right; exact Member|
          intros child Member; apply Right; right; exact Member|assumption].
    - assert (MatchingReference : exists other, right = HBlobRef other).
      { destruct (equiv_preserve_blob_ref_or_encode _ _ Related b Logic.eq_refl) as
          [Reference|[th [other [-> _]]]]; [exact Reference|discriminate]. }
      destruct MatchingReference as [other ->].
      pose proof (equiv_ref_to_blob _ _ Related) as ObjectsEquivalent.
      assert (SameContents : get_blob_data b = get_blob_data other).
      { destruct (equiv_blob_same_data _ _ ObjectsEquivalent b Logic.eq_refl) as
          [[other' [Same Contents]]|[th [other' [Impossible _]]]];
          [inversion Same; subst; exact Contents|discriminate]. }
      exact (ObserveBlobRef _ _ SameContents).
    - assert (MatchingReference : exists other, right = HTreeRef other).
      { destruct (equiv_preserve_tree_ref_or_encode _ _ Related t Logic.eq_refl) as
          [Reference|[th [other [-> _]]]]; [exact Reference|discriminate]. }
      destruct MatchingReference as [other ->].
      pose proof (equiv_ref_to_tree _ _ Related) as ObjectsEquivalent.
      apply ObserveTreeRef; exact (IH (HTreeObj t) (HTreeObj other) Left Right ObjectsEquivalent).
  Qed.
