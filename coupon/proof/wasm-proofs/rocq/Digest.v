From Stdlib Require Import List.
From FixProof Require Import Handle.
Module Digest (S : STORAGE).
  Module H := Handles S.
  Import H.
  Definition digest t := create_tree
    (map (fun x => HBlobObj (create_blob (from_nat (get_type x)))) (get_tree_raw t)).

  Lemma Forall2_map_types (X : handle -> handle -> Prop) xs ys :
    Forall2 X xs ys ->
    (forall x y, X x y -> not_encode x -> not_encode y -> get_type x = get_type y) ->
    Forall not_encode xs -> Forall not_encode ys -> map get_type xs = map get_type ys.
  Proof.
    intro R; induction R; intros P Nx Ny; simpl; auto.
    inversion Nx; inversion Ny; subst; f_equal; auto.
  Qed.

  (** The shape preservation assumptions of digest_X are isolated from the
      storage argument. This form also works for any later equivalence relation. *)
  Lemma digest_type_X (X : handle -> handle -> Prop)
      (tree_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> Forall2 X (get_tree_raw t1) (get_tree_raw t2))
      (type_preservation : forall x y, X x y -> not_encode x -> not_encode y -> get_type x = get_type y)
      (X_self : forall h, X h h) t1 t2 :
    X (HTreeObj t1) (HTreeObj t2) ->
    Forall not_encode (get_tree_raw t1) -> Forall not_encode (get_tree_raw t2) ->
    X (HTreeObj (digest t1)) (HTreeObj (digest t2)).
  Proof.
    intros R N1 N2.
    pose proof (Forall2_map_types X _ _ (tree_cong t1 t2 R) type_preservation N1 N2) as E.
    unfold digest.
    assert (forall xs, map (fun x => HBlobObj (create_blob (from_nat (get_type x)))) xs =
      map (fun n => HBlobObj (create_blob (from_nat n))) (map get_type xs)) as M.
    { intro xs; rewrite map_map; reflexivity. }
    rewrite !M.
    rewrite E; apply X_self.
  Qed.
  Section Congruence.
    Variable X : handle -> handle -> Prop.
    Hypothesis blob_cong : forall b1 b2, X (HBlobObj b1) (HBlobObj b2) -> get_blob_data b1 = get_blob_data b2.
    Hypothesis tree_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> Forall2 X (get_tree_raw t1) (get_tree_raw t2).
    Hypothesis blob_complete : forall d1 d2, d1 = d2 -> X (HBlobObj (create_blob d1)) (HBlobObj (create_blob d2)).
    Hypothesis tree_complete : forall xs ys, Forall2 X xs ys -> X (HTreeObj (create_tree xs)) (HTreeObj (create_tree ys)).
    Hypothesis X_preserve_tree_ref : forall t h, X (HTreeRef t) h -> exists t', h = HTreeRef t'.
    Hypothesis X_preserve_tree_ref_or_encode_rev : forall t h, X h (HTreeRef t) -> (exists t', h = HTreeRef t') \/ (exists th, h = Encode (Shallow th)).
    Hypothesis X_preserve_blob_ref : forall b h, X (HBlobRef b) h -> exists b', h = HBlobRef b'.
    Hypothesis X_preserve_blob_ref_or_encode_rev : forall b h, X h (HBlobRef b) -> (exists b', h = HBlobRef b') \/ (exists th, h = Encode (Shallow th)).
    Hypothesis X_preserve_tree : forall t h, X (HTreeObj t) h -> exists t', h = HTreeObj t'.
    Hypothesis X_preserve_tree_or_encode_rev : forall t h, X h (HTreeObj t) -> (exists t', h = HTreeObj t') \/ (exists th, h = Encode (Strict th)).
    Hypothesis X_preserve_blob : forall b h, X (HBlobObj b) h -> exists b', h = HBlobObj b'.
    Hypothesis X_preserve_blob_or_encode_rev : forall b h, X h (HBlobObj b) -> (exists b', h = HBlobObj b') \/ (exists th, h = Encode (Strict th)).
    Hypothesis X_preserve_thunk : forall h1 h2, X h1 h2 -> ((exists t1, h1 = Thunk t1) <-> (exists t2, h2 = Thunk t2)).
    Hypothesis X_self : forall h, X h h.

    Lemma X_preserves_type x y : X x y -> not_encode x -> not_encode y -> get_type x = get_type y.
    Proof.
      intros R Nx Ny; destruct x as [d|th|e]; [destruct d as [o|r]| |contradiction].
      - destruct o as [b|t].
        + destruct (X_preserve_blob b y R) as [b' ->]; reflexivity.
        + destruct (X_preserve_tree t y R) as [t' ->]; reflexivity.
      - destruct r as [b|t].
        + destruct (X_preserve_blob_ref b y R) as [b' ->]; reflexivity.
        + destruct (X_preserve_tree_ref t y R) as [t' ->]; reflexivity.
      - destruct (proj1 (X_preserve_thunk _ _ R) (ex_intro _ th eq_refl)) as [th' ->]; reflexivity.
    Qed.

    Lemma digest_X t1 t2 :
      X (HTreeObj t1) (HTreeObj t2) /\
      Forall not_encode (get_tree_raw t1) /\ Forall not_encode (get_tree_raw t2) ->
      X (HTreeObj (digest t1)) (HTreeObj (digest t2)).
    Proof.
      intros [R [N1 N2]]; apply (digest_type_X X tree_cong X_preserves_type X_self t1 t2); assumption.
    Qed.
  End Congruence.
End Digest.
