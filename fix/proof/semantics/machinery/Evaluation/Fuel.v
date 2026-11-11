  Fixpoint eval_list_using (f : handle -> option handle) xs :=
    match xs with
    | [] => Some []
    | x :: xs' => obind (f x) (fun y => omap (cons y) (eval_list_using f xs'))
    end.
  Record evaluators := Evaluators {
    think_fn : thunk -> option handle;
    force_fn : thunk -> option handle;
    execute_fn : encode -> option handle;
    eval_fn : handle -> option handle
  }.
  Definition eval_tree_using f t :=
    omap (create_tree_tagged (tree_tag t)) (eval_list_using f (get_tree_raw t)).
  (** Rocq structural recursion on fuel replaces Isabelle's mixed fuel/list
      recursion. List traversal is a separate structurally recursive helper. *)
  Definition next_evaluators (prev : evaluators) : evaluators :=
      let think := fun th =>
        match th with
        | Application t => obind (eval_tree_using (eval_fn prev) t) A.apply_tree
        | Identification d => I.identify d
        | Selection t => obind (eval_tree_using (eval_fn prev) t) Sl.select
        end in
      let force := fun th => obind (think_fn prev th) (fun h =>
        match h with
        | Data _ => Some h
        | Thunk th' | Encode (Strict th') | Encode (Shallow th') => force_fn prev th'
        end) in
      (** execute at S n uses force at S n, as in execute_with_fuel's
          non-fuel-consuming equation, rather than force at n. *)
      let execute := fun e => match e with
        | Strict th => omap lift (force th)
        | Shallow th => omap lower (force th) end in
      let eval := fun h => match h with
        | Thunk _ | Data (Ref _) | Data (Object (BlobObj _)) => Some h
        | Data (Object (TreeObj t)) => omap HTreeObj (eval_tree_using (eval_fn prev) t)
        | Encode e => obind (execute_fn prev e) (eval_fn prev)
        end in
      Evaluators think force execute eval.
  Fixpoint with_fuel n : evaluators :=
    match n with
    | 0 => Evaluators (fun _ => None) (fun _ => None) (fun _ => None)
        (fun h => match h with
          | Thunk _ | Data (Ref _) | Data (Object (BlobObj _)) => Some h
          | _ => None end)
    | S n' => next_evaluators (with_fuel n')
    end.
  Definition think_with_fuel n := think_fn (with_fuel n).
  Definition force_with_fuel n := force_fn (with_fuel n).
  Definition execute_with_fuel n := execute_fn (with_fuel n).
  Definition eval_with_fuel n := eval_fn (with_fuel n).
  Definition eval_list_with_fuel n := eval_list_using (eval_with_fuel n).
  Definition eval_tree_with_fuel n := eval_tree_using (eval_with_fuel n).
  Definition thinks_to th h := exists fuel, think_with_fuel fuel th = Some h.
  Definition forces_to th h := exists fuel, force_with_fuel fuel th = Some h.
  Definition executes_to e h := exists fuel, execute_with_fuel fuel e = Some h.
  Definition evals_to h r := exists fuel, eval_with_fuel fuel h = Some r.
  Definition evals_tree_to t r := exists fuel, eval_tree_with_fuel fuel t = Some r.

  (** A successful list evaluation supplies a successful evaluation for each corresponding
      pair of entries. *)
  Lemma eval_list_Forall2 f xs ys :
    eval_list_using f xs = Some ys -> Forall2 (fun x y => f x = Some y) xs ys.
  Proof.
    revert ys; induction xs as [|x xs IH]; intros ys E; simpl in E.
    - inversion E; constructor.
    - destruct (f x) eqn:Fx; simpl in E; try discriminate.
      destruct (eval_list_using f xs) eqn:Ex; simpl in E; try discriminate.
      inversion E; subst; constructor; auto.
  Qed.
  (** Successful evaluations of all entries give a successful list evaluation. *)
  Lemma Forall2_eval_list f xs ys :
    Forall2 (fun x y => f x = Some y) xs ys -> eval_list_using f xs = Some ys.
  Proof. induction 1; simpl; auto; rewrite H, IHForall2; reflexivity. Qed.

  (** Successful forcing with finite fuel returns Data. *)
  Lemma force_with_fuel_to_data n th r :
    force_with_fuel n th = Some r -> exists d, r = Data d.
  Proof.
    revert th r; induction n as [|n IH]; intros th r E; simpl in E; try discriminate.
    unfold force_with_fuel in E; simpl in E.
    destruct (think_fn (with_fuel n) th) as [h|] eqn:T; simpl in E; try discriminate.
    destruct h as [d|th'|e]; [inversion E; eauto|apply (IH th' r E)|].
    destruct e; eapply IH; exact E.
  Qed.
  (** Successful finite-fuel evaluation returns Data or a bare Thunk, never an Encode. *)
  Lemma eval_with_fuel_not_encode n h r :
    eval_with_fuel n h = Some r -> (exists d, r = Data d) \/ (exists th, r = Thunk th).
  Proof.
    revert h r; induction n as [|n IH]; intros h r E;
      destruct h as [d|th|e]; try (destruct d as [[b|t]|ref]);
      simpl in E; try discriminate; try (inversion E; eauto).
    - change (omap HTreeObj (eval_tree_using (eval_fn (with_fuel n)) t) = Some r) in E.
      destruct (eval_tree_using (eval_fn (with_fuel n)) t) as [t'|]; simpl in E; try discriminate.
      inversion E; subst; left; exists (Object (TreeObj t')); reflexivity.
    - change (obind (execute_fn (with_fuel n) e) (eval_fn (with_fuel n)) = Some r) in E.
      destruct (execute_fn (with_fuel n) e) eqn:Ex; simpl in E; try discriminate.
      eapply IH; exact E.
  Qed.
