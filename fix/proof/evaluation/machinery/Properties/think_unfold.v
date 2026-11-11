  Theorem think_unfold_proof th : think th = match th with
    | Application t => obind (eval_tree t) A.apply_tree
    | Identification d => I.identify d
    | Selection t => obind (eval_tree t) Sl.select
    end.
  Proof. destruct th; [apply think_application|apply think_identification|apply think_selection]. Qed.
