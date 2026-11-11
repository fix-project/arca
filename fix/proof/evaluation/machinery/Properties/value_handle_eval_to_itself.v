  Theorem value_handle_eval_to_itself_proof h : value_handle h -> eval h = Some h.
  Proof.
    intro V; inversion V; subst.
    - apply eval_blob.
    - apply value_tree_eval_to_itself; assumption.
    - apply eval_ref.
    - apply eval_thunk.
  Qed.
