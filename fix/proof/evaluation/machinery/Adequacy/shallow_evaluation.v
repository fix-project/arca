  Theorem shallow_evaluation_proof th :
    eval (Encode (Shallow th)) = execute (Shallow th).
  Proof.
    rewrite eval_encode, execute_unfold; cbn [encode_to_thunk].
    destruct (force th) as [h|] eqn:Forced; [|reflexivity].
    destruct (force_data _ _ Forced) as [d ->].
    destruct (lower_reference d) as [reference [Lower _]].
    cbn [omap obind]; rewrite Lower; apply eval_ref.
  Qed.
