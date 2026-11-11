  (** Reversing a pointwise relation reverses the order of the related lists. *)
  Lemma Forall2_flip (X : handle -> handle -> Prop) xs ys :
    Forall2 X xs ys -> Forall2 (fun y x => X x y) ys xs.
  Proof. induction 1; constructor; auto. Qed.
  (** Transporting each entry's evaluation transports the whole list evaluation with some
      sufficient fuel. *)
  Lemma eval_list_transport (X : handle -> handle -> Prop) n
      (eval_cong_n : forall h1 h2, X h1 h2 -> forall v1,
        eval_with_fuel n h1 = Some v1 -> exists v2, evals_to h2 v2 /\ X v1 v2)
      xs ys : Forall2 X xs ys -> forall out1,
      eval_list_with_fuel n xs = Some out1 ->
      exists m out2, eval_list_with_fuel m ys = Some out2 /\ Forall2 X out1 out2.
  Proof.
    (* The matching head evaluation and the matching tail evaluation may require
        different amounts of work. Use their separate existence proofs and enlarge both
        bounds to their sum; this avoids assuming that equivalent children finish in the
        same number of steps. *)
    intro Raw; induction Raw; intros out1 List.
    - inversion List; subst; exists 0, []; split; [reflexivity|constructor].
    - change (obind (eval_with_fuel n x)
        (fun z => omap (cons z) (eval_list_with_fuel n l)) = Some out1) in List.
      apply obind_some in List; destruct List as [v1 [Ev Tail]].
      apply omap_some in Tail; destruct Tail as [out1' [Tail ->]].
      destruct (eval_cong_n x y H v1 Ev) as [v2 [[k Ev2] Head]].
      destruct (IHRaw out1' Tail) as [m [out2 [Tail2 Related]]].
      exists (k+m), (v2::out2); split; [|constructor; assumption].
      change (obind (eval_with_fuel (k+m) y)
        (fun z => omap (cons z) (eval_list_with_fuel (k+m) l')) = Some (v2::out2)).
      rewrite (eval_padding k m y v2 Ev2); simpl.
      destruct (fuel_padding m k) as [_ [_ [_ [Pad _]]]].
      rewrite Nat.add_comm, (Pad l' out2 Tail2); reflexivity.
  Qed.
  (** Transporting child evaluations transports Tree evaluation while preserving the
      relation and Tag marker. *)
  Lemma eval_tree_transport (X : handle -> handle -> Prop) n
      (tree_complete : forall tag xs ys, Forall2 X xs ys -> X (HTreeObj (create_tree_tagged tag xs)) (HTreeObj (create_tree_tagged tag ys)))
      (eval_cong_n : forall h1 h2, X h1 h2 -> forall v1,
        eval_with_fuel n h1 = Some v1 -> exists v2, evals_to h2 v2 /\ X v1 v2)
      t1 t2 : tree_tag t1 = tree_tag t2 -> Forall2 X (get_tree_raw t1) (get_tree_raw t2) -> forall v1,
      eval_tree_with_fuel n t1 = Some v1 ->
      exists v2, evals_tree_to t2 v2 /\ X (HTreeObj v1) (HTreeObj v2).
  Proof.
    intros Tags Raw v1 Ev; unfold eval_tree_with_fuel, eval_tree_using in Ev.
    change (omap (create_tree_tagged (tree_tag t1)) (eval_list_with_fuel n (get_tree_raw t1)) = Some v1) in Ev.
    apply omap_some in Ev; destruct Ev as [xs [List ->]].
    destruct (eval_list_transport X n eval_cong_n _ _ Raw xs List) as [m [ys [List2 Related]]].
    exists (create_tree_tagged (tree_tag t2) ys); split; [|rewrite Tags; apply tree_complete; exact Related].
    exists m; change (omap (create_tree_tagged (tree_tag t2)) (eval_list_with_fuel m (get_tree_raw t2)) = Some (create_tree_tagged (tree_tag t2) ys)).
    rewrite List2; reflexivity.
  Qed.
  (** Tree evaluation transports in both directions when child evaluation does. *)
  Lemma eq_tree_eval_fuel_n (X : handle -> handle -> Prop) n
      (tree_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> Forall2 X (get_tree_raw t1) (get_tree_raw t2))
      (tag_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> tree_tag t1 = tree_tag t2)
      (tree_complete : forall tag xs ys, Forall2 X xs ys -> X (HTreeObj (create_tree_tagged tag xs)) (HTreeObj (create_tree_tagged tag ys)))
      (eval_cong_n : forall h1 h2, X h1 h2 ->
        (forall v1, eval_with_fuel n h1 = Some v1 -> exists v2, evals_to h2 v2 /\ X v1 v2) /\
        (forall v2, eval_with_fuel n h2 = Some v2 -> exists v1, evals_to h1 v1 /\ X v1 v2))
      t1 t2 : X (HTreeObj t1) (HTreeObj t2) ->
      (forall v1, eval_tree_with_fuel n t1 = Some v1 -> exists v2, evals_tree_to t2 v2 /\ X (HTreeObj v1) (HTreeObj v2)) /\
      (forall v2, eval_tree_with_fuel n t2 = Some v2 -> exists v1, evals_tree_to t1 v1 /\ X (HTreeObj v1) (HTreeObj v2)).
  Proof.
    intro R; split.
    - apply (eval_tree_transport X n tree_complete); [|apply tag_cong; exact R|apply tree_cong; exact R].
      intros h1 h2 H; exact (proj1 (eval_cong_n h1 h2 H)).
    - apply (eval_tree_transport (fun x y => X y x) n).
      + intros tag xs ys Related; apply tree_complete.
        apply Forall2_flip in Related; exact Related.
      + intros h2 h1 H; exact (proj2 (eval_cong_n h1 h2 H)).
      + symmetry; apply tag_cong; exact R.
      + apply Forall2_flip, tree_cong; exact R.
  Qed.
