  (** Thinking a Thunk without an explicit fuel bound succeeds exactly when some finite
      fuel succeeds. *)
  Lemma think_some th r : think th = Some r <-> thinks_to th r.
  Proof. apply unfuel_some; exact think_deterministic. Qed.
  (** Forcing a Thunk without an explicit fuel bound succeeds exactly when some finite fuel
      succeeds. *)
  Lemma force_some th r : force th = Some r <-> forces_to th r.
  Proof. apply unfuel_some; exact force_deterministic. Qed.
  (** Executing an Encode without an explicit fuel bound succeeds exactly when some finite
      fuel succeeds. *)
  Lemma execute_some e r : execute e = Some r <-> executes_to e r.
  Proof. apply unfuel_some; exact execute_deterministic. Qed.
  (** Evaluating a handle without an explicit fuel bound succeeds exactly when some finite
      fuel succeeds. *)
  Lemma eval_some h r : eval h = Some r <-> evals_to h r.
  Proof. apply unfuel_some; exact eval_deterministic. Qed.
  (** Evaluating a Tree without an explicit fuel bound succeeds exactly when some finite
      fuel succeeds. *)
  Lemma eval_tree_some t r : eval_tree t = Some r <-> evals_tree_to t r.
  Proof. apply unfuel_some; exact eval_tree_deterministic. Qed.
  (** A successful finite-fuel run determines the unfuelled think result. *)
  Lemma think_unique th r : thinks_to th r -> think th = Some r.
  Proof. apply think_some. Qed.
  (** A successful finite-fuel run determines the unfuelled force result. *)
  Lemma force_unique th r : forces_to th r -> force th = Some r.
  Proof. apply force_some. Qed.
  (** A successful finite-fuel run determines the unfuelled execute result. *)
  Lemma execute_unique e r : executes_to e r -> execute e = Some r.
  Proof. apply execute_some. Qed.
  (** A successful finite-fuel run determines the unfuelled eval result. *)
  Lemma eval_unique h r : evals_to h r -> eval h = Some r.
  Proof. apply eval_some. Qed.
  (** A successful finite-fuel run determines the unfuelled eval_tree result. *)
  Lemma eval_tree_unique t r : evals_tree_to t r -> eval_tree t = Some r.
  Proof. apply eval_tree_some. Qed.

  (** Adding fuel preserves a successful think result. *)
  Lemma think_padding n k th r : think_with_fuel n th = Some r -> think_with_fuel (n+k) th = Some r.
  Proof. apply (proj1 (fuel_padding n k)). Qed.
  (** Adding fuel preserves a successful force result. *)
  Lemma force_padding n k th r : force_with_fuel n th = Some r -> force_with_fuel (n+k) th = Some r.
  Proof. apply (proj1 (proj2 (fuel_padding n k))). Qed.
  (** Adding fuel preserves a successful execute result. *)
  Lemma execute_padding n k e r : execute_with_fuel n e = Some r -> execute_with_fuel (n+k) e = Some r.
  Proof. apply (proj1 (proj2 (proj2 (fuel_padding n k)))). Qed.
  (** Adding fuel preserves a successful eval result. *)
  Lemma eval_padding n k h r : eval_with_fuel n h = Some r -> eval_with_fuel (n+k) h = Some r.
  Proof. apply (proj2 (proj2 (proj2 (proj2 (proj2 (fuel_padding n k)))))). Qed.
  (** Adding fuel preserves a successful eval_tree result. *)
  Lemma eval_tree_padding n k t r : eval_tree_with_fuel n t = Some r -> eval_tree_with_fuel (n+k) t = Some r.
  Proof. apply (proj1 (proj2 (proj2 (proj2 (proj2 (fuel_padding n k)))))). Qed.

  (** Adding fuel preserves a successful list evaluation. *)
  Lemma eval_list_with_fuel_padding n k xs ys : eval_list_with_fuel n xs = Some ys ->
    eval_list_with_fuel (n+k) xs = Some ys.
  Proof. destruct (fuel_padding n k) as [_ [_ [_ [Pad _]]]]; apply Pad. Qed.

  (** Pointwise successful evaluations admit a single finite fuel bound for the whole list. *)
  Lemma evals_to_tree_to xs ys : Forall2 evals_to xs ys ->
    exists n, eval_list_with_fuel n xs = Some ys.
  Proof.
    intro Entries; induction Entries as [|x y xs ys Head Entries IH].
    - exists 0; reflexivity.
    - destruct Head as [n Head], IH as [m Tail].
      exists (n+m); change (obind (eval_with_fuel (n+m) x)
        (fun v => omap (cons v) (eval_list_with_fuel (n+m) xs)) = Some (y::ys)).
      rewrite (eval_padding n m x y Head); simpl.
      rewrite Nat.add_comm, (eval_list_with_fuel_padding m n xs ys Tail); reflexivity.
  Qed.

  (** Two optional values are equal if they have exactly the same successful payloads. *)
  Lemma option_ext_some {A} (x y : option A) :
    (forall r, x = Some r <-> y = Some r) -> x = y.
  Proof.
    intro H; destruct x as [x|], y as [y|]; auto.
    - specialize (proj1 (H x) eq_refl); congruence.
    - specialize (proj1 (H x) eq_refl); discriminate.
    - specialize (proj2 (H y) eq_refl); discriminate.
  Qed.
  (** An optional map succeeds exactly when its input succeeds and its payload maps to the
      result. *)
  Lemma omap_some {A B} (f : A -> B) x r :
    omap f x = Some r <-> exists v, x = Some v /\ r = f v.
  Proof. destruct x; simpl; split; intro H; try discriminate; inversion H; subst; eauto; firstorder congruence. Qed.
  (** An optional bind succeeds exactly when its input and the chosen continuation both
      succeed. *)
  Lemma obind_some {A B} x (f : A -> option B) r :
    obind x f = Some r <-> exists v, x = Some v /\ f v = Some r.
  Proof. destruct x; simpl; split; intro H; try discriminate; eauto; firstorder congruence. Qed.
