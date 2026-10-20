(** Defines index and half-open range selection on Blobs and Trees, and
    proves congruence under the supplied relations on handles. *)

From Stdlib Require Import List ZArith Arith Bool Lia.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle.
Import ListNotations.
Module Select (S : STORAGE).
  Module H := Handles S.
  Import H.
  Definition list_slice {A} (l : list A) a b :=
    if (a <=? b) && (b <=? length l)
    then Some (firstn (b-a) (skipn a l)) else None.

  (** A list slice succeeds exactly for valid half-open bounds and returns that range. *)
  Lemma list_slice_spec {A} (l : list A) a b result :
    list_slice l a b = Some result <->
    a <= b /\ b <= length l /\ result = firstn (b-a) (skipn a l).
  Proof.
    unfold list_slice; destruct ((a <=? b) && (b <=? length l)) eqn:Bounds.
    - apply andb_true_iff in Bounds; destruct Bounds as [Start End].
      apply Nat.leb_le in Start; apply Nat.leb_le in End.
      split; [intro Result; inversion Result; auto|intros [_ [_ ->]]; reflexivity].
    - split; [discriminate|intros [Start [End _]]].
      apply Nat.leb_le in Start; apply Nat.leb_le in End.
      rewrite Start, End in Bounds; discriminate.
  Qed.
  Definition tree_slice t x y := omap create_tree (list_slice (get_tree_raw t) x y).
  Definition blob_slice b x y := omap create_blob (list_slice (get_blob_data b) x y).
  Definition range_ref target a b := match target with
    | Data (Object (BlobObj blob)) | Data (Ref (BlobRef blob)) =>
      omap BlobRef (blob_slice blob a b)
    | Data (Object (TreeObj t)) | Data (Ref (TreeRef t)) =>
      omap TreeRef (tree_slice t a b)
    | _ => None
    end.
  Definition select_range target a b := omap (fun ref => Data (Ref ref)) (range_ref target a b).
  Definition select_index target i := match target with
    | Data (Object (TreeObj t)) | Data (Ref (TreeRef t)) =>
      nth_error (get_tree_raw t) i
    | _ => select_range target i (S i)
    end.
  Definition read_index h := match h with
    | Data (Object (BlobObj b)) => to_nat (get_blob_data b)
    | _ => None
    end.
  (** A decoded selection index comes from an eight-byte Blob and fits in an unsigned
      64-bit integer. *)
  Lemma read_index_spec h n : read_index h = Some n ->
    exists b, h = HBlobObj b /\ get_blob_size b = 8 /\
      (Z.of_nat n < 2 ^ 64)%Z.
  Proof.
    destruct h as [[[b|t]|[b|t]]|th|e]; cbn [read_index]; try discriminate.
    intro Decoded; exists b; split; [reflexivity|]; split.
    - apply (decode_u64_width _ _ Decoded).
    - apply (decode_u64_bound _ _ Decoded).
  Qed.

  Definition select_arguments target indices := match indices with
    | [i] => obind (read_index i) (select_index target)
    | [a; b] => obind (read_index a) (fun start =>
        obind (read_index b) (select_range target start))
    | _ => None
    end.
  Definition select t := match get_tree_raw t with
    | target :: indices => select_arguments target indices
    | [] => None
    end.

  (** Taking equally long prefixes preserves a pointwise list relation. *)
  Lemma Forall2_firstn {A B} (X : A -> B -> Prop) xs ys n :
    Forall2 X xs ys -> Forall2 X (firstn n xs) (firstn n ys).
  Proof. intro R; revert n; induction R; intros [|n]; simpl; constructor; auto. Qed.
  (** Dropping equally long prefixes preserves a pointwise list relation. *)
  Lemma Forall2_skipn {A B} (X : A -> B -> Prop) xs ys n :
    Forall2 X xs ys -> Forall2 X (skipn n xs) (skipn n ys).
  Proof. intro R; revert n; induction R; intros [|n]; simpl; auto; constructor; auto. Qed.
  (** Slicing related lists at the same bounds preserves the relation and agrees on
      failure. *)
  Lemma list_slice_X {A B} (X : A -> B -> Prop) xs ys a b :
    Forall2 X xs ys -> rel_opt (Forall2 X) (list_slice xs a b) (list_slice ys a b).
  Proof.
    (* Pointwise related lists have equal lengths, so the public bounds test accepts or
        rejects both ranges together. Once the bounds agree, selection merely retains a
        corresponding segment of the existing relation; it needs no computation of the
        selected elements. *)
    intro R; unfold list_slice; rewrite <- (List.Forall2_length R).
    destruct ((a <=? b) && (b <=? length xs)); simpl; auto.
    apply Forall2_firstn, Forall2_skipn; exact R.
  Qed.
  Section Congruence.
    Variable X : handle -> handle -> Prop.
    Variable Laws : congruence X.
    (** Equal observations of related Blobs give related Blob Refs for the same range. *)
    Lemma blob_slice_X b1 b2 x y :
      X (HBlobObj b1) (HBlobObj b2) ->
      rel_opt (fun r1 r2 => X (Data (Ref r1)) (Data (Ref r2)))
        (omap BlobRef (blob_slice b1 x y)) (omap BlobRef (blob_slice b2 x y)).
    Proof.
      intro R; unfold blob_slice; rewrite (blob_cong X Laws b1 b2 R).
      destruct (list_slice (get_blob_data b2) x y); simpl; auto.
      apply (blob_ref_cong X Laws), (blob_complete X Laws); reflexivity.
    Qed.
    (** Slicing related Trees at the same bounds gives related Tree Refs. *)
    Lemma tree_slice_X t1 t2 x y :
      X (HTreeObj t1) (HTreeObj t2) ->
      rel_opt (fun r1 r2 => X (Data (Ref r1)) (Data (Ref r2)))
        (omap TreeRef (tree_slice t1 x y)) (omap TreeRef (tree_slice t2 x y)).
    Proof.
      intro Related.
      assert (Slices : rel_opt (Forall2 X)
        (list_slice (get_tree_raw t1) x y) (list_slice (get_tree_raw t2) x y))
        by (hauto cases: - use: list_slice_X, (tree_cong X Laws)).
      unfold tree_slice.
      apply (rel_opt_map (fun a b => X (HTreeObj a) (HTreeObj b))); [exact (tree_ref_cong X Laws)|].
      apply (rel_opt_map (Forall2 X)); [exact (tree_complete X Laws false)|exact Slices].
    Qed.
    (** Related non-Encode handles have the same observable handle type. *)
    Lemma X_preserves_type x y : X x y -> not_encode x -> not_encode y -> get_type x = get_type y.
    Proof.
      intros R Nx Ny; destruct x as [d|th|e]; [destruct d as [o|r]| |contradiction].
      - destruct o as [b|t].
        + destruct (preserve_blob X Laws b y R) as [b' ->]; reflexivity.
        + destruct (preserve_tree X Laws t y R) as [t' ->]; reflexivity.
      - destruct r as [b|t].
        + destruct (preserve_blob_ref X Laws b y R) as [b' ->]; reflexivity.
        + destruct (preserve_tree_ref X Laws t y R) as [t' ->]; reflexivity.
      - destruct (proj1 (preserve_thunk X Laws _ _ R) (ex_intro _ th eq_refl)) as [th' ->]; reflexivity.
    Qed.

    (** Related non-Encode handles decode to the same selection index or both fail. *)
    Lemma read_index_X h1 h2 : X h1 h2 -> not_encode h1 -> not_encode h2 ->
      read_index h1 = read_index h2.
    Proof.
      intros Related N1 N2; pose proof (X_preserves_type _ _ Related N1 N2) as Types.
      destruct h1 as [[[b1|t1]|[b1|t1]]|th1|e1],
        h2 as [[[b2|t2]|[b2|t2]]|th2|e2];
        cbn [get_type] in Types; try discriminate; cbn [read_index]; try reflexivity.
      rewrite (blob_cong X Laws _ _ Related); reflexivity.
    Qed.

    (** Range selection on related non-Encode targets preserves the result relation and
        agreement on failure. *)
    Lemma select_range_X h1 h2 a b : X h1 h2 -> not_encode h1 -> not_encode h2 ->
      rel_opt X (select_range h1 a b) (select_range h2 a b).
    Proof.
      intros Related N1 N2; unfold select_range.
      eapply (rel_opt_map (fun r1 r2 => X (Data (Ref r1)) (Data (Ref r2))));
        [intros r1 r2 R; exact R|].
      pose proof (X_preserves_type _ _ Related N1 N2) as Types.
      destruct h1 as [[[b1|t1]|[b1|t1]]|th1|e1],
        h2 as [[[b2|t2]|[b2|t2]]|th2|e2];
        cbn [get_type] in Types; try discriminate; cbn [range_ref]; try exact I.
      - apply blob_slice_X; assumption.
      - apply tree_slice_X; assumption.
      - apply blob_slice_X; try assumption; apply (blob_ref_complete X Laws); exact Related.
      - apply tree_slice_X; try assumption; apply (tree_ref_complete X Laws); exact Related.
    Qed.

    (** Index selection on related non-Encode targets preserves the result relation and
        agreement on failure. *)
    Lemma select_index_X h1 h2 i : X h1 h2 -> not_encode h1 -> not_encode h2 ->
      rel_opt X (select_index h1 i) (select_index h2 i).
    Proof.
      intros Related N1 N2; pose proof (X_preserves_type _ _ Related N1 N2) as Types.
      destruct h1 as [[[b1|t1]|[b1|t1]]|th1|e1],
        h2 as [[[b2|t2]|[b2|t2]]|th2|e2];
        cbn [get_type] in Types; try discriminate; cbn [select_index];
        try solve [apply select_range_X; assumption]; try exact I.
      - apply Forall2_nth_error, (tree_cong X Laws); exact Related.
      - apply Forall2_nth_error, (tree_cong X Laws), (tree_ref_complete X Laws); exact Related.
    Qed.

    (** Related non-Encode targets and index operands give related selection results or
        both fail. *)
    Lemma select_arguments_X h1 h2 xs ys : X h1 h2 -> not_encode h1 -> not_encode h2 ->
      Forall2 X xs ys -> Forall not_encode xs -> Forall not_encode ys ->
      rel_opt X (select_arguments h1 xs) (select_arguments h2 ys).
    Proof.
      intros Related N1 N2 Indices; destruct Indices as [|i1 i2 xs ys Index Tail];
        intros Nx Ny; [exact I|].
      pose proof (read_index_X _ _ Index (Forall_inv Nx) (Forall_inv Ny)) as First.
      destruct Tail as [|j1 j2 xs ys J Tail].
      - cbn [select_arguments]; rewrite First.
        destruct (read_index i2); cbn [obind]; [apply select_index_X; assumption|exact I].
      - destruct Tail as [|k1 k2 xs ys K Tail]; [|exact I].
        pose proof (read_index_X _ _ J
          (Forall_inv (Forall_inv_tail Nx)) (Forall_inv (Forall_inv_tail Ny))) as Second.
        cbn [select_arguments]; rewrite First, Second.
        destruct (read_index i2), (read_index j2); cbn [obind];
          [apply select_range_X; assumption|exact I|exact I|exact I].
    Qed.

    (** Selection preserves the relation between Trees whose immediate operands contain no
        Encodes. *)
    Lemma select_X t1 t2 :
      X (HTreeObj t1) (HTreeObj t2) /\
      Forall not_encode (get_tree_raw t1) /\ Forall not_encode (get_tree_raw t2) ->
      rel_opt X (select t1) (select t2).
    Proof.
      intros [Related [LeftValues RightValues]].
      assert (Children : Forall2 X (get_tree_raw t1) (get_tree_raw t2))
        by (hauto cases: - use: (tree_cong X Laws)).
      unfold select.
      remember (get_tree_raw t1) as xs in *; remember (get_tree_raw t2) as ys in *.
      destruct Children; [exact I|].
      inversion LeftValues; inversion RightValues; subst; eauto using select_arguments_X.
    Qed.
  End Congruence.
End Select.
