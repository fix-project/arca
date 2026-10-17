From Stdlib Require Import List.
From FixProof Require Import Handle ApplyTree EvaluationProperties.

Module Equivalence (S : STORAGE) (P : PROGRAM S).
  Module EP := EvaluationProperties S P.
  Import EP EP.E EP.E.H.
  Definition squash h :=
    match h with Encode (Strict th) | Encode (Shallow th) => Thunk th | _ => h end.

  CoInductive R : handle -> handle -> Prop :=
  | RBlob : forall b1 b2, get_blob_data b1 = get_blob_data b2 -> R (HBlobObj b1) (HBlobObj b2)
  | RBlobRef : forall b1 b2, R (HBlobObj b1) (HBlobObj b2) -> R (HBlobRef b1) (HBlobRef b2)
  | RTreeNodes : forall t1 t2, R_handles (get_tree_raw t1) (get_tree_raw t2) -> R (HTreeObj t1) (HTreeObj t2)
  | RTreeRef : forall t1 t2, R (HTreeObj t1) (HTreeObj t2) -> R (HTreeRef t1) (HTreeRef t2)
  | RThunkNone : forall t1 t2, think t1 = None -> think t2 = None -> R (Thunk t1) (Thunk t2)
  | RThunkSomeResData : forall t1 t2 d1 d2,
      think t1 = Some (Data d1) -> think t2 = Some (Data d2) ->
      R (lift (Data d1)) (lift (Data d2)) -> R (Thunk t1) (Thunk t2)
  | RThunkSomeResNotData : forall t1 t2 h1 h2,
      think t1 = Some h1 -> think t2 = Some h2 ->
      ~(exists d, h1 = Data d) -> ~(exists d, h2 = Data d) ->
      R (squash h1) (squash h2) -> R (Thunk t1) (Thunk t2)
  | RThunkSomeResEncodeData : forall t1 t2 e1 d2,
      think t1 = Some (Encode e1) -> think t2 = Some (Data d2) ->
      R (Encode (Strict (encode_to_thunk e1))) (lift (Data d2)) -> R (Thunk t1) (Thunk t2)
  | RThunkSomeResEncodeEncode : forall t1 t2 e1 e2,
      think t1 = Some (Encode e1) -> think t2 = Some (Encode e2) ->
      R (Encode (Strict (encode_to_thunk e1))) (Encode (Strict (encode_to_thunk e2))) -> R (Thunk t1) (Thunk t2)
  | RThinkSingleStepThunk : forall t1 t2, think t1 = Some (Thunk t2) -> R (Thunk t1) (Thunk t2)
  | RThunkSingleStepEncodeShallow : forall t1 t2, think t1 = Some (Encode (Shallow t2)) -> R (Thunk t1) (Thunk t2)
  | RThunkSingleStepEncodeStrict : forall t1 t2, think t1 = Some (Encode (Strict t2)) -> R (Thunk t1) (Thunk t2)
  | REvalStrictNone : forall t1 t2, execute (Strict t1) = None -> execute (Strict t2) = None ->
      R (Encode (Strict t1)) (Encode (Strict t2))
  | REvalShallowNone : forall t1 t2, execute (Shallow t1) = None -> execute (Shallow t2) = None ->
      R (Encode (Shallow t1)) (Encode (Shallow t2))
  | REvalSomeRes : forall e1 e2 r1 r2, execute e1 = Some r1 -> execute e2 = Some r2 -> R r1 r2 -> R (Encode e1) (Encode e2)
  | REvalStep : forall e r, execute e = Some r -> R (Encode e) r
  | RSelf : forall h, R h h
  with R_handles : list handle -> list handle -> Prop :=
  | RNil : R_handles nil nil
  | RCons : forall h1 h2 xs ys, R h1 h2 -> R_handles xs ys -> R_handles (h1 :: xs) (h2 :: ys).

  (** Finite handle lists make the mutual coinductive lifting equivalent to
      Forall2 R. This exposes guarded recursive calls to Rocq while retaining
      the original tree rule through RTree. *)
  Lemma R_handles_Forall2 xs ys : R_handles xs ys -> Forall2 R xs ys.
  Proof.
    revert ys; induction xs; intros ys Related; inversion Related; subst; constructor; auto.
  Qed.
  Lemma Forall2_R_handles xs ys : Forall2 R xs ys -> R_handles xs ys.
  Proof. induction 1; constructor; auto. Qed.
  Lemma RTree t1 t2 : Forall2 R (get_tree_raw t1) (get_tree_raw t2) -> R (HTreeObj t1) (HTreeObj t2).
  Proof. intro Related; apply RTreeNodes, Forall2_R_handles; exact Related. Qed.

  CoInductive R' : handle -> handle -> Prop :=
  | R'_from_R : forall h1 h2, R h1 h2 -> R' h1 h2
  | R'_Tree : forall t1 t2,
      Forall2 (fun x y => R' x y \/ R x y) (get_tree_raw t1) (get_tree_raw t2) -> R' (HTreeObj t1) (HTreeObj t2)
  | R'_TreeRef : forall t1 t2, R' (HTreeObj t1) (HTreeObj t2) -> R' (HTreeRef t1) (HTreeRef t2)
  | R'_tree_to_application_thunk : forall t1 t2, R' (HTreeObj t1) (HTreeObj t2) -> R' (Thunk (Application t1)) (Thunk (Application t2))
  | R'_tree_to_selection_thunk : forall t1 t2, R' (HTreeObj t1) (HTreeObj t2) -> R' (Thunk (Selection t1)) (Thunk (Selection t2))
  | R'_tree_to_digestion_thunk : forall t1 t2, R' (HTreeObj t1) (HTreeObj t2) -> R' (Thunk (Digestion t1)) (Thunk (Digestion t2))
  | R'_data_to_identification_thunk : forall d1 d2, R' (Data d1) (Data d2) -> R' (Thunk (Identification d1)) (Thunk (Identification d2))
  | R'_thunk_data : forall t1 t2 d1 d2,
      think t1 = Some (Data d1) -> think t2 = Some (Data d2) ->
      R' (lift (Data d1)) (lift (Data d2)) -> R' (Thunk t1) (Thunk t2)
  | R'_thunk_not_data : forall t1 t2 h1 h2,
      think t1 = Some h1 -> think t2 = Some h2 ->
      ~(exists d, h1 = Data d) -> ~(exists d, h2 = Data d) ->
      R' (squash h1) (squash h2) -> R' (Thunk t1) (Thunk t2)
  | R'_thunk_encode_data : forall t1 t2 e1 d2,
      think t1 = Some (Encode e1) -> think t2 = Some (Data d2) ->
      R' (Encode (Strict (encode_to_thunk e1))) (lift (Data d2)) -> R' (Thunk t1) (Thunk t2)
  | R'_thunk_encode_encode : forall t1 t2 e1 e2,
      think t1 = Some (Encode e1) -> think t2 = Some (Encode e2) ->
      R' (Encode (Strict (encode_to_thunk e1))) (Encode (Strict (encode_to_thunk e2))) -> R' (Thunk t1) (Thunk t2)
  | R'_thunk_to_encode_strict : forall t1 t2, R' (Thunk t1) (Thunk t2) -> R' (Encode (Strict t1)) (Encode (Strict t2))
  | R'_thunk_to_encode_shallow : forall t1 t2, R' (Thunk t1) (Thunk t2) -> R' (Encode (Shallow t1)) (Encode (Shallow t2))
  | R'_encode_some_res : forall e1 e2 r1 r2, execute e1 = Some r1 -> execute e2 = Some r2 -> R' r1 r2 -> R' (Encode e1) (Encode e2).

  Lemma R_to_R' x y : R x y -> R' x y.
  Proof. apply R'_from_R. Qed.
  Lemma R'orR_to_R' x y : R' x y \/ R x y -> R' x y.
  Proof. intros [H|H]; [exact H|apply R_to_R'; exact H]. Qed.
  Lemma list_all2_R'_to_R
      (IH : forall x y, R' x y -> R x y) xs ys :
    Forall2 (fun x y => R' x y \/ R x y) xs ys -> Forall2 R xs ys.
  Proof. induction 1; constructor; auto; destruct H; auto. Qed.
  Lemma list_all2_R'orR_to_R' xs ys :
    Forall2 (fun x y => R' x y \/ R x y) xs ys -> Forall2 R' xs ys.
  Proof. induction 1; constructor; auto using R'orR_to_R'. Qed.
  Lemma list_all2_R_to_R' xs ys : Forall2 R xs ys -> Forall2 R' xs ys.
  Proof. induction 1; constructor; auto using R_to_R'. Qed.
  Lemma list_all2_R_imp_R'orR xs ys : Forall2 R xs ys -> Forall2 (fun x y => R' x y \/ R x y) xs ys.
  Proof. induction 1; constructor; auto. Qed.
  Lemma list_all2_R'_imp_R'orR xs ys : Forall2 R' xs ys -> Forall2 (fun x y => R' x y \/ R x y) xs ys.
  Proof. induction 1; constructor; auto. Qed.
  Lemma R'_refl h : R' h h.
  Proof. apply R_to_R', RSelf. Qed.
  Lemma list_all2_R'_self xs : Forall2 R' xs xs.
  Proof. induction xs; constructor; auto using R'_refl. Qed.
  Lemma blob_cong_R b1 b2 : R (HBlobObj b1) (HBlobObj b2) -> get_blob_data b1 = get_blob_data b2.
  Proof. intro H; inversion H; subst; auto. Qed.
  Lemma blob_cong_R' b1 b2 : R' (HBlobObj b1) (HBlobObj b2) -> get_blob_data b1 = get_blob_data b2.
  Proof. intro H; inversion H; subst; eauto using blob_cong_R. Qed.
  Lemma tree_cong_R t1 t2 : R (HTreeObj t1) (HTreeObj t2) -> Forall2 R (get_tree_raw t1) (get_tree_raw t2).
  Proof. intro H; inversion H; subst; auto using R_handles_Forall2. induction (get_tree_raw t2); constructor; auto using RSelf. Qed.
  Lemma tree_cong_R' t1 t2 : R' (HTreeObj t1) (HTreeObj t2) -> Forall2 R' (get_tree_raw t1) (get_tree_raw t2).
  Proof. intro H; inversion H; subst; eauto using tree_cong_R, list_all2_R_to_R', list_all2_R'orR_to_R'. Qed.
  Lemma blob_complete_R d1 d2 : d1 = d2 -> R (HBlobObj (create_blob d1)) (HBlobObj (create_blob d2)).
  Proof. intros ->; apply RSelf. Qed.
  Lemma tree_complete_R xs ys : Forall2 R xs ys -> R (HTreeObj (create_tree xs)) (HTreeObj (create_tree ys)).
  Proof. intro H; apply RTree; rewrite !get_tree_raw_create_tree; exact H. Qed.
  Lemma tree_complete_R' xs ys : Forall2 R' xs ys -> R' (HTreeObj (create_tree xs)) (HTreeObj (create_tree ys)).
  Proof. intro H; apply R'_Tree; rewrite !get_tree_raw_create_tree; apply list_all2_R'_imp_R'orR; exact H. Qed.
  Lemma blob_complete_R' d1 d2 : d1 = d2 -> R' (HBlobObj (create_blob d1)) (HBlobObj (create_blob d2)).
  Proof. intros ->; apply R'_refl. Qed.
  Lemma blob_ref_cong_R b1 b2 : R (HBlobObj b1) (HBlobObj b2) -> R (HBlobRef b1) (HBlobRef b2).
  Proof. apply RBlobRef. Qed.
  Lemma blob_ref_cong_R' b1 b2 : R' (HBlobObj b1) (HBlobObj b2) -> R' (HBlobRef b1) (HBlobRef b2).
  Proof. intro H; apply R_to_R', RBlobRef, RBlob; apply blob_cong_R'; exact H. Qed.
  Lemma tree_ref_cong_R t1 t2 : R (HTreeObj t1) (HTreeObj t2) -> R (HTreeRef t1) (HTreeRef t2).
  Proof. apply RTreeRef. Qed.
  Lemma tree_ref_cong_R' t1 t2 : R' (HTreeObj t1) (HTreeObj t2) -> R' (HTreeRef t1) (HTreeRef t2).
  Proof. apply R'_TreeRef. Qed.
  Lemma blob_ref_complete_R b1 b2 : R (HBlobRef b1) (HBlobRef b2) -> R (HBlobObj b1) (HBlobObj b2).
  Proof. intro H; inversion H; subst; auto using RSelf. Qed.
  Lemma blob_ref_complete_R' b1 b2 : R' (HBlobRef b1) (HBlobRef b2) -> R' (HBlobObj b1) (HBlobObj b2).
  Proof. intro H; inversion H; subst; eauto using blob_ref_complete_R, R_to_R'. Qed.
  Lemma tree_ref_complete_R t1 t2 : R (HTreeRef t1) (HTreeRef t2) -> R (HTreeObj t1) (HTreeObj t2).
  Proof. intro H; inversion H; subst; auto using RSelf. Qed.
  Lemma tree_ref_complete_R' t1 t2 : R' (HTreeRef t1) (HTreeRef t2) -> R' (HTreeObj t1) (HTreeObj t2).
  Proof. intro H; inversion H; subst; eauto using tree_ref_complete_R, R_to_R'. Qed.
  Lemma strict_encode_cong_R' t1 t2 : R' (Thunk t1) (Thunk t2) -> R' (Encode (Strict t1)) (Encode (Strict t2)).
  Proof. apply R'_thunk_to_encode_strict. Qed.
  Lemma shallow_encode_cong_R' t1 t2 : R' (Thunk t1) (Thunk t2) -> R' (Encode (Shallow t1)) (Encode (Shallow t2)).
  Proof. apply R'_thunk_to_encode_shallow. Qed.
  Lemma application_thunk_cong_R' t1 t2 : R' (HTreeObj t1) (HTreeObj t2) -> R' (Thunk (Application t1)) (Thunk (Application t2)).
  Proof. apply R'_tree_to_application_thunk. Qed.
  Lemma selection_thunk_cong_R' t1 t2 : R' (HTreeObj t1) (HTreeObj t2) -> R' (Thunk (Selection t1)) (Thunk (Selection t2)).
  Proof. apply R'_tree_to_selection_thunk. Qed.
  Lemma digestion_thunk_cong_R' t1 t2 : R' (HTreeObj t1) (HTreeObj t2) -> R' (Thunk (Digestion t1)) (Thunk (Digestion t2)).
  Proof. apply R'_tree_to_digestion_thunk. Qed.
  Lemma identification_thunk_cong_R' d1 d2 : R' (Data d1) (Data d2) -> R' (Thunk (Identification d1)) (Thunk (Identification d2)).
  Proof. apply R'_data_to_identification_thunk. Qed.

  Lemma execute_data e h : execute e = Some h -> exists d, h = Data d.
  Proof.
    rewrite execute_hs; destruct e; cbn; intro Exec; apply omap_some in Exec;
      destruct Exec as [v [Force ->]]; destruct (force_data _ _ Force) as [d ->];
      [exists (lift_data d)|exists (lower_data d)]; reflexivity.
  Qed.
  Lemma execute_strict_to_obj th h : execute (Strict th) = Some h -> exists o, h = Data (Object o).
  Proof.
    rewrite execute_hs; cbn; intro Exec; apply omap_some in Exec;
      destruct Exec as [v [Force ->]]; destruct (force_data _ _ Force) as [[[b|t]|[b|t]] ->];
      cbn [lift lift_data]; eauto.
  Qed.
  Lemma execute_shallow_to_ref th h : execute (Shallow th) = Some h -> exists r, h = Data (Ref r).
  Proof.
    rewrite execute_hs; cbn; intro Exec; apply omap_some in Exec;
      destruct Exec as [v [Force ->]]; destruct (force_data _ _ Force) as [[[b|t]|[b|t]] ->];
      cbn [lower lower_data]; eauto.
  Qed.
  Lemma R_preserve_thunk h1 h2 : R h1 h2 ->
    ((exists t, h1 = Thunk t) <-> (exists t, h2 = Thunk t)).
  Proof.
    intro H; inversion H; subst; try solve [split; intros [t E]; discriminate];
      try solve [split; eauto].
    match goal with E : execute _ = Some _ |- _ => destruct (execute_data _ _ E) as [d ->] end.
    split; intros [t E]; discriminate.
  Qed.
  Lemma R'_preserve_thunk h1 h2 : R' h1 h2 ->
    ((exists t, h1 = Thunk t) <-> (exists t, h2 = Thunk t)).
  Proof.
    intro H; inversion H; subst; try solve [apply R_preserve_thunk; assumption];
      try solve [split; intros [t E]; discriminate]; split; eauto.
  Qed.
  Lemma R_preserve_blob b h : R (HBlobObj b) h -> exists b', h = HBlobObj b'.
  Proof. intro H; inversion H; subst; eauto. Qed.
  Lemma R'_preserve_blob b h : R' (HBlobObj b) h -> exists b', h = HBlobObj b'.
  Proof. intro H; inversion H; subst; eauto using R_preserve_blob. Qed.
  Lemma R_preserve_tree t h : R (HTreeObj t) h -> exists t', h = HTreeObj t'.
  Proof. intro H; inversion H; subst; eauto. Qed.
  Lemma R'_preserve_tree t h : R' (HTreeObj t) h -> exists t', h = HTreeObj t'.
  Proof. intro H; inversion H; subst; eauto using R_preserve_tree. Qed.
  Lemma R_preserve_blob_ref b h : R (HBlobRef b) h -> exists b', h = HBlobRef b'.
  Proof. intro H; inversion H; subst; eauto. Qed.
  Lemma R'_preserve_blob_ref b h : R' (HBlobRef b) h -> exists b', h = HBlobRef b'.
  Proof. intro H; inversion H; subst; eauto using R_preserve_blob_ref. Qed.
  Lemma R_preserve_tree_ref t h : R (HTreeRef t) h -> exists t', h = HTreeRef t'.
  Proof. intro H; inversion H; subst; eauto. Qed.
  Lemma R'_preserve_tree_ref t h : R' (HTreeRef t) h -> exists t', h = HTreeRef t'.
  Proof. intro H; inversion H; subst; eauto using R_preserve_tree_ref. Qed.
  Lemma execute_to_obj_strict e o : execute e = Some (Data (Object o)) -> exists th, e = Strict th.
  Proof.
    destruct e as [th|th]; intro Exec; [eauto|].
    destruct (execute_shallow_to_ref _ _ Exec) as [r E]; discriminate.
  Qed.
  Lemma execute_to_ref_shallow e r : execute e = Some (Data (Ref r)) -> exists th, e = Shallow th.
  Proof.
    destruct e as [th|th]; intro Exec; [|eauto].
    destruct (execute_strict_to_obj _ _ Exec) as [o E]; discriminate.
  Qed.
  Lemma execute_not_encode e e' : execute e <> Some (Encode e').
  Proof. intro Exec; destruct (execute_data _ _ Exec) as [d E]; discriminate. Qed.
  Lemma R_preserve_blob_or_encode_rev b h : R h (HBlobObj b) ->
    (exists b', h = HBlobObj b') \/ (exists th, h = Encode (Strict th)).
  Proof.
    intro H; inversion H; subst; try solve [eauto].
    match goal with E : execute _ = Some _ |- _ =>
      destruct (execute_to_obj_strict _ _ E) as [th ->]; eauto end.
  Qed.
  Lemma R'_preserve_blob_or_encode_rev b h : R' h (HBlobObj b) ->
    (exists b', h = HBlobObj b') \/ (exists th, h = Encode (Strict th)).
  Proof. intro H; inversion H; subst; eauto using R_preserve_blob_or_encode_rev. Qed.
  Lemma R_preserve_tree_or_encode_rev t h : R h (HTreeObj t) ->
    (exists t', h = HTreeObj t') \/ (exists th, h = Encode (Strict th)).
  Proof.
    intro H; inversion H; subst; try solve [eauto].
    match goal with E : execute _ = Some _ |- _ =>
      destruct (execute_to_obj_strict _ _ E) as [th ->]; eauto end.
  Qed.
  Lemma R'_preserve_tree_or_encode_rev t h : R' h (HTreeObj t) ->
    (exists t', h = HTreeObj t') \/ (exists th, h = Encode (Strict th)).
  Proof. intro H; inversion H; subst; eauto using R_preserve_tree_or_encode_rev. Qed.
  Lemma R_preserve_blob_ref_or_encode_rev b h : R h (HBlobRef b) ->
    (exists b', h = HBlobRef b') \/ (exists th, h = Encode (Shallow th)).
  Proof.
    intro H; inversion H; subst; try solve [eauto].
    match goal with E : execute _ = Some _ |- _ =>
      destruct (execute_to_ref_shallow _ _ E) as [th ->]; eauto end.
  Qed.
  Lemma R'_preserve_blob_ref_or_encode_rev b h : R' h (HBlobRef b) ->
    (exists b', h = HBlobRef b') \/ (exists th, h = Encode (Shallow th)).
  Proof. intro H; inversion H; subst; eauto using R_preserve_blob_ref_or_encode_rev. Qed.
  Lemma R_preserve_tree_ref_or_encode_rev t h : R h (HTreeRef t) ->
    (exists t', h = HTreeRef t') \/ (exists th, h = Encode (Shallow th)).
  Proof.
    intro H; inversion H; subst; try solve [eauto].
    match goal with E : execute _ = Some _ |- _ =>
      destruct (execute_to_ref_shallow _ _ E) as [th ->]; eauto end.
  Qed.
  Lemma R'_preserve_tree_ref_or_encode_rev t h : R' h (HTreeRef t) ->
    (exists t', h = HTreeRef t') \/ (exists th, h = Encode (Shallow th)).
  Proof. intro H; inversion H; subst; eauto using R_preserve_tree_ref_or_encode_rev. Qed.
  Lemma R_encode_execute e h : R (Encode e) h -> ~(exists e', h = Encode e') -> executes_to e h.
  Proof.
    intros H Nh; inversion H; subst; try solve [exfalso; apply Nh; eauto].
    apply execute_some; assumption.
  Qed.
  Lemma R'_encode_execute e h : R' (Encode e) h -> ~(exists e', h = Encode e') -> executes_to e h.
  Proof.
    intros H Nh; inversion H; subst; try solve [exfalso; apply Nh; eauto].
    apply R_encode_execute; assumption.
  Qed.
  Lemma R_encode_execute_rev_does_not_exist e h : R h (Encode e) -> ~(exists e', h = Encode e') -> False.
  Proof. intros H Nh; inversion H; subst; apply Nh; eauto. Qed.
  Lemma R'_encode_execute_rev_does_not_exist e h : R' h (Encode e) -> ~(exists e', h = Encode e') -> False.
  Proof.
    intros H Nh; inversion H; subst; try solve [apply Nh; eauto].
    eapply R_encode_execute_rev_does_not_exist; eauto.
  Qed.
  Lemma rel_opt_R_to_R' x y : rel_opt R x y -> rel_opt R' x y.
  Proof. destruct x, y; cbn [rel_opt]; auto using R_to_R'. Qed.
  Lemma R_strict_encode_reasons t1 t2 : R (Encode (Strict t1)) (Encode (Strict t2)) ->
    rel_opt R (execute (Strict t1)) (execute (Strict t2)).
  Proof.
    intro H; inversion H; subst.
    - repeat match goal with E : execute _ = _ |- _ => rewrite E end; exact I.
    - repeat match goal with E : execute _ = _ |- _ => rewrite E end; assumption.
    - exfalso; eapply execute_not_encode; eauto.
    - destruct (execute (Strict t2)); cbn [rel_opt]; auto using RSelf.
  Qed.
  Lemma R_shallow_encode_reasons t1 t2 : R (Encode (Shallow t1)) (Encode (Shallow t2)) ->
    rel_opt R (execute (Shallow t1)) (execute (Shallow t2)).
  Proof.
    intro H; inversion H; subst.
    - repeat match goal with E : execute _ = _ |- _ => rewrite E end; exact I.
    - repeat match goal with E : execute _ = _ |- _ => rewrite E end; assumption.
    - exfalso; eapply execute_not_encode; eauto.
    - destruct (execute (Shallow t2)); cbn [rel_opt]; auto using RSelf.
  Qed.
  Lemma R'_strict_encode_reasons t1 t2 : R' (Encode (Strict t1)) (Encode (Strict t2)) ->
    R' (Thunk t1) (Thunk t2) \/ rel_opt R' (execute (Strict t1)) (execute (Strict t2)).
  Proof.
    intro H; inversion H; subst; auto.
    - right; apply rel_opt_R_to_R', R_strict_encode_reasons; assumption.
    - right; repeat match goal with E : execute _ = _ |- _ => rewrite E end; assumption.
  Qed.
  Lemma R'_shallow_encode_reasons t1 t2 : R' (Encode (Shallow t1)) (Encode (Shallow t2)) ->
    R' (Thunk t1) (Thunk t2) \/ rel_opt R' (execute (Shallow t1)) (execute (Shallow t2)).
  Proof.
    intro H; inversion H; subst; auto.
    - right; apply rel_opt_R_to_R', R_shallow_encode_reasons; assumption.
    - right; repeat match goal with E : execute _ = _ |- _ => rewrite E end; assumption.
  Qed.
  Lemma R_object_ref_absent o r : ~R (Data (Object o)) (Data (Ref r)).
  Proof.
    destruct o as [b|t]; intro H;
      [destruct (R_preserve_blob _ _ H) as [b' E]|destruct (R_preserve_tree _ _ H) as [t' E]]; discriminate.
  Qed.
  Lemma R_ref_object_absent r o : ~R (Data (Ref r)) (Data (Object o)).
  Proof.
    destruct r as [b|t]; intro H;
      [destruct (R_preserve_blob_ref _ _ H) as [b' E]|destruct (R_preserve_tree_ref _ _ H) as [t' E]]; discriminate.
  Qed.
  Lemma R'_object_ref_absent o r : ~R' (Data (Object o)) (Data (Ref r)).
  Proof.
    destruct o as [b|t]; intro H;
      [destruct (R'_preserve_blob _ _ H) as [b' E]|destruct (R'_preserve_tree _ _ H) as [t' E]]; discriminate.
  Qed.
  Lemma R'_ref_object_absent r o : ~R' (Data (Ref r)) (Data (Object o)).
  Proof.
    destruct r as [b|t]; intro H;
      [destruct (R'_preserve_blob_ref _ _ H) as [b' E]|destruct (R'_preserve_tree_ref _ _ H) as [t' E]]; discriminate.
  Qed.
  Ltac incompatible_execute :=
    repeat match goal with
    | E : execute (Strict _) = Some _ |- _ => destruct (execute_strict_to_obj _ _ E) as [? ->]; clear E
    | E : execute (Shallow _) = Some _ |- _ => destruct (execute_shallow_to_ref _ _ E) as [? ->]; clear E
    end.
  Lemma R_not_shallow_strict t1 t2 : ~R (Encode (Shallow t1)) (Encode (Strict t2)).
  Proof.
    intro H; inversion H; subst.
    - incompatible_execute; eapply R_ref_object_absent; eassumption.
    - eapply execute_not_encode; eassumption.
  Qed.
  Lemma R_not_strict_shallow t1 t2 : ~R (Encode (Strict t1)) (Encode (Shallow t2)).
  Proof.
    intro H; inversion H; subst.
    - incompatible_execute; eapply R_object_ref_absent; eassumption.
    - eapply execute_not_encode; eassumption.
  Qed.
  Lemma R'_not_shallow_strict t1 t2 : ~R' (Encode (Shallow t1)) (Encode (Strict t2)).
  Proof.
    intro H; inversion H; subst.
    - eapply R_not_shallow_strict; eassumption.
    - incompatible_execute; eapply R'_ref_object_absent; eassumption.
  Qed.
  Lemma R'_not_strict_shallow t1 t2 : ~R' (Encode (Strict t1)) (Encode (Shallow t2)).
  Proof.
    intro H; inversion H; subst.
    - eapply R_not_strict_shallow; eassumption.
    - incompatible_execute; eapply R'_object_ref_absent; eassumption.
  Qed.
  Lemma R_lower_to_lift_data d1 d2 : R (lower (Data d1)) (lower (Data d2)) -> R (lift (Data d1)) (lift (Data d2)).
  Proof.
    apply (lower_to_lift R blob_ref_complete_R tree_ref_complete_R R_preserve_tree_ref R_preserve_blob_ref).
  Qed.
  Lemma R'_lower_to_lift_data d1 d2 : R' (lower (Data d1)) (lower (Data d2)) -> R' (lift (Data d1)) (lift (Data d2)).
  Proof.
    apply (lower_to_lift R' blob_ref_complete_R' tree_ref_complete_R' R'_preserve_tree_ref R'_preserve_blob_ref).
  Qed.
  Lemma R'_lift_to_lower_data d1 d2 : R' (lift (Data d1)) (lift (Data d2)) -> R' (lower (Data d1)) (lower (Data d2)).
  Proof.
    apply (lift_to_lower R' blob_ref_cong_R' tree_ref_cong_R' R'_preserve_tree R'_preserve_blob).
  Qed.
  Lemma R_encode_to_force e1 e2 : R (Encode e1) (Encode e2) ->
    rel_opt (relaxed_X R) (force (encode_to_thunk e1)) (force (encode_to_thunk e2)).
  Proof.
    destruct e1 as [t1|t1], e2 as [t2|t2]; cbn [encode_to_thunk]; intro H.
    - apply (strict_force_to_relaxed R).
      pose proof (R_strict_encode_reasons _ _ H) as Exec; rewrite !execute_hs in Exec; exact Exec.
    - exfalso; exact (R_not_strict_shallow _ _ H).
    - exfalso; exact (R_not_shallow_strict _ _ H).
    - apply (force_to_lift R blob_ref_complete_R tree_ref_complete_R R_preserve_tree_ref R_preserve_blob_ref).
      pose proof (R_shallow_encode_reasons _ _ H) as Exec; rewrite !execute_hs in Exec; exact Exec.
  Qed.
  Lemma strengthen_R_self h : strengthen R h h.
  Proof. destruct h; [apply RSelf|apply RSelf|left; apply RSelf]. Qed.
  Lemma strengthen_R'_self h : strengthen R' h h.
  Proof. destruct h; [apply R'_refl|apply R'_refl|left; apply R'_refl]. Qed.
  Lemma squash_strengthen (X : handle -> handle -> Prop) h1 h2 :
    ~(exists d, h1 = Data d) -> ~(exists d, h2 = Data d) ->
    X (squash h1) (squash h2) -> strengthen X h1 h2.
  Proof.
    intros N1 N2 Related; destruct h1 as [d1|t1|e1], h2 as [d2|t2|e2];
      try solve [exfalso; apply N1; eauto]; try solve [exfalso; apply N2; eauto].
    - exact Related.
    - destruct e2; exact Related.
    - destruct e1; exact Related.
    - left; destruct e1, e2; exact Related.
  Qed.
  Lemma encode_data_strengthen (X : handle -> handle -> Prop) e d :
    X (Encode (Strict (encode_to_thunk e))) (lift (Data d)) -> strengthen X (Encode e) (Data d).
  Proof. destruct e; exact (fun H => H). Qed.
  Lemma R_thunk_reasons t1 t2 : R (Thunk t1) (Thunk t2) ->
    (think t1 = None /\ think t2 = None) \/
    (exists r1 r2, think t1 = Some r1 /\ think t2 = Some r2 /\ strengthen R r1 r2) \/
    think t1 = Some (Thunk t2) \/ think t1 = Some (Encode (Strict t2)) \/ think t1 = Some (Encode (Shallow t2)).
  Proof.
    intro H; inversion H; subst; try solve [eauto 8].
    - right; left; eexists; eexists; repeat split; try eassumption.
      eapply squash_strengthen; eassumption.
    - right; left; eexists; eexists; repeat split; try eassumption.
      apply encode_data_strengthen; assumption.
    - right; left; eexists; eexists; repeat split; try eassumption.
      right; match goal with Rel : R (Encode (Strict ?u)) (Encode (Strict ?v)) |- _ =>
        exact (R_encode_to_force (Strict u) (Strict v) Rel) end.
    - destruct (think t2) as [r|] eqn:Think.
      + right; left; exists r, r; repeat split; try reflexivity; apply strengthen_R_self.
      + left; auto.
  Qed.
  Lemma relaxed_R_to_R' h1 h2 : relaxed_X R h1 h2 -> relaxed_X R' h1 h2.
  Proof. destruct h1 as [d|t|[t|t]]; apply R_to_R'. Qed.
  Lemma rel_opt_relaxed_R_to_R' x y : rel_opt (relaxed_X R) x y -> rel_opt (relaxed_X R') x y.
  Proof. destruct x, y; cbn [rel_opt]; auto using relaxed_R_to_R'. Qed.
  Lemma strengthen_R_to_R' h1 h2 : strengthen R h1 h2 -> strengthen R' h1 h2.
  Proof.
    destruct h1 as [d1|t1|e1], h2 as [d2|t2|e2];
      try apply relaxed_R_to_R'; try apply R_to_R'.
    intros [Thunks|Forces]; [left; apply R_to_R'; exact Thunks|right; apply rel_opt_relaxed_R_to_R'; exact Forces].
  Qed.
  Lemma R'_thunk_reasons t1 t2 : R' (Thunk t1) (Thunk t2) -> thunk_reasons R' t1 t2.
  Proof.
    intro H; unfold thunk_reasons; inversion H; subst; try solve [eauto 12].
    - match goal with Rel : R (Thunk _) (Thunk _) |- _ =>
        destruct (R_thunk_reasons _ _ Rel) as [None|[[r1 [r2 [T1 [T2 Strength]]]]|[OneThunk|[OneStrict|OneShallow]]]] end;
        eauto 12 using strengthen_R_to_R'.
    - do 5 right; left; eexists; eexists; repeat split; try eassumption.
      eapply squash_strengthen; eassumption.
    - do 5 right; left; eexists; eexists; repeat split; try eassumption.
      apply encode_data_strengthen; assumption.
    - do 5 right; left; eexists; eexists; repeat split; try eassumption.
      match goal with Rel : R' (Encode (Strict ?u)) (Encode (Strict ?v)) |- _ =>
        destruct (R'_strict_encode_reasons u v Rel) as [Thunks|Exec] end.
      + left; exact Thunks.
      + right; apply (strict_force_to_relaxed R'); rewrite !execute_hs in Exec; exact Exec.
  Qed.
  Lemma R'_properties : relation_properties R'.
  Proof.
    refine {| blob_cong := blob_cong_R'; tree_cong := tree_cong_R';
      blob_complete := blob_complete_R'; tree_complete := tree_complete_R';
      blob_ref_cong := blob_ref_cong_R'; tree_ref_cong := tree_ref_cong_R';
      blob_ref_complete := blob_ref_complete_R'; tree_ref_complete := tree_ref_complete_R';
      strict_encode_cong := strict_encode_cong_R'; shallow_encode_cong := shallow_encode_cong_R';
      application_thunk_cong := application_thunk_cong_R'; selection_thunk_cong := selection_thunk_cong_R';
      digestion_thunk_cong := digestion_thunk_cong_R'; identification_thunk_cong := identification_thunk_cong_R';
      preserve_tree_ref := R'_preserve_tree_ref; preserve_tree_ref_rev := R'_preserve_tree_ref_or_encode_rev;
      preserve_blob_ref := R'_preserve_blob_ref; preserve_blob_ref_rev := R'_preserve_blob_ref_or_encode_rev;
      preserve_tree := R'_preserve_tree; preserve_tree_rev := R'_preserve_tree_or_encode_rev;
      preserve_blob := R'_preserve_blob; preserve_blob_rev := R'_preserve_blob_or_encode_rev;
      preserve_thunk := R'_preserve_thunk; encode_eval := R'_encode_execute;
      strict_encode_reasons := R'_strict_encode_reasons; shallow_encode_reasons := R'_shallow_encode_reasons;
      not_shallow_strict := R'_not_shallow_strict; not_strict_shallow := R'_not_strict_shallow;
      encode_reverse_absent := R'_encode_execute_rev_does_not_exist;
      related_thunk_reasons := R'_thunk_reasons; X_self := R'_refl |}.
  Qed.
  Lemma R'_thunk_think t1 t2 : R' (Thunk t1) (Thunk t2) ->
    rel_opt (strengthen R') (think t1) (think t2) \/
    think t1 = Some (Thunk t2) \/ think t1 = Some (Encode (Strict t2)) \/ think t1 = Some (Encode (Shallow t2)).
  Proof. apply (think_X R' R'_properties). Qed.
  Lemma R'_thunk_force t1 t2 : R' (Thunk t1) (Thunk t2) ->
    rel_opt (relaxed_X R') (force t1) (force t2).
  Proof. apply (forces_X R' R'_properties). Qed.
  (** A single productive unfolding of R. Its recursive premises are exposed
      as Q, so Rocq can check the subsequent cofixpoint's guardedness. *)
  Inductive R_step (Q : handle -> handle -> Prop) : handle -> handle -> Prop :=
  | StepExisting : forall h1 h2, R h1 h2 -> R_step Q h1 h2
  | StepTree : forall t1 t2, Forall2 Q (get_tree_raw t1) (get_tree_raw t2) -> R_step Q (HTreeObj t1) (HTreeObj t2)
  | StepTreeRef : forall t1 t2, Q (HTreeObj t1) (HTreeObj t2) -> R_step Q (HTreeRef t1) (HTreeRef t2)
  | StepThunkNone : forall t1 t2, think t1 = None -> think t2 = None -> R_step Q (Thunk t1) (Thunk t2)
  | StepThunkData : forall t1 t2 d1 d2, think t1 = Some (Data d1) -> think t2 = Some (Data d2) ->
      Q (lift (Data d1)) (lift (Data d2)) -> R_step Q (Thunk t1) (Thunk t2)
  | StepThunkNotData : forall t1 t2 h1 h2, think t1 = Some h1 -> think t2 = Some h2 ->
      ~(exists d, h1 = Data d) -> ~(exists d, h2 = Data d) ->
      Q (squash h1) (squash h2) -> R_step Q (Thunk t1) (Thunk t2)
  | StepThunkEncodeData : forall t1 t2 e1 d2, think t1 = Some (Encode e1) -> think t2 = Some (Data d2) ->
      Q (Encode (Strict (encode_to_thunk e1))) (lift (Data d2)) -> R_step Q (Thunk t1) (Thunk t2)
  | StepThunkEncodeEncode : forall t1 t2 e1 e2, think t1 = Some (Encode e1) -> think t2 = Some (Encode e2) ->
      Q (Encode (Strict (encode_to_thunk e1))) (Encode (Strict (encode_to_thunk e2))) -> R_step Q (Thunk t1) (Thunk t2)
  | StepSingleThunk : forall t1 t2, think t1 = Some (Thunk t2) -> R_step Q (Thunk t1) (Thunk t2)
  | StepSingleStrict : forall t1 t2, think t1 = Some (Encode (Strict t2)) -> R_step Q (Thunk t1) (Thunk t2)
  | StepSingleShallow : forall t1 t2, think t1 = Some (Encode (Shallow t2)) -> R_step Q (Thunk t1) (Thunk t2)
  | StepStrictNone : forall t1 t2, execute (Strict t1) = None -> execute (Strict t2) = None ->
      R_step Q (Encode (Strict t1)) (Encode (Strict t2))
  | StepShallowNone : forall t1 t2, execute (Shallow t1) = None -> execute (Shallow t2) = None ->
      R_step Q (Encode (Shallow t1)) (Encode (Shallow t2))
  | StepExecuteSome : forall e1 e2 h1 h2, execute e1 = Some h1 -> execute e2 = Some h2 -> Q h1 h2 ->
      R_step Q (Encode e1) (Encode e2).

  Theorem R_coinduct (Q : handle -> handle -> Prop)
      (Unfold : forall h1 h2, Q h1 h2 -> R_step Q h1 h2) : forall h1 h2, Q h1 h2 -> R h1 h2.
  Proof.
    refine (cofix IH (h1 h2 : handle) (Related : Q h1 h2) : R h1 h2 := _
      with IH_list (xs ys : list handle) (Related : Forall2 Q xs ys) : R_handles xs ys := _ for IH).
    - destruct (Unfold h1 h2 Related).
      + assumption.
      + apply RTreeNodes; apply IH_list; assumption.
      + apply RTreeRef; apply IH; assumption.
      + apply RThunkNone; assumption.
      + eapply RThunkSomeResData; [eassumption|eassumption|apply IH; eassumption].
      + eapply RThunkSomeResNotData; [eassumption|eassumption|eassumption|eassumption|apply IH; eassumption].
      + eapply RThunkSomeResEncodeData; [eassumption|eassumption|apply IH; eassumption].
      + eapply RThunkSomeResEncodeEncode; [eassumption|eassumption|apply IH; eassumption].
      + apply RThinkSingleStepThunk; assumption.
      + apply RThunkSingleStepEncodeStrict; assumption.
      + apply RThunkSingleStepEncodeShallow; assumption.
      + apply REvalStrictNone; assumption.
      + apply REvalShallowNone; assumption.
      + eapply REvalSomeRes; [eassumption|eassumption|apply IH; eassumption].
    - destruct Related; constructor; [apply IH; assumption|apply IH_list; assumption].
  Qed.
  Lemma forces_to_strict_R' t1 t2 : rel_opt (relaxed_X R') (force t1) (force t2) ->
    R' (Encode (Strict t1)) (Encode (Strict t2)).
  Proof.
    intro Related; destruct (force t1) as [h1|] eqn:F1, (force t2) as [h2|] eqn:F2;
      cbn [rel_opt] in Related; try contradiction.
    - destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->].
      eapply R'_encode_some_res; [rewrite execute_hs; cbn; rewrite F1; reflexivity|
        rewrite execute_hs; cbn; rewrite F2; reflexivity|exact Related].
    - apply R_to_R', REvalStrictNone; rewrite execute_hs; cbn; [rewrite F1|rewrite F2]; reflexivity.
  Qed.
  Lemma R'_thunk_step t1 t2 : R' (Thunk t1) (Thunk t2) -> R_step R' (Thunk t1) (Thunk t2).
  Proof.
    intro Related; destruct (R'_thunk_think t1 t2 Related) as [Replies|[OneThunk|[OneStrict|OneShallow]]].
    2: apply StepSingleThunk; exact OneThunk.
    2: apply StepSingleStrict; exact OneStrict.
    2: apply StepSingleShallow; exact OneShallow.
    destruct (think t1) as [h1|] eqn:T1, (think t2) as [h2|] eqn:T2;
      cbn [rel_opt] in Replies; try contradiction; [|apply StepThunkNone; assumption].
    destruct h1 as [d1|u1|e1], h2 as [d2|u2|e2].
    - eapply StepThunkData; eassumption.
    - exfalso; exact (data_not_thunk R' R'_properties (lift_data d1) u2 Replies).
    - exfalso; exact (data_not_encode R' R'_properties (lift_data d1) e2 Replies).
    - exfalso; exact (thunk_not_data R' R'_properties u1 (lift_data d2) Replies).
    - eapply StepThunkNotData; [eassumption|eassumption|intros [d E]; discriminate|
        intros [d E]; discriminate|exact Replies].
    - eapply StepThunkNotData; [eassumption|eassumption|intros [d E]; discriminate|
        intros [d E]; discriminate|destruct e2; exact Replies].
    - eapply StepThunkEncodeData; [eassumption|eassumption|destruct e1; exact Replies].
    - eapply StepThunkNotData; [eassumption|eassumption|intros [d E]; discriminate|
        intros [d E]; discriminate|destruct e1; exact Replies].
    - destruct Replies as [Thunks|Forces].
      + eapply StepThunkNotData; [eassumption|eassumption|intros [d E]; discriminate|
          intros [d E]; discriminate|destruct e1, e2; exact Thunks].
      + eapply StepThunkEncodeEncode; [eassumption|eassumption|apply forces_to_strict_R'; exact Forces].
  Qed.
  Lemma R'_force_execute_strict t1 t2 : rel_opt (relaxed_X R') (force t1) (force t2) ->
    rel_opt R' (execute (Strict t1)) (execute (Strict t2)).
  Proof.
    intro Related; rewrite !execute_hs; cbn.
    destruct (force t1) as [h1|] eqn:F1, (force t2) as [h2|] eqn:F2;
      cbn [rel_opt omap] in *; try contradiction; [|exact I].
    destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->]; exact Related.
  Qed.
  Lemma R'_force_execute_shallow t1 t2 : rel_opt (relaxed_X R') (force t1) (force t2) ->
    rel_opt R' (execute (Shallow t1)) (execute (Shallow t2)).
  Proof.
    intro Related; rewrite !execute_hs; cbn.
    destruct (force t1) as [h1|] eqn:F1, (force t2) as [h2|] eqn:F2;
      cbn [rel_opt omap] in *; try contradiction; [|exact I].
    destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->].
    apply R'_lift_to_lower_data; exact Related.
  Qed.
  Lemma R'_strict_step t1 t2 : R' (Thunk t1) (Thunk t2) ->
    R_step R' (Encode (Strict t1)) (Encode (Strict t2)).
  Proof.
    intro Related; pose proof (R'_force_execute_strict t1 t2 (R'_thunk_force t1 t2 Related)) as Exec.
    destruct (execute (Strict t1)) as [h1|] eqn:E1, (execute (Strict t2)) as [h2|] eqn:E2;
      cbn [rel_opt] in Exec; try contradiction.
    - eapply StepExecuteSome; eassumption.
    - apply StepStrictNone; assumption.
  Qed.
  Lemma R'_shallow_step t1 t2 : R' (Thunk t1) (Thunk t2) ->
    R_step R' (Encode (Shallow t1)) (Encode (Shallow t2)).
  Proof.
    intro Related; pose proof (R'_force_execute_shallow t1 t2 (R'_thunk_force t1 t2 Related)) as Exec.
    destruct (execute (Shallow t1)) as [h1|] eqn:E1, (execute (Shallow t2)) as [h2|] eqn:E2;
      cbn [rel_opt] in Exec; try contradiction.
    - eapply StepExecuteSome; eassumption.
    - apply StepShallowNone; assumption.
  Qed.
  Lemma R'_unfold h1 h2 : R' h1 h2 -> R_step R' h1 h2.
  Proof.
    intro Related; inversion Related; subst.
    - apply StepExisting; assumption.
    - apply StepTree, list_all2_R'orR_to_R'; assumption.
    - apply StepTreeRef; assumption.
    - apply R'_thunk_step; assumption.
    - apply R'_thunk_step; assumption.
    - apply R'_thunk_step; assumption.
    - apply R'_thunk_step; assumption.
    - eapply StepThunkData; eassumption.
    - eapply StepThunkNotData; eassumption.
    - eapply StepThunkEncodeData; eassumption.
    - eapply StepThunkEncodeEncode; eassumption.
    - apply R'_strict_step; assumption.
    - apply R'_shallow_step; assumption.
    - eapply StepExecuteSome; eassumption.
  Qed.
  Theorem R'_impl_R h1 h2 : R' h1 h2 -> R h1 h2.
  Proof. apply (R_coinduct R' R'_unfold). Qed.

  Lemma R_properties : relation_properties R.
  Proof.
    refine {| blob_cong := blob_cong_R; tree_cong := tree_cong_R;
      blob_complete := blob_complete_R; tree_complete := tree_complete_R;
      blob_ref_cong := blob_ref_cong_R; tree_ref_cong := tree_ref_cong_R;
      blob_ref_complete := blob_ref_complete_R; tree_ref_complete := tree_ref_complete_R;
      preserve_tree_ref := R_preserve_tree_ref; preserve_tree_ref_rev := R_preserve_tree_ref_or_encode_rev;
      preserve_blob_ref := R_preserve_blob_ref; preserve_blob_ref_rev := R_preserve_blob_ref_or_encode_rev;
      preserve_tree := R_preserve_tree; preserve_tree_rev := R_preserve_tree_or_encode_rev;
      preserve_blob := R_preserve_blob; preserve_blob_rev := R_preserve_blob_or_encode_rev;
      preserve_thunk := R_preserve_thunk; encode_eval := R_encode_execute;
      not_shallow_strict := R_not_shallow_strict; not_strict_shallow := R_not_strict_shallow;
      encode_reverse_absent := R_encode_execute_rev_does_not_exist; X_self := RSelf |}.
    - intros t1 t2 Related; apply R'_impl_R, strict_encode_cong_R', R_to_R'; exact Related.
    - intros t1 t2 Related; apply R'_impl_R, shallow_encode_cong_R', R_to_R'; exact Related.
    - intros t1 t2 Related; apply R'_impl_R, application_thunk_cong_R', R_to_R'; exact Related.
    - intros t1 t2 Related; apply R'_impl_R, selection_thunk_cong_R', R_to_R'; exact Related.
    - intros t1 t2 Related; apply R'_impl_R, digestion_thunk_cong_R', R_to_R'; exact Related.
    - intros d1 d2 Related; apply R'_impl_R, identification_thunk_cong_R', R_to_R'; exact Related.
    - intros t1 t2 Related; right; apply R_strict_encode_reasons; exact Related.
    - intros t1 t2 Related; right; apply R_shallow_encode_reasons; exact Related.
    - intros t1 t2 Related; unfold thunk_reasons; do 4 right; apply R_thunk_reasons; exact Related.
  Qed.
  Theorem eval_R h1 h2 : R h1 h2 -> rel_opt R (eval h1) (eval h2).
  Proof. apply (evals_X R R_properties). Qed.
  Theorem force_R t1 t2 : R (Thunk t1) (Thunk t2) -> rel_opt (relaxed_X R) (force t1) (force t2).
  Proof. apply (forces_X R R_properties). Qed.
  Lemma lower_not_obj h h' : h = lower h' -> ~(exists o, h = Data (Object o)).
  Proof.
    intros -> [o E]; destruct h' as [[[b|t]|[b|t]]|th|[th|th]]; discriminate.
  Qed.
  Lemma lift_not_ref h h' : h = lift h' -> ~(exists r, h = Data (Ref r)).
  Proof.
    intros -> [r E]; destruct h' as [[[b|t]|[b|t]]|th|[th|th]]; discriminate.
  Qed.
End Equivalence.
