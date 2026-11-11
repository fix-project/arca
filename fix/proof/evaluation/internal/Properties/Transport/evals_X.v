    (** Evaluation preserves the handle relation and agrees on whether it succeeds. *)
    Theorem evals_X h1 h2 : X h1 h2 -> rel_opt X (eval h1) (eval h2).
    Proof.
      (* Every actual successful run has some finite witness. The joint induction
          supplies a matching finite run on the other side at that witness bound. Applying
          this in both directions rules out a successful result on just one side. *)
      intro Related.
      assert (FiniteEvaluations : forall n, eval_transport n)
        by (hauto cases: - use: eq_forces_to_induct).
      apply option_transport;
        hauto cases: - use: eval_some, eval_unique unfold!: eval_transport.
    Qed.
    (** Forcing related Thunks preserves the strengthened relation and agrees on success. *)
    Lemma forces_strengthen t1 t2 : X (Thunk t1) (Thunk t2) ->
      rel_opt (strengthen X) (force t1) (force t2).
    Proof.
      intro Related.
      assert (FiniteForcing : forall n, force_transport n)
        by (hauto cases: - use: eq_forces_to_induct).
      apply option_transport;
        hauto cases: - use: force_some, force_unique unfold!: force_transport.
    Qed.
    (** Forcing related Thunks preserves the relaxed relation and agrees on success. *)
    Theorem forces_X t1 t2 : X (Thunk t1) (Thunk t2) ->
      rel_opt (relaxed_X X) (force t1) (force t2).
    Proof.
      intro Related; pose proof (forces_strengthen t1 t2 Related) as Result.
      destruct (force t1) as [h1|] eqn:F1, (force t2) as [h2|] eqn:F2;
        cbn [rel_opt] in *; try contradiction; [|exact I].
      destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->]; exact Result.
    Qed.
    (** Thinking related Thunks either returns strengthened-related results or directly
        unfolds the left Thunk to the right one. *)
    Theorem think_X t1 t2 : X (Thunk t1) (Thunk t2) ->
      rel_opt (strengthen X) (think t1) (think t2) \/
      think t1 = Some (Thunk t2) \/ think t1 = Some (Encode (Strict t2)) \/ think t1 = Some (Encode (Shallow t2)).
    Proof.
      intro Related; destruct (think t1) as [v1|] eqn:T1.
      - destruct (proj1 (think_some _ _) T1) as [n Fuel].
        destruct (eq_forces_to_induct n) as [Thinking _].
        destruct (Thinking t1 t2 Related) as [Forward _].
        destruct (Forward v1 Fuel) as
          [[v2 [T2 Strength]]|[OneThunk|[OneStrict|OneShallow]]].
        + left; rewrite (think_unique _ _ T2); exact Strength.
        + right; left; congruence.
        + right; right; left; congruence.
        + right; right; right; congruence.
      - destruct (think t2) as [v2|] eqn:T2; [|left; exact I].
        destruct (proj1 (think_some _ _) T2) as [n Fuel].
        destruct (eq_forces_to_induct n) as [Thinking _].
        destruct (Thinking t1 t2 Related) as [_ Backward].
        destruct (Backward v2 Fuel) as
          [[v1 [T1' Strength]]|[OneThunk|[OneStrict|OneShallow]]];
          exfalso; first [pose proof (think_unique _ _ T1')|pose proof (think_unique _ _ OneThunk)|
            pose proof (think_unique _ _ OneStrict)|pose proof (think_unique _ _ OneShallow)]; congruence.
    Qed.
