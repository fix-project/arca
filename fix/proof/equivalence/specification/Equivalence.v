  Definition computational_equiv first second :=
    EC.computational_equiv first second /\
    rel_opt indistinguishable (eval first) (eval second).

  Definition indistinguishable := indistinguishable.

  Definition force_result value result := match value, result with
    | Thunk thunk, Data data => exists actual,
      force thunk = Some (Data actual) /\ indistinguishable (lower (Data data)) (lower (Data actual))
    | _, _ => False end.

  Theorem equiv_eval first second : computational_equiv first second ->
    rel_opt indistinguishable (eval first) (eval second).
  Proof. intros [_ Evaluated]; exact Evaluated. Qed.

  Theorem indistinguishable_equiv first second : indistinguishable first second -> computational_equiv first second.
  Proof. intro Related; split; [exact (proj1 Related)|apply indistinguishable_eval; exact Related]. Qed.

  Theorem equiv_values first second : value_handle first -> value_handle second ->
    (computational_equiv first second <-> indistinguishable first second).
  Proof.
    intros First Second; split; [|apply indistinguishable_equiv].
    intros [_ Evaluated]; rewrite (value_handle_eval_to_itself _ First), (value_handle_eval_to_itself _ Second) in Evaluated.
    exact Evaluated.
  Qed.

  Lemma equiv_refl value : computational_equiv value value.
  Proof.
    split; [apply EC.equiv_refl|].
    destruct (eval value); cbn [rel_opt]; [apply indistinguishable_refl|exact I].
  Qed.

  Lemma equiv_sym first second : computational_equiv first second -> computational_equiv second first.
  Proof.
    intros [Related Evaluated]; split; [apply EC.equiv_sym; exact Related|].
    destruct (eval first), (eval second); cbn [rel_opt] in *; try contradiction;
      [apply indistinguishable_sym; exact Evaluated|exact I].
  Qed.

  Lemma equiv_trans first middle last : computational_equiv first middle -> computational_equiv middle last ->
    computational_equiv first last.
  Proof.
    intros [First Left] [Second Right]; split; [eapply EC.equiv_trans; eassumption|].
    destruct (eval first), (eval middle), (eval last); cbn [rel_opt] in *; try contradiction;
      [eapply indistinguishable_trans; eassumption|exact I].
  Qed.

  Theorem eval_to_equiv value result : eval value = Some result -> computational_equiv value result.
  Proof.
    intro Evaluated; split; [apply EC.eval_to_equiv; exact Evaluated|].
    rewrite Evaluated, (value_handle_eval_to_itself _ (eval_to_value_handle _ _ Evaluated)).
    apply indistinguishable_refl.
  Qed.

  Theorem equivalent_thunks first second : computational_equiv (Thunk first) (Thunk second) ->
    indistinguishable (Thunk first) (Thunk second).
  Proof. apply (proj1 (equiv_values _ _ (thunk_handle first) (thunk_handle second))). Qed.

  Theorem equivalent_references first second : computational_equiv (Data (Ref first)) (Data (Ref second)) ->
    indistinguishable (Data (Ref first)) (Data (Ref second)).
  Proof. apply (proj1 (equiv_values _ _ (ref_handle first) (ref_handle second))). Qed.

  Theorem equivalent_comparison first second other :
    value_handle first -> value_handle second -> computational_equiv first second ->
    compare_handles first other = compare_handles second other.
  Proof.
    intros First Second Related; apply compare_handles_congruence;
      [apply (proj1 (equiv_values _ _ First Second)); exact Related|apply indistinguishable_refl].
  Qed.

  Theorem equivalent_program program first second :
    A.rel_state computational_equiv first second ->
    Forall value_handle (A.hs first) -> Forall value_handle (A.hs second) ->
    rel_opt indistinguishable (A.exec program first) (A.exec program second).
  Proof.
    intros [Related Bytes] First Second; apply indistinguishable_program; split; [|exact Bytes].
    revert First Second; induction Related; intros First Second; constructor.
    - apply (proj1 (equiv_values _ _ (Forall_inv First) (Forall_inv Second))); exact H.
    - exact (IHRelated (Forall_inv_tail First) (Forall_inv_tail Second)).
  Qed.
