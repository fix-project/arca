  Theorem equiv_eval_proof h1 h2 : computational_equiv h1 h2 -> rel_opt computational_equiv (eval h1) (eval h2).
  Proof.
    (* The difficult computational argument has already shown this for R. An arbitrary
        equivalence merely joins and reverses R steps, and evaluation is a partial
        function, so those local result justifications join into a justification between
        the final results. *)
    apply (rel_opt_closure R R eval); eauto using rel_opt_R_to_equiv, eval_R.
  Qed.
