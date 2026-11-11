    (** Related computations can take different numbers of steps. Bound a successful run
        on one side by n and require only some finite successful run on the other; repeat
        in the opposite direction. This asymmetry of fuel bounds is what allows direct
        unfolding steps to be included in the relation. *)
    Definition eval_transport n := forall h1 h2, X h1 h2 ->
      (forall v1, eval_with_fuel n h1 = Some v1 -> exists v2, evals_to h2 v2 /\ X v1 v2) /\
      (forall v2, eval_with_fuel n h2 = Some v2 -> exists v1, evals_to h1 v1 /\ X v1 v2).
    Definition force_transport n := forall t1 t2, X (Thunk t1) (Thunk t2) ->
      (forall v1, force_with_fuel n t1 = Some v1 -> exists v2, forces_to t2 v2 /\ strengthen X v1 v2) /\
      (forall v2, force_with_fuel n t2 = Some v2 -> exists v1, forces_to t1 v1 /\ strengthen X v1 v2).
    Definition think_transport n := forall t1 t2, X (Thunk t1) (Thunk t2) ->
      (forall v1, think_with_fuel n t1 = Some v1 ->
        (exists v2, thinks_to t2 v2 /\ strengthen X v1 v2) \/ v1 = Thunk t2 \/ v1 = Encode (Strict t2) \/ v1 = Encode (Shallow t2)) /\
      (forall v2, think_with_fuel n t2 = Some v2 ->
        (exists v1, thinks_to t1 v1 /\ strengthen X v1 v2) \/ thinks_to t1 (Thunk t2) \/ thinks_to t1 (Encode (Strict t2)) \/ thinks_to t1 (Encode (Shallow t2))).
    Definition force_reply h := match h with
      | Data _ => Some h | Thunk th => force th | Encode e => force (encode_to_thunk e) end.
    (** Every handle is strengthened-related to itself. *)
    Lemma strengthen_self h : strengthen X h h.
    Proof. destruct h; [apply (X_self X Props)|apply (X_self X Props)|left; apply (X_self X Props)]. Qed.
    (** Data cannot be related to a bare Thunk. *)
    Lemma data_not_thunk d th : ~X (Data d) (Thunk th).
    Proof.
      intro R; destruct (proj2 (preserve_thunk X Props _ _ R) (ex_intro _ th eq_refl)) as [u E]; discriminate.
    Qed.
    (** A bare Thunk cannot be related to Data. *)
    Lemma thunk_not_data th d : ~X (Thunk th) (Data d).
    Proof.
      intro R; destruct (proj1 (preserve_thunk X Props _ _ R) (ex_intro _ th eq_refl)) as [u E]; discriminate.
    Qed.
    (** Data cannot be related to an Encode in the forward direction. *)
    Lemma data_not_encode d e : ~X (Data d) (Encode e).
    Proof. intro R; apply (encode_reverse_absent X Props e (Data d) R); intros [e' E]; discriminate. Qed.

    (** The structural and evaluation laws strengthen every related pair of handles. *)
    Lemma related_strengthen h1 h2 : X h1 h2 -> strengthen X h1 h2.
    Proof.
      (* The ordinary relation describes complete handles. A Force continuation needs a
          relation between the work remaining in their replies. Constructor preservation
          rules out incompatible Data/Thunk cases; Encode execution facts connect the
          remaining mixed cases to the same underlying Object. *)
      intro Related; destruct h1 as [d1|th1|e1], h2 as [d2|th2|e2].
      - apply related_data_strengthen; exact Related.
      - exfalso; eapply data_not_thunk; exact Related.
      - exfalso; eapply data_not_encode; exact Related.
      - exfalso; eapply thunk_not_data; exact Related.
      - exact Related.
      - exfalso; destruct (proj1 (preserve_thunk X Props _ _ Related)
          (ex_intro _ th1 eq_refl)) as [th E]; discriminate.
      - pose proof (execute_unique _ _ (encode_eval X Props _ _ Related
          ltac:(intros [e E]; discriminate))) as Exec.
        pose proof (execute_to_lift _ _ Exec) as Lifted.
        destruct e1; apply (execute_result X Props); exact Lifted.
      - exfalso; destruct (proj2 (preserve_thunk X Props _ _ Related)
          (ex_intro _ th2 eq_refl)) as [th E]; discriminate.
      - destruct e1, e2.
        + apply strict_strengthen; exact Related.
        + exfalso; eapply (not_strict_shallow X Props); exact Related.
        + exfalso; eapply (not_shallow_strict X Props); exact Related.
        + apply shallow_strengthen; exact Related.
    Qed.

    (** A successful finite continuation of the left Think result can be matched on the
        right with strengthened-related Data. *)
    Lemma force_reply_forward n (IH : force_transport n) h1 h2 v1 :
      strengthen X h1 h2 -> force_after n h1 = Some v1 ->
      exists v2, force_reply h2 = Some v2 /\ strengthen X v1 v2.
    Proof.
      (* When both replies defer work, the Force induction hypothesis supplies the match.
          The less immediate case is an Encode related to final Data: the relation already
          proves execution to the corresponding lifted Object, so no new computation fact
          needs to be assumed. *)
      intros Related Fuel; destruct h1 as [d1|t1|e1], h2 as [d2|t2|e2].
      - inversion Fuel; subst; exists (Data d2); split; [reflexivity|exact Related].
      - exfalso; eapply data_not_thunk; exact Related.
      - exfalso; eapply data_not_encode; exact Related.
      - exfalso; eapply thunk_not_data; exact Related.
      - destruct (proj1 (IH t1 t2 Related) v1 Fuel) as [v2 [F2 Strength]].
        eauto using force_unique.
      - destruct (proj1 (IH t1 (encode_to_thunk e2) Related) v1 Fuel) as [v2 [F2 Strength]].
        eauto using force_unique.
      - assert (X (Encode (Strict (encode_to_thunk e1))) (lift (Data d2))) as ExecRel by
          (destruct e1; exact Related).
        pose proof (execute_unique _ _ (encode_eval X Props _ _ ExecRel ltac:(intros [e E]; discriminate))) as Exec.
        rewrite execute_unfold in Exec; simpl in Exec.
        assert (force (encode_to_thunk e1) = Some v1) as F1 by
          (apply force_unique; exists n; destruct e1; exact Fuel).
        rewrite F1 in Exec; inversion Exec as [Lift].
        destruct (force_data _ _ F1) as [d ->].
        exists (Data d2); split; [reflexivity|].
        change (X (Data (lift_data d)) (Data (lift_data d2))); rewrite <- Lift; apply (X_self X Props).
      - destruct (proj1 (IH (encode_to_thunk e1) t2 Related) v1 ltac:(destruct e1; exact Fuel)) as [v2 [F2 Strength]].
        eauto using force_unique.
      - destruct Related as [Thunks|Forces].
        + destruct (proj1 (IH (encode_to_thunk e1) (encode_to_thunk e2) Thunks) v1 ltac:(destruct e1; exact Fuel)) as [v2 [F2 Strength]].
          eauto using force_unique.
        + assert (force (encode_to_thunk e1) = Some v1) as F1 by
            (apply force_unique; exists n; destruct e1; exact Fuel).
          rewrite F1 in Forces; destruct (force (encode_to_thunk e2)) as [v2|] eqn:F2; [|contradiction].
          destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->].
          exists (Data d2); split; [exact F2|exact Forces].
    Qed.
    (** A successful finite continuation of the right Think result can be matched on the
        left with strengthened-related Data. *)
    Lemma force_reply_backward n (IH : force_transport n) h1 h2 v2 :
      strengthen X h1 h2 -> force_after n h2 = Some v2 ->
      exists v1, force_reply h1 = Some v1 /\ strengthen X v1 v2.
    Proof.
      (* The same argument must work starting from a right-hand success. In an
          Encode/Data pair, the Data is already final, while the relation supplies the
          left execution needed to reach it. For two deferred replies, reuse the finite
          forcing argument. *)
      intros Related Fuel; destruct h1 as [d1|t1|e1], h2 as [d2|t2|e2].
      - inversion Fuel; subst; exists (Data d1); split; [reflexivity|exact Related].
      - exfalso; eapply data_not_thunk; exact Related.
      - exfalso; eapply data_not_encode; exact Related.
      - exfalso; eapply thunk_not_data; exact Related.
      - destruct (proj2 (IH t1 t2 Related) v2 Fuel) as [v1 [F1 Strength]].
        eauto using force_unique.
      - destruct (proj2 (IH t1 (encode_to_thunk e2) Related) v2 ltac:(destruct e2; exact Fuel)) as [v1 [F1 Strength]].
        eauto using force_unique.
      - inversion Fuel; subst v2.
        assert (X (Encode (Strict (encode_to_thunk e1))) (lift (Data d2))) as ExecRel by
          (destruct e1; exact Related).
        pose proof (execute_unique _ _ (encode_eval X Props _ _ ExecRel ltac:(intros [e E]; discriminate))) as Exec.
        rewrite execute_unfold in Exec; simpl in Exec; apply omap_some in Exec.
        destruct Exec as [v1 [F1 Lift]].
        destruct (force_data _ _ F1) as [d ->].
        cbn [lift] in Lift.
        exists (Data d); split; [exact F1|].
        change (X (Data (lift_data d)) (Data (lift_data d2))); rewrite <- Lift; apply (X_self X Props).
      - destruct (proj2 (IH (encode_to_thunk e1) t2 Related) v2 Fuel) as [v1 [F1 Strength]].
        eauto using force_unique.
      - destruct Related as [Thunks|Forces].
        + destruct (proj2 (IH (encode_to_thunk e1) (encode_to_thunk e2) Thunks) v2 ltac:(destruct e2; exact Fuel)) as [v1 [F1 Strength]].
          eauto using force_unique.
        + assert (force (encode_to_thunk e2) = Some v2) as F2 by
            (apply force_unique; exists n; destruct e2; exact Fuel).
          rewrite F2 in Forces; destruct (force (encode_to_thunk e1)) as [v1|] eqn:F1; [|contradiction].
          destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->].
          exists (Data d1); split; [exact F1|exact Forces].
    Qed.
    (** A successful Think step followed to Data gives a successful force of the original
        Thunk. *)
    Lemma force_reply_some th h v : thinks_to th h -> force_reply h = Some v -> forces_to th v.
    Proof.
      intros Think Reply; apply force_some; rewrite force_unfold.
      rewrite (think_unique _ _ Think); exact Reply.
    Qed.
    (** Transport for Think and Force at one fuel level gives Force transport at the next
        level. *)
    Theorem force_step n : think_transport n -> force_transport n -> force_transport (S n).
    Proof.
      (* A successful force has a first Think reply and a finite continuation to Data. A
          matched pair of replies uses the continuation relation. If the first Think
          instead directly unfolds to the other Thunk, both forces share the same
          remaining computation, so that continuation can be reused. *)
      intros ThinkIH ForceIH t1 t2 R; split.
      - intros v1 Fuel.
        change (obind (think_with_fuel n t1) (force_after n) = Some v1) in Fuel.
        apply obind_some in Fuel; destruct Fuel as [r1 [Think1 Reply1]].
        destruct (proj1 (ThinkIH t1 t2 R) r1 Think1) as
          [[r2 [Think2 Strength]]|[OneThunk|[OneStrict|OneShallow]]].
        + destruct (force_reply_forward n ForceIH r1 r2 v1 Strength Reply1) as [v2 [Reply2 Result]].
          (* In the reverse argument, first fix the right force's unique result. A direct
            unfolding on the left can then prepend itself to this completed right
            computation. *)
        assert (SecondForced : forces_to t2 v2).
          { eapply force_reply_some; [exact Think2|exact Reply2]. }
          exists v2; split; assumption.
        (* Here the left Think step has reached the right source itself. There is no
            reason to demand another matching Think step; the already successful
            continuation is a witness for the right force. *)
        + subst r1; exists v1; split; [exists n; exact Reply1|apply strengthen_self].
        + subst r1; exists v1; split; [exists n; exact Reply1|apply strengthen_self].
        + subst r1; exists v1; split; [exists n; exact Reply1|apply strengthen_self].
      - intros v2 Fuel.
        assert (SecondForced : force t2 = Some v2).
        { apply force_unique; exists (S n); exact Fuel. }
        change (obind (think_with_fuel n t2) (force_after n) = Some v2) in Fuel.
        apply obind_some in Fuel; destruct Fuel as [r2 [Think2 Reply2]].
        destruct (proj2 (ThinkIH t1 t2 R) r2 Think2) as
          [[r1 [Think1 Strength]]|[OneThunk|[OneStrict|OneShallow]]].
        + destruct (force_reply_backward n ForceIH r1 r2 v2 Strength Reply2) as [v1 [Reply1 Result]].
          assert (FirstForced : forces_to t1 v1).
          { eapply force_reply_some; [exact Think1|exact Reply1]. }
          exists v1; split; assumption.
        + exists v2; split; [|apply strengthen_self].
          eapply force_reply_some; [exact OneThunk|exact SecondForced].
        + exists v2; split; [|apply strengthen_self].
          eapply force_reply_some; [exact OneStrict|exact SecondForced].
        + exists v2; split; [|apply strengthen_self].
          eapply force_reply_some; [exact OneShallow|exact SecondForced].
    Qed.
    Definition execute_transport n e1 e2 :=
      (forall v1, execute_with_fuel n e1 = Some v1 -> exists v2, executes_to e2 v2 /\ X v1 v2) /\
      (forall v2, execute_with_fuel n e2 = Some v2 -> exists v1, executes_to e1 v1 /\ X v1 v2).
    (** Finite-fuel strict execution lifts its Thunk's forcing result. *)
    Lemma execute_fuel_strict n th : execute_with_fuel n (Strict th) = omap lift (force_with_fuel n th).
    Proof. destruct n; reflexivity. Qed.
    (** Finite-fuel shallow execution lowers its Thunk's forcing result. *)
    Lemma execute_fuel_shallow n th : execute_with_fuel n (Shallow th) = omap lower (force_with_fuel n th).
    Proof. destruct n; reflexivity. Qed.
    (** Strengthened-related Data has both related lifted Objects and related lowered Refs. *)
    Lemma lifted_force_results h1 h2 :
      (exists d1, h1 = Data d1) -> (exists d2, h2 = Data d2) ->
      strengthen X h1 h2 -> X (lift h1) (lift h2) /\ X (lower h1) (lower h2).
    Proof.
      intros [d1 ->] [d2 ->] R; split; [exact R|].
      apply (lift_to_lower X (blob_ref_cong X Props) (tree_ref_cong X Props)
        (preserve_tree X Props) (preserve_blob X Props)); exact R.
    Qed.
    (** Related unfuelled execution results supply both directions of finite execution
        transport. *)
    Lemma execute_related_options n e1 e2 :
      rel_opt X (execute e1) (execute e2) -> execute_transport n e1 e2.
    Proof.
      intro Related; split; intros v Fuel.
      - pose proof (execute_unique _ _ (ex_intro _ n Fuel)) as Exec.
        rewrite Exec in Related; destruct (execute e2) as [v2|] eqn:Exec2; [|contradiction].
        exists v2; split; [apply execute_some; exact Exec2|exact Related].
      - pose proof (execute_unique _ _ (ex_intro _ n Fuel)) as Exec.
        rewrite Exec in Related; destruct (execute e1) as [v1|] eqn:Exec1; [|contradiction].
        exists v1; split; [apply execute_some; exact Exec1|exact Related].
    Qed.
    (** Force transport yields Encode execution transport whenever the Encode's result
        mapping preserves the relation. *)
    Lemma execute_thunks_transport n (IH : force_transport n) wrap project
        (Fuel : forall k th, execute_with_fuel k (wrap th) = omap project (force_with_fuel k th))
        (Results : forall h1 h2, (exists d1, h1 = Data d1) -> (exists d2, h2 = Data d2) ->
          strengthen X h1 h2 -> X (project h1) (project h2)) t1 t2 :
      X (Thunk t1) (Thunk t2) -> execute_transport n (wrap t1) (wrap t2).
    Proof.
      (* Force supplies the common computational result; strictness only determines the
          observation made of that Data. Once the result mapping preserves the relation,
          the matching finite force can be reused to justify the whole Encode execution. *)
      intro Related; split; intros v Executed; rewrite Fuel in Executed;
        apply omap_some in Executed; destruct Executed as [h [Forced ->]].
      - destruct (proj1 (IH t1 t2 Related) h Forced) as [h2 [[k Forced2] Strength]].
        exists (project h2); split.
        + exists k; rewrite Fuel, Forced2; reflexivity.
        + apply Results; [eapply force_with_fuel_to_data; exact Forced|
            eapply force_with_fuel_to_data; exact Forced2|exact Strength].
      - destruct (proj2 (IH t1 t2 Related) h Forced) as [h1 [[k Forced1] Strength]].
        exists (project h1); split.
        + exists k; rewrite Fuel, Forced1; reflexivity.
        + apply Results; [eapply force_with_fuel_to_data; exact Forced1|
            eapply force_with_fuel_to_data; exact Forced|exact Strength].
    Qed.
    (** Related strict Encodes transport successful execution in both directions. *)
    Lemma execute_strict_transport n (IH : force_transport n) t1 t2 :
      X (Encode (Strict t1)) (Encode (Strict t2)) -> execute_transport n (Strict t1) (Strict t2).
    Proof.
      (* A relation between strict Encodes may come from their source Thunks or from
          their completed executions. The first justification needs Force transport; the
          second already gives the result relation. Requiring only one of these reasons
          would lose valid derivations. *)
      intro Related; destruct (strict_encode_reasons X Props _ _ Related) as [Thunks|Exec].
      - eapply (execute_thunks_transport n IH Strict lift execute_fuel_strict);
          [intros h1 h2 D1 D2 Strength; exact (proj1 (lifted_force_results h1 h2 D1 D2 Strength))|exact Thunks].
      - apply execute_related_options; exact Exec.
    Qed.
    (** Related shallow Encodes transport successful execution in both directions. *)
    Lemma execute_shallow_transport n (IH : force_transport n) t1 t2 :
      X (Encode (Shallow t1)) (Encode (Shallow t2)) -> execute_transport n (Shallow t1) (Shallow t2).
    Proof.
      (* The same two justifications occur for shallow Encodes. Completed forces are
          compared through Objects, but Ref congruence transfers those facts to the Refs
          that a shallow consumer actually receives. *)
      intro Related; destruct (shallow_encode_reasons X Props _ _ Related) as [Thunks|Exec].
      - eapply (execute_thunks_transport n IH Shallow lower execute_fuel_shallow);
          [intros h1 h2 D1 D2 Strength; exact (proj2 (lifted_force_results h1 h2 D1 D2 Strength))|exact Thunks].
      - apply execute_related_options; exact Exec.
    Qed.
    (** Related Encodes transport successful execution in both directions, with matching
        strictness. *)
    Lemma execute_encodes_transport n (IH : force_transport n) e1 e2 :
      X (Encode e1) (Encode e2) -> execute_transport n e1 e2.
    Proof.
      destruct e1 as [t1|t1], e2 as [t2|t2]; intro Related.
      - apply execute_strict_transport; assumption.
      - exfalso; exact (not_strict_shallow X Props t1 t2 Related).
      - exfalso; exact (not_shallow_strict X Props t1 t2 Related).
      - apply execute_shallow_transport; assumption.
    Qed.


    Section BoundedThink.
      Variable n : nat.
      (** Evaluation transports related inputs in both directions at the current fuel
          level. *)
      Hypothesis IH_eval : eval_transport n.
      (** Forcing transports related Thunks in both directions at the current fuel level. *)
      Hypothesis IH_force : force_transport n.

      (** A finite evaluation of the left Tree can be matched by evaluation of the related
          right Tree. *)
      Lemma tree_forward t1 t2 v1 : X (HTreeObj t1) (HTreeObj t2) ->
        eval_tree_with_fuel n t1 = Some v1 ->
        exists v2, eval_tree t2 = Some v2 /\ X (HTreeObj v1) (HTreeObj v2).
      Proof.
        intros R Ev; destruct (proj1 (eq_tree_eval_fuel_n X n (tree_cong X Props)
          (tag_cong X Props) (tree_complete X Props) IH_eval t1 t2 R) v1 Ev) as [v2 [Ev2 Related]].
        exists v2; split; [apply eval_tree_unique; exact Ev2|exact Related].
      Qed.
      (** A finite evaluation of the right Tree can be matched by evaluation of the related
          left Tree. *)
      Lemma tree_backward t1 t2 v2 : X (HTreeObj t1) (HTreeObj t2) ->
        eval_tree_with_fuel n t2 = Some v2 ->
        exists v1, eval_tree t1 = Some v1 /\ X (HTreeObj v1) (HTreeObj v2).
      Proof.
        intros R Ev; destruct (proj2 (eq_tree_eval_fuel_n X n (tree_cong X Props)
          (tag_cong X Props) (tree_complete X Props) IH_eval t1 t2 R) v2 Ev) as [v1 [Ev1 Related]].
        exists v1; split; [apply eval_tree_unique; exact Ev1|exact Related].
      Qed.
      (** A finite application Think step on the left has a strengthened-related matching
          step on the right. *)
      Lemma application_think_forward t1 t2 r1 : X (HTreeObj t1) (HTreeObj t2) ->
        think_with_fuel (S n) (Application t1) = Some r1 ->
        exists r2, thinks_to (Application t2) r2 /\ strengthen X r1 r2.
      Proof.
        intros R Ev; change (obind (eval_tree_with_fuel n t1) A.apply_tree = Some r1) in Ev.
        apply obind_some in Ev; destruct Ev as [v1 [Fuel1 App1]].
        destruct (tree_forward t1 t2 v1 R Fuel1) as [v2 [Tree2 Related]].
        assert (eval_tree t1 = Some v1) as Tree1 by (apply eval_tree_unique; exists n; exact Fuel1).
        pose proof (apply_tree_strengthen v1 v2 Related
          (eval_tree_to_value_handle _ _ Tree1) (eval_tree_to_value_handle _ _ Tree2)) as App.
        rewrite App1 in App; destruct (A.apply_tree v2) as [r2|] eqn:App2; [|contradiction].
        exists r2; split; [|exact App].
        apply think_some; rewrite think_application, Tree2; exact App2.
      Qed.
      (** A finite selection Think step on the left has a strengthened-related matching
          step on the right. *)
      Lemma selection_think_forward t1 t2 r1 : X (HTreeObj t1) (HTreeObj t2) ->
        think_with_fuel (S n) (Selection t1) = Some r1 ->
        exists r2, thinks_to (Selection t2) r2 /\ strengthen X r1 r2.
      Proof.
        intros R Ev; change (obind (eval_tree_with_fuel n t1) Sl.select = Some r1) in Ev.
        apply obind_some in Ev; destruct Ev as [v1 [Fuel1 Selected1]].
        destruct (tree_forward t1 t2 v1 R Fuel1) as [v2 [Tree2 Related]].
        assert (eval_tree t1 = Some v1) as Tree1 by (apply eval_tree_unique; exists n; exact Fuel1).
        pose proof (slice_values v1 v2 Related
          (eval_tree_to_not_encode _ _ Tree1) (eval_tree_to_not_encode _ _ Tree2)) as Selected.
        rewrite Selected1 in Selected; destruct (Sl.select v2) as [r2|] eqn:Selected2; [|contradiction].
        exists r2; split; [|apply related_strengthen; exact Selected].
        apply think_some; rewrite think_selection, Tree2; exact Selected2.
      Qed.
      (** A finite identification Think step on the left has a strengthened-related
          matching step on the right. *)
      Lemma identification_think_forward d1 d2 r1 : X (Data (Ref d1)) (Data (Ref d2)) ->
        think_with_fuel (S n) (Identification d1) = Some r1 ->
        exists r2, thinks_to (Identification d2) r2 /\ strengthen X r1 r2.
      Proof.
        intros R Ev; change (Some (Data (Ref d1)) = Some r1) in Ev; inversion Ev; subst.
        exists (Data (Ref d2)); split; [exists 1; reflexivity|apply related_data_strengthen; exact R].
      Qed.
      (** A finite application Think step on the right has a strengthened-related matching
          step on the left. *)
      Lemma application_think_backward t1 t2 r2 : X (HTreeObj t1) (HTreeObj t2) ->
        think_with_fuel (S n) (Application t2) = Some r2 ->
        exists r1, thinks_to (Application t1) r1 /\ strengthen X r1 r2.
      Proof.
        intros R Ev; change (obind (eval_tree_with_fuel n t2) A.apply_tree = Some r2) in Ev.
        apply obind_some in Ev; destruct Ev as [v2 [Fuel2 App2]].
        destruct (tree_backward t1 t2 v2 R Fuel2) as [v1 [Tree1 Related]].
        assert (eval_tree t2 = Some v2) as Tree2 by (apply eval_tree_unique; exists n; exact Fuel2).
        pose proof (apply_tree_strengthen v1 v2 Related
          (eval_tree_to_value_handle _ _ Tree1) (eval_tree_to_value_handle _ _ Tree2)) as App.
        rewrite App2 in App; destruct (A.apply_tree v1) as [r1|] eqn:App1; [|contradiction].
        exists r1; split; [|exact App].
        apply think_some; rewrite think_application, Tree1; exact App1.
      Qed.
      (** A finite selection Think step on the right has a strengthened-related matching
          step on the left. *)
      Lemma selection_think_backward t1 t2 r2 : X (HTreeObj t1) (HTreeObj t2) ->
        think_with_fuel (S n) (Selection t2) = Some r2 ->
        exists r1, thinks_to (Selection t1) r1 /\ strengthen X r1 r2.
      Proof.
        intros R Ev; change (obind (eval_tree_with_fuel n t2) Sl.select = Some r2) in Ev.
        apply obind_some in Ev; destruct Ev as [v2 [Fuel2 Selected2]].
        destruct (tree_backward t1 t2 v2 R Fuel2) as [v1 [Tree1 Related]].
        assert (eval_tree t2 = Some v2) as Tree2 by (apply eval_tree_unique; exists n; exact Fuel2).
        pose proof (slice_values v1 v2 Related
          (eval_tree_to_not_encode _ _ Tree1) (eval_tree_to_not_encode _ _ Tree2)) as Selected.
        rewrite Selected2 in Selected; destruct (Sl.select v1) as [r1|] eqn:Selected1; [|contradiction].
        exists r1; split; [|apply related_strengthen; exact Selected].
        apply think_some; rewrite think_selection, Tree1; exact Selected1.
      Qed.
      (** A finite identification Think step on the right has a strengthened-related
          matching step on the left. *)
      Lemma identification_think_backward d1 d2 r2 : X (Data (Ref d1)) (Data (Ref d2)) ->
        think_with_fuel (S n) (Identification d2) = Some r2 ->
        exists r1, thinks_to (Identification d1) r1 /\ strengthen X r1 r2.
      Proof.
        intros R Ev; change (Some (Data (Ref d2)) = Some r2) in Ev; inversion Ev; subst.
        exists (Data (Ref d1)); split; [exists 1; reflexivity|apply related_data_strengthen; exact R].
      Qed.
      (** A left Think step has a related right step, or directly returns the right Thunk
          or one of its Encodes. *)
      Theorem think_forward_from_eval t1 t2 r1 : X (Thunk t1) (Thunk t2) ->
        think_with_fuel (S n) t1 = Some r1 ->
        (exists r2, thinks_to t2 r2 /\ strengthen X r1 r2) \/
        r1 = Thunk t2 \/ r1 = Encode (Strict t2) \/ r1 = Encode (Shallow t2).
      Proof.
        (* An application or selection can compare its next replies once the operand
            Trees have been evaluated. Direct-unfolding constructors are different: the
            left reply may already be the right source Thunk, so the conclusion retains
            that possibility instead of demanding an extra right-hand computation. *)
        intros Related Fuel.
        assert (Computed : think t1 = Some r1)
          by (apply think_unique; exists (S n); exact Fuel).
        assert (Reasons : thunk_reasons X t1 t2)
          by (exact (related_thunk_reasons X Props _ _ Related)).
        hauto cases: - use: application_think_forward, selection_think_forward,
          identification_think_forward, think_some unfold!: thunk_reasons.
      Qed.
      (** A right Think step has a related left step, or the left Thunk directly reaches
          the right Thunk or one of its Encodes. *)
      Theorem think_backward_from_eval t1 t2 r2 : X (Thunk t1) (Thunk t2) ->
        think_with_fuel (S n) t2 = Some r2 ->
        (exists r1, thinks_to t1 r1 /\ strengthen X r1 r2) \/
        thinks_to t1 (Thunk t2) \/ thinks_to t1 (Encode (Strict t2)) \/ thinks_to t1 (Encode (Shallow t2)).
      Proof.
        (* A successful right Think step can normally be matched on the left. If the
            relation instead says that the left Thunk unfolds directly to the right one,
            preserve that left step as evidence; its subsequent continuation can consume
            the already known right result. *)
        intros Related Fuel.
        assert (Computed : think t2 = Some r2)
          by (apply think_unique; exists (S n); exact Fuel).
        assert (Reasons : thunk_reasons X t1 t2)
          by (exact (related_thunk_reasons X Props _ _ Related)).
        hauto cases: - use: application_think_backward, selection_think_backward,
          identification_think_backward, think_some unfold!: thunk_reasons.
      Qed.
    End BoundedThink.
    (** Evaluation and Force transport at one fuel level give Think transport at the next
        level. *)
    Theorem think_step n : eval_transport n -> force_transport n -> think_transport (S n).
    Proof.
      intros Eval Force t1 t2 Related; split.
      - intros r Fuel; exact (think_forward_from_eval n Eval t1 t2 r Related Fuel).
      - intros r Fuel; exact (think_backward_from_eval n Eval t1 t2 r Related Fuel).
    Qed.
    (** Executing an Encode and evaluating its result gives evaluation of the Encode. *)
    Lemma eval_after_execute e h v : executes_to e h -> evals_to h v -> evals_to (Encode e) v.
    Proof.
      intros Exec Ev; apply eval_some; rewrite eval_encode, (execute_unique _ _ Exec).
      exact (eval_unique _ _ Ev).
    Qed.
    (** Force and evaluation transport extend evaluation transport to pairs of Encodes. *)
    Lemma eval_encodes_transport n (IH_force : force_transport n) (IH_eval : eval_transport n) e1 e2 :
      X (Encode e1) (Encode e2) ->
      (forall v1, eval_with_fuel (S n) (Encode e1) = Some v1 -> exists v2, evals_to (Encode e2) v2 /\ X v1 v2) /\
      (forall v2, eval_with_fuel (S n) (Encode e2) = Some v2 -> exists v1, evals_to (Encode e1) v1 /\ X v1 v2).
    Proof.
      intro Related; pose proof (execute_encodes_transport n IH_force e1 e2 Related) as Execs.
      split; intros v Fuel.
      - change (obind (execute_with_fuel n e1) (eval_with_fuel n) = Some v) in Fuel.
        apply obind_some in Fuel; destruct Fuel as [h1 [Exec1 Ev1]].
        destruct (proj1 Execs h1 Exec1) as [h2 [Exec2 R]].
        destruct (proj1 (IH_eval h1 h2 R) v Ev1) as [v2 [Ev2 Result]].
        exists v2; split; [eapply eval_after_execute; eauto|exact Result].
      - change (obind (execute_with_fuel n e2) (eval_with_fuel n) = Some v) in Fuel.
        apply obind_some in Fuel; destruct Fuel as [h2 [Exec2 Ev2]].
        destruct (proj2 Execs h2 Exec2) as [h1 [Exec1 R]].
        destruct (proj2 (IH_eval h1 h2 R) v Ev2) as [v1 [Ev1 Result]].
        exists v1; split; [eapply eval_after_execute; eauto|exact Result].
    Qed.
    (** Related handles fixed by evaluation transport their evaluations in both directions. *)
    Lemma eval_fixed_transport k h1 h2 : X h1 h2 ->
      (forall n, eval_with_fuel n h1 = Some h1) ->
      (forall n, eval_with_fuel n h2 = Some h2) ->
      (forall v1, eval_with_fuel k h1 = Some v1 -> exists v2, evals_to h2 v2 /\ X v1 v2) /\
      (forall v2, eval_with_fuel k h2 = Some v2 -> exists v1, evals_to h1 v1 /\ X v1 v2).
    Proof.
      intros R F1 F2; split; intros v Fuel.
      - rewrite F1 in Fuel; inversion Fuel; subst; exists h2; split; [exists 0; apply F2|exact R].
      - rewrite F2 in Fuel; inversion Fuel; subst; exists h1; split; [exists 0; apply F1|exact R].
    Qed.
    (** Evaluation transport extends one fuel step for related inputs whose left handle is
        not an Encode. *)
    Lemma eval_nonencode_step n (IH : eval_transport n) h1 h2 :
      X h1 h2 -> ~(exists e, h1 = Encode e) ->
      (forall v1, eval_with_fuel (S n) h1 = Some v1 -> exists v2, evals_to h2 v2 /\ X v1 v2) /\
      (forall v2, eval_with_fuel (S n) h2 = Some v2 -> exists v1, evals_to h1 v1 /\ X v1 v2).
    Proof.
      intros R N1; destruct h1 as [[[b1|t1]|[b1|t1]]|th1|e1].
      - destruct (preserve_blob X Props b1 h2 R) as [b2 ->].
        apply eval_fixed_transport; [exact R|intros [|k]; reflexivity|intros [|k]; reflexivity].
      - destruct (preserve_tree X Props t1 h2 R) as [t2 ->].
        split; intros v Fuel;
          first [change (omap HTreeObj (eval_tree_with_fuel n t1) = Some v) in Fuel |
                 change (omap HTreeObj (eval_tree_with_fuel n t2) = Some v) in Fuel];
          apply omap_some in Fuel; destruct Fuel as [t [Tree ->]].
        + destruct (tree_forward n IH t1 t2 t R Tree) as [t' [Ev Result]].
          exists (HTreeObj t'); split; [apply eval_some; change (eval (HTreeObj t2) = Some (HTreeObj t')); rewrite eval_tree_handle, Ev; reflexivity|exact Result].
        + destruct (tree_backward n IH t1 t2 t R Tree) as [t' [Ev Result]].
          exists (HTreeObj t'); split; [apply eval_some; change (eval (HTreeObj t1) = Some (HTreeObj t')); rewrite eval_tree_handle, Ev; reflexivity|exact Result].
      - destruct (preserve_blob_ref X Props b1 h2 R) as [b2 ->].
        apply eval_fixed_transport; [exact R|intros [|k]; reflexivity|intros [|k]; reflexivity].
      - destruct (preserve_tree_ref X Props t1 h2 R) as [t2 ->].
        apply eval_fixed_transport; [exact R|intros [|k]; reflexivity|intros [|k]; reflexivity].
      - destruct (proj1 (preserve_thunk X Props _ _ R) (ex_intro _ th1 eq_refl)) as [th2 ->].
        apply eval_fixed_transport; [exact R|intros [|k]; reflexivity|intros [|k]; reflexivity].
      - exfalso; apply N1; eauto.
    Qed.
    (** A related Encode and non-Encode transport evaluations by the Encode's already-known
        execution result. *)
    Lemma eval_encode_nonencode n e h :
      X (Encode e) h -> ~(exists e', h = Encode e') ->
      (forall v1, eval_with_fuel (S n) (Encode e) = Some v1 -> exists v2, evals_to h v2 /\ X v1 v2) /\
      (forall v2, eval_with_fuel (S n) h = Some v2 -> exists v1, evals_to (Encode e) v1 /\ X v1 v2).
    Proof.
      intros R Nh; pose proof (encode_eval X Props e h R Nh) as Exec.
      split; intros v Fuel; exists v; split; try apply (X_self X Props).
      - change (obind (execute_with_fuel n e) (eval_with_fuel n) = Some v) in Fuel.
        apply obind_some in Fuel; destruct Fuel as [h' [Exec' Ev]].
        pose proof (execute_deterministic e h' h (ex_intro _ n Exec') Exec) as ->.
        exists n; exact Ev.
      - eapply eval_after_execute; [exact Exec|exists (S n); exact Fuel].
    Qed.
    (** Force and evaluation transport extend evaluation transport to the next fuel level. *)
    Theorem eval_step n : force_transport n -> eval_transport n -> eval_transport (S n).
    Proof.
      (* Leaf Values need no computation, and Trees use the argument for their children.
          Encodes require execution followed by evaluation of the produced Data. Because
          Think, Force, and evaluation call one another, these cases must advance their
          shared fuel induction together. *)
      intros IH_force IH_eval h1 h2 R; destruct h1 as [d|th|e].
      - apply eval_nonencode_step; [exact IH_eval|exact R|intros [e E]; discriminate].
      - apply eval_nonencode_step; [exact IH_eval|exact R|intros [e E]; discriminate].
      - destruct h2 as [d|th|e2].
        + apply eval_encode_nonencode; [exact R|intros [e' E]; discriminate].
        + apply eval_encode_nonencode; [exact R|intros [e' E]; discriminate].
        + apply eval_encodes_transport; assumption.
    Qed.
    (** Think transport holds at zero fuel, where no Think step can succeed. *)
    Lemma think_zero : think_transport 0.
    Proof. intros t1 t2 R; split; intros v Fuel; discriminate. Qed.
    (** Force transport holds at zero fuel, where no force can succeed. *)
    Lemma force_zero : force_transport 0.
    Proof. intros t1 t2 R; split; intros v Fuel; discriminate. Qed.
    (** Evaluation transport holds at zero fuel for the forms evaluation can already return
        unchanged. *)
    Lemma eval_zero : eval_transport 0.
    Proof.
      intros h1 h2 R; destruct h1 as [[[b1|t1]|[b1|t1]]|th1|e1].
      - destruct (preserve_blob X Props b1 h2 R) as [b2 ->].
        apply eval_fixed_transport; [exact R|intros [|k]; reflexivity|intros [|k]; reflexivity].
      - destruct (preserve_tree X Props t1 h2 R) as [t2 ->].
        split; intros v Fuel; discriminate.
      - destruct (preserve_blob_ref X Props b1 h2 R) as [b2 ->].
        apply eval_fixed_transport; [exact R|intros [|k]; reflexivity|intros [|k]; reflexivity].
      - destruct (preserve_tree_ref X Props t1 h2 R) as [t2 ->].
        apply eval_fixed_transport; [exact R|intros [|k]; reflexivity|intros [|k]; reflexivity].
      - destruct (proj1 (preserve_thunk X Props _ _ R) (ex_intro _ th1 eq_refl)) as [th2 ->].
        apply eval_fixed_transport; [exact R|intros [|k]; reflexivity|intros [|k]; reflexivity].
      - split; intros v Fuel; [discriminate|].
        destruct h2 as [d2|th2|e2]; [| |discriminate].
        + exists v; split; [|apply (X_self X Props)].
          eapply eval_after_execute; [eapply (encode_eval X Props); [exact R|intros [e E]; discriminate]|exists 0; exact Fuel].
        + exists v; split; [|apply (X_self X Props)].
          eapply eval_after_execute; [eapply (encode_eval X Props); [exact R|intros [e E]; discriminate]|exists 0; exact Fuel].
    Qed.
    (** The three mutually dependent preservation statements hold at every finite fuel
        bound. Their joint proof breaks the apparent cycle between evaluating operands,
        thinking a Thunk, and forcing its returned computation. *)
    Theorem eq_forces_to_induct n : think_transport n /\ force_transport n /\ eval_transport n.
    Proof.
      (* At a successor bound, each evaluator depends only on calls from the preceding
          bound. Prove all three successor statements from that common stock of
          hypotheses; no evaluator-preservation theorem is assumed in order to prove
          itself. *)
      induction n as [|n [Think [Force Eval]]].
      - split; [apply think_zero|split; [apply force_zero|apply eval_zero]].
      - assert (NextThink : think_transport (S n)) by (apply think_step; assumption).
        assert (NextForce : force_transport (S n)) by (apply force_step; assumption).
        assert (NextEval : eval_transport (S n)) by (apply eval_step; assumption).
        auto.
    Qed.
    (** Two-way transport of successful payloads gives the optional relation, including
        agreement on failure. *)
    Lemma option_transport {B} (R : B -> B -> Prop) (x y : option B) :
      (forall v, x = Some v -> exists w, y = Some w /\ R v w) ->
      (forall w, y = Some w -> exists v, x = Some v /\ R v w) -> rel_opt R x y.
    Proof.
      intros Forward Backward; destruct x as [v|], y as [w|]; cbn [rel_opt].
      - destruct (Forward v eq_refl) as [w' [E Related]]; inversion E; subst; exact Related.
      - destruct (Forward v eq_refl) as [w' [E Related]]; discriminate.
      - destruct (Backward w eq_refl) as [v' [E Related]]; discriminate.
      - exact I.
    Qed.
