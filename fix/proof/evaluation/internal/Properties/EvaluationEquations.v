  (** Evaluating a Blob Object returns it unchanged. *)
  Lemma eval_blob b : eval (HBlobObj b) = Some (HBlobObj b).
  Proof. apply eval_unique; exists 0; reflexivity. Qed.
  (** Evaluating a Ref returns it unchanged without loading its Object. *)
  Lemma eval_ref r : eval (Data (Ref r)) = Some (Data (Ref r)).
  Proof. apply eval_unique; exists 0; reflexivity. Qed.
  (** Evaluating a bare Thunk returns it unchanged without computing it. *)
  Lemma eval_thunk th : eval (Thunk th) = Some (Thunk th).
  Proof. apply eval_unique; exists 0; reflexivity. Qed.
  (** Evaluating a Tree Object wraps the result of Tree evaluation as an Object handle. *)
  Lemma eval_tree_handle t : eval (HTreeObj t) = omap HTreeObj (eval_tree t).
  Proof.
    apply option_ext_some; intro r; rewrite omap_some; split.
    - intro Ev; apply eval_some in Ev; destruct Ev as [n Ev].
      destruct n as [|n]; [discriminate|].
      change (omap HTreeObj (eval_tree_with_fuel n t) = Some r) in Ev.
      apply omap_some in Ev; destruct Ev as [t' [Ev ->]].
      exists t'; split; [apply eval_tree_unique; exists n; exact Ev|reflexivity].
    - intros [t' [Ev ->]]; apply eval_tree_some in Ev; destruct Ev as [n Ev].
      apply eval_unique; exists (S n).
      change (omap HTreeObj (eval_tree_with_fuel n t) = Some (HTreeObj t')).
      rewrite Ev; reflexivity.
  Qed.
  (** Tree-handle evaluation succeeds exactly when the corresponding Tree evaluation
      succeeds. *)
  Lemma eval_tree_handle_iff t u :
    eval (HTreeObj t) = Some (HTreeObj u) <-> eval_tree t = Some u.
  Proof. rewrite eval_tree_handle; hauto use: omap_some unfold!: HTreeObj. Qed.

  (** Evaluating an Encode executes it and then evaluates the returned handle. *)
  Lemma eval_encode e : eval (Encode e) = obind (execute e) eval.
  Proof.
    (* The unbounded equation must work in both directions. A successful Encode run
        contains finite witnesses for execution and subsequent evaluation. Conversely
        those witnesses may have different bounds, but their sum is sufficient for the
        composed computation. *)
    apply option_ext_some; intro r; rewrite obind_some; split.
    - intro Ev; apply eval_some in Ev; destruct Ev as [n Ev].
      destruct n as [|n]; [discriminate|].
      change (obind (execute_with_fuel n e) (eval_with_fuel n) = Some r) in Ev.
      apply obind_some in Ev; destruct Ev as [h [Ex Ev]].
      exists h; split; [apply execute_unique|apply eval_unique]; exists n; assumption.
    - intros [h [Ex Ev]]; apply execute_some in Ex; apply eval_some in Ev.
      destruct Ex as [n Ex]; destruct Ev as [m Ev].
      apply eval_unique; exists (S (n+m)).
      change (obind (execute_with_fuel (n+m) e) (eval_with_fuel (n+m)) = Some r).
      rewrite (execute_padding n m e h Ex); simpl.
      rewrite Nat.add_comm; apply eval_padding; exact Ev.
  Qed.
