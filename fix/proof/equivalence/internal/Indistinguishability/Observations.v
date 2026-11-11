  (** Every handle has the same observations as itself. *)
  Lemma observations_refl : forall h, same_observations h h.
  Proof.
    (* Observations can recurse through Tree children, Refs, and returned computations.
        Supply a matching outer observation before asking for matching observations
        recursively; this is why the argument is coinductive rather than a finite
        computation of all observations. *)
    refine (cofix Handles (h : handle) : same_observations h h := _
      with Children (xs : list handle) : observed_children xs xs := _
      with Results (result : option handle) : observed_result result result := _ for Handles).
    - destruct h as [[[b|t]|[b|t]]|th|[th|th]];
        constructor; auto using Children, Results.
    - destruct xs; constructor; auto.
    - destruct result; constructor; auto.
  Qed.

  (** Having the same observations is symmetric. *)
  Lemma observations_sym : forall h1 h2,
    same_observations h1 h2 -> same_observations h2 h1.
  Proof.
    refine (cofix Handles h1 h2 (Related : same_observations h1 h2) :
        same_observations h2 h1 := _
      with Children xs ys (Related : observed_children xs ys) :
        observed_children ys xs := _
      with Results first second (Related : observed_result first second) :
        observed_result second first := _ for Handles).
    - destruct Related; constructor; auto using Children, Results; congruence.
    - destruct Related; constructor; auto.
    - destruct Related; constructor; auto.
  Qed.

  (** Handles with the same observations have the same observable handle type. *)
  Lemma observations_type h1 h2 : same_observations h1 h2 ->
    get_type h1 = get_type h2.
  Proof. intro Related; destruct Related; reflexivity. Qed.

  (** Handles with the same observations have the same Object/Ref/Thunk/Encode
      classification. *)
  Lemma observations_value_type h1 h2 : same_observations h1 h2 ->
    value_type h1 = value_type h2.
  Proof. intro Related; destruct Related; reflexivity. Qed.

  (** Having the same observations is transitive.

      Compose matching observations coinductively, using transitivity for scalar fields and
      the recursive argument for children and results. *)
  Lemma observations_trans : forall h1 h2 h3,
    same_observations h1 h2 -> same_observations h2 h3 -> same_observations h1 h3.
  Proof.
    (* The middle handle fixes the observation constructor shared by both premises. Its
        scalar observations join by equality, and its children or optional computation
        results become the common middle observations for the recursive comparisons. *)
    refine (cofix Handles h1 h2 h3 (Left : same_observations h1 h2)
        (Right : same_observations h2 h3) : same_observations h1 h3 := _
      with Children xs ys zs (Left : observed_children xs ys)
        (Right : observed_children ys zs) : observed_children xs zs := _
      with Results first middle last (Left : observed_result first middle)
        (Right : observed_result middle last) : observed_result first last := _ for Handles).
    - destruct Left; inversion Right; subst; constructor;
        eauto using Children, Results; congruence.
    - destruct Left; inversion Right; subst; constructor; eauto.
    - destruct Left; inversion Right; subst; constructor; eauto.
  Qed.

  (** Handles with the same observations agree on Comparability. *)
  Lemma observations_comparable h1 h2 : same_observations h1 h2 ->
    is_comparable h1 = is_comparable h2.
  Proof.
    intro Related; destruct Related; try reflexivity; cbn [HBlobObj HBlobRef HTreeObj HTreeRef is_comparable];
      [assumption|match goal with H : same_observations _ _ |- _ => inversion H; assumption end].
  Qed.

  (** Handles with the same observations agree on full evaluation. *)
  Lemma observations_value h1 h2 : same_observations h1 h2 ->
    is_value h1 = is_value h2.
  Proof.
    enough (Trees : forall t1 t2, same_observations (HTreeObj t1) (HTreeObj t2) ->
      tree_value t1 = tree_value t2).
    { intro Related; destruct Related; try reflexivity.
      apply Trees; constructor; assumption. }
    intro t1; induction t1 as [t1 IH] using (well_founded_induction wfp_tree_child).
    intros t2 Related; inversion Related as [| |left right Tag Comparable Children| | | |]; subst.
    rewrite !tree_value_contents.
    assert (Members : forall x, In x (get_tree_raw t1) -> forall y,
      same_observations x y -> is_value x = is_value y).
    { intros x Member y Same; revert Member; destruct Same; intro Member; try reflexivity.
      apply IH; [exact Member|constructor; assumption]. }
    clear - Children Members.
    revert Members Children; generalize (get_tree_raw t2) as ys;
      generalize (get_tree_raw t1) as xs.
    induction xs as [|x xs Rest]; intros ys Members Children;
      inversion Children as [|x' y xs' ys' Same Others]; subst; [reflexivity|].
    cbn [forallb]; rewrite (Members x (or_introl eq_refl) y Same).
    rewrite (Rest ys'); [reflexivity| |exact Others].
    intros z Member w Related; exact (Members z (or_intror Member) w Related).
  Qed.

  (** Pointwise observationally matching child lists have the same length. *)
  Lemma observed_children_length xs ys : observed_children xs ys -> length xs = length ys.
  Proof. revert ys; induction xs; intros ys Related; inversion Related; subst; simpl; auto. Qed.

  (** Indistinguishable Data has the same length observation. For Blobs this follows from
      byte equality; for Trees it follows from the correspondence of children, so no
      identity of Tree names is needed. *)
  Lemma observations_length d1 d2 : same_observations (Data d1) (Data d2) ->
    data_length d1 = data_length d2.
  Proof.
    intro Related; inversion Related; subst; cbn [data_length].
    - unfold get_blob_size; congruence.
    - unfold get_blob_size; congruence.
    - apply observed_children_length; assumption.
    - match goal with H : same_observations _ _ |- _ => inversion H; subst end.
      apply observed_children_length; assumption.
  Qed.

  (** Lowering Data and Encodes preserves matching observations. *)
  Lemma observations_lower h1 h2 : same_observations h1 h2 ->
    same_observations (lower h1) (lower h2).
  Proof.
    intro Related; destruct Related; cbn [lower HBlobObj HBlobRef HTreeObj HTreeRef];
      try (constructor; assumption).
    constructor; constructor; assumption.
  Qed.

  (** Lifting Data and Encodes preserves matching observations. *)
  Lemma observations_lift h1 h2 : same_observations h1 h2 ->
    same_observations (lift h1) (lift h2).
  Proof.
    intro Related; destruct Related; cbn [lift HBlobObj HBlobRef HTreeObj HTreeRef];
      try (constructor; assumption).
    assumption.
  Qed.

  (** Observationally matching Encodes execute to observationally matching results or both
      fail. *)
  Lemma observations_execute e1 e2 : same_observations (Encode e1) (Encode e2) ->
    observed_result (execute e1) (execute e2).
  Proof. intro Related; inversion Related; subst; assumption. Qed.

  (** Strict Encode construction preserves matching observations of bare Thunks. *)
  Lemma observations_encode_strict th1 th2 :
    same_observations (Thunk th1) (Thunk th2) ->
    same_observations (Encode (Strict th1)) (Encode (Strict th2)).
  Proof.
    intro Related; inversion Related; subst; constructor.
    rewrite !execute_unfold; cbn [encode_to_thunk].
    match goal with H : observed_result _ _ |- _ => destruct H end;
      constructor; apply observations_lift; assumption.
  Qed.

  (** Shallow Encode construction preserves matching observations of bare Thunks. *)
  Lemma observations_encode_shallow th1 th2 :
    same_observations (Thunk th1) (Thunk th2) ->
    same_observations (Encode (Shallow th1)) (Encode (Shallow th2)).
  Proof.
    intro Related; inversion Related; subst; constructor.
    rewrite !execute_unfold; cbn [encode_to_thunk].
    match goal with H : observed_result _ _ |- _ => destruct H end;
      constructor; apply observations_lower; assumption.
  Qed.

  (** Indistinguishability is reflexive. *)
  Lemma indistinguishable_refl h : indistinguishable h h.
  Proof. split; [apply equiv_refl|apply observations_refl]. Qed.

  (** Indistinguishability is symmetric. *)
  Lemma indistinguishable_sym h1 h2 : indistinguishable h1 h2 -> indistinguishable h2 h1.
  Proof. intros [Equivalent Observed]; split; [apply equiv_sym|apply observations_sym]; assumption. Qed.

  (** Indistinguishable handles are computationally equivalent. *)
  Lemma indistinguishable_equivalent h1 h2 : indistinguishable h1 h2 -> computational_equiv h1 h2.
  Proof.
    unfold indistinguishable; tauto.
  Qed.

  (** Indistinguishability is transitive. *)
  Lemma indistinguishable_trans h1 h2 h3 :
    indistinguishable h1 h2 -> indistinguishable h2 h3 -> indistinguishable h1 h3.
  Proof. unfold indistinguishable; intuition eauto using equiv_trans, observations_trans. Qed.

  (** Indistinguishable handles agree on Comparability. *)
  Lemma indistinguishable_comparable h1 h2 : indistinguishable h1 h2 ->
    is_comparable h1 = is_comparable h2.
  Proof. intros [_ Observed]; apply observations_comparable; exact Observed. Qed.
