(** Models observable handle properties alongside computational equivalence.
    Proves that Comparable equivalents are indistinguishable and that the
    modeled observations are preserved. *)

From Stdlib Require Import List Bool ClassicalDescription.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Equivalence Require Import Closure.

Module Indistinguishability (S : STORAGE) (P : PROGRAM S).
  Module EC := EquivalenceClosure S P.
  Import EC EC.E EC.E.EP EC.E.EP.E EC.E.EP.E.H.

  Definition data_length d := match d with
    | Object (BlobObj b) | Ref (BlobRef b) => get_blob_size b
    | Object (TreeObj t) | Ref (TreeRef t) => get_tree_size t
    end.

  (** Computational equivalence justifies replacing computations, but an API observation
      may still see their packaging: an Encode and the Object it computes have different
      types. Matching observations separately records constructor, content, metadata, and
      the observations of future computation results. Indistinguishability requires both
      this observation agreement and computational equivalence. *)
  CoInductive same_observations : handle -> handle -> Prop :=
  | ObserveBlob : forall b1 b2,
      get_blob_data b1 = get_blob_data b2 ->
      same_observations (HBlobObj b1) (HBlobObj b2)
  | ObserveBlobRef : forall b1 b2,
      get_blob_data b1 = get_blob_data b2 ->
      same_observations (HBlobRef b1) (HBlobRef b2)
  | ObserveTree : forall t1 t2,
      tree_tag t1 = tree_tag t2 ->
      tree_comparable t1 = tree_comparable t2 ->
      observed_children (get_tree_raw t1) (get_tree_raw t2) ->
      same_observations (HTreeObj t1) (HTreeObj t2)
  | ObserveTreeRef : forall t1 t2,
      same_observations (HTreeObj t1) (HTreeObj t2) ->
      same_observations (HTreeRef t1) (HTreeRef t2)
  | ObserveThunk : forall th1 th2,
      observed_result (force th1) (force th2) ->
      same_observations (Thunk th1) (Thunk th2)
  | ObserveStrict : forall th1 th2,
      observed_result (execute (Strict th1)) (execute (Strict th2)) ->
      same_observations (Encode (Strict th1)) (Encode (Strict th2))
  | ObserveShallow : forall th1 th2,
      observed_result (execute (Shallow th1)) (execute (Shallow th2)) ->
      same_observations (Encode (Shallow th1)) (Encode (Shallow th2))
  with observed_children : list handle -> list handle -> Prop :=
  | ObserveNil : observed_children nil nil
  | ObserveCons : forall h1 h2 xs ys,
      same_observations h1 h2 ->
      observed_children xs ys ->
      observed_children (h1 :: xs) (h2 :: ys)
  with observed_result : option handle -> option handle -> Prop :=
  | ObserveNone : observed_result None None
  | ObserveSome : forall h1 h2,
      same_observations h1 h2 ->
      observed_result (Some h1) (Some h2).

  Definition indistinguishable h1 h2 := computational_equiv h1 h2 /\ same_observations h1 h2.

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

  Definition compare_handles left right :=
    if is_comparable left && is_comparable right then
      Some (if excluded_middle_informative (indistinguishable left right)
            then true else false)
    else None.

  (** Handle comparison returns true exactly for Comparable, indistinguishable operands. *)
  Lemma compare_handles_equal left right : compare_handles left right = Some true <->
    is_comparable left = true /\ is_comparable right = true /\
    indistinguishable left right.
  Proof.
    unfold compare_handles.
    destruct (is_comparable left && is_comparable right) eqn:Comparable.
    - apply andb_true_iff in Comparable as [Left Right].
      destruct (excluded_middle_informative _); intuition discriminate.
    - split; [discriminate|intros [Left [Right _]]].
      rewrite Left, Right in Comparable; discriminate.
  Qed.

  (** Handle comparison is defined exactly when both operands are Comparable. *)
  Lemma compare_handles_defined left right :
    (exists result, compare_handles left right = Some result) <->
    is_comparable left = true /\ is_comparable right = true.
  Proof.
    unfold compare_handles.
    destruct (is_comparable left && is_comparable right) eqn:Comparable.
    - apply andb_true_iff in Comparable; split; [auto|intros _; eexists; reflexivity].
    - split; [intros [? Impossible]; discriminate|intros [Left Right]].
      rewrite Left, Right in Comparable; discriminate.
  Qed.

  (** Replacing either operand by an indistinguishable handle leaves handle comparison
      unchanged.

      Indistinguishable replacements preserve comparability and the comparison predicate, by
      symmetry and transitivity. *)
  Theorem compare_handles_congruence left left' right right' :
    indistinguishable left left' -> indistinguishable right right' ->
    compare_handles left right = compare_handles left' right'.
  Proof.
    (* There are two potential ways to distinguish replacement operands: whether
        comparison is available and whether it returns true. Observational agreement
        preserves Comparability, while symmetry and transitivity preserve the truth of the
        indistinguishability predicate used by the comparison. *)
    intros Left Right.
    assert (SamePredicate : indistinguishable left right <-> indistinguishable left' right')
      by (split; eauto 6 using indistinguishable_sym, indistinguishable_trans).
    assert (SameLeft : is_comparable left = is_comparable left')
      by (hauto cases: - use: indistinguishable_comparable).
    assert (SameRight : is_comparable right = is_comparable right')
      by (hauto cases: - use: indistinguishable_comparable).
    sauto unfold!: compare_handles.
  Qed.

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

  (** On Comparable handles, the computation logic and the observation logic agree. Thus
      restricting handle comparison to Comparable operands avoids identifying computations
      merely because their eventual results agree. *)
  Theorem comparable_equivalence_indistinguishable left right :
    is_comparable left = true -> is_comparable right = true ->
    (computational_equiv left right <-> indistinguishable left right).
  Proof. unfold indistinguishable; intuition auto using comparable_equivalent_observations. Qed.

  (** A Tree Object indistinguishable from a Value Tree is also a Value Tree. *)
  Lemma indistinguishable_tree_value t1 t2 :
    indistinguishable (HTreeObj t1) (HTreeObj t2) -> value_tree t1 -> value_tree t2.
  Proof.
    (* Being a Value is determined by exposed structure: a Tree requires Value children,
        while Refs and bare Thunks stop the traversal. Matching observations preserve
        these stopping points, and well-founded Object containment propagates the argument
        through the exposed Tree children. *)
    revert t1 t2; refine (well_founded_induction wfp_tree_child
      (fun t1 => forall t2, indistinguishable (HTreeObj t1) (HTreeObj t2) ->
        value_tree t1 -> value_tree t2) _).
    intros t1 IH t2 Related Value.
    assert (Children : Forall2 indistinguishable (get_tree_raw t1) (get_tree_raw t2))
      by (hauto cases: - use: indistinguishable_trees).
    assert (Values : Forall value_handle (get_tree_raw t1))
      by (sauto inv: value_tree ctrs: - cases: -).
    assert (ChildValues : forall child child', In child (get_tree_raw t1) ->
      indistinguishable child child' -> value_handle child -> value_handle child').
    { intros child child' Member ChildRelated ChildValue.
      pose proof ChildRelated as [_ Observed].
      destruct Observed; inversion ChildValue; subst; constructor.
      eapply IH; [exact Member|exact ChildRelated|assumption]. }
    constructor; clear -Children Values ChildValues.
    revert Values ChildValues; induction Children;
      sauto inv: Forall ctrs: Forall cases: -.
  Qed.

  (** A handle indistinguishable from a Value is also a Value. *)
  Lemma indistinguishable_value h1 h2 :
    indistinguishable h1 h2 -> value_handle h1 -> value_handle h2.
  Proof.
    intros Related Value; pose proof Related as [_ Observed];
      destruct Observed; inversion Value; subst; constructor.
    eapply indistinguishable_tree_value; eassumption.
  Qed.

  (** Indistinguishable handles agree on whether they are Values.

      Value preservation in both directions follows from symmetry and preservation of the
      observed handle structure. *)
  Theorem indistinguishable_values h1 h2 : indistinguishable h1 h2 ->
    (value_handle h1 <-> value_handle h2).
  Proof. intuition eauto using indistinguishable_value, indistinguishable_sym. Qed.

  (** Lifting Data preserves indistinguishability. *)
  Lemma indistinguishable_lift_data d1 d2 :
    indistinguishable (Data d1) (Data d2) ->
    indistinguishable (Data (lift_data d1)) (Data (lift_data d2)).
  Proof.
    intros [Equivalent Observed]; split.
    - exact (equiv_to_lift _ _ Equivalent).
    - exact (observations_lift _ _ Observed).
  Qed.

  (** Lowering Data preserves indistinguishability. *)
  Lemma indistinguishable_lower_data d1 d2 :
    indistinguishable (Data d1) (Data d2) ->
    indistinguishable (Data (lower_data d1)) (Data (lower_data d2)).
  Proof.
    intros [Equivalent Observed]; split.
    - exact (equiv_to_lower _ _ Equivalent).
    - exact (observations_lower _ _ Observed).
  Qed.

  (** Equal-content Blob Objects give indistinguishable range-selection results or shared
      failure. *)
  Lemma indistinguishable_blob_range b1 b2 a b :
    get_blob_data b1 = get_blob_data b2 ->
    rel_opt indistinguishable (Sl.select_range (HBlobObj b1) a b)
      (Sl.select_range (HBlobObj b2) a b).
  Proof.
    intro Contents; unfold HBlobObj, Sl.select_range, Sl.range_ref, Sl.blob_slice; rewrite Contents.
    destruct (Sl.list_slice (get_blob_data b2) a b); cbn [omap rel_opt];
      [apply indistinguishable_refl|exact I].
  Qed.

  (** Indexing indistinguishable Tree Objects gives indistinguishable results or shared
      failure. *)
  Lemma indistinguishable_tree_index t1 t2 i :
    indistinguishable (HTreeObj t1) (HTreeObj t2) ->
    rel_opt indistinguishable (Sl.select_index (HTreeObj t1) i)
      (Sl.select_index (HTreeObj t2) i).
  Proof.
    intro Related; apply indistinguishable_trees in Related as [_ Children].
    apply Forall2_nth_error; exact Children.
  Qed.

  (** Index selection on indistinguishable Data gives indistinguishable results or shared
      failure. *)
  Theorem indistinguishable_select_index d1 d2 i :
    indistinguishable (Data d1) (Data d2) ->
    rel_opt indistinguishable (Sl.select_index (Data d1) i)
      (Sl.select_index (Data d2) i).
  Proof.
    (* A selection may expose either an immediate Tree child or a one-byte Blob range.
        The observational relation supplies the exact child correspondence or byte
        equality needed for those observations, independently of the names of the
        containing Objects. *)
    intros [Equivalent Observed]; pose proof (observations_type _ _ Observed) as Types.
    destruct d1 as [[b1|t1]|[b1|t1]], d2 as [[b2|t2]|[b2|t2]];
      cbn [get_type] in Types; try discriminate.
    - apply indistinguishable_blob_range; inversion Observed; assumption.
    - apply indistinguishable_tree_index; split; assumption.
    - apply indistinguishable_blob_range; inversion Observed; assumption.
    - apply indistinguishable_tree_index.
      apply (indistinguishable_lift_data (Ref (TreeRef t1)) (Ref (TreeRef t2))).
      split; assumption.
  Qed.

  (** Slicing indistinguishable Tree Objects gives indistinguishable range-selection
      results or shared failure. *)
  Lemma indistinguishable_tree_range t1 t2 a b :
    indistinguishable (HTreeObj t1) (HTreeObj t2) ->
    rel_opt indistinguishable (Sl.select_range (HTreeObj t1) a b)
      (Sl.select_range (HTreeObj t2) a b).
  Proof.
    intro Related; apply indistinguishable_trees in Related as [_ Children].
    pose proof (Sl.list_slice_X indistinguishable _ _ a b Children) as Slices.
    unfold HTreeObj, Sl.select_range, Sl.range_ref, Sl.tree_slice.
    destruct (Sl.list_slice (get_tree_raw t1) a b) as [xs|],
      (Sl.list_slice (get_tree_raw t2) a b) as [ys|];
      cbn [rel_opt omap] in *; try contradiction; [|exact I].
    apply (indistinguishable_lower_data (Object (TreeObj (create_tree xs)))
      (Object (TreeObj (create_tree ys)))).
    apply indistinguishable_trees; rewrite !tree_tag_create_tree, !get_tree_raw_create_tree; auto.
  Qed.

  (** Range selection on indistinguishable Data gives indistinguishable results or shared
      failure. *)
  Theorem indistinguishable_select_range d1 d2 a b :
    indistinguishable (Data d1) (Data d2) ->
    rel_opt indistinguishable (Sl.select_range (Data d1) a b)
      (Sl.select_range (Data d2) a b).
  Proof.
    intros [Equivalent Observed]; pose proof (observations_type _ _ Observed) as Types.
    destruct d1 as [[b1|t1]|[b1|t1]], d2 as [[b2|t2]|[b2|t2]];
      cbn [get_type] in Types; try discriminate.
    - apply indistinguishable_blob_range; inversion Observed; assumption.
    - apply indistinguishable_tree_range; split; assumption.
    - apply indistinguishable_blob_range; inversion Observed; assumption.
    - apply indistinguishable_tree_range.
      apply (indistinguishable_lift_data (Ref (TreeRef t1)) (Ref (TreeRef t2))).
      split; assumption.
  Qed.

  (** Indistinguishable handles decode to identical selection indices or shared failure. *)
  Lemma indistinguishable_read_index h1 h2 : indistinguishable h1 h2 ->
    Sl.read_index h1 = Sl.read_index h2.
  Proof.
    intros [_ Observed]; destruct Observed;
      cbn [HBlobObj HTreeObj HBlobRef HTreeRef Sl.read_index]; congruence.
  Qed.

  (** Selection on indistinguishable Data with pointwise indistinguishable operands gives
      indistinguishable results or shared failure. *)
  Theorem indistinguishable_select_arguments d1 d2 xs ys :
    indistinguishable (Data d1) (Data d2) -> Forall2 indistinguishable xs ys ->
    rel_opt indistinguishable (Sl.select_arguments (Data d1) xs)
      (Sl.select_arguments (Data d2) ys).
  Proof.
    (* Indistinguishable index operands decode to the same integers. Consequently both
        selections choose the same arity-specific operation and the same bounds; only the
        already related targets can affect the returned contents. *)
    intros Related Indices; destruct Indices as [|i1 i2 xs ys Index Tail]; [exact I|].
    pose proof (indistinguishable_read_index _ _ Index) as First.
    destruct Tail as [|j1 j2 xs ys Second Tail].
    - cbn [Sl.select_arguments]; rewrite First.
      destruct (Sl.read_index i2); cbn [obind];
        [apply indistinguishable_select_index; exact Related|exact I].
    - destruct Tail as [|k1 k2 xs ys Third Tail]; [|exact I].
      pose proof (indistinguishable_read_index _ _ Second) as Last.
      cbn [Sl.select_arguments]; rewrite First, Last.
      destruct (Sl.read_index i2), (Sl.read_index j2); cbn [obind];
        [apply indistinguishable_select_range; exact Related|exact I|exact I|exact I].
  Qed.
End Indistinguishability.
