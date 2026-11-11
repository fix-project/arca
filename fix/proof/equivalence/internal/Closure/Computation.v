  (** Every successful evaluation returns a computationally equivalent handle. *)
  Lemma eval_to_equiv h r : eval h = Some r -> computational_equiv h r.
  Proof. intro Ev; apply eval_some in Ev; destruct Ev as [n Ev]; eapply eval_with_fuel_equiv; exact Ev. Qed.
  (** Inputs evaluating to computationally equivalent results are themselves
      computationally equivalent. *)
  Lemma eval_both_to_equiv h1 h2 r1 r2 : eval h1 = Some r1 -> eval h2 = Some r2 -> computational_equiv r1 r2 -> computational_equiv h1 h2.
  Proof.
    (* The desired path is h1 ≃ r1 ≃ r2 ≃ h2. Successful evaluation supplies the two
        outside edges; the premise supplies the middle edge. Reverse the second evaluation
        edge before joining the three. *)
    intros Left Right Related.
    assert (LeftEvaluated : computational_equiv h1 r1)
      by (hauto cases: - use: eval_to_equiv).
    assert (RightEvaluated : computational_equiv h2 r2)
      by (hauto cases: - use: eval_to_equiv).
    hauto cases: - use: equiv_sym, equiv_trans.
  Qed.
  (** Relaxed R implies relaxed computational equivalence. *)
  Lemma relaxed_R_to_equiv h1 h2 : relaxed_X R h1 h2 -> relaxed_X computational_equiv h1 h2.
  Proof. destruct h1 as [d|th|[th|th]]; apply R_to_equiv. Qed.
  (** Optional relaxed-R results satisfy relaxed computational equivalence. *)
  Lemma rel_opt_relaxed_R_to_equiv x y : rel_opt (relaxed_X R) x y -> rel_opt (relaxed_X computational_equiv) x y.
  Proof. destruct x, y; cbn [rel_opt]; auto using relaxed_R_to_equiv. Qed.
  (** A Thunk's optional forcing result is relaxed-equivalent to itself. *)
  Lemma force_relaxed_equiv_refl th : rel_opt (relaxed_X computational_equiv) (force th) (force th).
  Proof.
    destruct (force th) as [h|] eqn:F; [|exact I].
    destruct (force_data _ _ F) as [d ->]; apply equiv_refl.
  Qed.
  (** Relaxed equivalence between optional forcing results is symmetric. *)
  Lemma force_relaxed_equiv_sym th1 th2 : rel_opt (relaxed_X computational_equiv) (force th1) (force th2) ->
    rel_opt (relaxed_X computational_equiv) (force th2) (force th1).
  Proof.
    intro Related; destruct (force th1) as [h1|] eqn:F1, (force th2) as [h2|] eqn:F2;
      cbn [rel_opt] in *; try contradiction; [|exact I].
    destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->].
    apply equiv_sym; exact Related.
  Qed.
  (** Relaxed equivalence between optional forcing results is transitive. *)
  Lemma force_relaxed_equiv_trans th1 th2 th3 : rel_opt (relaxed_X computational_equiv) (force th1) (force th2) ->
    rel_opt (relaxed_X computational_equiv) (force th2) (force th3) -> rel_opt (relaxed_X computational_equiv) (force th1) (force th3).
  Proof.
    intros R12 R23; destruct (force th1) as [h1|] eqn:F1, (force th2) as [h2|] eqn:F2,
      (force th3) as [h3|] eqn:F3; cbn [rel_opt] in *; try contradiction; [|exact I].
    destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->], (force_data _ _ F3) as [d3 ->].
    eapply equiv_trans; [exact R12|exact R23].
  Qed.
  (** A handle equivalent to a bare Thunk is a bare Thunk with relaxed-equivalent forcing
      behavior. *)
  Theorem force_equiv h1 h2 : computational_equiv h1 h2 -> forall th1, h1 = Thunk th1 ->
    exists th2, h2 = Thunk th2 /\ rel_opt (relaxed_X computational_equiv) (force th1) (force th2).
  Proof.
    (* The output of Force may be an Object or a Ref. Compare the underlying Objects by
        lifting successful Data results, so the eventual consumer's choice of strictness
        does not obstruct composition of the equivalence chain. *)
    intro Related; induction Related as [x y Rxy|x|x y Related IH|x y z Rxy IHxy Ryz IHyz]; intros th1 E1.
    - subst x.
      assert (MatchingThunk : exists th2, y = Thunk th2).
      { apply (proj1 (R_preserve_thunk _ _ Rxy)); eauto. }
      destruct MatchingThunk as [th2 ->].
      pose proof (force_R _ _ Rxy) as ResultsRelated.
      exists th2; split; [reflexivity|apply rel_opt_relaxed_R_to_equiv; exact ResultsRelated].
    - subst x; exists th1; split; [reflexivity|apply force_relaxed_equiv_refl].
    - assert (MatchingThunk : exists th2, x = Thunk th2).
      { apply (proj2 (equiv_preserve_thunk _ _ Related)); eauto. }
      destruct MatchingThunk as [th2 E2].
      destruct (IH th2 E2) as [th3 [E3 Forces]].
      rewrite E1 in E3; inversion E3; subst th3.
      exists th2; split; [exact E2|apply force_relaxed_equiv_sym; exact Forces].
    - destruct (IHxy th1 E1) as [th2 [E2 F12]], (IHyz th2 E2) as [th3 [E3 F23]].
      exists th3; split; [exact E3|eapply force_relaxed_equiv_trans; eassumption].
  Qed.
  (** A finite force can be traced to an equivalent last Thunk whose single Think step
      returns the final Data. *)
  Lemma force_with_fuel_to_the_last_thunk n : forall th h, force_with_fuel n th = Some h ->
    exists th', think th' = Some h /\ computational_equiv (Thunk th) (Thunk th').
  Proof.
    (* A successful force is a finite sequence of Think steps ending in Data. Each
        preceding returned Thunk or Encode justifies advancing to the next bare Thunk.
        Compose those advances to isolate the final step that actually supplies the Data. *)
    induction n as [|n IH]; intros th h Fuel; [discriminate|].
    change (obind (think_with_fuel n th) (force_after n) = Some h) in Fuel.
    apply obind_some in Fuel; destruct Fuel as [r [Think Reply]].
    pose proof (think_unique _ _ (ex_intro _ n Think)) as Think'.
    destruct r as [d|th1|[th1|th1]].
    - inversion Reply; subst h; exists th; split; [exact Think'|apply equiv_refl].
    - destruct (IH th1 h Reply) as [th' [Last Related]].
      exists th'; split; [exact Last|].
      eapply equiv_trans; [apply R_to_equiv, RThinkSingleStepThunk; exact Think'|exact Related].
    - destruct (IH th1 h Reply) as [th' [Last Related]].
      exists th'; split; [exact Last|].
      eapply equiv_trans; [apply R_to_equiv, RThunkSingleStepEncodeStrict; exact Think'|exact Related].
    - destruct (IH th1 h Reply) as [th' [Last Related]].
      exists th'; split; [exact Last|].
      eapply equiv_trans; [apply R_to_equiv, RThunkSingleStepEncodeShallow; exact Think'|exact Related].
  Qed.
  (** A successful force reaches an equivalent last Thunk whose Think step returns the
      final Data. *)
  Lemma force_to_the_last_thunk th h : force th = Some h ->
    exists th', think th' = Some h /\ computational_equiv (Thunk th) (Thunk th').
  Proof. intro F; apply force_some in F; destruct F as [n F]; eapply force_with_fuel_to_the_last_thunk; exact F. Qed.
  (** An equivalence between two Objects may pass through Encodes even though its
      endpoints are not Encodes. Shape and byte-content arguments therefore cannot simply
      assume that every intermediate handle has the endpoint's form. Executing only these
      exposed Encodes removes them from the path, yielding a path on which constructor
      invariants can be used. *)
  Definition unencode h := match h with Encode e => execute e | _ => Some h end.
  Definition unencoded h := ~(exists e, h = Encode e).
  Definition unencoded_R h1 h2 := R h1 h2 /\ unencoded h1 /\ unencoded h2.
  Definition unencoded_equiv := clos_refl_sym_trans handle unencoded_R.

  (** Executing an exposed Encode, when present, leaves a non-Encode result. *)
  Lemma unencode_some_unencoded h v : unencode h = Some v -> unencoded v.
  Proof.
    destruct h as [d|th|e]; cbn [unencode]; intro Result.
    - inversion Result; subst; intros [e E']; discriminate.
    - inversion Result; subst; intros [e E']; discriminate.
    - destruct (execute_data _ _ Result) as [d ->]; intros [e' E']; discriminate.
  Qed.
  (** Removing exposed Encodes preserves optional R-related results. *)
  Lemma R_unencode h1 h2 : R h1 h2 -> rel_opt R (unencode h1) (unencode h2).
  Proof.
    intro Related; destruct h1 as [d1|th1|e1], h2 as [d2|th2|e2]; cbn [unencode rel_opt]; try exact Related.
    - exfalso; eapply R_encode_execute_rev_does_not_exist; [exact Related|intros [e E']; discriminate].
    - exfalso; eapply R_encode_execute_rev_does_not_exist; [exact Related|intros [e E']; discriminate].
    - rewrite (execute_unique _ _ (R_encode_execute _ _ Related ltac:(intros [e E']; discriminate))); apply RSelf.
    - rewrite (execute_unique _ _ (R_encode_execute _ _ Related ltac:(intros [e E']; discriminate))); apply RSelf.
    - destruct e1 as [t1|t1], e2 as [t2|t2].
      + apply R_strict_encode_reasons; exact Related.
      + exfalso; exact (R_not_strict_shallow _ _ Related).
      + exfalso; exact (R_not_shallow_strict _ _ Related).
      + apply R_shallow_encode_reasons; exact Related.
  Qed.
  (** Removing exposed Encodes yields related results that are both non-Encodes. *)
  Lemma R_unencode_safe h1 h2 : R h1 h2 -> rel_opt unencoded_R (unencode h1) (unencode h2).
  Proof.
    intro Related; pose proof (R_unencode _ _ Related) as Result.
    destruct (unencode h1) as [v1|] eqn:U1, (unencode h2) as [v2|] eqn:U2;
      cbn [rel_opt] in *; try contradiction; [|exact I].
    split; [exact Result|split; eapply unencode_some_unencoded; eassumption].
  Qed.
  (** Equivalence restricted to non-Encode steps implies ordinary computational
      equivalence. *)
  Lemma unencoded_equiv_to_equiv h1 h2 : unencoded_equiv h1 h2 -> computational_equiv h1 h2.
  Proof.
    apply (closure_map unencoded_R R (fun h => h)).
    hauto cases: - unfold!: unencoded_R.
  Qed.
  (** Removing exposed Encodes from equivalent handles gives equivalent non-Encode results
      or shared failure. *)
  Theorem equiv_unencode h1 h2 : computational_equiv h1 h2 -> rel_opt unencoded_equiv (unencode h1) (unencode h2).
  Proof.
    (* Replace each exposed Encode in the path by its execution result. Preservation for
        R ensures that adjacent replacements still meet. When the observations exist, the
        resulting path stays entirely among non-Encodes; when one is absent, absence
        propagates through the path. *)
    apply (rel_opt_closure R unencoded_R unencode); intros x y Related.
    pose proof (R_unencode_safe _ _ Related) as Safe.
    destruct (unencode x), (unencode y); cbn [rel_opt] in *;
      auto using rst_step.
  Qed.
  (** Removing exposed Encodes preserves computational equivalence and shared failure. *)
  Lemma equiv_unencode_general h1 h2 : computational_equiv h1 h2 -> rel_opt computational_equiv (unencode h1) (unencode h2).
  Proof.
    intro Related; pose proof (equiv_unencode _ _ Related) as Result.
    destruct (unencode h1), (unencode h2); cbn [rel_opt] in *; auto using unencoded_equiv_to_equiv.
  Qed.
  (** Equivalent Encodes have equivalent execution results; an Encode equivalent to a
      non-Encode must execute to an equivalent result. *)
  Theorem equiv_encode_not_encode h1 h2 : computational_equiv h1 h2 -> forall e, h1 = Encode e ->
    (unencoded h2 -> exists r1, execute e = Some r1 /\ computational_equiv r1 h2) /\
    ((exists e2, h2 = Encode e2) -> exists e2, h2 = Encode e2 /\ rel_opt computational_equiv (execute e) (execute e2)).
  Proof.
    intros Related e ->; pose proof (equiv_unencode_general _ _ Related) as Result; cbn [unencode] in Result; split.
    - intro N2; destruct h2 as [d|th|e2]; [| |exfalso; apply N2; eauto];
        destruct (execute e) as [r1|] eqn:Exec; cbn [rel_opt] in Result; try contradiction;
        exists r1; split; first [reflexivity|exact Result].
    - intros [e2 ->]; exists e2; split; [reflexivity|exact Result].
  Qed.
  (** Every handle has the same shape as itself. *)
  Lemma same_shape_refl h : same_shape h h.
  Proof. repeat split; auto. Qed.
  (** Having the same shape is symmetric. *)
  Lemma same_shape_sym h1 h2 : same_shape h1 h2 -> same_shape h2 h1.
  Proof. unfold same_shape; congruence. Qed.
  (** Having the same shape is transitive. *)
  Lemma same_shape_trans h1 h2 h3 : same_shape h1 h2 -> same_shape h2 h3 -> same_shape h1 h3.
  Proof. unfold same_shape; congruence. Qed.
  (** Equivalence restricted to non-Encode steps preserves handle shape. *)
  Lemma unencoded_equiv_shape h1 h2 : unencoded_equiv h1 h2 -> same_shape h1 h2.
  Proof.
    (* Once Encodes are excluded from every edge, an edge cannot change the observable
        constructor class. Equality of that class then survives reversing or joining
        edges. *)
    intro Related; induction Related.
    - destruct H as [Rel [N1 N2]]; exact (value_shapes R R_properties _ _ Rel N1 N2).
    - apply same_shape_refl.
    - apply same_shape_sym; assumption.
    - eapply same_shape_trans; eassumption.
  Qed.
  (** Equivalent non-Encode Tree Objects have matching Tag markers and pointwise equivalent
      children.

      Basic steps preserve the marker and relate corresponding children. Symmetry and
      transitivity compose both properties together. *)
  Lemma unencoded_equiv_tree h1 h2 : unencoded_equiv h1 h2 -> forall t1 t2,
    h1 = HTreeObj t1 -> h2 = HTreeObj t2 ->
    tree_tag t1 = tree_tag t2 /\
    Forall2 computational_equiv (get_tree_raw t1) (get_tree_raw t2).
  Proof.
    (* After removing Encodes, every intermediate endpoint between Tree Objects must
        itself be a Tree Object. Its children supply the matching intermediate list
        through which child equivalences can be composed. *)
    intro Related; induction Related as
      [x y Step|x|x y Related IH|x y z First IHFirst Second IHSecond];
      intros t1 t2 E1 E2.
    - destruct Step as [Step _]; subst x y; split; [apply tag_cong_R; exact Step|].
      eapply A.Forall2_mono; [intros a b Pair; apply R_to_equiv; exact Pair|].
      apply tree_cong_R; exact Step.
    - rewrite E1 in E2; inversion E2; subst; split; [reflexivity|].
      induction (get_tree_raw t2); constructor; auto using equiv_refl.
    - destruct (IH t2 t1 E2 E1) as [Tags Backwards]; split; [congruence|].
      apply Forall2_flip in Backwards.
      eapply A.Forall2_mono; [intros a b Pair; apply equiv_sym; exact Pair|exact Backwards].
    - pose proof (unencoded_equiv_shape _ _ First) as Shape; rewrite E1 in Shape.
      destruct y as [[[b|middle]|[b|t]]|th|e];
        cbn [same_shape get_type HTreeObj] in Shape; try discriminate.
      destruct (IHFirst t1 middle E1 eq_refl) as [Tag1 Left].
      destruct (IHSecond middle t2 eq_refl E2) as [Tag2 Right].
      assert (SameTag : tree_tag t1 = tree_tag t2) by congruence.
      assert (ChildrenRelated : Forall2 computational_equiv (get_tree_raw t1) (get_tree_raw t2)).
      { clear -Left Right; revert Right.
        generalize (get_tree_raw t2) as rest; induction Left; intros rest Right;
          inversion Right; subst; constructor; eauto using equiv_trans. }
      split; assumption.
  Qed.
  (** Computationally equivalent Tree Objects have the same Tag marker. *)
  Theorem equiv_tree_tag t1 t2 : computational_equiv (HTreeObj t1) (HTreeObj t2) -> tree_tag t1 = tree_tag t2.
  Proof.
    intro Related; exact (proj1 (unencoded_equiv_tree _ _
      (equiv_unencode _ _ Related) t1 t2 eq_refl eq_refl)).
  Qed.
  (** Computationally equivalent Tree Objects have pointwise equivalent children. *)
  Theorem equiv_tree_children t1 t2 : computational_equiv (HTreeObj t1) (HTreeObj t2) ->
    Forall2 computational_equiv (get_tree_raw t1) (get_tree_raw t2).
  Proof.
    intro Related; exact (proj2 (unencoded_equiv_tree _ _
      (equiv_unencode _ _ Related) t1 t2 eq_refl eq_refl)).
  Qed.
  (** Tree Objects are computationally equivalent exactly when their markers and
      corresponding children agree up to equivalence. *)
  Lemma equiv_tree_iff t1 t2 :
    computational_equiv (HTreeObj t1) (HTreeObj t2) <->
    tree_tag t1 = tree_tag t2 /\
    Forall2 computational_equiv (get_tree_raw t1) (get_tree_raw t2).
  Proof.
    (* For necessity, remove exposed Encodes from the path and use its preserved Tree
        shape. For sufficiency, replace the children independently. The Tag marker remains
        part of the Tree structure throughout either argument. *)
    split; [hauto cases: - use: equiv_tree_tag, equiv_tree_children|].
    intros [Tags Children].
    pose proof (equiv_tree_tagged_Forall2 (tree_tag t1) _ _ Children) as Rebuilt.
    now rewrite create_tree_get_tree_raw, Tags, create_tree_get_tree_raw in Rebuilt.
  Qed.

  (** A handle equivalent to a non-Encode executes any exposed Encode to a result of the
      same shape. *)
  Lemma unencode_same_shape h1 h2 : computational_equiv h1 h2 -> unencoded h1 ->
    exists v2, unencode h2 = Some v2 /\ same_shape h1 v2.
  Proof.
    intros Related N1; pose proof (equiv_unencode _ _ Related) as Result.
    destruct h1 as [d|th|e]; [| |exfalso; apply N1; eauto]; cbn [unencode] in Result;
      destruct (unencode h2) as [v2|] eqn:U2; cbn [rel_opt] in Result; try contradiction;
      exists v2; split; first [reflexivity|apply unencoded_equiv_shape; exact Result].
  Qed.
  (** A handle equivalent to a Tree Object is either a Tree Object or a strict Encode
      executing to one. *)
  Lemma equiv_preserve_tree_or_encode h1 h2 : computational_equiv h1 h2 -> forall t1, h1 = HTreeObj t1 ->
    (exists t2, h2 = HTreeObj t2) \/
    (exists th t2, h2 = Encode (Strict th) /\ execute (Strict th) = Some (HTreeObj t2)).
  Proof.
    intros Related t1 ->; destruct (unencode_same_shape _ _ Related ltac:(intros [e E']; discriminate)) as
      [v2 [U2 Shape]].
    destruct v2 as [[[b2|t2]|[b2|t2]]|th2|e2];
      cbn [same_shape get_type HTreeObj HBlobObj HTreeRef HBlobRef] in Shape; try discriminate.
    change (unencode h2 = Some (HTreeObj t2)) in U2.
    destruct h2 as [d|th|e]; cbn [unencode] in U2.
    - left; exists t2; congruence.
    - discriminate.
    - destruct (execute_to_obj_strict _ _ U2) as [th ->]; right; exists th, t2; auto.
  Qed.
  (** A handle equivalent to a Blob Object is either a Blob Object or a strict Encode
      executing to one. *)
  Lemma equiv_preserve_blob_or_encode h1 h2 : computational_equiv h1 h2 -> forall b1, h1 = HBlobObj b1 ->
    (exists b2, h2 = HBlobObj b2) \/
    (exists th b2, h2 = Encode (Strict th) /\ execute (Strict th) = Some (HBlobObj b2)).
  Proof.
    intros Related b1 ->; destruct (unencode_same_shape _ _ Related ltac:(intros [e E']; discriminate)) as
      [v2 [U2 Shape]].
    destruct v2 as [[[b2|t2]|[b2|t2]]|th2|e2];
      cbn [same_shape get_type HTreeObj HBlobObj HTreeRef HBlobRef] in Shape; try discriminate.
    change (unencode h2 = Some (HBlobObj b2)) in U2.
    destruct h2 as [d|th|e]; cbn [unencode] in U2.
    - left; exists b2; congruence.
    - discriminate.
    - destruct (execute_to_obj_strict _ _ U2) as [th ->]; right; exists th, b2; auto.
  Qed.
  (** A handle equivalent to a Tree Ref is either a Tree Ref or a shallow Encode executing
      to one. *)
  Lemma equiv_preserve_tree_ref_or_encode h1 h2 : computational_equiv h1 h2 -> forall t1, h1 = HTreeRef t1 ->
    (exists t2, h2 = HTreeRef t2) \/
    (exists th t2, h2 = Encode (Shallow th) /\ execute (Shallow th) = Some (HTreeRef t2)).
  Proof.
    intros Related t1 ->; destruct (unencode_same_shape _ _ Related ltac:(intros [e E']; discriminate)) as
      [v2 [U2 Shape]].
    destruct v2 as [[[b2|t2]|[b2|t2]]|th2|e2];
      cbn [same_shape get_type HTreeObj HBlobObj HTreeRef HBlobRef] in Shape; try discriminate.
    change (unencode h2 = Some (HTreeRef t2)) in U2.
    destruct h2 as [d|th|e]; cbn [unencode] in U2.
    - left; exists t2; congruence.
    - discriminate.
    - destruct (execute_to_ref_shallow _ _ U2) as [th ->]; right; exists th, t2; auto.
  Qed.
  (** A handle equivalent to a Blob Ref is either a Blob Ref or a shallow Encode executing
      to one. *)
  Lemma equiv_preserve_blob_ref_or_encode h1 h2 : computational_equiv h1 h2 -> forall b1, h1 = HBlobRef b1 ->
    (exists b2, h2 = HBlobRef b2) \/
    (exists th b2, h2 = Encode (Shallow th) /\ execute (Shallow th) = Some (HBlobRef b2)).
  Proof.
    intros Related b1 ->; destruct (unencode_same_shape _ _ Related ltac:(intros [e E']; discriminate)) as
      [v2 [U2 Shape]].
    destruct v2 as [[[b2|t2]|[b2|t2]]|th2|e2];
      cbn [same_shape get_type HTreeObj HBlobObj HTreeRef HBlobRef] in Shape; try discriminate.
    change (unencode h2 = Some (HBlobRef b2)) in U2.
    destruct h2 as [d|th|e]; cbn [unencode] in U2.
    - left; exists b2; congruence.
    - discriminate.
    - destruct (execute_to_ref_shallow _ _ U2) as [th ->]; right; exists th, b2; auto.
  Qed.
  (** A Tree-to-Thunk constructor preserving R also preserves equivalence restricted to
      non-Encode Tree steps. *)
  Lemma unencoded_equiv_tree_cong (make_thunk : TreeName -> thunk)
      (Cong : forall t1 t2, R (HTreeObj t1) (HTreeObj t2) -> R (Thunk (make_thunk t1)) (Thunk (make_thunk t2)))
      h1 h2 : unencoded_equiv h1 h2 -> forall t1, h1 = HTreeObj t1 ->
      exists t2, h2 = HTreeObj t2 /\ computational_equiv (Thunk (make_thunk t1)) (Thunk (make_thunk t2)).
  Proof.
    (* Each intermediate endpoint is known to be a Tree Object, so the Tree-to-Thunk
        constructor is defined throughout the path. Applying it to every endpoint turns
        each Tree replacement into a Thunk replacement. *)
    intro Related; induction Related as [x y Rxy|x|x y Related IH|x y z Rxy IHxy Ryz IHyz]; intros t1 E1.
    - subst x; destruct Rxy as [Rxy _]; destruct (R_preserve_tree _ _ Rxy) as [t2 ->].
      exists t2; split; [reflexivity|apply R_to_equiv, Cong; exact Rxy].
    - subst x; exists t1; split; [reflexivity|apply equiv_refl].
    - pose proof (unencoded_equiv_shape _ _ Related) as Shape; rewrite E1 in Shape.
      destruct x as [[[b|t2]|[b|t]]|th|e];
        cbn [same_shape get_type HTreeObj] in Shape; try discriminate.
      assert (E2 : HTreeObj t2 = HTreeObj t2) by reflexivity.
      destruct (IH t2 E2) as [t3 [E3 Thunks]].
      rewrite E1 in E3; inversion E3; subst t3.
      exists t2; split; [exact E2|apply equiv_sym; exact Thunks].
    - destruct (IHxy t1 E1) as [t2 [E2 Thunks12]], (IHyz t2 E2) as [t3 [E3 Thunks23]].
      exists t3; split; [exact E3|eapply equiv_trans; eassumption].
  Qed.
  (** A relation-preserving Tree-to-Thunk constructor respects equivalence, after executing
      an Encode on the other side if necessary. *)
  Theorem equiv_tree_to_thunk (make_thunk : TreeName -> thunk)
      (Cong : forall t1 t2, R (HTreeObj t1) (HTreeObj t2) -> R (Thunk (make_thunk t1)) (Thunk (make_thunk t2)))
      h1 h2 : computational_equiv h1 h2 -> forall t1, h1 = HTreeObj t1 ->
      (exists t2, h2 = HTreeObj t2 /\ computational_equiv (Thunk (make_thunk t1)) (Thunk (make_thunk t2))) \/
      (exists e2 t2, h2 = Encode e2 /\ execute e2 = Some (HTreeObj t2) /\ computational_equiv (Thunk (make_thunk t1)) (Thunk (make_thunk t2))).
  Proof.
    (* The second endpoint may be an Encode, so its Tree cannot be passed to the
        constructor immediately. The Encode-removal argument supplies that Tree and its
        relation to the original operand; only then is constructor congruence applicable. *)
    intros Related t1 ->; pose proof (equiv_unencode _ _ Related) as Results; cbn [unencode] in Results.
    destruct (unencode h2) as [v2|] eqn:U2; [|contradiction].
    destruct (unencoded_equiv_tree_cong make_thunk Cong _ _ Results t1 Logic.eq_refl) as [t2 [-> Thunks]].
    destruct h2 as [d|th|e2]; cbn [unencode] in U2.
    - left; exists t2; split; [congruence|exact Thunks].
    - discriminate.
    - right; exists e2, t2; auto.
  Qed.
  (** Equivalent Tree operands give equivalent application Thunks, after executing an
      exposed Encode operand if necessary. *)
  Lemma equiv_tree_to_application_thunk h1 h2 : computational_equiv h1 h2 -> forall t1, h1 = HTreeObj t1 ->
      (exists t2, h2 = HTreeObj t2 /\ computational_equiv (Thunk (Application t1)) (Thunk (Application t2))) \/
      (exists e2 t2, h2 = Encode e2 /\ execute e2 = Some (HTreeObj t2) /\ computational_equiv (Thunk (Application t1)) (Thunk (Application t2))).
  Proof. apply (equiv_tree_to_thunk Application (application_thunk_cong R R_properties)). Qed.
  (** Equivalent Tree operands give equivalent selection Thunks, after executing an exposed
      Encode operand if necessary. *)
  Lemma equiv_tree_to_selection_thunk h1 h2 : computational_equiv h1 h2 -> forall t1, h1 = HTreeObj t1 ->
      (exists t2, h2 = HTreeObj t2 /\ computational_equiv (Thunk (Selection t1)) (Thunk (Selection t2))) \/
      (exists e2 t2, h2 = Encode e2 /\ execute e2 = Some (HTreeObj t2) /\ computational_equiv (Thunk (Selection t1)) (Thunk (Selection t2))).
  Proof. apply (equiv_tree_to_thunk Selection (selection_thunk_cong R R_properties)). Qed.

  (** A Tree-to-Thunk constructor preserving R also preserves computational equivalence
      between Tree Objects. *)
  Lemma tree_thunk_equiv (kind : TreeName -> thunk)
      (Cong : forall t1 t2, R (HTreeObj t1) (HTreeObj t2) -> R (Thunk (kind t1)) (Thunk (kind t2)))
      t1 t2 : computational_equiv (HTreeObj t1) (HTreeObj t2) -> computational_equiv (Thunk (kind t1)) (Thunk (kind t2)).
  Proof.
    intro Related; destruct (equiv_tree_to_thunk kind Cong _ _ Related t1 Logic.eq_refl) as
      [[u [E' Thunks]]|[e [u [E' Rest]]]]; [inversion E'; subst; exact Thunks|discriminate].
  Qed.

  (** R-related Data has R-related lifted Objects. *)
  Lemma data_to_lift d1 d2 : R (Data d1) (Data d2) -> relaxed_X R (Data d1) (Data d2).
  Proof. apply (related_data_strengthen R R_properties). Qed.
  (** Related lowered Data has related lifted Objects. *)
  Lemma lower_to_lift d1 d2 : R (lower (Data d1)) (lower (Data d2)) -> R (lift (Data d1)) (lift (Data d2)).
  Proof. apply R_lower_to_lift_data. Qed.
  (** Related lifted Objects have related lowered Refs. *)
  Lemma lift_to_lower d1 d2 : R (lift (Data d1)) (lift (Data d2)) -> R (lower (Data d1)) (lower (Data d2)).
  Proof. apply (E.EP.lift_to_lower R blob_ref_cong_R tree_ref_cong_R R_preserve_tree R_preserve_blob). Qed.
  (** Lowering Data does not change the Object obtained by subsequently lifting it. *)
  Lemma lower_to_lift_cancel d d' : lower (Data d) = Data d' -> lift (Data d) = lift (Data d').
  Proof.
    destruct d as [[b|t]|[b|t]], d' as [[b'|t']|[b'|t']];
      cbn [lower lower_data lift lift_data]; intro H; try discriminate; inversion H; subst; reflexivity.
  Qed.
  (** Lifting Data does not change the Ref obtained by subsequently lowering it. *)
  Lemma lift_to_lower_cancel d d' : lift (Data d) = Data d' -> lower (Data d) = lower (Data d').
  Proof.
    destruct d as [[b|t]|[b|t]], d' as [[b'|t']|[b'|t']];
      cbn [lower lower_data lift lift_data]; intro H; try discriminate; inversion H; subst; reflexivity.
  Qed.
  (** Successful shallow execution determines strict execution as the lifted result. *)
  Lemma execute_shallow_to_lift th d : execute (Shallow th) = Some (Data d) ->
    execute (Strict th) = Some (lift (Data d)).
  Proof.
    intro Exec; rewrite execute_unfold in Exec; cbn in Exec; apply omap_some in Exec.
    destruct Exec as [h [Force Result]]; destruct (force_data _ _ Force) as [d' ->].
    rewrite execute_unfold; cbn; rewrite Force; cbn [omap]; f_equal.
    apply lower_to_lift_cancel; symmetry; exact Result.
  Qed.
  (** Successful strict execution determines shallow execution as the lowered result. *)
  Lemma execute_strict_to_lower th d : execute (Strict th) = Some (Data d) ->
    execute (Shallow th) = Some (lower (Data d)).
  Proof.
    intro Exec; rewrite execute_unfold in Exec; cbn in Exec; apply omap_some in Exec.
    destruct Exec as [h [Force Result]]; destruct (force_data _ _ Force) as [d' ->].
    rewrite execute_unfold; cbn; rewrite Force; cbn [omap]; f_equal.
    apply lift_to_lower_cancel; symmetry; exact Result.
  Qed.
  (** R-related shallow Encodes have R-related strict counterparts. *)
  Lemma R_shallow_encode_to_strict_encode t1 t2 : R (Encode (Shallow t1)) (Encode (Shallow t2)) ->
    R (Encode (Strict t1)) (Encode (Strict t2)).
  Proof.
    intro Related; apply R'_impl_R, forces_to_strict_R', rel_opt_relaxed_R_to_R'.
    exact (R_encode_to_force (Shallow t1) (Shallow t2) Related).
  Qed.
  (** R-related strict Encodes have R-related shallow counterparts. *)
  Lemma R_strict_encode_to_shallow_encode t1 t2 : R (Encode (Strict t1)) (Encode (Strict t2)) ->
    R (Encode (Shallow t1)) (Encode (Shallow t2)).
  Proof.
    intro Related; pose proof (R'_force_execute_shallow t1 t2
      (rel_opt_relaxed_R_to_R' _ _ (R_encode_to_force (Strict t1) (Strict t2) Related))) as Exec.
    destruct (execute (Shallow t1)) as [h1|] eqn:E1, (execute (Shallow t2)) as [h2|] eqn:E2;
      cbn [rel_opt] in Exec; try contradiction.
    - apply R'_impl_R; eapply R'_encode_some_res; eassumption.
    - apply REvalShallowNone; assumption.
  Qed.
  Definition lower_data_and_encode h := match h with
    | Data d => lower (Data d) | Thunk th => Thunk th
    | Encode e => Encode (Shallow (encode_to_thunk e)) end.
  Definition lift_data_and_encode h := match h with
    | Data d => lift (Data d) | Thunk th => Thunk th
    | Encode e => Encode (Strict (encode_to_thunk e)) end.
  (** The shallow form of a successful Encode returns its lowered result. *)
  Lemma execute_to_lower e h : execute e = Some h ->
    execute (Shallow (encode_to_thunk e)) = Some (lower h).
  Proof.
    intro Exec; destruct (execute_data _ _ Exec) as [d ->]; destruct e as [th|th].
    - apply execute_strict_to_lower; exact Exec.
    - destruct (execute_shallow_to_ref _ _ Exec) as [r Result]; inversion Result; subst d; exact Exec.
  Qed.
  Ltac reject_thunk_data Related :=
    first [eapply (data_not_thunk R R_properties); exact Related|
           eapply (thunk_not_data R R_properties); exact Related].
  (** Lowering Data and changing strict Encodes to shallow ones preserves R. *)
  Lemma R_to_lower h1 h2 : R h1 h2 -> R (lower_data_and_encode h1) (lower_data_and_encode h2).
  Proof.
    intro Related; destruct h1 as [d1|th1|e1], h2 as [d2|th2|e2]; cbn [lower_data_and_encode].
    - apply lift_to_lower, data_to_lift; exact Related.
    - exfalso; reject_thunk_data Related.
    - exfalso; eapply R_encode_execute_rev_does_not_exist; [exact Related|intros [e E']; discriminate].
    - exfalso; reject_thunk_data Related.
    - exact Related.
    - exfalso; eapply R_encode_execute_rev_does_not_exist; [exact Related|intros [e E']; discriminate].
    - apply REvalStep, execute_to_lower, execute_unique.
      apply R_encode_execute; [exact Related|intros [e E']; discriminate].
    - exfalso; destruct (proj2 (R_preserve_thunk _ _ Related) (ex_intro _ th2 Logic.eq_refl)) as [th E']; discriminate.
    - destruct e1 as [th1|th1], e2 as [th2|th2]; cbn [encode_to_thunk].
      + apply R_strict_encode_to_shallow_encode; exact Related.
      + exfalso; exact (R_not_strict_shallow _ _ Related).
      + exfalso; exact (R_not_shallow_strict _ _ Related).
      + exact Related.
  Qed.
  (** Lifting Data and changing shallow Encodes to strict ones preserves R. *)
  Lemma R_to_lift h1 h2 : R h1 h2 -> R (lift_data_and_encode h1) (lift_data_and_encode h2).
  Proof.
    intro Related; destruct h1 as [d1|th1|e1], h2 as [d2|th2|e2]; cbn [lift_data_and_encode].
    - apply data_to_lift; exact Related.
    - exfalso; reject_thunk_data Related.
    - exfalso; eapply R_encode_execute_rev_does_not_exist; [exact Related|intros [e E']; discriminate].
    - exfalso; reject_thunk_data Related.
    - exact Related.
    - exfalso; eapply R_encode_execute_rev_does_not_exist; [exact Related|intros [e E']; discriminate].
    - apply REvalStep, execute_to_lift, execute_unique.
      apply R_encode_execute; [exact Related|intros [e E']; discriminate].
    - exfalso; destruct (proj2 (R_preserve_thunk _ _ Related) (ex_intro _ th2 Logic.eq_refl)) as [th E']; discriminate.
    - destruct e1 as [th1|th1], e2 as [th2|th2]; cbn [encode_to_thunk].
      + exact Related.
      + exfalso; exact (R_not_strict_shallow _ _ Related).
      + exfalso; exact (R_not_shallow_strict _ _ Related).
      + apply R_shallow_encode_to_strict_encode; exact Related.
  Qed.
  (** Applying the same lowering operation to both endpoints preserves their equivalence.
      This is a congruence property of changing how Data is exposed and how Encodes
      consume their results. *)
  Theorem equiv_to_lower h1 h2 : computational_equiv h1 h2 -> computational_equiv (lower_data_and_encode h1) (lower_data_and_encode h2).
  Proof.
    apply (closure_map _ _ lower_data_and_encode); auto using R_to_equiv, R_to_lower.
  Qed.
  (** Applying the same lifting operation to both endpoints preserves their equivalence.
      Thus a derivation can be reused when its consumer requests Objects rather than Refs. *)
  Theorem equiv_to_lift h1 h2 : computational_equiv h1 h2 -> computational_equiv (lift_data_and_encode h1) (lift_data_and_encode h2).
  Proof.
    apply (closure_map _ _ lift_data_and_encode); auto using R_to_equiv, R_to_lift.
  Qed.
  Definition identification_handle h := match h with Data d => Thunk (Identification (data_reference d)) | _ => h end.
  (** Replacing non-Encode Data by identification Thunks for their underlying Refs
      preserves R. *)
  Lemma unencoded_R_identification h1 h2 : unencoded_R h1 h2 ->
    R (identification_handle h1) (identification_handle h2).
  Proof.
    intros [Related [N1 N2]]; destruct h1 as [d1|th1|e1], h2 as [d2|th2|e2]; cbn [identification_handle];
      try solve [exfalso; apply N1; eauto]; try solve [exfalso; apply N2; eauto].
    - apply (identification_thunk_cong R R_properties).
      rewrite <- (lower_data_reference d1), <- (lower_data_reference d2).
      apply lift_to_lower, data_to_lift; exact Related.
    - exfalso; reject_thunk_data Related.
    - exfalso; reject_thunk_data Related.
    - exact Related.
  Qed.
  (** The identification-handle construction preserves equivalence restricted to non-Encode
      steps. *)
  Lemma unencoded_equiv_identification h1 h2 : unencoded_equiv h1 h2 ->
    computational_equiv (identification_handle h1) (identification_handle h2).
  Proof.
    apply (closure_map _ _ identification_handle); auto using R_to_equiv, unencoded_R_identification.
  Qed.
  (** A Thunk thinking to Data is equivalent to identification of the corresponding lifted
      Object's Ref. *)
  Lemma think_lift_identification th d : think th = Some (Data d) ->
    computational_equiv (Thunk th) (Thunk (Identification (data_reference (lift_data d)))).
  Proof.
    (* A Think step that returns Data and identification of that Data's Object have the
        same lifted result. The rule comparing Think results therefore supplies a common
        representative for these computations. *)
    intro Think; apply R_to_equiv; eapply RThunkSomeResData.
    - exact Think.
    - rewrite think_identification; reflexivity.
    - destruct d as [[b|t]|[b|t]]; apply RSelf.
  Qed.
  (** Thunks whose Think results have equivalent lifted Objects are computationally
      equivalent. *)
  Lemma equiv_lifted_data_to_think th1 th2 d1 d2 :
    think th1 = Some (Data d1) -> think th2 = Some (Data d2) ->
    computational_equiv (lift (Data d1)) (lift (Data d2)) -> computational_equiv (Thunk th1) (Thunk th2).
  Proof.
    (* Use identification as the common representation of a completed Think result. The
        supplied equivalence relates the two Object results, and identification congruence
        carries that equivalence back to their source Thunks. *)
    intros First Second Related.
    assert (ResultsIdentified : computational_equiv
      (Thunk (Identification (data_reference (lift_data d1))))
      (Thunk (Identification (data_reference (lift_data d2)))))
      by (hauto cases: - use: equiv_unencode, unencoded_equiv_identification).
    assert (FirstIdentified : computational_equiv (Thunk th1)
      (Thunk (Identification (data_reference (lift_data d1)))))
      by (hauto cases: - use: think_lift_identification).
    assert (SecondIdentified : computational_equiv (Thunk th2)
      (Thunk (Identification (data_reference (lift_data d2)))))
      by (hauto cases: - use: think_lift_identification).
    hauto cases: - use: equiv_sym, equiv_trans.
  Qed.
  (** A handle relaxed-R-related to Data has a lifted form equivalent to that Data's lifted
      Object. *)
  Lemma relaxed_data_lifted h d : relaxed_X R h (Data d) \/ relaxed_X R (Data d) h ->
    computational_equiv (lift_data_and_encode h) (lift (Data d)).
  Proof.
    intros [Forward|Backward].
    - destruct h as [d'|th|[th|th]]; apply R_to_equiv; exact Forward.
    - destruct h as [d'|th|e].
      + apply equiv_sym, R_to_equiv; exact Backward.
      + exfalso; exact (data_not_thunk R R_properties (lift_data d) th Backward).
      + exfalso; exact (data_not_encode R R_properties (lift_data d) e Backward).
  Qed.
  (** Equivalent witnesses relating two Think results through relaxed R establish
      equivalence of the original Thunks. *)
  Theorem think_to_equiv h1 h2 : computational_equiv h1 h2 -> forall th1 th2 d1 d2,
    think th1 = Some (Data d1) -> think th2 = Some (Data d2) ->
    (relaxed_X R h1 (Data d1) \/ relaxed_X R (Data d1) h1) /\
    (relaxed_X R h2 (Data d2) \/ relaxed_X R (Data d2) h2) -> computational_equiv (Thunk th1) (Thunk th2).
  Proof.
    intros Related th1 th2 d1 d2 First Second [Left Right].
    assert (LeftLifted : computational_equiv (lift_data_and_encode h1) (lift (Data d1)))
      by (hauto cases: - use: relaxed_data_lifted).
    assert (RightLifted : computational_equiv (lift_data_and_encode h2) (lift (Data d2)))
      by (hauto cases: - use: relaxed_data_lifted).
    assert (ResultsRelated : computational_equiv (lift (Data d1)) (lift (Data d2)))
      by (hauto cases: - use: equiv_to_lift, equiv_sym, equiv_trans).
    hauto cases: - use: equiv_lifted_data_to_think.
  Qed.
  Definition create_encode (kind : thunk -> encode) h := match h with Thunk th => Encode (kind th) | _ => h end.
  Definition create_strict_encode := create_encode Strict.
  Definition create_shallow_encode := create_encode Shallow.
  (** An Encode constructor preserving R on Thunks preserves R on general handles. *)
  Lemma R_create_encode (kind : thunk -> encode)
      (Cong : forall th1 th2, R (Thunk th1) (Thunk th2) -> R (Encode (kind th1)) (Encode (kind th2)))
      h1 h2 : R h1 h2 -> R (create_encode kind h1) (create_encode kind h2).
  Proof.
    intro Related; destruct h1 as [d1|th1|e1], h2 as [d2|th2|e2]; cbn [create_encode]; try exact Related.
    - exfalso; reject_thunk_data Related.
    - exfalso; reject_thunk_data Related.
    - apply Cong; exact Related.
    - exfalso; destruct (proj1 (R_preserve_thunk _ _ Related) (ex_intro _ th1 Logic.eq_refl)) as [th E']; discriminate.
    - exfalso; destruct (proj2 (R_preserve_thunk _ _ Related) (ex_intro _ th2 Logic.eq_refl)) as [th E']; discriminate.
  Qed.
  (** An Encode constructor preserving R on Thunks preserves computational equivalence on
      general handles. *)
  Lemma equiv_create_encode (kind : thunk -> encode)
      (Cong : forall th1 th2, R (Thunk th1) (Thunk th2) -> R (Encode (kind th1)) (Encode (kind th2)))
      h1 h2 : computational_equiv h1 h2 -> computational_equiv (create_encode kind h1) (create_encode kind h2).
  Proof.
    apply (closure_map _ _ (create_encode kind)); auto using R_to_equiv, R_create_encode.
  Qed.
  (** Constructing strict Encodes from equivalent Thunks preserves computational
      equivalence. *)
  Lemma equiv_thunk_to_strict_encode t1 t2 : computational_equiv (Thunk t1) (Thunk t2) ->
    computational_equiv (create_strict_encode (Thunk t1)) (create_strict_encode (Thunk t2)).
  Proof. apply (equiv_create_encode Strict (strict_encode_cong R R_properties)). Qed.
  (** Constructing shallow Encodes from equivalent Thunks preserves computational
      equivalence. *)
  Lemma equiv_thunk_to_shallow_encode t1 t2 : computational_equiv (Thunk t1) (Thunk t2) ->
    computational_equiv (create_shallow_encode (Thunk t1)) (create_shallow_encode (Thunk t2)).
  Proof. apply (equiv_create_encode Shallow (shallow_encode_cong R R_properties)). Qed.
  (** A non-Encode handle equivalent to a Blob Object is a Blob Object with identical
      bytes. *)
  Lemma unencoded_equiv_blob_cong h1 h2 : unencoded_equiv h1 h2 -> forall b1, h1 = HBlobObj b1 ->
    exists b2, h2 = HBlobObj b2 /\ get_blob_data b1 = get_blob_data b2.
  Proof.
    intro Related; induction Related as [x y Rxy|x|x y Related IH|x y z Rxy IHxy Ryz IHyz]; intros b1 E1.
    - subst x; destruct Rxy as [Rel _]; destruct (R_preserve_blob _ _ Rel) as [b2 ->].
      exists b2; split; [reflexivity|apply blob_cong_R; exact Rel].
    - subst x; exists b1; auto.
    - pose proof (unencoded_equiv_shape _ _ Related) as Shape; rewrite E1 in Shape.
      destruct x as [[[b2|t]|[b|u]]|th|e];
        cbn [same_shape get_type HBlobObj] in Shape; try discriminate.
      assert (E2 : HBlobObj b2 = HBlobObj b2) by reflexivity.
      destruct (IH b2 E2) as [b3 [E3 Data]].
      rewrite E1 in E3; inversion E3; subst b3; exists b2; split; [exact E2|symmetry; exact Data].
    - destruct (IHxy b1 E1) as [b2 [E2 Data12]], (IHyz b2 E2) as [b3 [E3 Data23]].
      exists b3; split; [exact E3|congruence].
  Qed.
