  (** Thinking an identification Thunk performs the identification operation on its Ref. *)
  Lemma think_identification d : think (Identification d) = I.identify d.
  Proof. apply think_unique; exists 1; reflexivity. Qed.
  (** Thinking an application evaluates its combination Tree, then performs one application
      step. *)
  Lemma think_application t : think (Application t) = obind (eval_tree t) A.apply_tree.
  Proof.
    apply option_ext_some; intro r; rewrite obind_some; split.
    - intro T; apply think_some in T; destruct T as [n T].
      destruct n as [|n]; [discriminate|].
      change (obind (eval_tree_with_fuel n t) A.apply_tree = Some r) in T.
      apply obind_some in T; destruct T as [t' [Ev App]].
      exists t'; split; [apply eval_tree_unique; exists n; exact Ev|exact App].
    - intros [t' [Ev App]]; apply eval_tree_some in Ev; destruct Ev as [n Ev].
      apply think_unique; exists (S n).
      change (obind (eval_tree_with_fuel n t) A.apply_tree = Some r); rewrite Ev; exact App.
  Qed.
  (** Thinking a selection evaluates its operand Tree, then performs selection. *)
  Lemma think_selection t : think (Selection t) = obind (eval_tree t) Sl.select.
  Proof.
    apply option_ext_some; intro r; rewrite obind_some; split.
    - intro T; apply think_some in T; destruct T as [n T].
      destruct n as [|n]; [discriminate|].
      change (obind (eval_tree_with_fuel n t) Sl.select = Some r) in T.
      apply obind_some in T; destruct T as [t' [Ev App]].
      exists t'; split; [apply eval_tree_unique; exists n; exact Ev|exact App].
    - intros [t' [Ev App]]; apply eval_tree_some in Ev; destruct Ev as [n Ev].
      apply think_unique; exists (S n).
      change (obind (eval_tree_with_fuel n t) Sl.select = Some r); rewrite Ev; exact App.
  Qed.
