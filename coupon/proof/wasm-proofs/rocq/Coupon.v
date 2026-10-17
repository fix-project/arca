From Stdlib Require Import List Arith.
From FixProof Require Import Handle ApplyTree EquivalenceClosure.

Module Coupon (S : STORAGE) (P : PROGRAM S).
  Module EC := EquivalenceClosure S P.
  Import EC EC.E EC.E.EP EC.E.EP.E EC.E.EP.E.H.

  (** Preserve the original eight mutually defined coupon judgements and all
      their inference rules. Semantic soundness is a separate theorem. *)
  Inductive coupon_force : handle -> handle -> Prop :=
  | CouponForce : forall th r, force th = Some r -> coupon_force (Thunk th) r
  | CouponThinktoForce : forall th d, coupon_think (Thunk th) (Data d) -> coupon_force (Thunk th) (Data d)
  | CouponForceEq : forall h1 h2 h3, coupon_eq h1 h2 -> coupon_force h1 h3 -> coupon_force h2 h3
  with coupon_storage : handle -> handle -> Prop :=
  | CouponStorage : forall b1 b2, get_blob_data b1 = get_blob_data b2 -> coupon_storage (HBlobObj b1) (HBlobObj b2)
  with coupon_apply : handle -> handle -> Prop :=
  | CouponApply : forall t h, A.apply_tree t = Some h -> coupon_apply (HTreeObj t) h
  with coupon_slice : handle -> handle -> Prop :=
  | CouponSlice : forall t r, Sl.slice t = Some r -> coupon_slice (HTreeObj t) (Data (Ref r))
  with coupon_identify : handle -> handle -> Prop :=
  | CouponIdentify : forall d h, I.identify d = Some h -> coupon_identify (Data d) h
  with coupon_think : handle -> handle -> Prop :=
  | CouponThinkApplication : forall t t' h,
      coupon_eval (HTreeObj t) (HTreeObj t') -> coupon_apply (HTreeObj t') h -> coupon_think (Thunk (Application t)) h
  | CouponThinkSelection : forall t t' h,
      coupon_eval (HTreeObj t) (HTreeObj t') -> coupon_slice (HTreeObj t') h -> coupon_think (Thunk (Selection t)) h
  | CouponThinkIdentification : forall d h, coupon_identify (Data d) h -> coupon_think (Thunk (Identification d)) h
  with coupon_eval : handle -> handle -> Prop :=
  | CouponEvalBlobObj : forall b, coupon_eval (HBlobObj b) (HBlobObj b)
  | CouponEvalTreeObj : forall t1 t2,
      Forall2 coupon_eval (get_tree_raw t1) (get_tree_raw t2) -> coupon_eval (HTreeObj t1) (HTreeObj t2)
  | CouponEvalRef : forall r, coupon_eval (Data (Ref r)) (Data (Ref r))
  | CouponEvalThunk : forall th, coupon_eval (Thunk th) (Thunk th)
  | CouponEvalEq : forall h1 h2 h3, coupon_eval h1 h2 -> coupon_eq h1 h3 -> coupon_eval h3 h2
  with coupon_eq : handle -> handle -> Prop :=
  | CouponThunkEq : forall t1 t2, coupon_think (Thunk t1) (Thunk t2) -> coupon_eq (Thunk t1) (Thunk t2)
  | CouponThunkEncodeEq : forall t1 t2, coupon_think (Thunk t1) (Encode (Strict t2)) -> coupon_eq (Thunk t1) (Thunk t2)
  | CouponThunkEncodeShallowEq : forall t1 t2, coupon_think (Thunk t1) (Encode (Shallow t2)) -> coupon_eq (Thunk t1) (Thunk t2)
  | CouponForceResultEq : forall h1 h1' h2 h2',
      coupon_force h1 h1' -> coupon_force h2 h2' -> coupon_eq h1' h2' -> coupon_eq h1 h2
  | CouponForcetoEncodeStrict : forall th h, coupon_force (Thunk th) h -> coupon_eq (Encode (Strict th)) (lift h)
  | CouponForcetoEncodeShallow : forall th h, coupon_force (Thunk th) h -> coupon_eq (Encode (Shallow th)) (lower h)
  | CouponEqApplication : forall t1 t2, coupon_eq (HTreeObj t1) (HTreeObj t2) -> coupon_eq (Thunk (Application t1)) (Thunk (Application t2))
  | CouponEqSelection : forall t1 t2, coupon_eq (HTreeObj t1) (HTreeObj t2) -> coupon_eq (Thunk (Selection t1)) (Thunk (Selection t2))
  | CouponEqIdentification : forall d1 d2, coupon_eq (Data d1) (Data d2) -> coupon_eq (Thunk (Identification d1)) (Thunk (Identification d2))
  | CouponEqEncodeStrict : forall t1 t2, coupon_eq (Thunk t1) (Thunk t2) -> coupon_eq (Encode (Strict t1)) (Encode (Strict t2))
  | CouponEqEncodeShallow : forall t1 t2, coupon_eq (Thunk t1) (Thunk t2) -> coupon_eq (Encode (Shallow t1)) (Encode (Shallow t2))
  | CouponEvalResultEq : forall h1 h2, coupon_eval h1 h2 -> coupon_eq h1 h2
  | CouponTreeRefEq : forall t1 t2, coupon_eq (HTreeObj t1) (HTreeObj t2) -> coupon_eq (HTreeRef t1) (HTreeRef t2)
  | CouponBlobRefEq : forall b1 b2, coupon_eq (HBlobObj b1) (HBlobObj b2) -> coupon_eq (HBlobRef b1) (HBlobRef b2)
  | CouponTreeEq : forall t1 t2,
      Forall2 coupon_eq (get_tree_raw t1) (get_tree_raw t2) -> coupon_eq (HTreeObj t1) (HTreeObj t2)
  | CouponStorageEq : forall h1 h2, coupon_storage h1 h2 -> coupon_eq h1 h2
  | CouponSelf : forall h, coupon_eq h h
  | CouponSym : forall h1 h2, coupon_eq h1 h2 -> coupon_eq h2 h1
  | CouponTrans : forall h1 h2 h3, coupon_eq h1 h2 -> coupon_eq h2 h3 -> coupon_eq h1 h3.

  Lemma coupon_storage_sound h1 h2 : coupon_storage h1 h2 ->
    exists b1 b2, h1 = HBlobObj b1 /\ h2 = HBlobObj b2 /\ get_blob_data b1 = get_blob_data b2.
  Proof. intro H; inversion H; subst; eauto. Qed.
  Lemma coupon_apply_sound h r : coupon_apply h r -> exists t, h = HTreeObj t /\ A.apply_tree t = Some r.
  Proof. intro H; inversion H; subst; eauto. Qed.
  Lemma coupon_slice_sound h r : coupon_slice h r -> exists t ref, h = HTreeObj t /\ r = Data (Ref ref) /\ Sl.slice t = Some ref.
  Proof. intro H; inversion H; subst; eauto. Qed.
  Lemma coupon_identify_sound h r : coupon_identify h r -> exists d, h = Data d /\ I.identify d = Some r.
  Proof. intro H; inversion H; subst; eauto. Qed.
  Definition force_result h r := exists th res d,
    h = Thunk th /\ force th = Some res /\ relaxed_X eq r res /\ r = Data d.
  Definition think_result h r := exists th th',
    h = Thunk th /\ think th' = Some r /\ eq h (Thunk th').
  Definition eval_result h r := exists v, eval h = Some v /\ eq r v /\ value_handle r.

  Lemma eval_result_eq h r : eval_result h r -> eq h r.
  Proof.
    intros [v [Ev [Related Value]]]; eapply eq_trans;
      [apply eval_to_eq; exact Ev|apply eq_sym; exact Related].
  Qed.
  Lemma value_tree_fixed_eval t : value_handle (HTreeObj t) -> eval_tree t = Some t.
  Proof.
    intro Value; pose proof (value_handle_eval_to_itself _ Value) as Ev.
    rewrite eval_tree_handle in Ev; apply omap_some in Ev; destruct Ev as [u [Tree E']].
    inversion E'; subst u; exact Tree.
  Qed.
  Lemma tree_thunk_eq (kind : TreeName -> thunk)
      (Cong : forall t1 t2, R (HTreeObj t1) (HTreeObj t2) -> R (Thunk (kind t1)) (Thunk (kind t2)))
      t1 t2 : eq (HTreeObj t1) (HTreeObj t2) -> eq (Thunk (kind t1)) (Thunk (kind t2)).
  Proof.
    intro Related; destruct (eq_tree_to_thunk kind Cong _ _ Related t1 Logic.eq_refl) as
      [[u [E' Thunks]]|[e [u [E' Rest]]]]; [inversion E'; subst; exact Thunks|discriminate].
  Qed.
  Lemma force_result_direct th r : force th = Some r -> force_result (Thunk th) r.
  Proof.
    intro Force; destruct (force_data _ _ Force) as [d ->].
    exists th, (Data d), d; repeat split; try reflexivity; [exact Force|apply eq_refl].
  Qed.
  Lemma force_result_think th d : think_result (Thunk th) (Data d) -> force_result (Thunk th) (Data d).
  Proof.
    intros [u [u' [E' [Think Related]]]]; inversion E'; subst u.
    assert (force u' = Some (Data d)) as F' by (rewrite force_hs, Think; reflexivity).
    destruct (force_eq _ _ Related th Logic.eq_refl) as [v [E'' Forces]].
    inversion E''; subst v; rewrite F' in Forces.
    destruct (force th) as [r|] eqn:F; [|contradiction].
    exists th, r, d; repeat split; try reflexivity; [exact F|].
    destruct (force_data _ _ F) as [d' ->]; apply eq_sym; exact Forces.
  Qed.
  Lemma force_result_eq h1 h2 r : eq h1 h2 -> force_result h1 r -> force_result h2 r.
  Proof.
    intros Related [th1 [res1 [d [-> [F1 [Output ->]]]]]].
    destruct (force_eq _ _ Related th1 Logic.eq_refl) as [th2 [-> Forces]].
    rewrite F1 in Forces; destruct (force th2) as [res2|] eqn:F2; [|contradiction].
    exists th2, res2, d; repeat split; try reflexivity; [exact F2|].
    destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->].
    eapply eq_trans; [exact Output|exact Forces].
  Qed.
  Lemma think_result_application t t' h : eval_result (HTreeObj t) (HTreeObj t') ->
    A.apply_tree t' = Some h -> think_result (Thunk (Application t)) h.
  Proof.
    intros Eval App; pose proof (eval_result_eq _ _ Eval) as Related.
    destruct Eval as [v [Ev [Out Value]]].
    exists (Application t), (Application t'); split; [reflexivity|split].
    - rewrite think_application, (value_tree_fixed_eval _ Value); exact App.
    - apply (tree_thunk_eq Application (application_thunk_cong R R_properties)); exact Related.
  Qed.
  Lemma think_result_selection t t' h : eval_result (HTreeObj t) (HTreeObj t') ->
    (exists ref, h = Data (Ref ref) /\ Sl.slice t' = Some ref) -> think_result (Thunk (Selection t)) h.
  Proof.
    intros Eval [ref [-> Slice]]; pose proof (eval_result_eq _ _ Eval) as Related.
    destruct Eval as [v [Ev [Out Value]]].
    exists (Selection t), (Selection t'); split; [reflexivity|split].
    - rewrite think_selection, (value_tree_fixed_eval _ Value); cbn [obind]; rewrite Slice; reflexivity.
    - apply (tree_thunk_eq Selection (selection_thunk_cong R R_properties)); exact Related.
  Qed.
  Lemma think_result_identification d h : I.identify d = Some h -> think_result (Thunk (Identification d)) h.
  Proof.
    intro Identify; exists (Identification d), (Identification d); split; [reflexivity|split];
      [rewrite think_identification; exact Identify|apply eq_refl].
  Qed.
  Lemma think_result_single th1 th2 : think_result (Thunk th1) (Thunk th2) -> eq (Thunk th1) (Thunk th2).
  Proof.
    intros [u [u' [E' [Think Related]]]]; inversion E'; subst u.
    eapply eq_trans; [exact Related|apply R_to_eq, RThinkSingleStepThunk; exact Think].
  Qed.
  Lemma think_result_single_strict th1 th2 : think_result (Thunk th1) (Encode (Strict th2)) -> eq (Thunk th1) (Thunk th2).
  Proof.
    intros [u [u' [E' [Think Related]]]]; inversion E'; subst u.
    eapply eq_trans; [exact Related|apply R_to_eq, RThunkSingleStepEncodeStrict; exact Think].
  Qed.
  Lemma think_result_single_shallow th1 th2 : think_result (Thunk th1) (Encode (Shallow th2)) -> eq (Thunk th1) (Thunk th2).
  Proof.
    intros [u [u' [E' [Think Related]]]]; inversion E'; subst u.
    eapply eq_trans; [exact Related|apply R_to_eq, RThunkSingleStepEncodeShallow; exact Think].
  Qed.
  Lemma force_results_eq h1 r1 h2 r2 : force_result h1 r1 -> force_result h2 r2 -> eq r1 r2 -> eq h1 h2.
  Proof.
    intros [th1 [res1 [d1 [-> [F1 [Rel1 ->]]]]]] [th2 [res2 [d2 [-> [F2 [Rel2 ->]]]]]] Related.
    destruct (force_data _ _ F1) as [dr1 ->], (force_data _ _ F2) as [dr2 ->].
    eapply force_some_to_eq; [exact F1|exact F2|].
    eapply eq_trans; [apply eq_sym; exact Rel1|].
    eapply eq_trans; [exact (eq_to_lift (Data d1) (Data d2) Related)|exact Rel2].
  Qed.
  Lemma force_result_strict th h : force_result (Thunk th) h -> eq (Encode (Strict th)) (lift h).
  Proof.
    intros [u [res [d [E' [Force [Related ->]]]]]]; inversion E'; subst u.
    eapply eq_trans.
    - apply R_to_eq, REvalStep; rewrite execute_hs; cbn; rewrite Force; reflexivity.
    - apply eq_sym; exact Related.
  Qed.
  Lemma lower_lift_data d : lower (lift (Data d)) = lower (Data d).
  Proof. destruct d as [[b|t]|[b|t]]; reflexivity. Qed.
  Lemma force_result_shallow th h : force_result (Thunk th) h -> eq (Encode (Shallow th)) (lower h).
  Proof.
    intros [u [res [d [E' [Force [Related ->]]]]]]; inversion E'; subst u.
    destruct (force_data _ _ Force) as [dr ->].
    pose proof (eq_to_lower _ _ Related) as Lower.
    change (eq (lower (lift (Data d))) (lower (lift (Data dr)))) in Lower.
    rewrite !lower_lift_data in Lower.
    eapply eq_trans.
    - apply R_to_eq, REvalStep; rewrite execute_hs; cbn; rewrite Force; reflexivity.
    - apply eq_sym; exact Lower.
  Qed.
  Lemma eval_result_blob b : eval_result (HBlobObj b) (HBlobObj b).
  Proof. exists (HBlobObj b); split; [apply eval_blob|split; [apply eq_refl|constructor]]. Qed.
  Lemma eval_result_ref ref : eval_result (Data (Ref ref)) (Data (Ref ref)).
  Proof. exists (Data (Ref ref)); split; [apply eval_ref|split; [apply eq_refl|constructor]]. Qed.
  Lemma eval_result_thunk th : eval_result (Thunk th) (Thunk th).
  Proof. exists (Thunk th); split; [apply eval_thunk|split; [apply eq_refl|constructor]]. Qed.
  Lemma eval_result_eq_input h1 h2 h3 : eval_result h1 h2 -> eq h1 h3 -> eval_result h3 h2.
  Proof.
    intros [v [Ev [Out Value]]] Related; pose proof (eq_eval _ _ Related) as Evals.
    rewrite Ev in Evals; destruct (eval h3) as [v'|] eqn:Ev'; [|contradiction].
    exists v'; split; [exact Ev'|split; [eapply eq_trans; [exact Out|exact Evals]|exact Value]].
  Qed.
  Lemma eq_identification_data d1 d2 : eq (Data d1) (Data d2) -> eq (Thunk (Identification d1)) (Thunk (Identification d2)).
  Proof.
    intro Related; apply (think_to_eq (Data d1) (Data d2) Related (Identification d1) (Identification d2) d1 d2).
    - rewrite think_identification; reflexivity.
    - rewrite think_identification; reflexivity.
    - split; left; apply RSelf.
  Qed.
  Lemma eq_tree_entries t1 t2 : Forall2 eq (get_tree_raw t1) (get_tree_raw t2) -> eq (HTreeObj t1) (HTreeObj t2).
  Proof.
    intro Related; rewrite <- (create_tree_get_tree_raw t1) at 1; rewrite <- (create_tree_get_tree_raw t2) at 1.
    apply eq_tree_list_all2; exact Related.
  Qed.
  Lemma eval_list_unbounded_fuel xs ys : Forall2 (fun h v => eval h = Some v) xs ys ->
    exists n, eval_list_with_fuel n xs = Some ys.
  Proof.
    intro Related; apply evals_to_tree_to.
    eapply Forall2_impl; [|exact Related].
    intros h v Ev; apply eval_some; exact Ev.
  Qed.
  Lemma eval_list_helper xs : Forall (fun h => exists v, eval h = Some v) xs ->
    exists ys, Forall2 evals_to xs ys.
  Proof.
    intro Values; induction Values.
    - exists nil; constructor.
    - destruct H as [v Ev], IHValues as [ys Evs].
      exists (v :: ys); constructor; [apply eval_some; exact Ev|exact Evs].
  Qed.
  Lemma eval_result_list xs ys : Forall2 eval_result xs ys -> exists zs,
    Forall2 (fun h v => eval h = Some v) xs zs /\ Forall2 eq ys zs /\ Forall value_handle ys.
  Proof.
    intro Related; induction Related.
    - exists nil; repeat split; constructor.
    - destruct H as [v [Ev [Out Value]]], IHRelated as [zs [Evs [Outs Values]]].
      exists (v :: zs); split; [constructor; assumption|split; constructor; assumption].
  Qed.
  Lemma eval_result_tree t1 t2 : Forall2 eval_result (get_tree_raw t1) (get_tree_raw t2) ->
    eval_result (HTreeObj t1) (HTreeObj t2).
  Proof.
    intro Related; destruct (eval_result_list _ _ Related) as [zs [Evs [Outs Values]]].
    destruct (eval_list_unbounded_fuel _ _ Evs) as [n Fuel].
    exists (HTreeObj (create_tree zs)); split.
    - apply eval_unique; exists (S n).
      change (omap HTreeObj (omap create_tree (eval_list_with_fuel n (get_tree_raw t1))) = Some (HTreeObj (create_tree zs))).
      rewrite Fuel; reflexivity.
    - split.
      + rewrite <- (create_tree_get_tree_raw t2) at 1; apply eq_tree_list_all2; exact Outs.
      + apply tree_obj_handle, value_tree_intro; exact Values.
  Qed.

  Fixpoint coupon_force_sound (h r : handle) (Derivation : coupon_force h r) {struct Derivation} : force_result h r
  with coupon_think_sound (h r : handle) (Derivation : coupon_think h r) {struct Derivation} : think_result h r
  with coupon_eval_sound (h r : handle) (Derivation : coupon_eval h r) {struct Derivation} : eval_result h r
  with coupon_eq_sound (h r : handle) (Derivation : coupon_eq h r) {struct Derivation} : eq h r.
  Proof.
    - destruct Derivation.
      + apply force_result_direct; assumption.
      + apply force_result_think; apply coupon_think_sound; assumption.
      + eapply force_result_eq; [apply coupon_eq_sound; eassumption|apply coupon_force_sound; eassumption].
    - destruct Derivation.
      + eapply think_result_application; [apply coupon_eval_sound; eassumption|].
        match goal with D : coupon_apply _ _ |- _ => inversion D; subst; assumption end.
      + eapply think_result_selection; [apply coupon_eval_sound; eassumption|].
        match goal with D : coupon_slice _ _ |- _ => inversion D; subst; eauto end.
      + apply think_result_identification.
        match goal with D : coupon_identify _ _ |- _ => inversion D; subst; assumption end.
    - destruct Derivation.
      + apply eval_result_blob.
      + apply eval_result_tree.
        match goal with ListProof : Forall2 coupon_eval _ _ |- _ =>
          refine ((fix list_sound xs ys (D : Forall2 coupon_eval xs ys) {struct D} : Forall2 eval_result xs ys := _) _ _ ListProof) end.
        destruct D; constructor; [apply coupon_eval_sound; assumption|apply list_sound; assumption].
      + apply eval_result_ref.
      + apply eval_result_thunk.
      + eapply eval_result_eq_input; [apply coupon_eval_sound; eassumption|apply coupon_eq_sound; eassumption].
    - destruct Derivation.
      + apply think_result_single, coupon_think_sound; assumption.
      + apply think_result_single_strict, coupon_think_sound; assumption.
      + apply think_result_single_shallow, coupon_think_sound; assumption.
      + eapply force_results_eq; [apply coupon_force_sound; eassumption|apply coupon_force_sound; eassumption|apply coupon_eq_sound; eassumption].
      + apply force_result_strict, coupon_force_sound; assumption.
      + apply force_result_shallow, coupon_force_sound; assumption.
      + apply (tree_thunk_eq Application (application_thunk_cong R R_properties)), coupon_eq_sound; assumption.
      + apply (tree_thunk_eq Selection (selection_thunk_cong R R_properties)), coupon_eq_sound; assumption.
      + apply eq_identification_data, coupon_eq_sound; assumption.
      + apply eq_thunk_to_strict_encode, coupon_eq_sound; assumption.
      + apply eq_thunk_to_shallow_encode, coupon_eq_sound; assumption.
      + apply eval_result_eq, coupon_eval_sound; assumption.
      + apply eq_tree_to_ref, coupon_eq_sound; assumption.
      + apply eq_blob_to_ref, coupon_eq_sound; assumption.
      + apply eq_tree_entries.
        match goal with ListProof : Forall2 coupon_eq _ _ |- _ =>
          refine ((fix list_sound xs ys (D : Forall2 coupon_eq xs ys) {struct D} : Forall2 eq xs ys := _) _ _ ListProof) end.
        destruct D; constructor; [apply coupon_eq_sound; assumption|apply list_sound; assumption].
      + match goal with D : coupon_storage _ _ |- _ => inversion D; subst; apply R_to_eq, RBlob; assumption end.
      + apply eq_refl.
      + apply eq_sym, coupon_eq_sound; assumption.
      + eapply eq_trans; [apply coupon_eq_sound; eassumption|apply coupon_eq_sound; eassumption].
  Defined.
  Theorem coupon_sound :
    (forall h r, coupon_force h r -> force_result h r) /\
    (forall h1 h2, coupon_storage h1 h2 -> exists b1 b2,
      h1 = HBlobObj b1 /\ h2 = HBlobObj b2 /\ get_blob_data b1 = get_blob_data b2) /\
    (forall h r, coupon_apply h r -> exists t, h = HTreeObj t /\ A.apply_tree t = Some r) /\
    (forall h r, coupon_slice h r -> exists t ref, h = HTreeObj t /\ r = Data (Ref ref) /\ Sl.slice t = Some ref) /\
    (forall h r, coupon_identify h r -> exists d, h = Data d /\ I.identify d = Some r) /\
    (forall h r, coupon_think h r -> think_result h r) /\
    (forall h r, coupon_eval h r -> eval_result h r) /\
    (forall h1 h2, coupon_eq h1 h2 -> eq h1 h2).
  Proof.
    repeat split; [exact coupon_force_sound|exact coupon_storage_sound|exact coupon_apply_sound|
      exact coupon_slice_sound|exact coupon_identify_sound|exact coupon_think_sound|exact coupon_eval_sound|exact coupon_eq_sound].
  Qed.
  Corollary coupon_blob_same_data b1 b2 : coupon_eq (HBlobObj b1) (HBlobObj b2) -> get_blob_data b1 = get_blob_data b2.
  Proof.
    intro CouponEq; destruct (eq_blob_same_data _ _ (coupon_eq_sound _ _ CouponEq) b1 Logic.eq_refl) as
      [[b [E' Same]]|[th [b [E' Rest]]]]; [inversion E'; subst; exact Same|discriminate].
  Qed.
End Coupon.
