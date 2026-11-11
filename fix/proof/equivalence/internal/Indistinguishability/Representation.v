Import ListNotations.
  CoInductive representation : handle -> handle -> Prop :=
  | RepresentBlob b1 b2 : get_blob_data b1 = get_blob_data b2 -> representation (HBlobObj b1) (HBlobObj b2)
  | RepresentBlobRef b1 b2 : get_blob_data b1 = get_blob_data b2 -> representation (HBlobRef b1) (HBlobRef b2)
  | RepresentTree t1 t2 : tree_tag t1 = tree_tag t2 ->
      tree_comparable t1 = tree_comparable t2 -> tree_value t1 = tree_value t2 ->
      represented_children (get_tree_raw t1) (get_tree_raw t2) -> representation (HTreeObj t1) (HTreeObj t2)
  | RepresentTreeRef t1 t2 : representation (HTreeObj t1) (HTreeObj t2) -> representation (HTreeRef t1) (HTreeRef t2)
  | RepresentApplication t1 t2 : representation (HTreeObj t1) (HTreeObj t2) ->
      representation (Thunk (Application t1)) (Thunk (Application t2))
  | RepresentSelection t1 t2 : representation (HTreeObj t1) (HTreeObj t2) ->
      representation (Thunk (Selection t1)) (Thunk (Selection t2))
  | RepresentIdentification r1 r2 : representation (Data (Ref r1)) (Data (Ref r2)) ->
      representation (Thunk (Identification r1)) (Thunk (Identification r2))
  | RepresentStrict t1 t2 : representation (Thunk t1) (Thunk t2) -> representation (Encode (Strict t1)) (Encode (Strict t2))
  | RepresentShallow t1 t2 : representation (Thunk t1) (Thunk t2) -> representation (Encode (Shallow t1)) (Encode (Shallow t2))
  | RepresentBehavior t1 t2 :
      represented_results (omap lower (force t1)) (omap lower (force t2)) ->
      representation (Thunk t1) (Thunk t2)
  with represented_children : list handle -> list handle -> Prop :=
  | RepresentNil : represented_children [] []
  | RepresentCons x y xs ys : representation x y -> represented_children xs ys -> represented_children (x :: xs) (y :: ys)
  with represented_results : option handle -> option handle -> Prop :=
  | RepresentNone : represented_results None None
  | RepresentSome x y : representation x y -> represented_results (Some x) (Some y).

  Lemma representation_refl : forall h, representation h h.
  Proof.
    refine (cofix Handles h : representation h h := _
      with Children xs : represented_children xs xs := _ for Handles).
    - destruct h as [[[b|t]|[b|t]]|[t|r|t]|[t|t]];
        constructor; auto using Handles, Children.
    - destruct xs; constructor; auto.
  Qed.

  Lemma represented_Forall2 xs ys : represented_children xs ys <-> Forall2 representation xs ys.
  Proof.
    split.
    - revert ys; induction xs; intros ys Related; inversion Related; subst; constructor; auto.
    - induction 1; constructor; assumption.
  Qed.

  Lemma representation_comparable left right : representation left right -> is_comparable left = is_comparable right.
  Proof.
    intro Related; destruct Related; cbn [HBlobObj HBlobRef HTreeObj HTreeRef is_comparable];
      try reflexivity; try assumption.
    match goal with Same : representation (HTreeObj _) (HTreeObj _) |- _ => inversion Same; assumption end.
  Qed.

  Lemma representation_value left right : representation left right -> is_value left = is_value right.
  Proof.
    intro Related; destruct Related; cbn [HBlobObj HBlobRef HTreeObj HTreeRef is_value]; auto.
  Qed.

  Lemma representation_tree tag xs ys : Forall2 representation xs ys ->
    representation (HTreeObj (create_tree_tagged tag xs)) (HTreeObj (create_tree_tagged tag ys)).
  Proof.
    intro Children; constructor.
    - rewrite !tree_tag_create_tree_tagged; reflexivity.
    - rewrite !tree_comparable_create_tree_tagged; induction Children; cbn;
        [reflexivity|rewrite (representation_comparable _ _ H), IHChildren; reflexivity].
    - rewrite !tree_value_create_tree_tagged; induction Children; cbn;
        [reflexivity|rewrite (representation_value _ _ H), IHChildren; reflexivity].
    - rewrite !get_tree_raw_create_tree_tagged; apply represented_Forall2; exact Children.
  Qed.

  Lemma representation_tree_children first second : representation (HTreeObj first) (HTreeObj second) ->
    Forall2 representation (get_tree_raw first) (get_tree_raw second).
  Proof. intro Related; inversion Related; apply represented_Forall2; assumption. Qed.

  Lemma representation_shape left right : representation left right -> get_type left = get_type right.
  Proof. intro Related; destruct Related; reflexivity. Qed.

  Lemma representation_typed : forall left right, representation left right -> same_typed_handle left right.
  Proof.
    assert (Trees : forall first second, representation (HTreeObj first) (HTreeObj second) -> same_typed_tree first second).
    { intro first; induction first as [first IH] using (well_founded_induction wfp_tree_child).
      intros second Related; constructor.
      - inversion Related; assumption.
      - pose proof (representation_tree_children _ _ Related) as Children.
        assert (Members : forall x y, In x (get_tree_raw first) -> representation x y -> same_typed_handle x y).
        { intros x y Present Same; destruct Same; constructor.
          apply IH; [exact Present|constructor; assumption]. }
        clear - Children Members.
        induction Children; constructor; [apply Members; [left; reflexivity|assumption]|].
        apply IHChildren; intros x0 y0 Present Same; apply Members; [right; exact Present|exact Same]. }
    intros left right Related; destruct Related; constructor; apply Trees; constructor; assumption.
  Qed.

  Lemma representation_laws : congruence representation.
  Proof.
    constructor; intros.
    - inversion H; assumption.
    - apply representation_tree_children; assumption.
    - inversion H; assumption.
    - apply representation_tree; assumption.
    - inversion H; constructor; assumption.
    - constructor; assumption.
    - inversion H; constructor; assumption.
    - inversion H; assumption.
    - constructor; assumption.
    - constructor; assumption.
    - constructor; assumption.
    - constructor; assumption.
    - constructor; assumption.
    - inversion H; subst; eexists; reflexivity.
    - inversion H; subst; eexists; reflexivity.
    - inversion H; subst; eexists; reflexivity.
    - inversion H; subst; eexists; reflexivity.
    - inversion H; subst; split; intros [th Equal]; try discriminate; eauto.
    - apply representation_refl.
  Qed.

  Lemma representation_apply first second : representation (HTreeObj first) (HTreeObj second) ->
    rel_opt representation (A.apply_tree first) (A.apply_tree second).
  Proof.
    intro Related; apply (proj1 (A.apply_tree_X representation representation_laws _ _
      (conj Related (representation_typed _ _ Related)))).
  Qed.

  Lemma representation_sym : forall left right, representation left right -> representation right left.
  Proof.
    refine (cofix Handles left right (Related : representation left right) : representation right left := _
      with Children xs ys (Related : represented_children xs ys) : represented_children ys xs := _
      with Results first second (Related : represented_results first second) : represented_results second first := _ for Handles).
    - destruct Related; constructor; eauto using Handles, Children, Results; congruence.
    - destruct Related; constructor; eauto.
    - destruct Related; constructor; eauto.
  Qed.

  Lemma represented_results_iff first second : represented_results first second <-> rel_opt representation first second.
  Proof.
    destruct first, second; cbn [rel_opt]; split; intro Related;
      try solve [inversion Related|constructor; assumption|constructor|exact I].
    inversion Related; assumption.
  Qed.

  Lemma representation_lower left right : representation left right -> representation (lower left) (lower right).
  Proof.
    intro Related; destruct Related; cbn [lower lower_data HBlobObj HBlobRef HTreeObj HTreeRef];
      try solve [constructor; assumption].
    constructor; constructor; assumption.
  Qed.

  Lemma representation_lift left right : representation left right -> representation (lift left) (lift right).
  Proof.
    intro Related; destruct Related; cbn [lift lift_data HBlobObj HBlobRef HTreeObj HTreeRef];
      try solve [constructor; assumption].
    assumption.
  Qed.

  Lemma representation_lift_lower first second : representation (lower (Data first)) (lower (Data second)) ->
    representation (lift (Data first)) (lift (Data second)).
  Proof.
    intro Related; pose proof (representation_lift _ _ Related) as Lifted.
    destruct first as [[b|t]|[b|t]], second as [[b'|t']|[b'|t']]; exact Lifted.
  Qed.
