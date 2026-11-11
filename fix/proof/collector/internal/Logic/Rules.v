  (** An Eval result is computationally equivalent to its source. *)
  Lemma eval_result_equiv h r : eval_result h r -> computational_equiv h r.
  Proof.
    intros [actual [Evaluated [Related Value]]].
    eapply EC.equiv_trans; [apply EC.eval_to_equiv; exact Evaluated|apply EC.equiv_sym; exact Related].
  Qed.
  (** An actual successful force satisfies the abstract Force-result claim. *)
  Lemma force_result_direct th r : force th = Some r -> force_result (Thunk th) r.
  Proof.
    apply EC.force_result_direct.
  Qed.
  (** A Force-result claim remains true when its input is replaced by a computationally
      equivalent handle. *)
  Lemma force_result_equiv h1 h2 r : computational_equiv h1 h2 -> force_result h1 r -> force_result h2 r.
  Proof.
    apply EC.force_result_equiv.
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
    intros [actual [Evaluated [Related Value]]] Equivalent.
    pose proof (EC.equiv_eval _ _ Equivalent) as EvaluationsRelated.
    rewrite Evaluated in EvaluationsRelated.
    destruct (eval equivalent) as [other|] eqn:EquivalentEvaluated;
      cbn [rel_opt] in EvaluationsRelated; [|contradiction].
    exists other; split; [exact EquivalentEvaluated|split; [|exact Value]].
    eapply equiv_trans; [exact Related|apply EC.indistinguishable_equiv; exact EvaluationsRelated].
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
    - apply EC.equiv_selection; exact Related.
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
    eapply equiv_trans; [exact Related|apply EC.equivalent_think_thunk; exact Thought].
  Qed.
  (** A Think-result claim returning a strict Encode establishes equivalence to its
      underlying Thunk. *)
  Lemma think_result_single_strict th1 th2 : think_result (Thunk th1) (Encode (Strict th2)) -> computational_equiv (Thunk th1) (Thunk th2).
  Proof.
    intros [actual [Thought Related]].
    eapply equiv_trans; [exact Related|apply EC.equivalent_think_strict; exact Thought].
  Qed.
  (** A Think-result claim returning a shallow Encode establishes equivalence to its
      underlying Thunk. *)
  Lemma think_result_single_shallow th1 th2 : think_result (Thunk th1) (Encode (Shallow th2)) -> computational_equiv (Thunk th1) (Thunk th2).
  Proof.
    intros [actual [Thought Related]].
    eapply equiv_trans; [exact Related|apply EC.equivalent_think_shallow; exact Thought].
  Qed.
  (** A Force-result claim makes the strict Encode equivalent to the result's lifted
      Object. *)
  Lemma force_result_strict th h : force_result (Thunk th) h -> computational_equiv (Encode (Strict th)) (lift h).
  Proof.
    apply EC.force_result_strict.
  Qed.
  (** Lifting Data before lowering it leaves the final Ref unchanged. *)
  Lemma lower_lift_data d : lower (lift (Data d)) = lower (Data d).
  Proof. destruct d as [[b|t]|[b|t]]; reflexivity. Qed.
  (** A Force-result claim makes the shallow Encode equivalent to the lowered result. *)
  Lemma force_result_shallow th h : force_result (Thunk th) h -> computational_equiv (Encode (Shallow th)) (lower h).
  Proof.
    apply EC.force_result_shallow.
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
    apply EC.equiv_identification.
  Qed.
  (** Matching markers and pointwise equivalent children give equivalent Tree Objects. *)
  Lemma equiv_tree_entries t1 t2 : tree_tag t1 = tree_tag t2 -> Forall2 computational_equiv (get_tree_raw t1) (get_tree_raw t2) -> computational_equiv (HTreeObj t1) (HTreeObj t2).
  Proof.
    apply EC.equiv_tree_entries.
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
