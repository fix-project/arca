  (** Successful Tree evaluation returns a Value Tree. *)
  Lemma eval_tree_to_value_handle t1 t2 : eval_tree t1 = Some t2 -> value_handle (HTreeObj t2).
  Proof.
    intro Ev; apply (eval_to_value_handle (HTreeObj t1)).
    rewrite eval_tree_handle, Ev; reflexivity.
  Qed.
  (** A successfully evaluated Tree has no immediate Encode children. *)
  Lemma eval_tree_to_not_encode t1 t2 : eval_tree t1 = Some t2 -> Forall not_encode (get_tree_raw t2).
  Proof.
    intro Ev; apply eval_tree_to_value_handle in Ev; inversion Ev; subst.
    match goal with V : value_tree _ |- _ => inversion V; subst end.
    apply Forall_impl with (P:=value_handle); [|assumption].
    intros h V; destruct h; [exact I|exact I|].
    exfalso; apply (value_handle_not_encode _ V); eauto.
  Qed.

  Definition force_after n h :=
    match h with
    | Data _ => Some h
    | Thunk th | Encode (Strict th) | Encode (Shallow th) => force_with_fuel n th
    end.
