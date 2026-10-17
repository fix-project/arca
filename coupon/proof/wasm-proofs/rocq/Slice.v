From Stdlib Require Import List Arith Bool Lia.
From FixProof Require Import Handle Digest.
Import ListNotations.
Module Slice (S : STORAGE).
  Module H := Handles S.
  Module D := Digest S.
  Import H.
  (** Preserve the original strict upper bound, including rejection of b=length
      and acceptance of an empty slice when a=b<length. *)
  Definition list_slice {A} (l : list A) a b :=
    if (a <? length l) && (b <? length l) && (a <=? b)
    then Some (firstn (b-a) (skipn a l)) else None.
  Definition tree_slice t x y := omap create_tree (list_slice (get_tree_raw t) x y).
  Definition blob_slice b x y := omap create_blob (list_slice (get_blob_data b) x y).
  Definition slice t :=
    if get_tree_size t =? 3 then
      match get_tree_data t 1, get_tree_data t 2 with
      | Data (Object (BlobObj b1)), Data (Object (BlobObj b2)) =>
        let x := to_nat (get_blob_data b1) in
        let y := to_nat (get_blob_data b2) in
        match get_tree_data t 0 with
        | Data (Ref (TreeRef t')) => omap TreeRef (tree_slice t' x y)
        | Data (Ref (BlobRef b)) => omap BlobRef (blob_slice b x y)
        | _ => None
        end
      | _, _ => None
      end
    else None.

  Lemma Forall2_length {A B} (X : A -> B -> Prop) xs ys :
    Forall2 X xs ys -> length xs = length ys.
  Proof. induction 1; simpl; congruence. Qed.
  Lemma Forall2_firstn {A B} (X : A -> B -> Prop) xs ys n :
    Forall2 X xs ys -> Forall2 X (firstn n xs) (firstn n ys).
  Proof. intro R; revert n; induction R; intros [|n]; simpl; constructor; auto. Qed.
  Lemma Forall2_skipn {A B} (X : A -> B -> Prop) xs ys n :
    Forall2 X xs ys -> Forall2 X (skipn n xs) (skipn n ys).
  Proof. intro R; revert n; induction R; intros [|n]; simpl; auto; constructor; auto. Qed.
  Lemma list_slice_X {A B} (X : A -> B -> Prop) xs ys a b :
    Forall2 X xs ys -> rel_opt (Forall2 X) (list_slice xs a b) (list_slice ys a b).
  Proof.
    intro R; unfold list_slice; rewrite <- (Forall2_length X xs ys R).
    destruct ((a <? length xs) && (b <? length xs) && (a <=? b)); simpl; auto.
    apply Forall2_firstn, Forall2_skipn; exact R.
  Qed.
  Lemma Forall2_nth (X : handle -> handle -> Prop) xs ys i d1 d2 :
    Forall2 X xs ys -> i < length xs -> X (nth i xs d1) (nth i ys d2).
  Proof.
    intro R; revert i; induction R; intros [|i] B; simpl in *; try lia; auto.
    apply IHR; lia.
  Qed.
  Lemma Forall_nth (X : handle -> Prop) xs i d :
    Forall X xs -> i < length xs -> X (nth i xs d).
  Proof.
    intro R; revert i; induction R; intros [|i] B; simpl in *; try lia; auto.
    apply IHR; lia.
  Qed.
  Lemma blob_slice_X (X : handle -> handle -> Prop)
      (blob_cong : forall b1 b2, X (HBlobObj b1) (HBlobObj b2) -> get_blob_data b1 = get_blob_data b2)
      (blob_complete : forall d1 d2, d1 = d2 -> X (HBlobObj (create_blob d1)) (HBlobObj (create_blob d2)))
      (blob_ref_cong : forall b1 b2, X (HBlobObj b1) (HBlobObj b2) -> X (HBlobRef b1) (HBlobRef b2))
      b1 b2 x y :
    X (HBlobObj b1) (HBlobObj b2) ->
    rel_opt (fun r1 r2 => X (Data (Ref r1)) (Data (Ref r2)))
      (omap BlobRef (blob_slice b1 x y)) (omap BlobRef (blob_slice b2 x y)).
  Proof.
    intro R; unfold blob_slice; rewrite (blob_cong b1 b2 R).
    destruct (list_slice (get_blob_data b2) x y); simpl; auto.
    apply blob_ref_cong, blob_complete; reflexivity.
  Qed.
  Lemma tree_slice_X (X : handle -> handle -> Prop)
      (tree_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> Forall2 X (get_tree_raw t1) (get_tree_raw t2))
      (tree_complete : forall xs ys, Forall2 X xs ys -> X (HTreeObj (create_tree xs)) (HTreeObj (create_tree ys)))
      (tree_ref_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> X (HTreeRef t1) (HTreeRef t2))
      t1 t2 x y :
    X (HTreeObj t1) (HTreeObj t2) ->
    rel_opt (fun r1 r2 => X (Data (Ref r1)) (Data (Ref r2)))
      (omap TreeRef (tree_slice t1 x y)) (omap TreeRef (tree_slice t2 x y)).
  Proof.
    intro R; pose proof (list_slice_X X _ _ x y (tree_cong t1 t2 R)) as L.
    unfold tree_slice; destruct (list_slice (get_tree_raw t1) x y),
      (list_slice (get_tree_raw t2) x y); simpl in *; try contradiction; auto.
    apply tree_ref_cong, tree_complete; exact L.
  Qed.
  Section Congruence.
    Variable X : handle -> handle -> Prop.
    Hypothesis blob_cong : forall b1 b2, X (HBlobObj b1) (HBlobObj b2) -> get_blob_data b1 = get_blob_data b2.
    Hypothesis tree_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> Forall2 X (get_tree_raw t1) (get_tree_raw t2).
    Hypothesis blob_complete : forall d1 d2, d1 = d2 -> X (HBlobObj (create_blob d1)) (HBlobObj (create_blob d2)).
    Hypothesis tree_complete : forall xs ys, Forall2 X xs ys -> X (HTreeObj (create_tree xs)) (HTreeObj (create_tree ys)).
    Hypothesis blob_ref_cong : forall b1 b2, X (HBlobObj b1) (HBlobObj b2) -> X (HBlobRef b1) (HBlobRef b2).
    Hypothesis tree_ref_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> X (HTreeRef t1) (HTreeRef t2).
    Hypothesis blob_ref_complete : forall b1 b2, X (HBlobRef b1) (HBlobRef b2) -> X (HBlobObj b1) (HBlobObj b2).
    Hypothesis tree_ref_complete : forall t1 t2, X (HTreeRef t1) (HTreeRef t2) -> X (HTreeObj t1) (HTreeObj t2).
    Hypothesis X_preserve_tree_ref : forall t h, X (HTreeRef t) h -> exists t', h = HTreeRef t'.
    Hypothesis X_preserve_blob_ref : forall b h, X (HBlobRef b) h -> exists b', h = HBlobRef b'.
    Hypothesis X_preserve_tree : forall t h, X (HTreeObj t) h -> exists t', h = HTreeObj t'.
    Hypothesis X_preserve_blob : forall b h, X (HBlobObj b) h -> exists b', h = HBlobObj b'.
    Hypothesis X_preserve_thunk : forall h1 h2, X h1 h2 -> ((exists t1, h1 = Thunk t1) <-> (exists t2, h2 = Thunk t2)).

    Lemma slice_X t1 t2 :
      X (HTreeObj t1) (HTreeObj t2) /\
      Forall not_encode (get_tree_raw t1) /\ Forall not_encode (get_tree_raw t2) ->
      rel_opt (fun r1 r2 => X (Data (Ref r1)) (Data (Ref r2))) (slice t1) (slice t2).
    Proof.
      intros [R [N1 N2]].
      pose proof (tree_cong t1 t2 R) as Raw.
      assert (get_tree_size t1 = get_tree_size t2) as L by
        (apply Forall2_length with (X:=X); exact Raw).
      unfold slice; rewrite <- L.
      destruct (get_tree_size t1 =? 3) eqn:Size; [|exact I].
      apply Nat.eqb_eq in Size.
      assert (forall i, i < 3 -> X (get_tree_data t1 i) (get_tree_data t2 i)) as Rel.
      { intros i B; unfold get_tree_data; eapply Forall2_nth; [exact Raw|].
        change (i < get_tree_size t1); lia. }
      assert (forall i, i < 3 -> get_type (get_tree_data t1 i) = get_type (get_tree_data t2 i)) as Ty.
      { intros i B; eapply D.X_preserves_type;
          [exact X_preserve_tree_ref|exact X_preserve_blob_ref|exact X_preserve_tree|exact X_preserve_blob|exact X_preserve_thunk|apply Rel; exact B| |].
        - unfold get_tree_data; eapply Forall_nth; [exact N1|].
          change (i < get_tree_size t1); lia.
        - unfold get_tree_data; eapply Forall_nth; [exact N2|].
          change (i < get_tree_size t2); lia. }
      pose proof (Rel 0 ltac:(lia)) as R0; pose proof (Ty 0 ltac:(lia)) as T0.
      pose proof (Rel 1 ltac:(lia)) as R1; pose proof (Ty 1 ltac:(lia)) as T1.
      pose proof (Rel 2 ltac:(lia)) as R2; pose proof (Ty 2 ltac:(lia)) as T2.
      destruct (get_tree_data t1 1) as [[[b1|u1]|[b1|u1]]|th1|e1];
        destruct (get_tree_data t2 1) as [[[b1'|u1']|[b1'|u1']]|th1'|e1'];
        simpl in T1; try discriminate; try exact I.
      destruct (get_tree_data t1 2) as [[[b2|u2]|[b2|u2]]|th2|e2];
        destruct (get_tree_data t2 2) as [[[b2'|u2']|[b2'|u2']]|th2'|e2'];
        simpl in T2; try discriminate; try exact I.
      rewrite (blob_cong b1 b1' R1), (blob_cong b2 b2' R2).
      destruct (get_tree_data t1 0) as [[[b0|u0]|[b0|u0]]|th0|e0];
        destruct (get_tree_data t2 0) as [[[b0'|u0']|[b0'|u0']]|th0'|e0'];
        simpl in T0; try discriminate; try exact I.
      - apply blob_slice_X; try assumption. apply blob_ref_complete; exact R0.
      - apply tree_slice_X; try assumption. apply tree_ref_complete; exact R0.
    Qed.
  End Congruence.
End Slice.
