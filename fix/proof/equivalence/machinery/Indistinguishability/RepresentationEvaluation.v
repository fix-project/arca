  Definition represented_eval_at fuel := forall first second result,
    representation first second -> eval_with_fuel fuel first = Some result ->
    exists output, eval second = Some output /\ representation result output.

  Definition represented_force_at fuel := forall first second result,
    representation (Thunk first) (Thunk second) -> force_with_fuel fuel first = Some result ->
    exists output, force second = Some output /\ representation (lower result) (lower output).

  Lemma represented_list_forward f g xs ys results :
    (forall x y result, representation x y -> f x = Some result ->
      exists output, g y = Some output /\ representation result output) ->
    Forall2 representation xs ys -> eval_list_using f xs = Some results ->
    exists outputs, eval_list_using g ys = Some outputs /\ Forall2 representation results outputs.
  Proof.
    intros Step Related; revert results; induction Related; intros results Computed.
    - cbn [eval_list_using] in Computed; inversion Computed; exists []; split; [reflexivity|constructor].
    - cbn [eval_list_using] in Computed.
      destruct (f x) as [head|] eqn:Head; cbn [obind] in Computed; [|discriminate].
      destruct (eval_list_using f l) as [tail|] eqn:Tail; cbn [omap] in Computed; [|discriminate].
      inversion Computed; subst results.
      destruct (Step x y head H Head) as [other [Other Same]].
      destruct (IHRelated tail eq_refl) as [others [Others Rest]].
      exists (other :: others); split; [cbn [eval_list_using]; rewrite Other, Others; reflexivity|constructor; assumption].
  Qed.

  Lemma eval_tree_using_eval tree : eval_tree tree = eval_tree_using eval tree.
  Proof.
    pose proof (eval_tree_handle tree) as Equation; rewrite eval_tree_children in Equation.
    unfold eval_tree_using.
    destruct (eval_tree tree), (eval_list_using eval (get_tree_raw tree)); cbn [omap] in *; unfold HTreeObj in Equation; congruence.
  Qed.

  Lemma represented_tree_forward fuel first second result :
    represented_eval_at fuel -> representation (HTreeObj first) (HTreeObj second) ->
    eval_tree_with_fuel fuel first = Some result ->
    exists output, eval_tree second = Some output /\ representation (HTreeObj result) (HTreeObj output).
  Proof.
    intros Step Related Computed.
    unfold eval_tree_with_fuel, eval_tree_using in Computed.
    destruct (eval_list_using (eval_with_fuel fuel) (get_tree_raw first)) as [children|] eqn:Children;
      cbn [omap] in Computed; [|discriminate].
    inversion Computed; subst result.
    destruct (represented_list_forward _ _ _ _ children Step (representation_tree_children _ _ Related) Children)
      as [outputs [Output Same]].
    exists (create_tree_tagged (tree_tag second) outputs); split.
    - rewrite eval_tree_using_eval; unfold eval_tree_using; rewrite Output; reflexivity.
    - assert (Tag : tree_tag first = tree_tag second) by (inversion Related; assumption).
      rewrite Tag; apply representation_tree; exact Same.
  Qed.

  Lemma finite_tree_values fuel tree result : eval_tree_with_fuel fuel tree = Some result ->
    Forall not_encode (get_tree_raw result).
  Proof.
    intro Computed.
    assert (Evaluated : eval_with_fuel (S fuel) (HTreeObj tree) = Some (HTreeObj result)).
    { change (omap HTreeObj (eval_tree_with_fuel fuel tree) = Some (HTreeObj result)); rewrite Computed; reflexivity. }
    pose proof (eval_with_fuel_to_value_handle _ _ _ Evaluated) as Value.
    inversion Value; subst; match goal with V : value_tree _ |- _ => inversion V; subst end.
    eapply Forall_impl; [|eassumption]; intros h Val; destruct h; try exact I; inversion Val.
  Qed.

  Lemma unbounded_tree_values tree result : eval_tree tree = Some result -> Forall not_encode (get_tree_raw result).
  Proof.
    intro Computed; apply eval_tree_some in Computed as [fuel Computed].
    eapply finite_tree_values; exact Computed.
  Qed.

  Definition force_follow value := match value with
    | Data _ => Some value
    | Thunk next => force next
    | Encode encoded => force (encode_to_thunk encoded)
    end.

  Lemma represented_follow_forward fuel first second result :
    represented_force_at fuel -> representation first second -> force_after fuel first = Some result ->
    exists output, force_follow second = Some output /\ representation (lower result) (lower output).
  Proof.
    intros Step Related Computed.
    pose proof Related as Whole.
    destruct Related; cbn [force_after force_follow HBlobObj HBlobRef HTreeObj HTreeRef] in Computed |- *;
      try solve [inversion Computed; subst; eexists; split; [reflexivity|apply representation_lower; exact Whole]].
    all: match type of Computed with force_with_fuel _ ?left = Some _ =>
      match goal with |- context [force ?right] =>
        exact (Step left right result ltac:(first [exact Whole|assumption]) Computed)
      end end.
  Qed.

  Lemma represented_application_think fuel first second result :
    represented_eval_at fuel -> representation (HTreeObj first) (HTreeObj second) ->
    think_with_fuel (S fuel) (Application first) = Some result ->
    exists output, think (Application second) = Some output /\ representation result output.
  Proof.
    intros Step Related Computed.
    change (obind (eval_tree_with_fuel fuel first) A.apply_tree = Some result) in Computed.
    destruct (eval_tree_with_fuel fuel first) as [ready|] eqn:Ready; cbn [obind] in Computed; [|discriminate].
    destruct (represented_tree_forward _ _ _ _ Step Related Ready) as [other [Other Same]].
    pose proof (representation_apply _ _ Same) as Applied.
    rewrite Computed in Applied; destruct (A.apply_tree other) as [output|] eqn:Output; cbn [rel_opt] in Applied; [|contradiction].
    exists output; split; [rewrite think_application, Other; exact Output|exact Applied].
  Qed.

  Lemma represented_selection_think fuel first second result :
    represented_eval_at fuel -> representation (HTreeObj first) (HTreeObj second) ->
    think_with_fuel (S fuel) (Selection first) = Some result ->
    exists output, think (Selection second) = Some output /\ representation result output.
  Proof.
    intros Step Related Computed.
    change (obind (eval_tree_with_fuel fuel first) Sl.select = Some result) in Computed.
    destruct (eval_tree_with_fuel fuel first) as [ready|] eqn:Ready; cbn [obind] in Computed; [|discriminate].
    destruct (represented_tree_forward _ _ _ _ Step Related Ready) as [other [Other Same]].
    pose proof (Sl.select_X representation representation_laws ready other
      (conj Same (conj (finite_tree_values _ _ _ Ready) (unbounded_tree_values _ _ Other)))) as Selected.
    rewrite Computed in Selected; destruct (Sl.select other) as [output|] eqn:Output; cbn [rel_opt] in Selected; [|contradiction].
    exists output; split; [rewrite think_selection, Other; exact Output|exact Selected].
  Qed.

  Lemma represented_identification_think fuel first second result :
    representation (Data (Ref first)) (Data (Ref second)) ->
    think_with_fuel (S fuel) (Identification first) = Some result ->
    exists output, think (Identification second) = Some output /\ representation result output.
  Proof.
    intros Related Computed; change (Some (Data (Ref first)) = Some result) in Computed; inversion Computed; subst result.
    exists (Data (Ref second)); split; [rewrite think_identification; reflexivity|exact Related].
  Qed.

  Lemma represented_behavior_force fuel first second result :
    represented_results (omap lower (force first)) (omap lower (force second)) ->
    force_with_fuel fuel first = Some result ->
    exists output, force second = Some output /\ representation (lower result) (lower output).
  Proof.
    intros Related Computed; pose proof (force_unique _ _ (ex_intro _ fuel Computed)) as First.
    rewrite First in Related; destruct (force second) as [output|] eqn:Other; cbn [omap] in Related; inversion Related; subst.
    exists output; auto.
  Qed.

  Lemma represented_fuel : forall fuel, represented_force_at fuel /\ represented_eval_at fuel.
  Proof.
    intro fuel; induction fuel as [fuel IH] using lt_wf_ind; split.
    { intros first second result Related Computed.
      destruct fuel as [|previous]; [discriminate|].
      inversion Related; subst; try solve [eapply represented_behavior_force; eassumption].
      all: destruct previous as [|previous]; [discriminate|].
      all: match type of Computed with
        | force_with_fuel _ ?thunk = _ =>
          change (obind (think_with_fuel (S previous) thunk) (force_after (S previous)) = Some result) in Computed;
          destruct (think_with_fuel (S previous) thunk) as [reply|] eqn:Thought; cbn [obind] in Computed; [|discriminate]
        end.
      all: assert (EvalStep : represented_eval_at previous) by (apply (proj2 (IH previous ltac:(lia)))).
      all: assert (ForceStep : represented_force_at (S previous)) by (apply (proj1 (IH (S previous) ltac:(lia)))).
      all: match goal with |- context [force ?target] =>
        assert (Other : exists output, think target = Some output /\ representation reply output)
          by (eauto using represented_application_think, represented_selection_think, represented_identification_think)
        end.
      all: destruct Other as [other [Other Same]].
      all: destruct (represented_follow_forward _ _ _ _ ForceStep Same Computed) as [output [Follow Result]].
      all: exists output; split; [rewrite force_unfold, Other; exact Follow|exact Result].
    }
    { intros first second result Related Computed.
      destruct fuel as [|previous].
      + destruct Related; cbn [eval_with_fuel with_fuel eval_fn HBlobObj HBlobRef HTreeObj HTreeRef] in Computed; try discriminate;
          inversion Computed; subst; eexists; split; try apply eval_blob; try apply eval_ref; try apply eval_thunk;
          constructor; assumption.
      + inversion Related; subst;
          try solve [match type of Computed with eval_with_fuel _ ?input = _ =>
            change (Some input = Some result) in Computed; inversion Computed; subst result;
            eexists; split; [first [apply eval_blob|apply eval_ref|apply eval_thunk]|exact Related]
            end].
        * match type of Computed with eval_with_fuel _ (HTreeObj ?tree) = _ =>
            change (omap HTreeObj (eval_tree_with_fuel previous tree) = Some result) in Computed;
            destruct (eval_tree_with_fuel previous tree) as [ready|] eqn:Ready; cbn [omap] in Computed; [|discriminate]
          end.
          inversion Computed; subst result.
          destruct (represented_tree_forward previous _ _ ready (proj2 (IH previous ltac:(lia))) ltac:(eassumption) Ready)
            as [output [Output Same]].
          exists (HTreeObj output); split; [rewrite eval_tree_handle, Output; reflexivity|exact Same].
        * match type of Computed with eval_with_fuel _ (Encode ?encoded) = _ =>
            change (obind (execute_with_fuel previous encoded) (eval_with_fuel previous) = Some result) in Computed
          end.
          rewrite execute_with_fuel_force in Computed; cbn [encode_to_thunk] in Computed.
          match type of Computed with context [force_with_fuel previous ?thunk] =>
            destruct (force_with_fuel previous thunk) as [forced|] eqn:Forced; cbn [omap obind] in Computed; [|discriminate]
          end.
          destruct (proj1 (IH previous ltac:(lia)) _ _ forced ltac:(eassumption) Forced) as [other [Other Same]].
          destruct (force_with_fuel_to_data _ _ _ Forced) as [left ->].
          destruct (force_data _ _ Other) as [right ->].
          pose proof (representation_lift_lower _ _ Same) as Lifted.
          destruct (proj2 (IH previous ltac:(lia)) _ _ result Lifted Computed) as [output [Output Result]].
          exists output; split; [rewrite eval_encode, execute_unfold; cbn [encode_to_thunk]; rewrite Other; exact Output|exact Result].
        * match type of Computed with eval_with_fuel _ (Encode ?encoded) = _ =>
            change (obind (execute_with_fuel previous encoded) (eval_with_fuel previous) = Some result) in Computed
          end.
          rewrite execute_with_fuel_force in Computed; cbn [encode_to_thunk] in Computed.
          match type of Computed with context [force_with_fuel previous ?thunk] =>
            destruct (force_with_fuel previous thunk) as [forced|] eqn:Forced; cbn [omap obind] in Computed; [|discriminate]
          end.
          destruct (proj1 (IH previous ltac:(lia)) _ _ forced ltac:(eassumption) Forced) as [other [Other Same]].
          destruct (proj2 (IH previous ltac:(lia)) _ _ result Same Computed) as [output [Output Result]].
          exists output; split; [rewrite eval_encode, execute_unfold; cbn [encode_to_thunk]; rewrite Other; exact Output|exact Result].
    }
  Qed.

  Lemma represented_force_forward first second result : representation (Thunk first) (Thunk second) ->
    force first = Some result -> exists output, force second = Some output /\ representation (lower result) (lower output).
  Proof.
    intros Related Computed; apply force_some in Computed as [fuel Computed].
    exact (proj1 (represented_fuel fuel) _ _ _ Related Computed).
  Qed.

  Lemma represented_force first second : representation (Thunk first) (Thunk second) ->
    represented_results (omap lower (force first)) (omap lower (force second)).
  Proof.
    intro Related; destruct (force first) as [left|] eqn:Left, (force second) as [right|] eqn:Right;
      cbn [omap].
    - destruct (represented_force_forward _ _ _ Related Left) as [output [Output Same]].
      rewrite Right in Output; inversion Output; subst; constructor; exact Same.
    - destruct (represented_force_forward _ _ _ Related Left) as [output [Output _]]; congruence.
    - destruct (represented_force_forward _ _ _ (representation_sym _ _ Related) Right) as [output [Output _]]; congruence.
    - constructor.
  Qed.

  Lemma represented_eval_forward first second result : representation first second -> eval first = Some result ->
    exists output, eval second = Some output /\ representation result output.
  Proof.
    intros Related Computed; apply eval_some in Computed as [fuel Computed].
    exact (proj2 (represented_fuel fuel) _ _ _ Related Computed).
  Qed.

  Lemma represented_eval first second : representation first second -> rel_opt representation (eval first) (eval second).
  Proof.
    intro Related; destruct (eval first) as [left|] eqn:Left, (eval second) as [right|] eqn:Right;
      cbn [rel_opt]; try exact I.
    - destruct (represented_eval_forward _ _ _ Related Left) as [output [Output Same]].
      rewrite Right in Output; inversion Output; subst; exact Same.
    - destruct (represented_eval_forward _ _ _ Related Left) as [output [Output _]]; congruence.
    - destruct (represented_eval_forward _ _ _ (representation_sym _ _ Related) Right) as [output [Output _]]; congruence.
  Qed.
