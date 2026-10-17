From Stdlib Require Import List Relations Relation_Operators.
From FixProof Require Import Handle ApplyTree Equivalence.
Import ListNotations.

Module EquivalenceClosure (S : STORAGE) (P : PROGRAM S).
  Module E := Equivalence S P.
  Import E E.EP E.EP.E E.EP.E.H.
  Definition eq := clos_refl_sym_trans handle R.
  Lemma eq_refl h : eq h h.
  Proof. apply rst_refl. Qed.
  Lemma eq_sym h1 h2 : eq h1 h2 -> eq h2 h1.
  Proof. apply rst_sym. Qed.
  Lemma eq_trans h1 h2 h3 : eq h1 h2 -> eq h2 h3 -> eq h1 h3.
  Proof. apply rst_trans. Qed.
  Lemma R_to_eq h1 h2 : R h1 h2 -> eq h1 h2.
  Proof. apply rst_step. Qed.
  Lemma R_tree_update1 a b pre post :
    R a b -> R (HTreeObj (create_tree (pre ++ a :: post))) (HTreeObj (create_tree (pre ++ b :: post))).
  Proof.
    intro H; apply tree_complete_R.
    assert (Forall2 R post post) as Tail by (induction post; constructor; auto using RSelf).
    induction pre; simpl; constructor; auto using RSelf.
  Qed.
  Lemma equivclp_tree_update1 a b pre post :
    eq a b -> eq (HTreeObj (create_tree (pre ++ a :: post))) (HTreeObj (create_tree (pre ++ b :: post))).
  Proof.
    intro H; induction H.
    - apply R_to_eq, R_tree_update1; assumption.
    - apply eq_refl.
    - apply eq_sym; assumption.
    - eapply eq_trans; eassumption.
  Qed.
  Lemma equivclp_tree_list_all2_prefix xs ys :
    Forall2 eq xs ys -> forall pre,
    eq (HTreeObj (create_tree (pre ++ xs))) (HTreeObj (create_tree (pre ++ ys))).
  Proof.
    intro H; induction H; intro pre; [apply eq_refl|].
    eapply eq_trans.
    - apply equivclp_tree_update1; exact H.
    - specialize (IHForall2 (pre ++ [y])).
      rewrite <- !app_assoc in IHForall2; simpl in IHForall2; exact IHForall2.
  Qed.
  Lemma eq_tree_list_all2 xs ys : Forall2 eq xs ys ->
    eq (HTreeObj (create_tree xs)) (HTreeObj (create_tree ys)).
  Proof. intro H; exact (equivclp_tree_list_all2_prefix xs ys H []). Qed.
  Lemma eq_preserve_thunk h1 h2 : eq h1 h2 ->
    ((exists th, h1 = Thunk th) <-> (exists th, h2 = Thunk th)).
  Proof.
    intro Related; induction Related.
    - apply R_preserve_thunk; assumption.
    - reflexivity.
    - tauto.
    - tauto.
  Qed.
  Lemma rel_opt_eq_refl x : rel_opt eq x x.
  Proof. destruct x; cbn [rel_opt]; auto using eq_refl. Qed.
  Lemma rel_opt_eq_sym x y : rel_opt eq x y -> rel_opt eq y x.
  Proof. destruct x, y; cbn [rel_opt]; auto using eq_sym. Qed.
  Lemma rel_opt_eq_trans x y z : rel_opt eq x y -> rel_opt eq y z -> rel_opt eq x z.
  Proof. destruct x, y, z; cbn [rel_opt]; intros; try contradiction; eauto using eq_trans. Qed.
  Lemma rel_opt_R_to_eq x y : rel_opt R x y -> rel_opt eq x y.
  Proof. destruct x, y; cbn [rel_opt]; auto using R_to_eq. Qed.
  Theorem eq_eval h1 h2 : eq h1 h2 -> rel_opt eq (eval h1) (eval h2).
  Proof.
    intro Related; induction Related.
    - apply rel_opt_R_to_eq, eval_R; assumption.
    - apply rel_opt_eq_refl.
    - apply rel_opt_eq_sym; assumption.
    - eapply rel_opt_eq_trans; eassumption.
  Qed.
  Theorem eval_with_fuel_to_R_single n : forall h r, eval_with_fuel n h = Some r -> eq h r.
  Proof.
    induction n as [|n IH]; intros h r Fuel.
    - destruct h as [[[b|t]|ref]|th|e]; cbn [eval_with_fuel with_fuel eval_fn] in Fuel;
        try discriminate; inversion Fuel; subst; apply eq_refl.
    - destruct h as [[[b|t]|ref]|th|e].
      + inversion Fuel; subst; apply eq_refl.
      + change (omap HTreeObj (eval_tree_with_fuel n t) = Some r) in Fuel.
        apply omap_some in Fuel; destruct Fuel as [t' [Tree ->]].
        unfold eval_tree_with_fuel, eval_tree_using in Tree; apply omap_some in Tree.
        destruct Tree as [ys [List ->]].
        change (eq (HTreeObj t) (HTreeObj (create_tree ys))).
        rewrite <- (create_tree_get_tree_raw t) at 1; apply eq_tree_list_all2.
        apply eval_list_to_list_all in List; induction List; constructor; eauto.
      + inversion Fuel; subst; apply eq_refl.
      + inversion Fuel; subst; apply eq_refl.
      + change (obind (execute_with_fuel n e) (eval_with_fuel n) = Some r) in Fuel.
        apply obind_some in Fuel; destruct Fuel as [h' [Exec Ev]].
        eapply eq_trans; [apply R_to_eq, REvalStep; apply execute_unique; exists n; exact Exec|apply IH; exact Ev].
  Qed.
  Lemma eval_to_eq h r : eval h = Some r -> eq h r.
  Proof. intro Ev; apply eval_some in Ev; destruct Ev as [n Ev]; eapply eval_with_fuel_to_R_single; exact Ev. Qed.
  Lemma eval_both_to_eq h1 h2 r1 r2 : eval h1 = Some r1 -> eval h2 = Some r2 -> eq r1 r2 -> eq h1 h2.
  Proof.
    intros Ev1 Ev2 Related; eapply eq_trans; [apply eval_to_eq; exact Ev1|].
    eapply eq_trans; [exact Related|apply eq_sym, eval_to_eq; exact Ev2].
  Qed.
  Lemma relaxed_R_to_eq h1 h2 : relaxed_X R h1 h2 -> relaxed_X eq h1 h2.
  Proof. destruct h1 as [d|th|[th|th]]; apply R_to_eq. Qed.
  Lemma rel_opt_relaxed_R_to_eq x y : rel_opt (relaxed_X R) x y -> rel_opt (relaxed_X eq) x y.
  Proof. destruct x, y; cbn [rel_opt]; auto using relaxed_R_to_eq. Qed.
  Lemma force_relaxed_eq_refl th : rel_opt (relaxed_X eq) (force th) (force th).
  Proof.
    destruct (force th) as [h|] eqn:F; [|exact I].
    destruct (force_data _ _ F) as [d ->]; apply eq_refl.
  Qed.
  Lemma force_relaxed_eq_sym th1 th2 : rel_opt (relaxed_X eq) (force th1) (force th2) ->
    rel_opt (relaxed_X eq) (force th2) (force th1).
  Proof.
    intro Related; destruct (force th1) as [h1|] eqn:F1, (force th2) as [h2|] eqn:F2;
      cbn [rel_opt] in *; try contradiction; [|exact I].
    destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->].
    apply eq_sym; exact Related.
  Qed.
  Lemma force_relaxed_eq_trans th1 th2 th3 : rel_opt (relaxed_X eq) (force th1) (force th2) ->
    rel_opt (relaxed_X eq) (force th2) (force th3) -> rel_opt (relaxed_X eq) (force th1) (force th3).
  Proof.
    intros R12 R23; destruct (force th1) as [h1|] eqn:F1, (force th2) as [h2|] eqn:F2,
      (force th3) as [h3|] eqn:F3; cbn [rel_opt] in *; try contradiction; [|exact I].
    destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->], (force_data _ _ F3) as [d3 ->].
    eapply eq_trans; [exact R12|exact R23].
  Qed.
  Theorem force_eq h1 h2 : eq h1 h2 -> forall th1, h1 = Thunk th1 ->
    exists th2, h2 = Thunk th2 /\ rel_opt (relaxed_X eq) (force th1) (force th2).
  Proof.
    intro Related; induction Related as [x y Rxy|x|x y Related IH|x y z Rxy IHxy Ryz IHyz]; intros th1 E1.
    - subst x; destruct (proj1 (R_preserve_thunk _ _ Rxy) (ex_intro _ th1 Logic.eq_refl)) as [th2 ->].
      exists th2; split; [reflexivity|apply rel_opt_relaxed_R_to_eq, force_R; exact Rxy].
    - subst x; exists th1; split; [reflexivity|apply force_relaxed_eq_refl].
    - destruct (proj2 (eq_preserve_thunk _ _ Related) (ex_intro _ th1 E1)) as [th2 E2].
      destruct (IH th2 E2) as [th3 [E3 Forces]].
      rewrite E1 in E3; inversion E3; subst th3.
      exists th2; split; [exact E2|apply force_relaxed_eq_sym; exact Forces].
    - destruct (IHxy th1 E1) as [th2 [E2 F12]], (IHyz th2 E2) as [th3 [E3 F23]].
      exists th3; split; [exact E3|eapply force_relaxed_eq_trans; eassumption].
  Qed.
  Lemma force_with_fuel_to_the_last_thunk n : forall th h, force_with_fuel n th = Some h ->
    exists th', think th' = Some h /\ eq (Thunk th) (Thunk th').
  Proof.
    induction n as [|n IH]; intros th h Fuel; [discriminate|].
    change (obind (think_with_fuel n th) (force_after n) = Some h) in Fuel.
    apply obind_some in Fuel; destruct Fuel as [r [Think Reply]].
    pose proof (think_unique _ _ (ex_intro _ n Think)) as Think'.
    destruct r as [d|th1|[th1|th1]].
    - inversion Reply; subst h; exists th; split; [exact Think'|apply eq_refl].
    - destruct (IH th1 h Reply) as [th' [Last Related]].
      exists th'; split; [exact Last|].
      eapply eq_trans; [apply R_to_eq, RThinkSingleStepThunk; exact Think'|exact Related].
    - destruct (IH th1 h Reply) as [th' [Last Related]].
      exists th'; split; [exact Last|].
      eapply eq_trans; [apply R_to_eq, RThunkSingleStepEncodeStrict; exact Think'|exact Related].
    - destruct (IH th1 h Reply) as [th' [Last Related]].
      exists th'; split; [exact Last|].
      eapply eq_trans; [apply R_to_eq, RThunkSingleStepEncodeShallow; exact Think'|exact Related].
  Qed.
  Lemma force_to_the_last_thunk th h : force th = Some h ->
    exists th', think th' = Some h /\ eq (Thunk th) (Thunk th').
  Proof. intro F; apply force_some in F; destruct F as [n F]; eapply force_with_fuel_to_the_last_thunk; exact F. Qed.
  Lemma thunk_for_all_data d : exists th, think th = Some (Data d).
  Proof. exists (Identification d); rewrite think_identification; reflexivity. Qed.
  Definition unencode h := match h with Encode e => execute e | _ => Some h end.
  Definition unencoded h := ~(exists e, h = Encode e).
  Definition unencoded_R h1 h2 := R h1 h2 /\ unencoded h1 /\ unencoded h2.
  Definition unencoded_eq := clos_refl_sym_trans handle unencoded_R.

  Lemma unencode_some_unencoded h v : unencode h = Some v -> unencoded v.
  Proof.
    destruct h as [d|th|e]; cbn [unencode]; intro Result.
    - inversion Result; subst; intros [e E']; discriminate.
    - inversion Result; subst; intros [e E']; discriminate.
    - destruct (execute_data _ _ Result) as [d ->]; intros [e' E']; discriminate.
  Qed.
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
  Lemma R_unencode_safe h1 h2 : R h1 h2 -> rel_opt unencoded_R (unencode h1) (unencode h2).
  Proof.
    intro Related; pose proof (R_unencode _ _ Related) as Result.
    destruct (unencode h1) as [v1|] eqn:U1, (unencode h2) as [v2|] eqn:U2;
      cbn [rel_opt] in *; try contradiction; [|exact I].
    split; [exact Result|split; eapply unencode_some_unencoded; eassumption].
  Qed.
  Lemma rel_opt_unencoded_refl x : rel_opt unencoded_eq x x.
  Proof. destruct x; [apply rst_refl|exact I]. Qed.
  Lemma rel_opt_unencoded_sym x y : rel_opt unencoded_eq x y -> rel_opt unencoded_eq y x.
  Proof. destruct x, y; cbn [rel_opt]; intro H; try contradiction; [apply rst_sym; exact H|exact I]. Qed.
  Lemma rel_opt_unencoded_trans x y z : rel_opt unencoded_eq x y -> rel_opt unencoded_eq y z -> rel_opt unencoded_eq x z.
  Proof. destruct x, y, z; cbn [rel_opt]; intros; try contradiction; [eapply rst_trans; eassumption|exact I]. Qed.
  Lemma unencoded_eq_to_eq h1 h2 : unencoded_eq h1 h2 -> eq h1 h2.
  Proof.
    intro Related; induction Related.
    - apply R_to_eq; exact (proj1 H).
    - apply eq_refl.
    - apply eq_sym; assumption.
    - eapply eq_trans; eassumption.
  Qed.
  Theorem eq_unencode h1 h2 : eq h1 h2 -> rel_opt unencoded_eq (unencode h1) (unencode h2).
  Proof.
    intro Related; induction Related.
    - pose proof (R_unencode_safe _ _ H) as Safe.
      destruct (unencode x), (unencode y); cbn [rel_opt] in *; try contradiction;
        [apply rst_step; exact Safe|exact I].
    - apply rel_opt_unencoded_refl.
    - apply rel_opt_unencoded_sym; assumption.
    - eapply rel_opt_unencoded_trans; eassumption.
  Qed.
  Lemma eq_unencode_general h1 h2 : eq h1 h2 -> rel_opt eq (unencode h1) (unencode h2).
  Proof.
    intro Related; pose proof (eq_unencode _ _ Related) as Result.
    destruct (unencode h1), (unencode h2); cbn [rel_opt] in *; auto using unencoded_eq_to_eq.
  Qed.
  Theorem eq_encode_not_encode h1 h2 : eq h1 h2 -> forall e, h1 = Encode e ->
    (unencoded h2 -> exists r1, execute e = Some r1 /\ eq r1 h2) /\
    ((exists e2, h2 = Encode e2) -> exists e2, h2 = Encode e2 /\ rel_opt eq (execute e) (execute e2)).
  Proof.
    intros Related e ->; pose proof (eq_unencode_general _ _ Related) as Result; cbn [unencode] in Result; split.
    - intro N2; destruct h2 as [d|th|e2]; [| |exfalso; apply N2; eauto];
        destruct (execute e) as [r1|] eqn:Exec; cbn [rel_opt] in Result; try contradiction;
        exists r1; split; first [reflexivity|exact Result].
    - intros [e2 ->]; exists e2; split; [reflexivity|exact Result].
  Qed.
  Lemma same_shape_refl h : same_shape h h.
  Proof. repeat split; auto. Qed.
  Lemma same_shape_sym h1 h2 : same_shape h1 h2 -> same_shape h2 h1.
  Proof. unfold same_shape; tauto. Qed.
  Lemma same_shape_trans h1 h2 h3 : same_shape h1 h2 -> same_shape h2 h3 -> same_shape h1 h3.
  Proof. unfold same_shape; tauto. Qed.
  Lemma unencoded_eq_shape h1 h2 : unencoded_eq h1 h2 -> same_shape h1 h2.
  Proof.
    intro Related; induction Related.
    - destruct H as [Rel [N1 N2]]; exact (value_shapes R R_properties _ _ Rel N1 N2).
    - apply same_shape_refl.
    - apply same_shape_sym; assumption.
    - eapply same_shape_trans; eassumption.
  Qed.
  Lemma unencode_same_shape h1 h2 : eq h1 h2 -> unencoded h1 ->
    exists v2, unencode h2 = Some v2 /\ same_shape h1 v2.
  Proof.
    intros Related N1; pose proof (eq_unencode _ _ Related) as Result.
    destruct h1 as [d|th|e]; [| |exfalso; apply N1; eauto]; cbn [unencode] in Result;
      destruct (unencode h2) as [v2|] eqn:U2; cbn [rel_opt] in Result; try contradiction;
      exists v2; split; first [reflexivity|apply unencoded_eq_shape; exact Result].
  Qed.
  Lemma eq_preserve_tree_or_encode h1 h2 : eq h1 h2 -> forall t1, h1 = HTreeObj t1 ->
    (exists t2, h2 = HTreeObj t2) \/
    (exists th t2, h2 = Encode (Strict th) /\ execute (Strict th) = Some (HTreeObj t2)).
  Proof.
    intros Related t1 ->; destruct (unencode_same_shape _ _ Related ltac:(intros [e E']; discriminate)) as
      [v2 [U2 [_ [Tree _]]]].
    destruct (proj1 Tree (ex_intro _ t1 Logic.eq_refl)) as [t2 ->].
    destruct h2 as [d|th|e]; cbn [unencode] in U2.
    - left; exists t2; congruence.
    - discriminate.
    - destruct (execute_to_obj_strict _ _ U2) as [th ->]; right; exists th, t2; auto.
  Qed.
  Lemma eq_preserve_blob_or_encode h1 h2 : eq h1 h2 -> forall b1, h1 = HBlobObj b1 ->
    (exists b2, h2 = HBlobObj b2) \/
    (exists th b2, h2 = Encode (Strict th) /\ execute (Strict th) = Some (HBlobObj b2)).
  Proof.
    intros Related b1 ->; destruct (unencode_same_shape _ _ Related ltac:(intros [e E']; discriminate)) as
      [v2 [U2 [Blob _]]].
    destruct (proj1 Blob (ex_intro _ b1 Logic.eq_refl)) as [b2 ->].
    destruct h2 as [d|th|e]; cbn [unencode] in U2.
    - left; exists b2; congruence.
    - discriminate.
    - destruct (execute_to_obj_strict _ _ U2) as [th ->]; right; exists th, b2; auto.
  Qed.
  Lemma eq_preserve_tree_ref_or_encode h1 h2 : eq h1 h2 -> forall t1, h1 = HTreeRef t1 ->
    (exists t2, h2 = HTreeRef t2) \/
    (exists th t2, h2 = Encode (Shallow th) /\ execute (Shallow th) = Some (HTreeRef t2)).
  Proof.
    intros Related t1 ->; destruct (unencode_same_shape _ _ Related ltac:(intros [e E']; discriminate)) as
      [v2 [U2 [_ [_ [_ [TreeRef _]]]]]].
    destruct (proj1 TreeRef (ex_intro _ t1 Logic.eq_refl)) as [t2 ->].
    destruct h2 as [d|th|e]; cbn [unencode] in U2.
    - left; exists t2; congruence.
    - discriminate.
    - destruct (execute_to_ref_shallow _ _ U2) as [th ->]; right; exists th, t2; auto.
  Qed.
  Lemma eq_preserve_blob_ref_or_encode h1 h2 : eq h1 h2 -> forall b1, h1 = HBlobRef b1 ->
    (exists b2, h2 = HBlobRef b2) \/
    (exists th b2, h2 = Encode (Shallow th) /\ execute (Shallow th) = Some (HBlobRef b2)).
  Proof.
    intros Related b1 ->; destruct (unencode_same_shape _ _ Related ltac:(intros [e E']; discriminate)) as
      [v2 [U2 [_ [_ [BlobRef _]]]]].
    destruct (proj1 BlobRef (ex_intro _ b1 Logic.eq_refl)) as [b2 ->].
    destruct h2 as [d|th|e]; cbn [unencode] in U2.
    - left; exists b2; congruence.
    - discriminate.
    - destruct (execute_to_ref_shallow _ _ U2) as [th ->]; right; exists th, b2; auto.
  Qed.
  Lemma R_or_to_eq h1 h2 : R h1 h2 \/ R h2 h1 -> eq h1 h2.
  Proof. intros [Related|Related]; [apply R_to_eq|apply eq_sym, R_to_eq]; exact Related. Qed.
  Lemma R_or_preserve_tree y z : R y z \/ R z y -> (exists t, y = HTreeObj t) ->
    (exists t2, z = HTreeObj t2) \/
    (exists th t2, z = Encode (Strict th) /\ execute (Strict th) = Some (HTreeObj t2)).
  Proof. intros Related [t E']; eapply eq_preserve_tree_or_encode; [apply R_or_to_eq; exact Related|exact E']. Qed.
  Lemma R_or_preserve_blob y z : R y z \/ R z y -> (exists b, y = HBlobObj b) ->
    (exists b2, z = HBlobObj b2) \/
    (exists th b2, z = Encode (Strict th) /\ execute (Strict th) = Some (HBlobObj b2)).
  Proof. intros Related [b E']; eapply eq_preserve_blob_or_encode; [apply R_or_to_eq; exact Related|exact E']. Qed.
  Lemma R_or_preserve_tree_ref y z : R y z \/ R z y -> (exists t, y = HTreeRef t) ->
    (exists t2, z = HTreeRef t2) \/
    (exists th t2, z = Encode (Shallow th) /\ execute (Shallow th) = Some (HTreeRef t2)).
  Proof. intros Related [t E']; eapply eq_preserve_tree_ref_or_encode; [apply R_or_to_eq; exact Related|exact E']. Qed.
  Lemma R_or_preserve_blob_ref y z : R y z \/ R z y -> (exists b, y = HBlobRef b) ->
    (exists b2, z = HBlobRef b2) \/
    (exists th b2, z = Encode (Shallow th) /\ execute (Shallow th) = Some (HBlobRef b2)).
  Proof. intros Related [b E']; eapply eq_preserve_blob_ref_or_encode; [apply R_or_to_eq; exact Related|exact E']. Qed.
  Lemma unencoded_eq_tree_cong (make_thunk : TreeName -> thunk)
      (Cong : forall t1 t2, R (HTreeObj t1) (HTreeObj t2) -> R (Thunk (make_thunk t1)) (Thunk (make_thunk t2)))
      h1 h2 : unencoded_eq h1 h2 -> forall t1, h1 = HTreeObj t1 ->
      exists t2, h2 = HTreeObj t2 /\ eq (Thunk (make_thunk t1)) (Thunk (make_thunk t2)).
  Proof.
    intro Related; induction Related as [x y Rxy|x|x y Related IH|x y z Rxy IHxy Ryz IHyz]; intros t1 E1.
    - subst x; destruct Rxy as [Rxy _]; destruct (R_preserve_tree _ _ Rxy) as [t2 ->].
      exists t2; split; [reflexivity|apply R_to_eq, Cong; exact Rxy].
    - subst x; exists t1; split; [reflexivity|apply eq_refl].
    - pose proof (unencoded_eq_shape _ _ Related) as [_ [Tree _]].
      destruct (proj2 Tree (ex_intro _ t1 E1)) as [t2 E2].
      destruct (IH t2 E2) as [t3 [E3 Thunks]].
      rewrite E1 in E3; inversion E3; subst t3.
      exists t2; split; [exact E2|apply eq_sym; exact Thunks].
    - destruct (IHxy t1 E1) as [t2 [E2 Thunks12]], (IHyz t2 E2) as [t3 [E3 Thunks23]].
      exists t3; split; [exact E3|eapply eq_trans; eassumption].
  Qed.
  Theorem eq_tree_to_thunk (make_thunk : TreeName -> thunk)
      (Cong : forall t1 t2, R (HTreeObj t1) (HTreeObj t2) -> R (Thunk (make_thunk t1)) (Thunk (make_thunk t2)))
      h1 h2 : eq h1 h2 -> forall t1, h1 = HTreeObj t1 ->
      (exists t2, h2 = HTreeObj t2 /\ eq (Thunk (make_thunk t1)) (Thunk (make_thunk t2))) \/
      (exists e2 t2, h2 = Encode e2 /\ execute e2 = Some (HTreeObj t2) /\ eq (Thunk (make_thunk t1)) (Thunk (make_thunk t2))).
  Proof.
    intros Related t1 ->; pose proof (eq_unencode _ _ Related) as Results; cbn [unencode] in Results.
    destruct (unencode h2) as [v2|] eqn:U2; [|contradiction].
    destruct (unencoded_eq_tree_cong make_thunk Cong _ _ Results t1 Logic.eq_refl) as [t2 [-> Thunks]].
    destruct h2 as [d|th|e2]; cbn [unencode] in U2.
    - left; exists t2; split; [congruence|exact Thunks].
    - discriminate.
    - right; exists e2, t2; auto.
  Qed.
  Lemma eq_tree_to_application_thunk h1 h2 : eq h1 h2 -> forall t1, h1 = HTreeObj t1 ->
      (exists t2, h2 = HTreeObj t2 /\ eq (Thunk (Application t1)) (Thunk (Application t2))) \/
      (exists e2 t2, h2 = Encode e2 /\ execute e2 = Some (HTreeObj t2) /\ eq (Thunk (Application t1)) (Thunk (Application t2))).
  Proof. apply (eq_tree_to_thunk Application (application_thunk_cong R R_properties)). Qed.
  Lemma eq_tree_to_selection_thunk h1 h2 : eq h1 h2 -> forall t1, h1 = HTreeObj t1 ->
      (exists t2, h2 = HTreeObj t2 /\ eq (Thunk (Selection t1)) (Thunk (Selection t2))) \/
      (exists e2 t2, h2 = Encode e2 /\ execute e2 = Some (HTreeObj t2) /\ eq (Thunk (Selection t1)) (Thunk (Selection t2))).
  Proof. apply (eq_tree_to_thunk Selection (selection_thunk_cong R R_properties)). Qed.
  Lemma eq_tree_to_digestion_thunk h1 h2 : eq h1 h2 -> forall t1, h1 = HTreeObj t1 ->
      (exists t2, h2 = HTreeObj t2 /\ eq (Thunk (Digestion t1)) (Thunk (Digestion t2))) \/
      (exists e2 t2, h2 = Encode e2 /\ execute e2 = Some (HTreeObj t2) /\ eq (Thunk (Digestion t1)) (Thunk (Digestion t2))).
  Proof. apply (eq_tree_to_thunk Digestion (digestion_thunk_cong R R_properties)). Qed.
  Lemma data_to_lift d1 d2 : R (Data d1) (Data d2) -> relaxed_X R (Data d1) (Data d2).
  Proof. apply (related_data_strengthen R R_properties). Qed.
  Lemma lower_to_lift d1 d2 : R (lower (Data d1)) (lower (Data d2)) -> R (lift (Data d1)) (lift (Data d2)).
  Proof. apply R_lower_to_lift_data. Qed.
  Lemma lift_to_lower d1 d2 : R (lift (Data d1)) (lift (Data d2)) -> R (lower (Data d1)) (lower (Data d2)).
  Proof. apply (E.EP.lift_to_lower R blob_ref_cong_R tree_ref_cong_R R_preserve_tree R_preserve_blob). Qed.
  Lemma lower_to_lift_cancel d d' : lower (Data d) = Data d' -> lift (Data d) = lift (Data d').
  Proof.
    destruct d as [[b|t]|[b|t]], d' as [[b'|t']|[b'|t']];
      cbn [lower lower_data lift lift_data]; intro H; try discriminate; inversion H; subst; reflexivity.
  Qed.
  Lemma lift_to_lower_cancel d d' : lift (Data d) = Data d' -> lower (Data d) = lower (Data d').
  Proof.
    destruct d as [[b|t]|[b|t]], d' as [[b'|t']|[b'|t']];
      cbn [lower lower_data lift lift_data]; intro H; try discriminate; inversion H; subst; reflexivity.
  Qed.
  Lemma execute_shallow_to_lift th d : execute (Shallow th) = Some (Data d) ->
    execute (Strict th) = Some (lift (Data d)).
  Proof.
    intro Exec; rewrite execute_hs in Exec; cbn in Exec; apply omap_some in Exec.
    destruct Exec as [h [Force Result]]; destruct (force_data _ _ Force) as [d' ->].
    rewrite execute_hs; cbn; rewrite Force; cbn [omap]; f_equal.
    apply lower_to_lift_cancel; symmetry; exact Result.
  Qed.
  Lemma execute_strict_to_lower th d : execute (Strict th) = Some (Data d) ->
    execute (Shallow th) = Some (lower (Data d)).
  Proof.
    intro Exec; rewrite execute_hs in Exec; cbn in Exec; apply omap_some in Exec.
    destruct Exec as [h [Force Result]]; destruct (force_data _ _ Force) as [d' ->].
    rewrite execute_hs; cbn; rewrite Force; cbn [omap]; f_equal.
    apply lift_to_lower_cancel; symmetry; exact Result.
  Qed.
  Lemma shallow_to_relaxed th d : execute (Shallow th) = Some (Data d) ->
    relaxed_X R (Encode (Shallow th)) (Data d).
  Proof. intro Exec; apply REvalStep, execute_shallow_to_lift; exact Exec. Qed.
  Lemma shallow_to_relaxed_strict th d : execute (Shallow th) = Some (Data d) ->
    relaxed_X R (Encode (Strict th)) (Data d).
  Proof. intro Exec; apply REvalStep, execute_shallow_to_lift; exact Exec. Qed.
  Lemma strict_to_relaxed th d : execute (Strict th) = Some (Data d) ->
    relaxed_X R (Encode (Strict th)) (Data d).
  Proof.
    intro Exec; destruct (execute_strict_to_obj _ _ Exec) as [o Result]; inversion Result; subst d.
    apply REvalStep; exact Exec.
  Qed.
  Lemma R_shallow_encode_to_strict_encode t1 t2 : R (Encode (Shallow t1)) (Encode (Shallow t2)) ->
    R (Encode (Strict t1)) (Encode (Strict t2)).
  Proof.
    intro Related; apply R'_impl_R, forces_to_strict_R', rel_opt_relaxed_R_to_R'.
    exact (R_encode_to_force (Shallow t1) (Shallow t2) Related).
  Qed.
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
  Lemma execute_to_lower e h : execute e = Some h ->
    execute (Shallow (encode_to_thunk e)) = Some (lower h).
  Proof.
    intro Exec; destruct (execute_data _ _ Exec) as [d ->]; destruct e as [th|th].
    - apply execute_strict_to_lower; exact Exec.
    - destruct (execute_shallow_to_ref _ _ Exec) as [r Result]; inversion Result; subst d; exact Exec.
  Qed.
  Lemma execute_to_lift e h : execute e = Some h ->
    execute (Strict (encode_to_thunk e)) = Some (lift h).
  Proof.
    intro Exec; destruct (execute_data _ _ Exec) as [d ->]; destruct e as [th|th].
    - destruct (execute_strict_to_obj _ _ Exec) as [o Result]; inversion Result; subst d; exact Exec.
    - apply execute_shallow_to_lift; exact Exec.
  Qed.
  Ltac reject_thunk_data Related :=
    first [eapply (data_not_thunk R R_properties); exact Related|
           eapply (thunk_not_data R R_properties); exact Related].
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
  Theorem eq_to_lower h1 h2 : eq h1 h2 -> eq (lower_data_and_encode h1) (lower_data_and_encode h2).
  Proof.
    intro Related; induction Related.
    - apply R_to_eq, R_to_lower; assumption.
    - apply eq_refl.
    - apply eq_sym; assumption.
    - eapply eq_trans; eassumption.
  Qed.
  Theorem eq_to_lift h1 h2 : eq h1 h2 -> eq (lift_data_and_encode h1) (lift_data_and_encode h2).
  Proof.
    intro Related; induction Related.
    - apply R_to_eq, R_to_lift; assumption.
    - apply eq_refl.
    - apply eq_sym; assumption.
    - eapply eq_trans; eassumption.
  Qed.
  Definition identification_handle h := match h with Data d => Thunk (Identification d) | _ => h end.
  Lemma unencoded_R_identification h1 h2 : unencoded_R h1 h2 ->
    R (identification_handle h1) (identification_handle h2).
  Proof.
    intros [Related [N1 N2]]; destruct h1 as [d1|th1|e1], h2 as [d2|th2|e2]; cbn [identification_handle];
      try solve [exfalso; apply N1; eauto]; try solve [exfalso; apply N2; eauto].
    - apply (identification_thunk_cong R R_properties); exact Related.
    - exfalso; reject_thunk_data Related.
    - exfalso; reject_thunk_data Related.
    - exact Related.
  Qed.
  Lemma unencoded_eq_identification h1 h2 : unencoded_eq h1 h2 ->
    eq (identification_handle h1) (identification_handle h2).
  Proof.
    intro Related; induction Related.
    - apply R_to_eq, unencoded_R_identification; assumption.
    - apply eq_refl.
    - apply eq_sym; assumption.
    - eapply eq_trans; eassumption.
  Qed.
  Lemma lift_idempotent d : lift (Data (lift_data d)) = lift (Data d).
  Proof. destruct d as [[b|t]|[b|t]]; reflexivity. Qed.
  Lemma think_lift_identification th d : think th = Some (Data d) ->
    eq (Thunk th) (Thunk (Identification (lift_data d))).
  Proof.
    intro Think; apply R_to_eq; eapply RThunkSomeResData.
    - exact Think.
    - rewrite think_identification; reflexivity.
    - rewrite lift_idempotent; apply RSelf.
  Qed.
  Lemma eq_lifted_data_to_think th1 th2 d1 d2 :
    think th1 = Some (Data d1) -> think th2 = Some (Data d2) ->
    eq (lift (Data d1)) (lift (Data d2)) -> eq (Thunk th1) (Thunk th2).
  Proof.
    intros T1 T2 Related; pose proof (eq_unencode _ _ Related) as Safe.
    change (unencoded_eq (lift (Data d1)) (lift (Data d2))) in Safe.
    pose proof (unencoded_eq_identification _ _ Safe) as Canonical.
    eapply eq_trans; [apply think_lift_identification; exact T1|].
    eapply eq_trans; [exact Canonical|apply eq_sym, think_lift_identification; exact T2].
  Qed.
  Lemma relaxed_data_lifted h d : relaxed_X R h (Data d) \/ relaxed_X R (Data d) h ->
    eq (lift_data_and_encode h) (lift (Data d)).
  Proof.
    intros [Forward|Backward].
    - destruct h as [d'|th|[th|th]]; apply R_to_eq; exact Forward.
    - destruct h as [d'|th|e].
      + apply eq_sym, R_to_eq; exact Backward.
      + exfalso; exact (data_not_thunk R R_properties (lift_data d) th Backward).
      + exfalso; exact (data_not_encode R R_properties (lift_data d) e Backward).
  Qed.
  Theorem think_to_eq h1 h2 : eq h1 h2 -> forall th1 th2 d1 d2,
    think th1 = Some (Data d1) -> think th2 = Some (Data d2) ->
    (relaxed_X R h1 (Data d1) \/ relaxed_X R (Data d1) h1) /\
    (relaxed_X R h2 (Data d2) \/ relaxed_X R (Data d2) h2) -> eq (Thunk th1) (Thunk th2).
  Proof.
    intros Related th1 th2 d1 d2 T1 T2 [Rel1 Rel2]; eapply eq_lifted_data_to_think; [exact T1|exact T2|].
    eapply eq_trans; [apply eq_sym, relaxed_data_lifted; exact Rel1|].
    eapply eq_trans; [apply eq_to_lift; exact Related|apply relaxed_data_lifted; exact Rel2].
  Qed.
  Definition create_encode (kind : thunk -> encode) h := match h with Thunk th => Encode (kind th) | _ => h end.
  Definition create_strict_encode := create_encode Strict.
  Definition create_shallow_encode := create_encode Shallow.
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
  Lemma eq_create_encode (kind : thunk -> encode)
      (Cong : forall th1 th2, R (Thunk th1) (Thunk th2) -> R (Encode (kind th1)) (Encode (kind th2)))
      h1 h2 : eq h1 h2 -> eq (create_encode kind h1) (create_encode kind h2).
  Proof.
    intro Related; induction Related.
    - apply R_to_eq, R_create_encode; assumption.
    - apply eq_refl.
    - apply eq_sym; assumption.
    - eapply eq_trans; eassumption.
  Qed.
  Lemma eq_thunk_to_strict_encode t1 t2 : eq (Thunk t1) (Thunk t2) ->
    eq (create_strict_encode (Thunk t1)) (create_strict_encode (Thunk t2)).
  Proof. apply (eq_create_encode Strict (strict_encode_cong R R_properties)). Qed.
  Lemma eq_thunk_to_shallow_encode t1 t2 : eq (Thunk t1) (Thunk t2) ->
    eq (create_shallow_encode (Thunk t1)) (create_shallow_encode (Thunk t2)).
  Proof. apply (eq_create_encode Shallow (shallow_encode_cong R R_properties)). Qed.
  Lemma R_or_unencode_some y z v1 : R y z \/ R z y -> unencode y = Some v1 ->
    exists v2, unencode z = Some v2 /\ (R v1 v2 \/ R v2 v1).
  Proof.
    intros [Forward|Backward] U1.
    - pose proof (R_unencode _ _ Forward) as Result; rewrite U1 in Result.
      destruct (unencode z) as [v2|] eqn:U2; [|contradiction]; exists v2; auto.
    - pose proof (R_unencode _ _ Backward) as Result; rewrite U1 in Result.
      destruct (unencode z) as [v2|] eqn:U2; [|contradiction]; exists v2; auto.
  Qed.
  Lemma R_or_unencode_shape {A : Type} (make_data : A -> data)
      (Forward : forall h1 h2, R h1 h2 -> (exists a, h1 = Data (make_data a)) -> exists a, h2 = Data (make_data a))
      (Backward : forall h1 h2, R h1 h2 -> (exists a, h2 = Data (make_data a)) ->
        (exists a, h1 = Data (make_data a)) \/ (exists e, h1 = Encode e))
      y z a1 : R y z \/ R z y -> unencode y = Some (Data (make_data a1)) ->
      exists a2, unencode z = Some (Data (make_data a2)).
  Proof.
    intros Related U1; destruct (R_or_unencode_some _ _ _ Related U1) as [v2 [U2 [Rel|Rel]]].
    - destruct (Forward _ _ Rel (ex_intro _ a1 Logic.eq_refl)) as [a2 ->]; exists a2; exact U2.
    - destruct (Backward _ _ Rel (ex_intro _ a1 Logic.eq_refl)) as [[a2 ->]|[e ->]].
      + exists a2; exact U2.
      + exfalso; apply (unencode_some_unencoded _ _ U2); eauto.
  Qed.
  Lemma R_or_preserve_obj_or_encode_rev {A : Type} (make_obj : A -> data)
      (Forward : forall h1 h2, R h1 h2 -> (exists a, h1 = Data (make_obj a)) -> exists a, h2 = Data (make_obj a))
      (Backward : forall h1 h2, R h1 h2 -> (exists a, h2 = Data (make_obj a)) ->
        (exists a, h1 = Data (make_obj a)) \/ (exists th, h1 = Encode (Strict th)))
      y z : R y z \/ R z y ->
      (exists th a, y = Encode (Strict th) /\ execute (Strict th) = Some (Data (make_obj a))) ->
      (exists a, z = Data (make_obj a)) \/
      (exists th a, z = Encode (Strict th) /\ execute (Strict th) = Some (Data (make_obj a))).
  Proof.
    intros Related [th1 [a1 [-> Exec]]].
    assert (forall h1 h2, R h1 h2 -> (exists a, h2 = Data (make_obj a)) ->
      (exists a, h1 = Data (make_obj a)) \/ (exists e, h1 = Encode e)) as Back.
    { intros h1 h2 Rel Shape; destruct (Backward _ _ Rel Shape) as [Shape'|[th ->]]; [left; exact Shape'|right; eauto]. }
    destruct (R_or_unencode_shape make_obj Forward Back _ _ a1 Related Exec) as [a2 U2].
    destruct z as [d|th|[th2|th2]]; cbn [unencode] in U2.
    - left; exists a2; congruence.
    - discriminate.
    - right; exists th2, a2; auto.
    - exfalso; destruct Related as [Rel|Rel];
        [exact (R_not_strict_shallow _ _ Rel)|exact (R_not_shallow_strict _ _ Rel)].
  Qed.
  Lemma R_or_preserve_ref_or_encode_rev {A : Type} (make_ref : A -> data)
      (Forward : forall h1 h2, R h1 h2 -> (exists a, h1 = Data (make_ref a)) -> exists a, h2 = Data (make_ref a))
      (Backward : forall h1 h2, R h1 h2 -> (exists a, h2 = Data (make_ref a)) ->
        (exists a, h1 = Data (make_ref a)) \/ (exists th, h1 = Encode (Shallow th)))
      y z : R y z \/ R z y ->
      (exists th a, y = Encode (Shallow th) /\ execute (Shallow th) = Some (Data (make_ref a))) ->
      (exists a, z = Data (make_ref a)) \/
      (exists th a, z = Encode (Shallow th) /\ execute (Shallow th) = Some (Data (make_ref a))).
  Proof.
    intros Related [th1 [a1 [-> Exec]]].
    assert (forall h1 h2, R h1 h2 -> (exists a, h2 = Data (make_ref a)) ->
      (exists a, h1 = Data (make_ref a)) \/ (exists e, h1 = Encode e)) as Back.
    { intros h1 h2 Rel Shape; destruct (Backward _ _ Rel Shape) as [Shape'|[th ->]]; [left; exact Shape'|right; eauto]. }
    destruct (R_or_unencode_shape make_ref Forward Back _ _ a1 Related Exec) as [a2 U2].
    destruct z as [d|th|[th2|th2]]; cbn [unencode] in U2.
    - left; exists a2; congruence.
    - discriminate.
    - exfalso; destruct Related as [Rel|Rel];
        [exact (R_not_shallow_strict _ _ Rel)|exact (R_not_strict_shallow _ _ Rel)].
    - right; exists th2, a2; auto.
  Qed.
  Lemma R_or_preserve_tree_or_encode_rev y z : R y z \/ R z y ->
      (exists th t, y = Encode (Strict th) /\ execute (Strict th) = Some (HTreeObj t)) ->
      (exists t, z = HTreeObj t) \/ (exists th t, z = Encode (Strict th) /\ execute (Strict th) = Some (HTreeObj t)).
  Proof.
    apply (R_or_preserve_obj_or_encode_rev (fun t => Object (TreeObj t))).
    - intros h1 h2 Rel [t ->]; eapply R_preserve_tree; exact Rel.
    - intros h1 h2 Rel [t ->]; eapply R_preserve_tree_or_encode_rev; exact Rel.
  Qed.
  Lemma R_or_preserve_blob_or_encode_rev y z : R y z \/ R z y ->
      (exists th b, y = Encode (Strict th) /\ execute (Strict th) = Some (HBlobObj b)) ->
      (exists b, z = HBlobObj b) \/ (exists th b, z = Encode (Strict th) /\ execute (Strict th) = Some (HBlobObj b)).
  Proof.
    apply (R_or_preserve_obj_or_encode_rev (fun b => Object (BlobObj b))).
    - intros h1 h2 Rel [b ->]; eapply R_preserve_blob; exact Rel.
    - intros h1 h2 Rel [b ->]; eapply R_preserve_blob_or_encode_rev; exact Rel.
  Qed.
  Lemma R_or_preserve_tree_ref_or_encode_rev y z : R y z \/ R z y ->
      (exists th t, y = Encode (Shallow th) /\ execute (Shallow th) = Some (HTreeRef t)) ->
      (exists t, z = HTreeRef t) \/ (exists th t, z = Encode (Shallow th) /\ execute (Shallow th) = Some (HTreeRef t)).
  Proof.
    apply (R_or_preserve_ref_or_encode_rev (fun t => Ref (TreeRef t))).
    - intros h1 h2 Rel [t ->]; eapply R_preserve_tree_ref; exact Rel.
    - intros h1 h2 Rel [t ->]; eapply R_preserve_tree_ref_or_encode_rev; exact Rel.
  Qed.
  Lemma R_or_preserve_blob_ref_or_encode_rev y z : R y z \/ R z y ->
      (exists th b, y = Encode (Shallow th) /\ execute (Shallow th) = Some (HBlobRef b)) ->
      (exists b, z = HBlobRef b) \/ (exists th b, z = Encode (Shallow th) /\ execute (Shallow th) = Some (HBlobRef b)).
  Proof.
    apply (R_or_preserve_ref_or_encode_rev (fun b => Ref (BlobRef b))).
    - intros h1 h2 Rel [b ->]; eapply R_preserve_blob_ref; exact Rel.
    - intros h1 h2 Rel [b ->]; eapply R_preserve_blob_ref_or_encode_rev; exact Rel.
  Qed.
  Lemma unencoded_eq_blob_cong h1 h2 : unencoded_eq h1 h2 -> forall b1, h1 = HBlobObj b1 ->
    exists b2, h2 = HBlobObj b2 /\ get_blob_data b1 = get_blob_data b2.
  Proof.
    intro Related; induction Related as [x y Rxy|x|x y Related IH|x y z Rxy IHxy Ryz IHyz]; intros b1 E1.
    - subst x; destruct Rxy as [Rel _]; destruct (R_preserve_blob _ _ Rel) as [b2 ->].
      exists b2; split; [reflexivity|apply blob_cong_R; exact Rel].
    - subst x; exists b1; auto.
    - pose proof (unencoded_eq_shape _ _ Related) as [Blob _].
      destruct (proj2 Blob (ex_intro _ b1 E1)) as [b2 E2].
      destruct (IH b2 E2) as [b3 [E3 Data]].
      rewrite E1 in E3; inversion E3; subst b3; exists b2; split; [exact E2|symmetry; exact Data].
    - destruct (IHxy b1 E1) as [b2 [E2 Data12]], (IHyz b2 E2) as [b3 [E3 Data23]].
      exists b3; split; [exact E3|congruence].
  Qed.
  Theorem eq_blob_same_data h1 h2 : eq h1 h2 -> forall b1, h1 = HBlobObj b1 ->
    (exists b2, h2 = HBlobObj b2 /\ get_blob_data b1 = get_blob_data b2) \/
    (exists th b2, h2 = Encode (Strict th) /\ eval h2 = Some (HBlobObj b2) /\ get_blob_data b1 = get_blob_data b2).
  Proof.
    intros Related b1 ->; pose proof (eq_unencode _ _ Related) as Results; cbn [unencode] in Results.
    destruct (unencode h2) as [v2|] eqn:U2; [|contradiction].
    destruct (unencoded_eq_blob_cong _ _ Results b1 Logic.eq_refl) as [b2 [-> Data]].
    destruct h2 as [d|th|e]; cbn [unencode] in U2.
    - left; exists b2; split; [congruence|exact Data].
    - discriminate.
    - destruct (execute_to_obj_strict _ _ U2) as [th ->].
      right; exists th, b2; split; [reflexivity|split; [rewrite eval_encode, U2; exact (eval_blob b2)|exact Data]].
  Qed.
  Lemma eq_blob_to_ref b1 b2 : eq (HBlobObj b1) (HBlobObj b2) -> eq (HBlobRef b1) (HBlobRef b2).
  Proof. apply (eq_to_lower (HBlobObj b1) (HBlobObj b2)). Qed.
  Lemma eq_tree_to_ref t1 t2 : eq (HTreeObj t1) (HTreeObj t2) -> eq (HTreeRef t1) (HTreeRef t2).
  Proof. apply (eq_to_lower (HTreeObj t1) (HTreeObj t2)). Qed.
  Lemma eq_strict_to_shallow t1 t2 : eq (Encode (Strict t1)) (Encode (Strict t2)) -> eq (Encode (Shallow t1)) (Encode (Shallow t2)).
  Proof. apply (eq_to_lower (Encode (Strict t1)) (Encode (Strict t2))). Qed.
  Lemma eq_ref_to_blob b1 b2 : eq (HBlobRef b1) (HBlobRef b2) -> eq (HBlobObj b1) (HBlobObj b2).
  Proof. apply (eq_to_lift (HBlobRef b1) (HBlobRef b2)). Qed.
  Lemma eq_ref_to_tree t1 t2 : eq (HTreeRef t1) (HTreeRef t2) -> eq (HTreeObj t1) (HTreeObj t2).
  Proof. apply (eq_to_lift (HTreeRef t1) (HTreeRef t2)). Qed.
  Lemma eq_shallow_to_strict t1 t2 : eq (Encode (Shallow t1)) (Encode (Shallow t2)) -> eq (Encode (Strict t1)) (Encode (Strict t2)).
  Proof. apply (eq_to_lift (Encode (Shallow t1)) (Encode (Shallow t2))). Qed.
  Theorem force_some_to_eq th1 th2 r1 r2 : force th1 = Some r1 -> force th2 = Some r2 ->
    relaxed_X eq r1 r2 -> eq (Thunk th1) (Thunk th2).
  Proof.
    intros F1 F2 Related; destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->].
    destruct (force_to_the_last_thunk _ _ F1) as [th1' [T1 Thunks1]],
      (force_to_the_last_thunk _ _ F2) as [th2' [T2 Thunks2]].
    eapply eq_trans; [exact Thunks1|].
    eapply eq_trans; [eapply eq_lifted_data_to_think; [exact T1|exact T2|exact Related]|apply eq_sym; exact Thunks2].
  Qed.
End EquivalenceClosure.
