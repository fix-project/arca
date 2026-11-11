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
