  Theorem eval_unfold_proof h : eval h =
    match h with
    | Thunk _ | Data (Ref _) | Data (Object (BlobObj _)) => Some h
    | Data (Object (TreeObj t)) => omap HTreeObj (eval_tree t)
    | Encode e => obind (execute e) eval
    end.
  Proof.
    destruct h as [d|th|e]; [destruct d as [[b|t]|r]| |].
    - exact (eval_blob b).
    - exact (eval_tree_handle t).
    - exact (eval_ref r).
    - exact (eval_thunk th).
    - exact (eval_encode e).
  Qed.
