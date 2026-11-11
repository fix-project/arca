  (** Every evaluation derivation is realized by a successful evaluator run. *)
  Fixpoint eval_derivation_sound h result (Proof : eval_derivation h result) {struct Proof} :
    eval h = Some result
  (** Every following derivation satisfies the corresponding continuation-of-computation
      relation. *)
  with follow_derivation_sound h result (Proof : follow_derivation h result) {struct Proof} :
    follow_result h result.
  Proof.
    (* Each derivation supplies the evidence for its own computation: application and
        selection supply their primitive result, while recursive premises justify
        evaluating inputs and following the result. Thus the induction realizes a given
        finite derivation; it makes no termination claim about arbitrary programs. *)
    - destruct Proof as [h Leaf|tree children Entries|th reference result Shallow Loaded|
        d|tree ready intermediate result Evaluated Applied Follow|
        tree ready selected result Evaluated Selected Follow].
      + destruct h as [[[b|t]|ref]|th|e]; cbn [leaf] in Leaf; try contradiction;
          apply eval_unique; exists 0; reflexivity.
      + apply evaluation_tree.
        refine ((fix entries xs ys (Proof : Forall2 eval_derivation xs ys) {struct Proof} :
          Forall2 (fun h result => eval h = Some result) xs ys := _) _ _ Entries).
        destruct Proof; constructor; eauto using eval_derivation_sound.
      + assert (ReferenceEvaluated : eval (Encode (H.Shallow th)) = Some (Data (Ref reference)))
          by exact (eval_derivation_sound _ _ Shallow).
        assert (ObjectEvaluated : eval (lift (Data (Ref reference))) = Some result)
          by exact (eval_derivation_sound _ _ Loaded).
        apply strict_evaluation; exists reference; auto.
      + apply (proj2 (shallow_step _ _ _ (think_identification d))).
        reflexivity.
      + assert (CombinationEvaluated : eval_tree tree = Some ready)
          by (hauto cases: - use: eval_derivation_sound, eval_tree_handle_iff).
        assert (Thought : think (Application tree) = Some intermediate)
          by (hauto cases: - use: think_application).
        assert (ResultFollowed : follow_result intermediate result)
          by exact (follow_derivation_sound _ _ Follow).
        apply (proj2 (shallow_step _ _ _ Thought)); exact ResultFollowed.
      + assert (CombinationEvaluated : eval_tree tree = Some ready)
          by (hauto cases: - use: eval_derivation_sound, eval_tree_handle_iff).
        assert (Thought : think (Selection tree) = Some selected)
          by (hauto cases: - use: think_selection).
        assert (ResultFollowed : follow_result selected result)
          by exact (follow_derivation_sound _ _ Follow).
        apply (proj2 (shallow_step _ _ _ Thought)); exact ResultFollowed.
    - destruct Proof as [d|h th result Next Evaluated]; [reflexivity|].
      destruct h as [d|next|[next|next]]; cbn [next_thunk] in Next;
        try discriminate; inversion Next; subst next; exact (eval_derivation_sound _ _ Evaluated).
  Qed.

  (** If entry evaluations have derivations, a successful finite-fuel Tree evaluation has a
      derivation too. *)
  Lemma tree_fuel_complete n
      (Complete : forall h result, eval_with_fuel n h = Some result -> eval_derivation h result)
      tree ready : eval_tree_with_fuel n tree = Some ready ->
    eval_derivation (HTreeObj tree) (HTreeObj ready).
  Proof.
    unfold eval_tree_with_fuel, eval_tree_using; intro Evaluated.
    apply omap_some in Evaluated as [children [Entries ->]].
    apply EvalTree; apply eval_list_Forall2 in Entries.
    eapply A.Forall2_mono; [exact Complete|exact Entries].
  Qed.

  (** Every successful finite-fuel evaluation has a derivation in the judgement rules;
      forcing and following intermediate results are proved complete at the same time.
      Fuel bounds the depth of the successful computation, not the size of the input: an
      application can produce further computations of unrelated size. *)
  Theorem fuel_complete n :
    (forall h result, eval_with_fuel n h = Some result -> eval_derivation h result) /\
    (forall th result, force_with_fuel n th = Some result ->
      eval_derivation (Encode (Shallow th)) (lower result)) /\
    (forall th intermediate result, think_with_fuel n th = Some intermediate ->
      follow_derivation intermediate result -> eval_derivation (Encode (Shallow th)) result).
  Proof.
    induction n as [|n [Eval [Force Think]]].
    - repeat split; intros.
      + destruct h as [[[b|t]|ref]|th|e]; cbn in H; try discriminate;
          inversion H; subst; apply EvalLeaf; exact I.
      + discriminate.
      + discriminate.
    - assert (EvaluationsComplete : forall h result,
        eval_with_fuel (S n) h = Some result -> eval_derivation h result).
      { intros h result Evaluated; destruct h as [[[b|tree]|ref]|th|e];
          try solve [inversion Evaluated; apply EvalLeaf; exact I].
        * change (omap HTreeObj (eval_tree_with_fuel n tree) = Some result) in Evaluated.
          apply omap_some in Evaluated as [ready [Evaluated ->]].
          exact (tree_fuel_complete n Eval tree ready Evaluated).
        * change (obind (execute_with_fuel n e) (eval_with_fuel n) = Some result) in Evaluated.
          apply obind_some in Evaluated as [intermediate [Executed Evaluated]].
          rewrite execute_with_fuel_force in Executed.
          apply omap_some in Executed as [actual [Forced ->]].
          destruct (force_with_fuel_to_data _ _ _ Forced) as [d ->].
          destruct (lower_reference d) as [reference [Lower Lift]].
          destruct e as [th|th]; cbn [encode_to_thunk] in Forced |- *.
          -- assert (ReferenceDerived : eval_derivation (Encode (Shallow th)) (Data (Ref reference))).
             { rewrite <- Lower; apply Force; exact Forced. }
             assert (ObjectDerived : eval_derivation (lift (Data (Ref reference))) result).
             { rewrite Lift; apply Eval; exact Evaluated. }
             exact (EvalStrict th reference result ReferenceDerived ObjectDerived).
          -- rewrite Lower in Evaluated.
             assert (eval_with_fuel n (Data (Ref reference)) = Some (Data (Ref reference)))
               as Unchanged by (destruct n; reflexivity).
             rewrite Unchanged in Evaluated; inversion Evaluated; subst result.
             rewrite <- Lower; apply Force; exact Forced. }
      assert (ForcingComplete : forall th result,
        force_with_fuel (S n) th = Some result ->
        eval_derivation (Encode (Shallow th)) (lower result)).
      { intros th result Forced.
        change (obind (think_with_fuel n th) (force_after n) = Some result) in Forced.
        apply obind_some in Forced as [intermediate [Thought Forced]].
        eapply Think; [exact Thought|].
        destruct intermediate as [d|next|[next|next]]; cbn [force_after] in Forced;
          first [inversion Forced; apply FollowData |
            eapply FollowThunk; [reflexivity|auto]]. }
      assert (ThinkingComplete : forall th intermediate result,
        think_with_fuel (S n) th = Some intermediate -> follow_derivation intermediate result ->
        eval_derivation (Encode (Shallow th)) result).
      { intros th intermediate result Thought Follow; destruct th as [tree|d|tree].
        * change (obind (eval_tree_with_fuel n tree) A.apply_tree = Some intermediate) in Thought.
          apply obind_some in Thought as [ready [Evaluated Applied]].
          assert (CombinationDerived : eval_derivation (HTreeObj tree) (HTreeObj ready)).
          { exact (tree_fuel_complete n Eval tree ready Evaluated). }
          exact (EvalApplication tree ready intermediate result CombinationDerived Applied Follow).
        * inversion Thought; subst intermediate; inversion Follow; subst;
            cbn [next_thunk] in *; try discriminate; apply EvalIdentification.
        * change (obind (eval_tree_with_fuel n tree) Sl.select = Some intermediate) in Thought.
          apply obind_some in Thought as [ready [Evaluated Selected]].
          assert (CombinationDerived : eval_derivation (HTreeObj tree) (HTreeObj ready)).
          { exact (tree_fuel_complete n Eval tree ready Evaluated). }
          exact (EvalSelection tree ready intermediate result CombinationDerived Selected Follow). }
      auto.
  Qed.

  (** Every successful evaluator run has a finite evaluation derivation. *)
  Theorem eval_derivation_complete h result : eval h = Some result -> eval_derivation h result.
  Proof. rewrite eval_some; intros [n Evaluated]; eapply (proj1 (fuel_complete n)); exact Evaluated. Qed.
