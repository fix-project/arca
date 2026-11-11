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
