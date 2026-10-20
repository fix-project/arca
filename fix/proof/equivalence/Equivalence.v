(** Defines the coinductive relations underlying computational equivalence
    and proves their structural and evaluation properties. *)

From Stdlib Require Import List.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Evaluation Require Import Properties.

Module Equivalence (S : STORAGE) (P : PROGRAM S).
  Module EP := EvaluationProperties S P.
  Import EP EP.E EP.E.H.
  Definition squash h :=
    match h with Encode (Strict th) | Encode (Shallow th) => Thunk th | _ => h end.

  (** R compares recursive behavior rather than counting reduction steps. It can relate
      computations whose next replies differ in how they defer the remaining work,
      provided the replies represent related eventual Objects. Coinduction permits
      recursive comparisons with no finite bound on their depth. The directed execution
      rule also admits replacing an Encode by its actual result; symmetry and finite
      composition are added separately in Closure.v. *)
  CoInductive R : handle -> handle -> Prop :=
  | RBlob : forall b1 b2, get_blob_data b1 = get_blob_data b2 -> R (HBlobObj b1) (HBlobObj b2)
  | RBlobRef : forall b1 b2, R (HBlobObj b1) (HBlobObj b2) -> R (HBlobRef b1) (HBlobRef b2)
  | RTreeNodes : forall t1 t2, tree_tag t1 = tree_tag t2 -> R_handles (get_tree_raw t1) (get_tree_raw t2) -> R (HTreeObj t1) (HTreeObj t2)
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

  (** The coinductive child relation has the same finite-list content as the usual
      pointwise relation. This lets the coinduction expose its recursive calls directly to
      Rocq while the rest of the argument uses familiar list reasoning. *)
  Lemma R_handles_Forall2 xs ys : R_handles xs ys -> Forall2 R xs ys.
  Proof.
    revert ys; induction xs; intros ys Related; inversion Related; subst; constructor; auto.
  Qed.
  (** The ordinary pointwise R relation supplies the coinductive list relation. *)
  Lemma Forall2_R_handles xs ys : Forall2 R xs ys -> R_handles xs ys.
  Proof. induction 1; constructor; auto. Qed.
  (** Matching Tag markers and pointwise R-related children give R-related Tree Objects. *)
  Lemma RTree t1 t2 : tree_tag t1 = tree_tag t2 -> Forall2 R (get_tree_raw t1) (get_tree_raw t2) -> R (HTreeObj t1) (HTreeObj t2).
  Proof. intros Tags Related; apply RTreeNodes; [exact Tags|apply Forall2_R_handles; exact Related]. Qed.

  (** The congruence rules needed during evaluation are not explicit constructors of R. R′
      temporarily admits those rules so that evaluating related combinations, selections,
      and Encodes can be proved compositionally. The subsequent coinduction shows that
      these extra rules are already justified by R; they do not enlarge the final
      relation. *)
  CoInductive R' : handle -> handle -> Prop :=
  | R'_from_R : forall h1 h2, R h1 h2 -> R' h1 h2
  | R'_Tree : forall t1 t2, tree_tag t1 = tree_tag t2 ->
      Forall2 (fun x y => R' x y \/ R x y) (get_tree_raw t1) (get_tree_raw t2) -> R' (HTreeObj t1) (HTreeObj t2)
  | R'_TreeRef : forall t1 t2, R' (HTreeObj t1) (HTreeObj t2) -> R' (HTreeRef t1) (HTreeRef t2)
  | R'_tree_to_application_thunk : forall t1 t2, R' (HTreeObj t1) (HTreeObj t2) -> R' (Thunk (Application t1)) (Thunk (Application t2))
  | R'_tree_to_selection_thunk : forall t1 t2, R' (HTreeObj t1) (HTreeObj t2) -> R' (Thunk (Selection t1)) (Thunk (Selection t2))
  | R'_data_to_identification_thunk : forall d1 d2, R' (Data (Ref d1)) (Data (Ref d2)) -> R' (Thunk (Identification d1)) (Thunk (Identification d2))
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

  (** Every R-related pair is also related by the auxiliary relation R′. *)
  Lemma R_to_R' x y : R x y -> R' x y.
  Proof. apply R'_from_R. Qed.
  (** Either an R′ step or an R step is an R′ step. *)
  Lemma R'orR_to_R' x y : R' x y \/ R x y -> R' x y.
  Proof. intros [H|H]; [exact H|apply R_to_R'; exact H]. Qed.
  (** A list of mixed R′ and R steps is pointwise R′-related. *)
  Lemma Forall2_R'orR_to_R' xs ys :
    Forall2 (fun x y => R' x y \/ R x y) xs ys -> Forall2 R' xs ys.
  Proof. induction 1; constructor; auto using R'orR_to_R'. Qed.
  (** Pointwise R-related lists are pointwise R′-related. *)
  Lemma Forall2_R_to_R' xs ys : Forall2 R xs ys -> Forall2 R' xs ys.
  Proof. induction 1; constructor; auto using R_to_R'. Qed.
  (** Pointwise R′-related lists satisfy the mixed R′-or-R relation. *)
  Lemma Forall2_R'_imp_R'orR xs ys : Forall2 R' xs ys -> Forall2 (fun x y => R' x y \/ R x y) xs ys.
  Proof. induction 1; constructor; auto. Qed.
  (** The auxiliary relation R′ is reflexive. *)
  Lemma R'_refl h : R' h h.
  Proof. apply R_to_R', RSelf. Qed.
  (** An R justification between Blob Objects is constrained by byte equality. This is the
      local invariant later used to show that closing the relation under equivalence
      cannot identify different Blob contents. *)
  Lemma blob_cong_R b1 b2 : R (HBlobObj b1) (HBlobObj b2) -> get_blob_data b1 = get_blob_data b2.
  Proof. intro H; inversion H; subst; auto. Qed.
  (** Related Blob Objects have identical bytes under R′. *)
  Lemma blob_cong_R' b1 b2 : R' (HBlobObj b1) (HBlobObj b2) -> get_blob_data b1 = get_blob_data b2.
  Proof. intro H; inversion H; subst; eauto using blob_cong_R. Qed.
  (** An R justification between Tree Objects carries one justification for each
      corresponding child. Evaluation can therefore reuse those child premises instead of
      treating the Tree as an opaque computation. *)
  Lemma tree_cong_R t1 t2 : R (HTreeObj t1) (HTreeObj t2) -> Forall2 R (get_tree_raw t1) (get_tree_raw t2).
  Proof. intro H; inversion H; subst; auto using R_handles_Forall2. induction (get_tree_raw t2); constructor; auto using RSelf. Qed.
  (** Related Tree Objects have pointwise related children under R′. *)
  Lemma tree_cong_R' t1 t2 : R' (HTreeObj t1) (HTreeObj t2) -> Forall2 R' (get_tree_raw t1) (get_tree_raw t2).
  Proof. intro H; inversion H; subst; eauto using tree_cong_R, Forall2_R_to_R', Forall2_R'orR_to_R'. Qed.
  (** R retains the Tag marker as part of Tree identity. An authority-bearing Tag cannot
      become an ordinary Tree merely by replacing equivalent children. *)
  Lemma tag_cong_R t1 t2 : R (HTreeObj t1) (HTreeObj t2) -> tree_tag t1 = tree_tag t2.
  Proof. intro Related; inversion Related; subst; auto. Qed.
  (** Related Tree Objects have the same Tag marker under R′. *)
  Lemma tag_cong_R' t1 t2 : R' (HTreeObj t1) (HTreeObj t2) -> tree_tag t1 = tree_tag t2.
  Proof. intro Related; inversion Related; subst; eauto using tag_cong_R. Qed.
  (** Constructing Trees with a shared marker preserves the pointwise relation under R. *)
  Lemma tree_tagged_complete_R tag xs ys : Forall2 R xs ys ->
    R (HTreeObj (create_tree_tagged tag xs)) (HTreeObj (create_tree_tagged tag ys)).
  Proof.
    intro Related; apply RTree; [rewrite !tree_tag_create_tree_tagged; reflexivity|].
    rewrite !get_tree_raw_create_tree_tagged; exact Related.
  Qed.
  (** Constructing Trees with a shared marker preserves the pointwise relation under R′. *)
  Lemma tree_tagged_complete_R' tag xs ys : Forall2 R' xs ys ->
    R' (HTreeObj (create_tree_tagged tag xs)) (HTreeObj (create_tree_tagged tag ys)).
  Proof.
    intro Related; apply R'_Tree; [rewrite !tree_tag_create_tree_tagged; reflexivity|].
    rewrite !get_tree_raw_create_tree_tagged; apply Forall2_R'_imp_R'orR; exact Related.
  Qed.
  (** Related Blob Objects have related Refs under R. *)
  Lemma blob_ref_cong_R b1 b2 : R (HBlobObj b1) (HBlobObj b2) -> R (HBlobRef b1) (HBlobRef b2).
  Proof. apply RBlobRef. Qed.
  (** Related Blob Objects have related Refs under R′. *)
  Lemma blob_ref_cong_R' b1 b2 : R' (HBlobObj b1) (HBlobObj b2) -> R' (HBlobRef b1) (HBlobRef b2).
  Proof. intro H; apply R_to_R', RBlobRef, RBlob; apply blob_cong_R'; exact H. Qed.
  (** Related Tree Objects have related Refs under R. *)
  Lemma tree_ref_cong_R t1 t2 : R (HTreeObj t1) (HTreeObj t2) -> R (HTreeRef t1) (HTreeRef t2).
  Proof. apply RTreeRef. Qed.
  (** Related Tree Objects have related Refs under R′. *)
  Lemma tree_ref_cong_R' t1 t2 : R' (HTreeObj t1) (HTreeObj t2) -> R' (HTreeRef t1) (HTreeRef t2).
  Proof. apply R'_TreeRef. Qed.
  (** Related Blob Refs have related underlying Objects under R. *)
  Lemma blob_ref_complete_R b1 b2 : R (HBlobRef b1) (HBlobRef b2) -> R (HBlobObj b1) (HBlobObj b2).
  Proof. intro H; inversion H; subst; auto using RSelf. Qed.
  (** Related Blob Refs have related underlying Objects under R′. *)
  Lemma blob_ref_complete_R' b1 b2 : R' (HBlobRef b1) (HBlobRef b2) -> R' (HBlobObj b1) (HBlobObj b2).
  Proof. intro H; inversion H; subst; eauto using blob_ref_complete_R, R_to_R'. Qed.
  (** Related Tree Refs have related underlying Objects under R. *)
  Lemma tree_ref_complete_R t1 t2 : R (HTreeRef t1) (HTreeRef t2) -> R (HTreeObj t1) (HTreeObj t2).
  Proof. intro H; inversion H; subst; auto using RSelf. Qed.
  (** Related Tree Refs have related underlying Objects under R′. *)
  Lemma tree_ref_complete_R' t1 t2 : R' (HTreeRef t1) (HTreeRef t2) -> R' (HTreeObj t1) (HTreeObj t2).
  Proof. intro H; inversion H; subst; eauto using tree_ref_complete_R, R_to_R'. Qed.
  (** Related bare Thunks have related strict Encodes under R′. *)
  Lemma strict_encode_cong_R' t1 t2 : R' (Thunk t1) (Thunk t2) -> R' (Encode (Strict t1)) (Encode (Strict t2)).
  Proof. apply R'_thunk_to_encode_strict. Qed.
  (** Related bare Thunks have related shallow Encodes under R′. *)
  Lemma shallow_encode_cong_R' t1 t2 : R' (Thunk t1) (Thunk t2) -> R' (Encode (Shallow t1)) (Encode (Shallow t2)).
  Proof. apply R'_thunk_to_encode_shallow. Qed.
  (** Related combinations give related application Thunks under R′. *)
  Lemma application_thunk_cong_R' t1 t2 : R' (HTreeObj t1) (HTreeObj t2) -> R' (Thunk (Application t1)) (Thunk (Application t2)).
  Proof. apply R'_tree_to_application_thunk. Qed.
  (** Related selection operand Trees give related selection Thunks under R′. *)
  Lemma selection_thunk_cong_R' t1 t2 : R' (HTreeObj t1) (HTreeObj t2) -> R' (Thunk (Selection t1)) (Thunk (Selection t2)).
  Proof. apply R'_tree_to_selection_thunk. Qed.
  (** Related Refs give related identification Thunks under R′. *)
  Lemma identification_thunk_cong_R' d1 d2 : R' (Data (Ref d1)) (Data (Ref d2)) -> R' (Thunk (Identification d1)) (Thunk (Identification d2)).
  Proof. apply R'_data_to_identification_thunk. Qed.

  (** Successful strict Encode execution returns an Object. *)
  Lemma execute_strict_to_obj th h : execute (Strict th) = Some h -> exists o, h = Data (Object o).
  Proof.
    rewrite execute_unfold; cbn; intro Exec; apply omap_some in Exec;
      destruct Exec as [v [Force ->]]; destruct (force_data _ _ Force) as [[[b|t]|[b|t]] ->];
      cbn [lift lift_data]; eauto.
  Qed.
  (** Successful shallow Encode execution returns a Ref. *)
  Lemma execute_shallow_to_ref th h : execute (Shallow th) = Some h -> exists r, h = Data (Ref r).
  Proof.
    rewrite execute_unfold; cbn; intro Exec; apply omap_some in Exec;
      destruct Exec as [v [Force ->]]; destruct (force_data _ _ Force) as [[[b|t]|[b|t]] ->];
      cbn [lower lower_data]; eauto.
  Qed.
  (** R preserves whether the whole handle is a bare Thunk. Consequently an equivalent
      source for Force still has the form that Force requires, even when its internal
      computation is different. *)
  Lemma R_preserve_thunk h1 h2 : R h1 h2 ->
    ((exists t, h1 = Thunk t) <-> (exists t, h2 = Thunk t)).
  Proof.
    intro H; inversion H; subst; try solve [split; intros [t E]; discriminate];
      try solve [split; eauto].
    match goal with E : execute _ = Some _ |- _ => destruct (execute_data _ _ E) as [d ->] end.
    split; intros [t E]; discriminate.
  Qed.
  (** R′ preserves whether a handle is a bare Thunk. *)
  Lemma R'_preserve_thunk h1 h2 : R' h1 h2 ->
    ((exists t, h1 = Thunk t) <-> (exists t, h2 = Thunk t)).
  Proof.
    intro H; inversion H; subst; try solve [apply R_preserve_thunk; assumption];
      try solve [split; intros [t E]; discriminate]; split; eauto.
  Qed.
  (** A handle R-related from a Blob Object is also a Blob Object. *)
  Lemma R_preserve_blob b h : R (HBlobObj b) h -> exists b', h = HBlobObj b'.
  Proof. intro H; inversion H; subst; eauto. Qed.
  (** A handle R′-related from a Blob Object is also a Blob Object. *)
  Lemma R'_preserve_blob b h : R' (HBlobObj b) h -> exists b', h = HBlobObj b'.
  Proof. intro H; inversion H; subst; eauto using R_preserve_blob. Qed.
  (** A handle R-related from a Tree Object is also a Tree Object. *)
  Lemma R_preserve_tree t h : R (HTreeObj t) h -> exists t', h = HTreeObj t'.
  Proof. intro H; inversion H; subst; eauto. Qed.
  (** A handle R′-related from a Tree Object is also a Tree Object. *)
  Lemma R'_preserve_tree t h : R' (HTreeObj t) h -> exists t', h = HTreeObj t'.
  Proof. intro H; inversion H; subst; eauto using R_preserve_tree. Qed.
  (** A handle R-related from a Blob Ref is also a Blob Ref. *)
  Lemma R_preserve_blob_ref b h : R (HBlobRef b) h -> exists b', h = HBlobRef b'.
  Proof. intro H; inversion H; subst; eauto. Qed.
  (** A handle R′-related from a Blob Ref is also a Blob Ref. *)
  Lemma R'_preserve_blob_ref b h : R' (HBlobRef b) h -> exists b', h = HBlobRef b'.
  Proof. intro H; inversion H; subst; eauto using R_preserve_blob_ref. Qed.
  (** A handle R-related from a Tree Ref is also a Tree Ref. *)
  Lemma R_preserve_tree_ref t h : R (HTreeRef t) h -> exists t', h = HTreeRef t'.
  Proof. intro H; inversion H; subst; eauto. Qed.
  (** A handle R′-related from a Tree Ref is also a Tree Ref. *)
  Lemma R'_preserve_tree_ref t h : R' (HTreeRef t) h -> exists t', h = HTreeRef t'.
  Proof. intro H; inversion H; subst; eauto using R_preserve_tree_ref. Qed.
  (** An Encode that executes to an Object must be strict. *)
  Lemma execute_to_obj_strict e o : execute e = Some (Data (Object o)) -> exists th, e = Strict th.
  Proof.
    destruct e as [th|th]; intro Exec; [eauto|].
    destruct (execute_shallow_to_ref _ _ Exec) as [r E]; discriminate.
  Qed.
  (** An Encode that executes to a Ref must be shallow. *)
  Lemma execute_to_ref_shallow e r : execute e = Some (Data (Ref r)) -> exists th, e = Shallow th.
  Proof.
    destruct e as [th|th]; intro Exec; [|eauto].
    destruct (execute_strict_to_obj _ _ Exec) as [o E]; discriminate.
  Qed.
  (** Executing an Encode cannot return another Encode. *)
  Lemma execute_not_encode e e' : execute e <> Some (Encode e').
  Proof. intro Exec; destruct (execute_data _ _ Exec) as [d E]; discriminate. Qed.
  (** A handle R-related to a Blob Object is either a Blob Object or a strict Encode. *)
  Lemma R_preserve_blob_or_encode_rev b h : R h (HBlobObj b) ->
    (exists b', h = HBlobObj b') \/ (exists th, h = Encode (Strict th)).
  Proof.
    sauto inv: R ctrs: - cases: - use: execute_to_obj_strict.
  Qed.
  (** A handle R′-related to a Blob Object is either a Blob Object or a strict Encode. *)
  Lemma R'_preserve_blob_or_encode_rev b h : R' h (HBlobObj b) ->
    (exists b', h = HBlobObj b') \/ (exists th, h = Encode (Strict th)).
  Proof. intro H; inversion H; subst; eauto using R_preserve_blob_or_encode_rev. Qed.
  (** A handle R-related to a Tree Object is either a Tree Object or a strict Encode. *)
  Lemma R_preserve_tree_or_encode_rev t h : R h (HTreeObj t) ->
    (exists t', h = HTreeObj t') \/ (exists th, h = Encode (Strict th)).
  Proof.
    sauto inv: R ctrs: - cases: - use: execute_to_obj_strict.
  Qed.
  (** A handle R′-related to a Tree Object is either a Tree Object or a strict Encode. *)
  Lemma R'_preserve_tree_or_encode_rev t h : R' h (HTreeObj t) ->
    (exists t', h = HTreeObj t') \/ (exists th, h = Encode (Strict th)).
  Proof. intro H; inversion H; subst; eauto using R_preserve_tree_or_encode_rev. Qed.
  (** A handle R-related to a Blob Ref is either a Blob Ref or a shallow Encode. *)
  Lemma R_preserve_blob_ref_or_encode_rev b h : R h (HBlobRef b) ->
    (exists b', h = HBlobRef b') \/ (exists th, h = Encode (Shallow th)).
  Proof.
    sauto inv: R ctrs: - cases: - use: execute_to_ref_shallow.
  Qed.
  (** A handle R′-related to a Blob Ref is either a Blob Ref or a shallow Encode. *)
  Lemma R'_preserve_blob_ref_or_encode_rev b h : R' h (HBlobRef b) ->
    (exists b', h = HBlobRef b') \/ (exists th, h = Encode (Shallow th)).
  Proof. intro H; inversion H; subst; eauto using R_preserve_blob_ref_or_encode_rev. Qed.
  (** A handle R-related to a Tree Ref is either a Tree Ref or a shallow Encode. *)
  Lemma R_preserve_tree_ref_or_encode_rev t h : R h (HTreeRef t) ->
    (exists t', h = HTreeRef t') \/ (exists th, h = Encode (Shallow th)).
  Proof.
    sauto inv: R ctrs: - cases: - use: execute_to_ref_shallow.
  Qed.
  (** A handle R′-related to a Tree Ref is either a Tree Ref or a shallow Encode. *)
  Lemma R'_preserve_tree_ref_or_encode_rev t h : R' h (HTreeRef t) ->
    (exists t', h = HTreeRef t') \/ (exists th, h = Encode (Shallow th)).
  Proof. intro H; inversion H; subst; eauto using R_preserve_tree_ref_or_encode_rev. Qed.
  (** An Encode R-related to a non-Encode must execute to that handle. *)
  Lemma R_encode_execute e h : R (Encode e) h -> ~(exists e', h = Encode e') -> executes_to e h.
  Proof.
    intros H Nh; inversion H; subst; try solve [exfalso; apply Nh; eauto].
    apply execute_some; assumption.
  Qed.
  (** An Encode R′-related to a non-Encode must execute to that handle. *)
  Lemma R'_encode_execute e h : R' (Encode e) h -> ~(exists e', h = Encode e') -> executes_to e h.
  Proof.
    intros H Nh; inversion H; subst; try solve [exfalso; apply Nh; eauto].
    apply R_encode_execute; assumption.
  Qed.
  (** A non-Encode cannot be R-related to an Encode in the forward direction. *)
  Lemma R_encode_execute_rev_does_not_exist e h : R h (Encode e) -> ~(exists e', h = Encode e') -> False.
  Proof. intros H Nh; inversion H; subst; apply Nh; eauto. Qed.
  (** A non-Encode cannot be R′-related to an Encode in the forward direction. *)
  Lemma R'_encode_execute_rev_does_not_exist e h : R' h (Encode e) -> ~(exists e', h = Encode e') -> False.
  Proof.
    intros H Nh; inversion H; subst; try solve [apply Nh; eauto].
    eapply R_encode_execute_rev_does_not_exist; eauto.
  Qed.
  (** The optional extension of R implies the optional extension of R′. *)
  Lemma rel_opt_R_to_R' x y : rel_opt R x y -> rel_opt R' x y.
  Proof. destruct x, y; cbn [rel_opt]; auto using R_to_R'. Qed.
  (** A relation between strict requests constrains their actual executed Objects. This
      connects the structural relation on requests to the relation needed after the
      evaluator has consumed them. *)
  Lemma R_strict_encode_reasons t1 t2 : R (Encode (Strict t1)) (Encode (Strict t2)) ->
    rel_opt R (execute (Strict t1)) (execute (Strict t2)).
  Proof.
    sauto inv: R ctrs: - use: execute_not_encode, RSelf unfold!: rel_opt.
  Qed.
  (** R-related shallow Encodes have related execution results or shared failure. *)
  Lemma R_shallow_encode_reasons t1 t2 : R (Encode (Shallow t1)) (Encode (Shallow t2)) ->
    rel_opt R (execute (Shallow t1)) (execute (Shallow t2)).
  Proof.
    sauto inv: R ctrs: - use: execute_not_encode, RSelf unfold!: rel_opt.
  Qed.
  (** R′ can justify related strict requests through either their source Thunks or their
      executed results. Evaluation transport must handle both justifications because
      constructor congruence need not expose completed computation. *)
  Lemma R'_strict_encode_reasons t1 t2 : R' (Encode (Strict t1)) (Encode (Strict t2)) ->
    R' (Thunk t1) (Thunk t2) \/ rel_opt R' (execute (Strict t1)) (execute (Strict t2)).
  Proof.
    intro H; inversion H; subst; auto.
    - right; apply rel_opt_R_to_R', R_strict_encode_reasons; assumption.
    - right; repeat match goal with E : execute _ = _ |- _ => rewrite E end; assumption.
  Qed.
  (** R′-related shallow Encodes have R′-related Thunks or execution results, including
      shared failure. *)
  Lemma R'_shallow_encode_reasons t1 t2 : R' (Encode (Shallow t1)) (Encode (Shallow t2)) ->
    R' (Thunk t1) (Thunk t2) \/ rel_opt R' (execute (Shallow t1)) (execute (Shallow t2)).
  Proof.
    intro H; inversion H; subst; auto.
    - right; apply rel_opt_R_to_R', R_shallow_encode_reasons; assumption.
    - right; repeat match goal with E : execute _ = _ |- _ => rewrite E end; assumption.
  Qed.
  (** R does not directly relate Objects to Refs. *)
  Lemma R_object_ref_absent o r : ~R (Data (Object o)) (Data (Ref r)).
  Proof.
    destruct o as [b|t]; intro H;
      [destruct (R_preserve_blob _ _ H) as [b' E]|destruct (R_preserve_tree _ _ H) as [t' E]]; discriminate.
  Qed.
  (** R does not directly relate Refs to Objects. *)
  Lemma R_ref_object_absent r o : ~R (Data (Ref r)) (Data (Object o)).
  Proof.
    destruct r as [b|t]; intro H;
      [destruct (R_preserve_blob_ref _ _ H) as [b' E]|destruct (R_preserve_tree_ref _ _ H) as [t' E]]; discriminate.
  Qed.
  (** R′ does not directly relate Objects to Refs. *)
  Lemma R'_object_ref_absent o r : ~R' (Data (Object o)) (Data (Ref r)).
  Proof.
    destruct o as [b|t]; intro H;
      [destruct (R'_preserve_blob _ _ H) as [b' E]|destruct (R'_preserve_tree _ _ H) as [t' E]]; discriminate.
  Qed.
  (** R′ does not directly relate Refs to Objects. *)
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
  (** R does not directly relate shallow Encodes to strict Encodes. *)
  Lemma R_not_shallow_strict t1 t2 : ~R (Encode (Shallow t1)) (Encode (Strict t2)).
  Proof.
    intro H; inversion H; subst.
    - incompatible_execute; eapply R_ref_object_absent; eassumption.
    - eapply execute_not_encode; eassumption.
  Qed.
  (** R does not directly relate strict Encodes to shallow Encodes. *)
  Lemma R_not_strict_shallow t1 t2 : ~R (Encode (Strict t1)) (Encode (Shallow t2)).
  Proof.
    intro H; inversion H; subst.
    - incompatible_execute; eapply R_object_ref_absent; eassumption.
    - eapply execute_not_encode; eassumption.
  Qed.
  (** R′ does not directly relate shallow Encodes to strict Encodes. *)
  Lemma R'_not_shallow_strict t1 t2 : ~R' (Encode (Shallow t1)) (Encode (Strict t2)).
  Proof.
    intro H; inversion H; subst.
    - eapply R_not_shallow_strict; eassumption.
    - incompatible_execute; eapply R'_ref_object_absent; eassumption.
  Qed.
  (** R′ does not directly relate strict Encodes to shallow Encodes. *)
  Lemma R'_not_strict_shallow t1 t2 : ~R' (Encode (Strict t1)) (Encode (Shallow t2)).
  Proof.
    intro H; inversion H; subst.
    - eapply R_not_strict_shallow; eassumption.
    - incompatible_execute; eapply R'_object_ref_absent; eassumption.
  Qed.
  (** R-related lowered Data has R-related lifted Objects. *)
  Lemma R_lower_to_lift_data d1 d2 : R (lower (Data d1)) (lower (Data d2)) -> R (lift (Data d1)) (lift (Data d2)).
  Proof.
    (* Lowering preserves the underlying Blob or Tree name. For Data, the Object and Ref
        congruence laws therefore allow a relation between the two Refs to be read as a
        relation between the underlying Objects. *)
    apply (lower_to_lift R blob_ref_complete_R tree_ref_complete_R R_preserve_tree_ref R_preserve_blob_ref).
  Qed.
  (** R′-related lifted Objects have R′-related lowered Refs. *)
  Lemma R'_lift_to_lower_data d1 d2 : R' (lift (Data d1)) (lift (Data d2)) -> R' (lower (Data d1)) (lower (Data d2)).
  Proof.
    apply (lift_to_lower R' blob_ref_cong_R' tree_ref_cong_R' R'_preserve_tree R'_preserve_blob).
  Qed.
  (** R-related Encodes have relaxed-R-related forcing results, with agreement on success. *)
  Lemma R_encode_to_force e1 e2 : R (Encode e1) (Encode e2) ->
    rel_opt (relaxed_X R) (force (encode_to_thunk e1)) (force (encode_to_thunk e2)).
  Proof.
    destruct e1 as [t1|t1], e2 as [t2|t2]; cbn [encode_to_thunk]; intro H.
    - apply (strict_force_to_relaxed R).
      pose proof (R_strict_encode_reasons _ _ H) as Exec; rewrite !execute_unfold in Exec; exact Exec.
    - exfalso; exact (R_not_strict_shallow _ _ H).
    - exfalso; exact (R_not_shallow_strict _ _ H).
    - apply (force_to_lift R blob_ref_complete_R tree_ref_complete_R R_preserve_tree_ref R_preserve_blob_ref).
      pose proof (R_shallow_encode_reasons _ _ H) as Exec; rewrite !execute_unfold in Exec; exact Exec.
  Qed.
  (** Every handle is strengthened-R-related to itself. *)
  Lemma strengthen_R_self h : strengthen R h h.
  Proof. destruct h; [apply RSelf|apply RSelf|left; apply RSelf]. Qed.
  (** For non-Data handles, relating their strict forms suffices for the strengthened
      relation. *)
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
  (** Relating an Encode's strict form to lifted Data suffices for the strengthened
      Encode-to-Data relation. *)
  Lemma encode_data_strengthen (X : handle -> handle -> Prop) e d :
    X (Encode (Strict (encode_to_thunk e))) (lift (Data d)) -> strengthen X (Encode e) (Data d).
  Proof. destruct e; exact (fun H => H). Qed.
  (** R-related Thunks either both fail, have strengthened-related Think results, or
      directly unfold from left to right. *)
  Lemma R_thunk_reasons t1 t2 : R (Thunk t1) (Thunk t2) ->
    (think t1 = None /\ think t2 = None) \/
    (exists r1 r2, think t1 = Some r1 /\ think t2 = Some r2 /\ strengthen R r1 r2) \/
    think t1 = Some (Thunk t2) \/ think t1 = Some (Encode (Strict t2)) \/ think t1 = Some (Encode (Shallow t2)).
  Proof.
    (* The evaluator must know more than the fact that the source Thunks are related: it
        needs a justification about their next replies. Most rules supply two related
        replies; direct unfolding instead reaches the other Thunk itself, so there may be
        no corresponding Think step on that side. *)
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
  (** The relaxed relation built from R implies the relaxed relation built from R′. *)
  Lemma relaxed_R_to_R' h1 h2 : relaxed_X R h1 h2 -> relaxed_X R' h1 h2.
  Proof. destruct h1 as [d|t|[t|t]]; apply R_to_R'. Qed.
  (** Optional relaxed-R results are also optional relaxed-R′ results. *)
  Lemma rel_opt_relaxed_R_to_R' x y : rel_opt (relaxed_X R) x y -> rel_opt (relaxed_X R') x y.
  Proof. destruct x, y; cbn [rel_opt]; auto using relaxed_R_to_R'. Qed.
  (** The strengthened relation built from R implies the strengthened relation built from
      R′. *)
  Lemma strengthen_R_to_R' h1 h2 : strengthen R h1 h2 -> strengthen R' h1 h2.
  Proof.
    destruct h1 as [d1|t1|e1], h2 as [d2|t2|e2];
      try apply relaxed_R_to_R'; try apply R_to_R'.
    intros [Thunks|Forces]; [left; apply R_to_R'; exact Thunks|right; apply rel_opt_relaxed_R_to_R'; exact Forces].
  Qed.
  (** R′-related Thunks satisfy the cases required for Think and Force transport. *)
  Lemma R'_thunk_reasons t1 t2 : R' (Thunk t1) (Thunk t2) -> thunk_reasons R' t1 t2.
  Proof.
    intro H; unfold thunk_reasons; inversion H; subst; try solve [eauto 12].
    - match goal with Rel : R (Thunk _) (Thunk _) |- _ =>
        destruct (R_thunk_reasons _ _ Rel) as [None|[[r1 [r2 [T1 [T2 Strength]]]]|[OneThunk|[OneStrict|OneShallow]]]] end;
        eauto 12 using strengthen_R_to_R'.
    - do 4 right; left; eexists; eexists; repeat split; try eassumption.
      eapply squash_strengthen; eassumption.
    - do 4 right; left; eexists; eexists; repeat split; try eassumption.
      apply encode_data_strengthen; assumption.
    - do 4 right; left; eexists; eexists; repeat split; try eassumption.
      match goal with Rel : R' (Encode (Strict ?u)) (Encode (Strict ?v)) |- _ =>
        destruct (R'_strict_encode_reasons u v Rel) as [Thunks|Exec] end.
      + left; exact Thunks.
      + right; apply (strict_force_to_relaxed R'); rewrite !execute_unfold in Exec; exact Exec.
  Qed.
  (** The enlarged relation R′ satisfies the invariants required by the common
      evaluation-transport argument. This packages previously established facts; no new
      runtime assumption is introduced. *)
  Lemma R'_properties : relation_properties R'.
  Proof.
    refine {| structural := {| blob_cong := blob_cong_R';
      tree_cong := tree_cong_R'; tag_cong := tag_cong_R'; tree_complete := tree_tagged_complete_R';
      blob_ref_cong := blob_ref_cong_R';
      tree_ref_cong := tree_ref_cong_R'; blob_ref_complete := blob_ref_complete_R';
      tree_ref_complete := tree_ref_complete_R';
      strict_encode_cong := strict_encode_cong_R'; shallow_encode_cong := shallow_encode_cong_R';
      application_thunk_cong := application_thunk_cong_R';
      selection_thunk_cong := selection_thunk_cong_R'; identification_thunk_cong := identification_thunk_cong_R';
      preserve_tree_ref := R'_preserve_tree_ref;
      preserve_blob_ref := R'_preserve_blob_ref; preserve_tree := R'_preserve_tree;
      preserve_blob := R'_preserve_blob;
      preserve_thunk := R'_preserve_thunk; X_self := R'_refl |};
      preserve_tree_ref_rev := R'_preserve_tree_ref_or_encode_rev;
      preserve_blob_ref_rev := R'_preserve_blob_ref_or_encode_rev;
      preserve_tree_rev := R'_preserve_tree_or_encode_rev;
      preserve_blob_rev := R'_preserve_blob_or_encode_rev; encode_eval := R'_encode_execute;
      execute_result := fun e h Exec => R_to_R' _ _ (REvalStep e h Exec);
      strict_encode_reasons := R'_strict_encode_reasons; shallow_encode_reasons := R'_shallow_encode_reasons;
      not_shallow_strict := R'_not_shallow_strict;
      not_strict_shallow := R'_not_strict_shallow; encode_reverse_absent := R'_encode_execute_rev_does_not_exist;
      related_thunk_reasons := R'_thunk_reasons |}.
  Qed.
  (** Thinking R′-related Thunks gives strengthened-related results or a direct unfolding
      from left to right. *)
  Lemma R'_thunk_think t1 t2 : R' (Thunk t1) (Thunk t2) ->
    rel_opt (strengthen R') (think t1) (think t2) \/
    think t1 = Some (Thunk t2) \/ think t1 = Some (Encode (Strict t2)) \/ think t1 = Some (Encode (Shallow t2)).
  Proof. apply (think_X R' R'_properties). Qed.
  (** Forcing R′-related Thunks gives relaxed-R′-related results or shared failure. *)
  Lemma R'_thunk_force t1 t2 : R' (Thunk t1) (Thunk t2) ->
    rel_opt (relaxed_X R') (force t1) (force t2).
  Proof. apply (forces_X R' R'_properties). Qed.
  (** A single productive unfolding of R. Its recursive premises are exposed
      as Q, so Rocq can check the subsequent cofixpoint's guardedness. *)
  (** An R-step exposes one layer of the coinductive definition while leaving recursive
      child comparisons as a parameter Q. Showing Q ⊆ R_step(Q) is enough to establish Q ⊆
      R: every proposed comparison can keep supplying another justified layer, which is
      the defining greatest-fixed-point property of R. *)
  Inductive R_step (Q : handle -> handle -> Prop) : handle -> handle -> Prop :=
  | StepExisting : forall h1 h2, R h1 h2 -> R_step Q h1 h2
  | StepTree : forall t1 t2, tree_tag t1 = tree_tag t2 -> Forall2 Q (get_tree_raw t1) (get_tree_raw t2) -> R_step Q (HTreeObj t1) (HTreeObj t2)
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

  (** Any proposed relation that can justify one R-layer for each of its pairs is
      contained in R. Recursive premises may stay in the proposed relation, which is what
      makes this principle usable for constructor congruence. *)
  Theorem R_coinduct (Q : handle -> handle -> Prop)
      (Unfold : forall h1 h2, Q h1 h2 -> R_step Q h1 h2) : forall h1 h2, Q h1 h2 -> R h1 h2.
  Proof.
    (* Construct the R evidence one layer at a time. The hypothesis supplies that layer
        immediately; recursive comparisons occur only as premises of the chosen
        constructor. This guardedness is the productive step needed for a coinductive
        proof. *)
    refine (cofix IH (h1 h2 : handle) (Related : Q h1 h2) : R h1 h2 := _
      with IH_list (xs ys : list handle) (Related : Forall2 Q xs ys) : R_handles xs ys := _ for IH).
    (* Each Q pair provides an admissible outer R constructor. Its recursive premises
        remain Q pairs, so the same argument can supply their evidence beneath that
        constructor. *)
    - destruct (Unfold h1 h2 Related).
      + assumption.
      + apply RTreeNodes; [assumption|apply IH_list; assumption].
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
    (* A Tree constructor needs evidence for its whole child list. The mutually
        constructed list evidence makes each child comparison guarded in the same way. *)
    - destruct Related; constructor; [apply IH; assumption|apply IH_list; assumption].
  Qed.
  (** Relaxed-R′-related forcing results give R′-related strict Encodes. *)
  Lemma forces_to_strict_R' t1 t2 : rel_opt (relaxed_X R') (force t1) (force t2) ->
    R' (Encode (Strict t1)) (Encode (Strict t2)).
  Proof.
    intro Related; destruct (force t1) as [h1|] eqn:F1, (force t2) as [h2|] eqn:F2;
      cbn [rel_opt] in Related; try contradiction.
    - destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->].
      eapply R'_encode_some_res; [rewrite execute_unfold; cbn; rewrite F1; reflexivity|
        rewrite execute_unfold; cbn; rewrite F2; reflexivity|exact Related].
    - apply R_to_R', REvalStrictNone; rewrite execute_unfold; cbn; [rewrite F1|rewrite F2]; reflexivity.
  Qed.
  (** An R′ relation between bare Thunks exposes one valid coinductive R step. *)
  Lemma R'_thunk_step t1 t2 : R' (Thunk t1) (Thunk t2) -> R_step R' (Thunk t1) (Thunk t2).
  Proof.
    (* To show that an R′ pair belongs to R, expose what happens next. A pair of matching
        replies supplies an R step immediately. A direct unfolding needs no matching reply
        from the other side: it already reaches that Thunk, exactly as an R single-step
        constructor permits. *)
    intro Related; destruct (R'_thunk_think t1 t2 Related) as [Replies|[OneThunk|[OneStrict|OneShallow]]].
    - destruct (think t1) as [h1|] eqn:T1, (think t2) as [h2|] eqn:T2;
        cbn [rel_opt] in Replies; try contradiction; [|apply StepThunkNone; assumption].
      destruct h1 as [d1|u1|e1], h2 as [d2|u2|e2].
      + eapply StepThunkData; eassumption.
      + exfalso; exact (data_not_thunk R' R'_properties (lift_data d1) u2 Replies).
      + exfalso; exact (data_not_encode R' R'_properties (lift_data d1) e2 Replies).
      + exfalso; exact (thunk_not_data R' R'_properties u1 (lift_data d2) Replies).
      + eapply StepThunkNotData; [eassumption|eassumption|intros [d E]; discriminate|
          intros [d E]; discriminate|exact Replies].
      + eapply StepThunkNotData; [eassumption|eassumption|intros [d E]; discriminate|
          intros [d E]; discriminate|destruct e2; exact Replies].
      + eapply StepThunkEncodeData; [eassumption|eassumption|destruct e1; exact Replies].
      + eapply StepThunkNotData; [eassumption|eassumption|intros [d E]; discriminate|
          intros [d E]; discriminate|destruct e1; exact Replies].
      (* For two Encode replies, related source Thunks and related completed forces are
          alternative justifications. In the latter case, strict execution exposes the
          Objects that R must compare. *)
      + destruct Replies as [Thunks|Forces].
        * eapply StepThunkNotData; [eassumption|eassumption|intros [d E]; discriminate|
            intros [d E]; discriminate|destruct e1, e2; exact Thunks].
        * eapply StepThunkEncodeEncode; [eassumption|eassumption|apply forces_to_strict_R'; exact Forces].
    - apply StepSingleThunk; exact OneThunk.
    - apply StepSingleStrict; exact OneStrict.
    - apply StepSingleShallow; exact OneShallow.
  Qed.
  (** Relaxed-R′-related forcing results give R′-related strict execution results. *)
  Lemma R'_force_execute_strict t1 t2 : rel_opt (relaxed_X R') (force t1) (force t2) ->
    rel_opt R' (execute (Strict t1)) (execute (Strict t2)).
  Proof.
    intro Related; rewrite !execute_unfold; cbn.
    destruct (force t1) as [h1|] eqn:F1, (force t2) as [h2|] eqn:F2;
      cbn [rel_opt omap] in *; try contradiction; [|exact I].
    destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->]; exact Related.
  Qed.
  (** Relaxed-R′-related forcing results give R′-related shallow execution results. *)
  Lemma R'_force_execute_shallow t1 t2 : rel_opt (relaxed_X R') (force t1) (force t2) ->
    rel_opt R' (execute (Shallow t1)) (execute (Shallow t2)).
  Proof.
    intro Related; rewrite !execute_unfold; cbn.
    destruct (force t1) as [h1|] eqn:F1, (force t2) as [h2|] eqn:F2;
      cbn [rel_opt omap] in *; try contradiction; [|exact I].
    destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->].
    apply R'_lift_to_lower_data; exact Related.
  Qed.
  (** R′-related Thunks give a valid coinductive step between their strict Encodes. *)
  Lemma R'_strict_step t1 t2 : R' (Thunk t1) (Thunk t2) ->
    R_step R' (Encode (Strict t1)) (Encode (Strict t2)).
  Proof.
    (* The constructor rule for strict Encodes in R′ must be justified using R's
        execution-based rule. Related forcing behavior supplies related Objects on success
        and shared absence of a result otherwise; these are exactly the two cases
        available in R. *)
    intro Related.
    pose proof (R'_thunk_force t1 t2 Related) as ForcedResultsRelated.
    pose proof (R'_force_execute_strict t1 t2 ForcedResultsRelated) as ExecutedResultsRelated.
    destruct (execute (Strict t1)) as [h1|] eqn:E1, (execute (Strict t2)) as [h2|] eqn:E2;
      cbn [rel_opt] in ExecutedResultsRelated; try contradiction.
    - eapply StepExecuteSome; eassumption.
    - apply StepStrictNone; assumption.
  Qed.
  (** R′-related Thunks give a valid coinductive step between their shallow Encodes. *)
  Lemma R'_shallow_step t1 t2 : R' (Thunk t1) (Thunk t2) ->
    R_step R' (Encode (Shallow t1)) (Encode (Shallow t2)).
  Proof.
    (* The same argument is made through Refs for a shallow consumer. Ref congruence
        transfers the relation on the underlying Objects to the actual shallow execution
        results. *)
    intro Related.
    pose proof (R'_thunk_force t1 t2 Related) as ForcedResultsRelated.
    pose proof (R'_force_execute_shallow t1 t2 ForcedResultsRelated) as ExecutedResultsRelated.
    destruct (execute (Shallow t1)) as [h1|] eqn:E1, (execute (Shallow t2)) as [h2|] eqn:E2;
      cbn [rel_opt] in ExecutedResultsRelated; try contradiction.
    - eapply StepExecuteSome; eassumption.
    - apply StepShallowNone; assumption.
  Qed.
  (** Every R′ pair exposes one valid coinductive R step. *)
  Lemma R'_unfold h1 h2 : R' h1 h2 -> R_step R' h1 h2.
  Proof.
    (* An R′-specific constructor cannot be copied blindly into R. The Thunk and Encode
        lemmas above replace those extra constructors by evidence about actual Think or
        execution behavior, leaving only admissible R layers. *)
    intro Related; inversion Related; subst;
      eauto using StepExisting, StepTree, StepTreeRef, StepThunkData, StepThunkNotData,
        StepThunkEncodeData, StepThunkEncodeEncode, StepExecuteSome,
        Forall2_R'orR_to_R', R'_thunk_step, R'_strict_step, R'_shallow_step.
  Qed.
  (** The extra congruence rules in R′ are admissible in R. Thus the relation used to
      obtain compositional evaluation arguments makes no additional semantic
      identifications. *)
  Theorem R'_impl_R h1 h2 : R' h1 h2 -> R h1 h2.
  Proof.
    (* Use R′ itself as the candidate relation in the coinduction principle. Its
        unfolding theorem supplies the required R layer for every candidate pair,
        completing the argument that the temporary enlargement added no new pairs. *) apply (R_coinduct R' R'_unfold). Qed.

  (** R satisfies the structural laws and evaluator transport requirements. *)
  Lemma R_properties : relation_properties R.
  Proof.
    (* R′ already supports constructor congruence. Since R and R′ have now been shown to
        contain one another, those established congruence facts can be reused for R
        instead of reproving evaluation case by case. *)
    refine {| structural := {| blob_cong := blob_cong_R;
      tree_cong := tree_cong_R; tag_cong := tag_cong_R; tree_complete := tree_tagged_complete_R;
      blob_ref_cong := blob_ref_cong_R;
      tree_ref_cong := tree_ref_cong_R; blob_ref_complete := blob_ref_complete_R;
      tree_ref_complete := tree_ref_complete_R;
      preserve_tree_ref := R_preserve_tree_ref; preserve_blob_ref := R_preserve_blob_ref;
      preserve_tree := R_preserve_tree;
      preserve_blob := R_preserve_blob; preserve_thunk := R_preserve_thunk; X_self := RSelf |};
      preserve_tree_ref_rev := R_preserve_tree_ref_or_encode_rev;
      preserve_blob_ref_rev := R_preserve_blob_ref_or_encode_rev;
      preserve_tree_rev := R_preserve_tree_or_encode_rev;
      preserve_blob_rev := R_preserve_blob_or_encode_rev; encode_eval := R_encode_execute;
      execute_result := REvalStep;
      not_shallow_strict := R_not_shallow_strict; not_strict_shallow := R_not_strict_shallow;
      encode_reverse_absent := R_encode_execute_rev_does_not_exist |}.
    - intros t1 t2 Related; apply R'_impl_R, strict_encode_cong_R', R_to_R'; exact Related.
    - intros t1 t2 Related; apply R'_impl_R, shallow_encode_cong_R', R_to_R'; exact Related.
    - intros t1 t2 Related; apply R'_impl_R, application_thunk_cong_R', R_to_R'; exact Related.
    - intros t1 t2 Related; apply R'_impl_R, selection_thunk_cong_R', R_to_R'; exact Related.
    - intros d1 d2 Related; apply R'_impl_R, identification_thunk_cong_R', R_to_R'; exact Related.
    - intros t1 t2 Related; right; apply R_strict_encode_reasons; exact Related.
    - intros t1 t2 Related; right; apply R_shallow_encode_reasons; exact Related.
    - intros t1 t2 Related; unfold thunk_reasons; do 3 right; apply R_thunk_reasons; exact Related.
  Qed.
  (** One generating justification between inputs survives evaluation. This is the
      computational part of equivalence preservation: Closure.v needs only to join and
      reverse such local justifications afterward. *)
  Theorem eval_R h1 h2 : R h1 h2 -> rel_opt R (eval h1) (eval h2).
  Proof. apply (evals_X R R_properties). Qed.
  (** Related source Thunks agree on whether Force has a result, and successful results
      name related underlying Objects. The comparison is lifted so it remains usable by
      both strict and shallow consumers. *)
  Theorem force_R t1 t2 : R (Thunk t1) (Thunk t2) -> rel_opt (relaxed_X R) (force t1) (force t2).
  Proof. apply (forces_X R R_properties). Qed.
  (** A lowered handle cannot be an Object. *)
  Lemma lower_not_obj h h' : h = lower h' -> ~(exists o, h = Data (Object o)).
  Proof.
    intros -> [o E]; destruct h' as [[[b|t]|[b|t]]|th|[th|th]]; discriminate.
  Qed.
  (** A lifted handle cannot be a Ref. *)
  Lemma lift_not_ref h h' : h = lift h' -> ~(exists r, h = Data (Ref r)).
  Proof.
    intros -> [r E]; destruct h' as [[[b|t]|[b|t]]|th|[th|th]]; discriminate.
  Qed.
End Equivalence.
