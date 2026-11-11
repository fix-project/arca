  (** Evaluating an author list cannot remove a requirement imposed by an original Value
      author: that author is unchanged and still occurs at the same position in the
      result. *)
  Lemma eval_preserves_stable_requirements (requirement : handle -> Prop) t u :
    eval (HTreeObj t) = Some (HTreeObj u) ->
    Forall (fun h => value_handle h -> requirement h) (get_tree_raw u) ->
    Forall (fun h => value_handle h -> requirement h) (get_tree_raw t).
  Proof.
    (* Match each original entry with its evaluated entry. Whenever the original is a
        Value, evaluation fixes it, so the requirement already known for that result
        applies to the original as well. Non-Value entries impose no stable-author
        requirement. *)
    intros Evaluated Requirements.
    apply eval_tree_iff in Evaluated as (_ & Children).
    revert Requirements; induction Children;
      sauto inv: Forall ctrs: Forall cases: - use: value_handle_eval_to_itself.
  Qed.

  (** The Tree evaluator leaves a Value Tree unchanged. *)
  Lemma value_tree_fixed_eval t : value_handle (HTreeObj t) -> eval_tree t = Some t.
  Proof.
    intro Value; pose proof (value_handle_eval_to_itself _ Value) as Ev.
    rewrite eval_tree_handle in Ev; apply omap_some in Ev; destruct Ev as [u [Tree E']].
    inversion E'; subst u; exact Tree.
  Qed.

  Definition same_shape h1 h2 := get_type h1 = get_type h2.
  (** Handles of the same shape have the same observable type. *)
  Lemma same_shape_get_type h1 h2 : same_shape h1 h2 -> get_type h1 = get_type h2.
  Proof. auto. Qed.
  (** A pointwise relation between Value lists can be strengthened using a rule for each
      corresponding child pair. *)
  Lemma Forall2_value_transform (X Y : handle -> handle -> Prop) xs ys :
    Forall2 X xs ys ->
    (forall x y, In x xs -> X x y -> value_handle x -> value_handle y -> Y x y) ->
    Forall value_handle xs -> Forall value_handle ys -> Forall2 Y xs ys.
  Proof.
    intro R; induction R; intros Safe V1 V2; [constructor|].
    inversion V1; inversion V2; subst; constructor.
    - apply Safe; simpl; auto.
    - apply IHR; auto. intros a b InTail; apply Safe; simpl; auto.
  Qed.

  Section ValueTypes.
    Variable X : handle -> handle -> Prop.
    (** Related Tree Objects have pointwise related children. *)
    Hypothesis tree_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> Forall2 X (get_tree_raw t1) (get_tree_raw t2).
    (** Related Tree Objects have the same Tag marker. *)
    Hypothesis tag_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> tree_tag t1 = tree_tag t2.
    (** Related non-Encode handles have the same shape. *)
    Hypothesis X_preserve_value_handle : forall h1 h2, X h1 h2 ->
      ~(exists e, h1 = Encode e) -> ~(exists e, h2 = Encode e) -> same_shape h1 h2.

    (** Related Value Trees have matching recursive handle types. *)
    Lemma value_tree_to_sametypedness t1 : value_tree t1 -> forall t2,
      X (HTreeObj t1) (HTreeObj t2) -> value_tree t2 -> same_typed_tree t1 t2.
    Proof.
      (* Program execution is allowed to inspect handle constructors and Tree positions.
          Before using relational congruence, related Value inputs must therefore have
          corresponding constructors, recursively through Object Trees. Well-founded
          Object containment lets the outer Tree fact be assembled from the child facts. *)
      revert t1; refine (well_founded_induction wfp_tree_child
        (fun t1 => value_tree t1 -> forall t2, X (HTreeObj t1) (HTreeObj t2) -> value_tree t2 -> same_typed_tree t1 t2) _).
      intros t1 IH V1 t2 R V2; apply tree; [apply tag_cong; exact R|].
      inversion V1 as [t Values1]; inversion V2 as [u Values2]; subst.
      eapply (Forall2_value_transform X same_typed_handle); [apply tree_cong; exact R| |exact Values1|exact Values2].
      intros x y Member HX VX VY.
      pose proof (same_shape_get_type x y
        (X_preserve_value_handle x y HX (value_handle_not_encode x VX) (value_handle_not_encode y VY))) as Types.
      destruct x as [[[b|t]|[b|t]]|th|e];
        destruct y as [[[b'|t']|[b'|t']]|th'|e'];
        cbn [get_type] in Types; try discriminate;
        try solve [constructor];
        try solve [exfalso; apply (value_handle_not_encode _ VX); eauto].
      apply tree_obj; apply IH; [exact Member| |exact HX|];
        inversion VX; inversion VY; subst; assumption.
    Qed.
    (** Related Values have matching handle types. *)
    Lemma value_handle_to_sametypedness h1 h2 :
      X h1 h2 -> value_handle h1 -> value_handle h2 -> same_typed_handle h1 h2.
    Proof.
      intros R V1 V2.
      pose proof (same_shape_get_type h1 h2
        (X_preserve_value_handle h1 h2 R (value_handle_not_encode h1 V1) (value_handle_not_encode h2 V2))) as Types.
      destruct h1 as [[[b|t]|[b|t]]|th|e];
        destruct h2 as [[[b'|t']|[b'|t']]|th'|e'];
        cbn [get_type] in Types; try discriminate;
        try solve [constructor];
        try solve [exfalso; apply (value_handle_not_encode _ V1); eauto].
      apply tree_obj; apply value_tree_to_sametypedness; [|exact R|];
        inversion V1; inversion V2; subst; assumption.
    Qed.
  End ValueTypes.
