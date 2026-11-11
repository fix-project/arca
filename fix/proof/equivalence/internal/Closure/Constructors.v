  (** A handle is equivalent to itself: an unchanged endpoint needs no computation
      witness, represented here by an empty chain of replacements. *)
  Lemma equiv_refl h : computational_equiv h h.
  Proof. apply rst_refl. Qed.
  (** A derivation of equivalence can be read in either direction. The reversal changes
      the justification, without asking either endpoint to compute. *)
  Lemma equiv_sym h1 h2 : computational_equiv h1 h2 -> computational_equiv h2 h1.
  Proof. apply rst_sym. Qed.
  (** Equivalence derivations with a common endpoint can be joined. Intermediate handles
      are therefore irrelevant to the final claim once their replacements have been
      justified. *)
  Lemma equiv_trans h1 h2 h3 : computational_equiv h1 h2 -> computational_equiv h2 h3 -> computational_equiv h1 h3.
  Proof. apply rst_trans. Qed.
  (** The base relation R implies computational equivalence. *)
  Lemma R_to_equiv h1 h2 : R h1 h2 -> computational_equiv h1 h2.
  Proof. apply rst_step. Qed.
  (** A context that respects each generating replacement respects every finite
      equivalence derivation. This lets constructor congruence be proved locally instead
      of repeating the equivalence-chain argument for every constructor. *)
  Local Lemma closure_map (input output : handle -> handle -> Prop) f :
    (forall x y, input x y -> output (f x) (f y)) ->
    forall x y, clos_refl_sym_trans handle input x y ->
      clos_refl_sym_trans handle output (f x) (f y).
  Proof. intros Step x y Related; induction Related; eauto using rst_step, rst_refl, rst_sym, rst_trans. Qed.

  (** Replacing one Tree child by an R-related child preserves R. *)
  Lemma R_tree_update1 tag a b pre post :
    R a b -> R (HTreeObj (create_tree_tagged tag (pre ++ a :: post)))
      (HTreeObj (create_tree_tagged tag (pre ++ b :: post))).
  Proof.
    (* The common prefix and suffix need only reflexivity. All nontrivial justification
        is confined to the one changed child, so the enclosing Tree can reuse the child
        replacement without any evaluation. *)
    intro H; apply tree_tagged_complete_R.
    apply Forall2_app; [induction pre; constructor; auto using RSelf|].
    constructor; [exact H|induction post; constructor; auto using RSelf].
  Qed.
  (** Replacing one Tree child by a computationally equivalent child preserves
      computational equivalence. *)
  Lemma equivclp_tree_update1 tag a b pre post :
    computational_equiv a b -> computational_equiv (HTreeObj (create_tree_tagged tag (pre ++ a :: post)))
      (HTreeObj (create_tree_tagged tag (pre ++ b :: post))).
  Proof.
    (* A replacement of the child may itself pass through many intermediate handles. Keep
        the enclosing Tree fixed around each intermediate child; this lifts the entire
        child derivation into a derivation between Trees. *)
    induction 1; eauto using R_to_equiv, R_tree_update1, equiv_refl, equiv_sym, equiv_trans.
  Qed.
  (** Pointwise equivalent suffixes remain equivalent when placed after the same Tree
      prefix. *)
  Lemma equivclp_tree_Forall2_prefix tag xs ys :
    Forall2 computational_equiv xs ys -> forall pre,
    computational_equiv (HTreeObj (create_tree_tagged tag (pre ++ xs)))
      (HTreeObj (create_tree_tagged tag (pre ++ ys))).
  Proof.
    (* Different children may have unrelated equivalence derivations. Replace them one at
        a time, rather than trying to align the lengths or shapes of those derivations. *)
    intro Children; induction Children; intro pre; [apply equiv_refl|].
    (* This first path changes only the current child; the old suffix remains available
        as the common context. *)
    assert (HeadReplaced : computational_equiv
      (HTreeObj (create_tree_tagged tag (pre ++ x :: l)))
      (HTreeObj (create_tree_tagged tag (pre ++ y :: l)))).
    { apply equivclp_tree_update1; exact H. }
    (* The second path starts at exactly the first path's endpoint. Extending the fixed
        prefix by the new head makes the recursive replacement composable. *)
    assert (TailReplaced : computational_equiv
      (HTreeObj (create_tree_tagged tag (pre ++ y :: l)))
      (HTreeObj (create_tree_tagged tag (pre ++ y :: l')))).
    { specialize (IHChildren (pre ++ [y])).
      rewrite <- !app_assoc in IHChildren; exact IHChildren. }
    exact (equiv_trans _ _ _ HeadReplaced TailReplaced).
  Qed.
  (** Constructing Trees with the same marker from pointwise equivalent children preserves
      computational equivalence. *)
  Lemma equiv_tree_tagged_Forall2 tag xs ys : Forall2 computational_equiv xs ys ->
    computational_equiv (HTreeObj (create_tree_tagged tag xs)) (HTreeObj (create_tree_tagged tag ys)).
  Proof. intro H; exact (equivclp_tree_Forall2_prefix tag xs ys H []). Qed.
  (** Constructing ordinary Trees from pointwise equivalent children preserves
      computational equivalence. *)
  Lemma equiv_tree_Forall2 xs ys : Forall2 computational_equiv xs ys ->
    computational_equiv (HTreeObj (create_tree xs)) (HTreeObj (create_tree ys)).
  Proof. apply equiv_tree_tagged_Forall2. Qed.
  (** An equivalence chain cannot cross the boundary between a bare Thunk and other handle
      forms. This is needed before applying Force to the other endpoint: Force must
      receive a Thunk there too. *)
  Lemma equiv_preserve_thunk h1 h2 : computational_equiv h1 h2 ->
    ((exists th, h1 = Thunk th) <-> (exists th, h2 = Thunk th)).
  Proof.
    intro Related; induction Related.
    - apply R_preserve_thunk; assumption.
    - reflexivity.
    - tauto.
    - tauto.
  Qed.
  (** A partial observation that respects each generating replacement respects the entire
      equivalence. Respect includes existence of a result: related inputs must either both
      have results or both have none. *)
  Local Lemma rel_opt_closure (input output : handle -> handle -> Prop)
      (f : handle -> option handle)
      (Step : forall x y, input x y ->
        rel_opt (clos_refl_sym_trans handle output) (f x) (f y)) :
    forall x y, clos_refl_sym_trans handle input x y ->
      rel_opt (clos_refl_sym_trans handle output) (f x) (f y).
  Proof.
    (* View an equivalence as a finite path. Lift each edge through f. For two adjacent
        edges, the shared input has a single f-result, so the lifted edges meet and can be
        composed. The same argument applies to paths containing reversed edges. *)
    intros x y Related; induction Related.
    (* The supplied premise handles a generating edge of the path. *)
    - auto.
    (* A path of length zero observes the same optional result at both endpoints. *)
    - destruct (f x); cbn [rel_opt]; auto using rst_refl.
    (* Reversing a path reverses its result equivalence; shared absence of a result is
        also symmetric. *)
    - destruct (f x), (f y); cbn [rel_opt] in *; auto using rst_sym.
    (* The middle observation is common to both paths. If it is absent, both endpoints
        must be absent; if present, its payload joins the two result equivalences. *)
    - destruct (f x), (f y), (f z); cbn [rel_opt] in *;
        try contradiction; eauto using rst_trans.
  Qed.
  (** Optional R-related results are computationally equivalent, with shared failure. *)
  Lemma rel_opt_R_to_equiv x y : rel_opt R x y -> rel_opt computational_equiv x y.
  Proof. destruct x, y; cbn [rel_opt]; auto using R_to_equiv. Qed.
