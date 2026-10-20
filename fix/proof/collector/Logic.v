(** Defines coupon inference rules and proves their semantic soundness,
    including that equivalent Blob Objects have identical contents. *)

From Stdlib Require Import List Arith.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Equivalence Require Import Closure Indistinguishability.

Module Coupon (S : STORAGE) (P : PROGRAM S).
  Module O := Indistinguishability S P.
  Module EC := EquivalenceClosure S P.
  Import EC EC.E EC.E.EP EC.E.EP.E EC.E.EP.E.H.

  Definition coupon_indistinguishable := O.indistinguishable.

  Reserved Notation "h =[ 'Force' ]=> r" (at level 70, no associativity).
  Reserved Notation "h =[ 'Apply' ]=> r" (at level 70, no associativity).
  Reserved Notation "h =[ 'Select' ]=> r" (at level 70, no associativity).
  Reserved Notation "h =[ 'Think' ]=> r" (at level 70, no associativity).
  Reserved Notation "h =[ 'Eval' ]=> r" (at level 70, no associativity).
  Reserved Notation "h =[ 'Equivalent' ]=> r" (at level 70, no associativity).

  Inductive coupon_force : handle -> handle -> Prop :=
  | CouponForce : forall th r,
      force th = Some r ->
      Thunk th =[ Force ]=> r
  | CouponThinkToForce : forall th d,
      Thunk th =[ Think ]=> Data d ->
      Thunk th =[ Force ]=> Data d
  | CouponForceEq : forall h1 h2 h3,
      h1 =[ Equivalent ]=> h2 ->
      h1 =[ Force ]=> h3 ->
      h2 =[ Force ]=> h3
  with coupon_apply : handle -> handle -> Prop :=
  | CouponApply : forall t result,
      value_handle (HTreeObj t) ->
      A.apply_tree t = Some result ->
      HTreeObj t =[ Apply ]=> result
  with coupon_select : handle -> handle -> Prop :=
  | CouponSelect : forall t r,
      Sl.select t = Some r ->
      HTreeObj t =[ Select ]=> r
  with coupon_think : handle -> handle -> Prop :=
  | CouponThinkApplication : forall t t' h,
      HTreeObj t =[ Eval ]=> HTreeObj t' ->
      HTreeObj t' =[ Apply ]=> h ->
      Thunk (Application t) =[ Think ]=> h
  | CouponThinkSelection : forall t t' h,
      HTreeObj t =[ Eval ]=> HTreeObj t' ->
      HTreeObj t' =[ Select ]=> h ->
      Thunk (Selection t) =[ Think ]=> h
  | CouponThinkIdentification : forall d,
      Thunk (Identification d) =[ Think ]=> Data (Ref d)
  with coupon_eval : handle -> handle -> Prop :=
  | CouponEvalBlobObj : forall b,
      HBlobObj b =[ Eval ]=> HBlobObj b
  | CouponEvalTreeObj : forall t1 t2,
      tree_tag t1 = tree_tag t2 ->
      Forall2 coupon_eval (get_tree_raw t1) (get_tree_raw t2) ->
      HTreeObj t1 =[ Eval ]=> HTreeObj t2
  | CouponEvalRef : forall r,
      Data (Ref r) =[ Eval ]=> Data (Ref r)
  | CouponEvalThunk : forall th,
      Thunk th =[ Eval ]=> Thunk th
  | CouponEvalEq : forall h1 h2 h3,
      h1 =[ Eval ]=> h2 ->
      h1 =[ Equivalent ]=> h3 ->
      h3 =[ Eval ]=> h2
  with coupon_equiv : handle -> handle -> Prop :=
  | CouponThunkEq : forall t1 t2,
      Thunk t1 =[ Think ]=> Thunk t2 ->
      Thunk t1 =[ Equivalent ]=> Thunk t2
  | CouponThunkEncodeEq : forall t1 t2,
      Thunk t1 =[ Think ]=> Encode (Strict t2) ->
      Thunk t1 =[ Equivalent ]=> Thunk t2
  | CouponThunkEncodeShallowEq : forall t1 t2,
      Thunk t1 =[ Think ]=> Encode (Shallow t2) ->
      Thunk t1 =[ Equivalent ]=> Thunk t2
  | CouponForceResultEq : forall h1 h1' h2 h2',
      h1 =[ Force ]=> h1' ->
      h2 =[ Force ]=> h2' ->
      h1' =[ Equivalent ]=> h2' ->
      h1 =[ Equivalent ]=> h2
  | CouponForceToEncodeStrict : forall th h,
      Thunk th =[ Force ]=> h ->
      Encode (Strict th) =[ Equivalent ]=> lift h
  | CouponForceToEncodeShallow : forall th h,
      Thunk th =[ Force ]=> h ->
      Encode (Shallow th) =[ Equivalent ]=> lower h
  | CouponEqApplication : forall t1 t2,
      HTreeObj t1 =[ Equivalent ]=> HTreeObj t2 ->
      Thunk (Application t1) =[ Equivalent ]=> Thunk (Application t2)
  | CouponEqSelection : forall t1 t2,
      HTreeObj t1 =[ Equivalent ]=> HTreeObj t2 ->
      Thunk (Selection t1) =[ Equivalent ]=> Thunk (Selection t2)
  | CouponEqIdentification : forall d1 d2,
      Data (Ref d1) =[ Equivalent ]=> Data (Ref d2) ->
      Thunk (Identification d1) =[ Equivalent ]=> Thunk (Identification d2)
  | CouponEqEncodeStrict : forall t1 t2,
      Thunk t1 =[ Equivalent ]=> Thunk t2 ->
      Encode (Strict t1) =[ Equivalent ]=> Encode (Strict t2)
  | CouponEqEncodeShallow : forall t1 t2,
      Thunk t1 =[ Equivalent ]=> Thunk t2 ->
      Encode (Shallow t1) =[ Equivalent ]=> Encode (Shallow t2)
  | CouponEvalResultEq : forall h1 h2,
      h1 =[ Eval ]=> h2 ->
      h1 =[ Equivalent ]=> h2
  | CouponTreeRefEq : forall t1 t2,
      HTreeObj t1 =[ Equivalent ]=> HTreeObj t2 ->
      HTreeRef t1 =[ Equivalent ]=> HTreeRef t2
  | CouponBlobRefEq : forall b1 b2,
      HBlobObj b1 =[ Equivalent ]=> HBlobObj b2 ->
      HBlobRef b1 =[ Equivalent ]=> HBlobRef b2
  | CouponTreeEq : forall t1 t2,
      tree_tag t1 = tree_tag t2 ->
      Forall2 coupon_equiv (get_tree_raw t1) (get_tree_raw t2) ->
      HTreeObj t1 =[ Equivalent ]=> HTreeObj t2
  | CouponBlobEq : forall b1 b2,
      get_blob_data b1 = get_blob_data b2 ->
      HBlobObj b1 =[ Equivalent ]=> HBlobObj b2
  | CouponSelf : forall h,
      h =[ Equivalent ]=> h
  | CouponSym : forall h1 h2,
      h1 =[ Equivalent ]=> h2 ->
      h2 =[ Equivalent ]=> h1
  | CouponTrans : forall h1 h2 h3,
      h1 =[ Equivalent ]=> h2 ->
      h2 =[ Equivalent ]=> h3 ->
      h1 =[ Equivalent ]=> h3
  where "h =[ 'Force' ]=> r" := (coupon_force h r)
  and "h =[ 'Apply' ]=> r" := (coupon_apply h r)
  and "h =[ 'Select' ]=> r" := (coupon_select h r)
  and "h =[ 'Think' ]=> r" := (coupon_think h r)
  and "h =[ 'Eval' ]=> r" := (coupon_eval h r)
  and "h =[ 'Equivalent' ]=> r" := (coupon_equiv h r).

  (** An Apply derivation describes one application of a Value combination and its exact
      result. *)
  Lemma coupon_apply_sound h r : coupon_apply h r -> exists t,
    h = HTreeObj t /\ value_handle h /\ A.apply_tree t = Some r.
  Proof. destruct 1; eauto. Qed.
  (** A Select derivation describes selection from its operand Tree and its exact result. *)
  Lemma coupon_select_sound h r : coupon_select h r -> exists t, h = HTreeObj t /\ Sl.select t = Some r.
  Proof. intro H; inversion H; subst; eauto. Qed.
  (** A derived computation claim may use an equivalent representative of a result or
      source. The semantic interpretation must accommodate that substitution, or soundness
      would fail at the equivalence exchanges. Force compares the lifted Objects behind
      its Data results; Eval additionally requires the claimed representative to be a
      Value. Primitive Apply and Select claims retain their exact results. *)
  Definition force_result h r := match h, r with
    | Thunk th, Data d => exists actual,
      force th = Some (Data actual) /\
      computational_equiv (lift (Data d)) (lift (Data actual))
    | _, _ => False end.
  Definition think_result h r := match h with
    | Thunk th => exists actual,
      think actual = Some r /\ computational_equiv (Thunk th) (Thunk actual)
    | _ => False end.
  Definition eval_result h r := exists v, eval h = Some v /\ computational_equiv r v /\ value_handle r.

  (** An Eval result is computationally equivalent to its source. *)
  Lemma eval_result_equiv h r : eval_result h r -> computational_equiv h r.
  Proof.
    hauto cases: - use: eval_to_equiv, equiv_sym, equiv_trans unfold!: eval_result.
  Qed.
  (** An actual successful force satisfies the abstract Force-result claim. *)
  Lemma force_result_direct th r : force th = Some r -> force_result (Thunk th) r.
  Proof.
    intro Forced; destruct (force_data _ _ Forced) as [data ->].
    exists data; auto using equiv_refl.
  Qed.
  (** A Force-result claim remains true when its input is replaced by a computationally
      equivalent handle. *)
  Lemma force_result_equiv h1 h2 r : computational_equiv h1 h2 -> force_result h1 r -> force_result h2 r.
  Proof.
    (* The old claim relates its chosen Data to the actual lifted forcing result. Force
        preservation supplies an actual result for the equivalent source; transitivity
        connects the same chosen Data to this new result, so the claim can retain its
        endpoint. *)
    intros Related.
    destruct h1 as [data|th1|encoded], r as [d|th|e]; try contradiction.
    intros [actual1 [First Output]].
    destruct (force_equiv _ _ Related th1 eq_refl) as (th2 & -> & Forces).
    rewrite First in Forces.
    destruct (force th2) as [result|] eqn:Second; [|contradiction].
    destruct (force_data _ _ Second) as [actual2 ->].
    exists actual2; split; [exact Second|].
    eapply equiv_trans; eassumption.
  Qed.
  (** A Think-result claim returning Data also satisfies the Force-result claim. *)
  Lemma think_result_data h d : think_result h (Data d) -> force_result h (Data d).
  Proof.
    (* Once a Think step returns Data, Force has no deferred work left. The claim may
        have used an equivalent source for that Think step; forcing congruence transfers
        the completed result back to the stated source. *)
    destruct h as [data|source|encoded]; [contradiction| |contradiction].
    intros [actual [Thought Related]].
    assert (Finished : force actual = Some (Data d))
      by (rewrite force_unfold, Thought; reflexivity).
    eapply force_result_equiv; [apply equiv_sym; exact Related|].
    exact (force_result_direct _ _ Finished).
  Qed.

  (** An Eval-result claim remains true for a computationally equivalent input. *)
  Lemma eval_result_substitute h equivalent result :
    eval_result h result -> computational_equiv h equivalent -> eval_result equivalent result.
  Proof.
    (* The old claim need not name the literal output of its run. Evaluation preservation
        supplies an output for the new source, equivalent to the old actual output. Join
        that equivalence with the one already relating the claimed result to the old
        actual output. *)
    intros [actual [Evaluated [Related Value]]] Equivalent.
    (* A success on the original input rules out absence of a result on the equivalent
        input. This existence fact is necessary before the result equivalences can be
        composed. *)
    assert (EvaluationsRelated : rel_opt computational_equiv (Some actual) (eval equivalent))
      by (rewrite <- Evaluated; apply equiv_eval; exact Equivalent).
    destruct (eval equivalent) eqn:EquivalentEvaluated; cbn [rel_opt] in EvaluationsRelated; [|contradiction].
    hauto cases: - use: equiv_trans unfold!: eval_result.
  Qed.

  (** Evaluating a selection's operand Tree and selecting from the result establishes its
      Think-result claim. *)
  Lemma think_result_selection t t' h : eval_result (HTreeObj t) (HTreeObj t') ->
    Sl.select t' = Some h -> think_result (Thunk (Selection t)) h.
  Proof.
    intros Eval Select; pose proof (eval_result_equiv _ _ Eval) as Related.
    destruct Eval as [v [Ev [Out Value]]].
    exists (Selection t'); split.
    - rewrite think_selection, (value_tree_fixed_eval _ Value); exact Select.
    - apply (tree_thunk_equiv Selection (selection_thunk_cong R R_properties)); exact Related.
  Qed.
  (** Successful identification establishes the corresponding Think-result claim. *)
  Lemma think_result_identification d h : I.identify d = Some h -> think_result (Thunk (Identification d)) h.
  Proof.
    intro Identify; exists (Identification d); split;
      [rewrite think_identification; exact Identify|apply equiv_refl].
  Qed.
  (** A Think-result claim returning a bare Thunk establishes equivalence to that Thunk. *)
  Lemma think_result_single th1 th2 : think_result (Thunk th1) (Thunk th2) -> computational_equiv (Thunk th1) (Thunk th2).
  Proof.
    intros [actual [Thought Related]].
    eapply equiv_trans; [exact Related|apply R_to_equiv, RThinkSingleStepThunk; exact Thought].
  Qed.
  (** A Think-result claim returning a strict Encode establishes equivalence to its
      underlying Thunk. *)
  Lemma think_result_single_strict th1 th2 : think_result (Thunk th1) (Encode (Strict th2)) -> computational_equiv (Thunk th1) (Thunk th2).
  Proof.
    intros [actual [Thought Related]].
    eapply equiv_trans; [exact Related|apply R_to_equiv, RThunkSingleStepEncodeStrict; exact Thought].
  Qed.
  (** A Think-result claim returning a shallow Encode establishes equivalence to its
      underlying Thunk. *)
  Lemma think_result_single_shallow th1 th2 : think_result (Thunk th1) (Encode (Shallow th2)) -> computational_equiv (Thunk th1) (Thunk th2).
  Proof.
    intros [actual [Thought Related]].
    eapply equiv_trans; [exact Related|apply R_to_equiv, RThunkSingleStepEncodeShallow; exact Thought].
  Qed.
  (** A Force-result claim makes the strict Encode equivalent to the result's lifted
      Object. *)
  Lemma force_result_strict th h : force_result (Thunk th) h -> computational_equiv (Encode (Strict th)) (lift h).
  Proof.
    (* Strict execution exposes the actual forced Object. The Force claim already relates
        the chosen result's Object to that actual Object. Reversing this latter relation
        gives the path from the strict request to the claimed lifted result. *)
    destruct h as [d|thunk|encoded]; try contradiction.
    intros [actual [Forced Related]].
    assert (Executed : execute (Strict th) = Some (lift (Data actual)))
      by (rewrite execute_unfold; cbn; rewrite Forced; reflexivity).
    hauto cases: - use: R_to_equiv, REvalStep, equiv_sym, equiv_trans.
  Qed.
  (** Lifting Data before lowering it leaves the final Ref unchanged. *)
  Lemma lower_lift_data d : lower (lift (Data d)) = lower (Data d).
  Proof. destruct d as [[b|t]|[b|t]]; reflexivity. Qed.
  (** A Force-result claim makes the shallow Encode equivalent to the lowered result. *)
  Lemma force_result_shallow th h : force_result (Thunk th) h -> computational_equiv (Encode (Shallow th)) (lower h).
  Proof.
    (* A shallow consumer obtains a Ref to the same underlying Object used by the strict
        argument. Applying lowering to the strict equivalence therefore reuses the
        computation justification while changing only how its result is exposed. *)
    destruct h as [d|thunk|encoded]; try contradiction.
    intro Forced.
    pose proof (equiv_to_lower _ _ (force_result_strict _ _ Forced)) as Lowered.
    change (computational_equiv (Encode (Shallow th)) (lower (lift (Data d)))) in Lowered.
    now rewrite lower_lift_data in Lowered.
  Qed.
  (** A Blob Object satisfies its own unchanged Eval-result claim. *)
  Lemma eval_result_blob b : eval_result (HBlobObj b) (HBlobObj b).
  Proof. exists (HBlobObj b); split; [apply eval_blob|split; [apply equiv_refl|constructor]]. Qed.
  (** A Ref satisfies its own unchanged Eval-result claim. *)
  Lemma eval_result_ref ref : eval_result (Data (Ref ref)) (Data (Ref ref)).
  Proof. exists (Data (Ref ref)); split; [apply eval_ref|split; [apply equiv_refl|constructor]]. Qed.
  (** A bare Thunk satisfies its own unchanged Eval-result claim. *)
  Lemma eval_result_thunk th : eval_result (Thunk th) (Thunk th).
  Proof. exists (Thunk th); split; [apply eval_thunk|split; [apply equiv_refl|constructor]]. Qed.
  (** Equivalent Refs give equivalent identification Thunks. *)
  Lemma equiv_identification_data d1 d2 : computational_equiv (Data (Ref d1)) (Data (Ref d2)) -> computational_equiv (Thunk (Identification d1)) (Thunk (Identification d2)).
  Proof.
    intro Related; apply (think_to_equiv (Data (Ref d1)) (Data (Ref d2)) Related (Identification d1) (Identification d2) (Ref d1) (Ref d2)).
    - rewrite think_identification; reflexivity.
    - rewrite think_identification; reflexivity.
    - split; left; apply RSelf.
  Qed.
  (** Matching markers and pointwise equivalent children give equivalent Tree Objects. *)
  Lemma equiv_tree_entries t1 t2 : tree_tag t1 = tree_tag t2 -> Forall2 computational_equiv (get_tree_raw t1) (get_tree_raw t2) -> computational_equiv (HTreeObj t1) (HTreeObj t2).
  Proof.
    hauto cases: - use: equiv_tree_iff.
  Qed.
  (** Pointwise Eval-result claims admit actual evaluated children equivalent to the
      claimed Value results. *)
  Lemma eval_result_list xs ys : Forall2 eval_result xs ys -> exists zs,
    Forall2 (fun h v => eval h = Some v) xs zs /\ Forall2 computational_equiv ys zs /\ Forall value_handle ys.
  Proof.
    intro Related; induction Related.
    - exists nil; repeat split; constructor.
    - destruct H as [v [Ev [Out Value]]], IHRelated as [zs [Evs [Outs Values]]].
      exists (v :: zs); split; [constructor; assumption|split; constructor; assumption].
  Qed.
  (** Matching markers and pointwise Eval-result claims establish the whole Tree's
      Eval-result claim. *)
  Lemma eval_result_tree t1 t2 : tree_tag t1 = tree_tag t2 -> Forall2 eval_result (get_tree_raw t1) (get_tree_raw t2) ->
    eval_result (HTreeObj t1) (HTreeObj t2).
  Proof.
    (* Each child claim may choose an equivalent Value rather than the literal child
        result. Collect the actual child runs to obtain a real Tree evaluation, then use
        pointwise equivalence to relate the claimed Tree to that actual result. *)
    intros Tags Related; destruct (eval_result_list _ _ Related) as [zs [Evs [Outs Values]]].
    exists (HTreeObj (create_tree_tagged (tree_tag t1) zs)); split.
    - apply evaluation_tree; exact Evs.
    - split.
      + rewrite <- (create_tree_get_tree_raw t2) at 1; rewrite Tags; apply equiv_tree_tagged_Forall2; exact Outs.
      + apply tree_obj_handle, value_tree_intro; exact Values.
  Qed.

  (** Every Force coupon derivation satisfies the Force-result relation. *)
  Fixpoint coupon_force_sound (h r : handle) (Derivation : coupon_force h r) {struct Derivation} : force_result h r
  (** Every Think coupon derivation satisfies the Think-result relation. *)
  with coupon_think_sound (h r : handle) (Derivation : coupon_think h r) {struct Derivation} : think_result h r
  (** Every Eval coupon derivation satisfies the Eval-result relation. *)
  with coupon_eval_sound (h r : handle) (Derivation : coupon_eval h r) {struct Derivation} : eval_result h r
  (** Every equivalence coupon derivation establishes computational equivalence. *)
  with coupon_equiv_sound (h r : handle) (Derivation : coupon_equiv h r) {struct Derivation} : computational_equiv h r.
  Proof.
    (* Induct over the derivation evidence, not over a fresh evaluation of its endpoints.
        Primitive Apply and Select premises supply completed computations; the other cases
        compose those facts using the semantic congruence lemmas. *)
    - destruct Derivation as [th result Forced|th d Thought|a b result Related Forced].
      + apply force_result_direct; exact Forced.
      + exact (think_result_data _ _ (coupon_think_sound _ _ Thought)).
      + pose proof (coupon_equiv_sound _ _ Related) as InputsRelated.
        pose proof (coupon_force_sound _ _ Forced) as ResultForced.
        eapply force_result_equiv; eassumption.
    - destruct Derivation as [input ready result Evaluated Applied|
        input ready result Evaluated Selected|reference].
      + pose proof (coupon_eval_sound _ _ Evaluated) as CombinationEvaluated.
        pose proof (eval_result_equiv _ _ CombinationEvaluated) as CombinationsRelated.
        destruct CombinationEvaluated as [actual [Evaluation [ResultRelated Value]]].
        destruct (coupon_apply_sound _ _ Applied) as [tree [Same [Ready AppliedStep]]];
          inversion Same; subst tree.
        (* The Apply premise concerns the ready combination. Since that combination is a
            Value, evaluating it again does not change it; the application fact
            consequently supplies a genuine Think result. *)
        assert (ApplicationComputed : think (Application ready) = Some result).
        { rewrite think_application, (value_tree_fixed_eval _ Value); exact AppliedStep. }
        exists (Application ready); split.
        * exact ApplicationComputed.
        * apply (tree_thunk_equiv Application (application_thunk_cong R R_properties));
            exact CombinationsRelated.
      + pose proof (coupon_eval_sound _ _ Evaluated) as CombinationEvaluated.
        destruct (coupon_select_sound _ _ Selected) as [tree [Same SelectedStep]];
          inversion Same; subst tree.
        eapply think_result_selection; eassumption.
      + apply think_result_identification; reflexivity.
    - destruct Derivation as [blob|input output Tags Children|reference|th|
        input result equivalent Evaluated Related].
      + apply eval_result_blob.
      + assert (ChildrenEvaluated : Forall2 eval_result (get_tree_raw input) (get_tree_raw output)).
        { refine ((fix list_sound xs ys (D : Forall2 coupon_eval xs ys) {struct D} :
            Forall2 eval_result xs ys := _) _ _ Children).
          destruct D; constructor; [apply coupon_eval_sound; assumption|apply list_sound; assumption]. }
        apply eval_result_tree; assumption.
      + apply eval_result_ref.
      + apply eval_result_thunk.
      + exact (eval_result_substitute _ _ _ (coupon_eval_sound _ _ Evaluated)
          (coupon_equiv_sound _ _ Related)).
    - destruct Derivation as [first second Thought|first second Thought|first second Thought|
        first first_result second second_result First Second Related|
        th result Forced|th result Forced|input output Related|input output Related|
        first second Related|first second Related|first second Related|
        input result Evaluated|input output Related|input output Related|
        input output Tags Children|first second Contents|value|first second Related|
        first middle last First Second].
      + apply think_result_single; exact (coupon_think_sound _ _ Thought).
      + apply think_result_single_strict; exact (coupon_think_sound _ _ Thought).
      + apply think_result_single_shallow; exact (coupon_think_sound _ _ Thought).
      + pose proof (coupon_force_sound _ _ First) as FirstForced.
        pose proof (coupon_force_sound _ _ Second) as SecondForced.
        pose proof (coupon_equiv_sound _ _ Related) as ResultsRelated.
        destruct first as [data|th1|encoded], first_result as [d1|th|e]; try contradiction.
        destruct second as [data|th2|encoded], second_result as [d2|th|e]; try contradiction.
        destruct FirstForced as (actual1 & F1 & Rel1), SecondForced as (actual2 & F2 & Rel2).
        (* The premises may name equivalent representatives of their forced Data.
            Transport the stated result equivalence through those representative-to-actual
            equivalences before comparing the source Thunks. *)
        assert (ActualResultsRelated : computational_equiv (lift (Data actual1)) (lift (Data actual2)))
          by (hauto cases: - use: equiv_sym, equiv_trans, equiv_to_lift).
        exact (force_some_to_equiv _ _ _ _ F1 F2 ActualResultsRelated).
      + apply force_result_strict; exact (coupon_force_sound _ _ Forced).
      + apply force_result_shallow; exact (coupon_force_sound _ _ Forced).
      + apply (tree_thunk_equiv Application (application_thunk_cong R R_properties)).
        exact (coupon_equiv_sound _ _ Related).
      + apply (tree_thunk_equiv Selection (selection_thunk_cong R R_properties)).
        exact (coupon_equiv_sound _ _ Related).
      + apply equiv_identification_data; exact (coupon_equiv_sound _ _ Related).
      + apply equiv_thunk_to_strict_encode; exact (coupon_equiv_sound _ _ Related).
      + apply equiv_thunk_to_shallow_encode; exact (coupon_equiv_sound _ _ Related).
      + apply eval_result_equiv; exact (coupon_eval_sound _ _ Evaluated).
      + apply equiv_tree_to_ref; exact (coupon_equiv_sound _ _ Related).
      + apply equiv_blob_to_ref; exact (coupon_equiv_sound _ _ Related).
      (* Every child coupon supplies one semantic equivalence. Tree congruence then lifts
          this whole family to the parent; it requires no execution or inspection of the
          children's computation histories. *)
      + assert (ChildrenRelated : Forall2 computational_equiv (get_tree_raw input) (get_tree_raw output)).
        { refine ((fix list_sound xs ys (D : Forall2 coupon_equiv xs ys) {struct D} :
            Forall2 computational_equiv xs ys := _) _ _ Children).
          destruct D; constructor; [apply coupon_equiv_sound; assumption|apply list_sound; assumption]. }
        apply equiv_tree_entries; assumption.
      + apply R_to_equiv, RBlob; exact Contents.
      + apply equiv_refl.
      + apply equiv_sym; exact (coupon_equiv_sound _ _ Related).
      + pose proof (coupon_equiv_sound _ _ First) as FirstRelated.
        pose proof (coupon_equiv_sound _ _ Second) as SecondRelated.
        eapply equiv_trans; eassumption.
  Defined.
  (** Every abstract coupon derivation describes a valid computation claim or a valid
      computational equivalence. This is the logical soundness boundary on which the
      exchange checker relies. *)
  Theorem coupon_sound :
    (forall h r, coupon_force h r -> force_result h r) /\
    (forall h r, coupon_apply h r -> exists t,
      h = HTreeObj t /\ value_handle h /\ A.apply_tree t = Some r) /\
    (forall h r, coupon_select h r -> exists t, h = HTreeObj t /\ Sl.select t = Some r) /\
    (forall h r, coupon_think h r -> think_result h r) /\
    (forall h r, coupon_eval h r -> eval_result h r) /\
    (forall h1 h2, coupon_equiv h1 h2 -> computational_equiv h1 h2).
  Proof.
    (* The mutually recursive argument justifies each rule from its premises, including
        substitution of equivalent representatives. The combined theorem exposes those
        separate judgements without introducing a new logical rule. *)
    repeat split; auto using coupon_force_sound, coupon_apply_sound, coupon_select_sound,
      coupon_think_sound, coupon_eval_sound, coupon_equiv_sound.
  Qed.
  (** The collector cannot derive an equivalence between two Blob Objects of different
      contents. Soundness turns the coupon derivation into computational equivalence,
      whose Blob-content invariant then excludes such a claim. *)
  Corollary coupon_blob_same_data b1 b2 : coupon_equiv (HBlobObj b1) (HBlobObj b2) -> get_blob_data b1 = get_blob_data b2.
  Proof.
    intro CouponEq; destruct (equiv_blob_same_data _ _ (coupon_equiv_sound _ _ CouponEq) b1 Logic.eq_refl) as
      [[b [E' Same]]|[th [b [E' Rest]]]]; [inversion E'; subst; exact Same|discriminate].
  Qed.
End Coupon.
