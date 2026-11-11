  Theorem shallow_step_proof th intermediate result :
    think th = Some intermediate ->
    (eval (Encode (Shallow th)) = Some result <-> follow_result intermediate result).
  Proof.
    (* An application may return another computation rather than final Data. The first
        Think step accounts only for that application; follow_result accounts for the
        remaining computation. This separation prevents a certificate for one application
        from silently certifying the whole evaluation. *)
    intro Thought; rewrite shallow_evaluation, execute_unfold; cbn [encode_to_thunk].
    rewrite force_unfold, Thought; cbn [obind].
    destruct intermediate as [d|next|[next|next]]; cbn [follow_result];
      try (rewrite shallow_evaluation, execute_unfold; reflexivity).
    cbn [omap]; split; congruence.
  Qed.
