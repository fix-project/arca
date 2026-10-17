From Stdlib Require Import List.
From FixProof Require Import Handle Identify Slice Digest ApplyTree.
Import ListNotations.

Module Evaluation (S : STORAGE) (P : PROGRAM S).
  Module H := Handles S.
  Module A := ApplyTree S P.
  Module I := Identify S.
  Module Sl := Slice S.
  Module D := Digest S.
  Import H.
  Definition is_thunk h := match h with Thunk _ => true | _ => false end.
  Definition lift_data d :=
    match d with
    | Ref (TreeRef t) => Object (TreeObj t)
    | Ref (BlobRef b) => Object (BlobObj b)
    | Object o => Object o
    end.
  Definition lift h := match h with Data d => Data (lift_data d) | _ => h end.
  Definition lower_data d :=
    match d with
    | Object (TreeObj t) => Ref (TreeRef t)
    | Object (BlobObj b) => Ref (BlobRef b)
    | Ref r => Ref r
    end.
  Definition lower h := match h with Data d => Data (lower_data d) | _ => h end.
  Definition get_thunk_inner th :=
    match th with
    | Application t | Selection t | Digestion t => HTreeObj t
    | Identification d => Data d
    end.

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
  Definition eval_tree_using f t := omap create_tree (eval_list_using f (get_tree_raw t)).
  (** Rocq structural recursion on fuel replaces Isabelle's mixed fuel/list
      recursion. List traversal is a separate structurally recursive helper. *)
  Definition next_evaluators (prev : evaluators) : evaluators :=
      let think := fun th =>
        match th with
        | Application t => obind (eval_tree_using (eval_fn prev) t) A.apply_tree
        | Identification d => I.identify d
        | Selection t => obind (eval_tree_using (eval_fn prev) t)
            (fun t' => omap (fun r => Data (Ref r)) (Sl.slice t'))
        | Digestion t => obind (eval_tree_using (eval_fn prev) t)
            (fun t' => obind (Sl.slice t') (fun r =>
              match r with
              | BlobRef _ => None
              | TreeRef t'' => omap (fun t''' => HTreeObj (D.digest t'''))
                  (eval_tree_using (eval_fn prev) t'') end))
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

  Lemma eval_list_to_list_all f xs ys :
    eval_list_using f xs = Some ys -> Forall2 (fun x y => f x = Some y) xs ys.
  Proof.
    revert ys; induction xs as [|x xs IH]; intros ys E; simpl in E.
    - inversion E; constructor.
    - destruct (f x) eqn:Fx; simpl in E; try discriminate.
      destruct (eval_list_using f xs) eqn:Ex; simpl in E; try discriminate.
      inversion E; subst; constructor; auto.
  Qed.
  Lemma list_all_to_eval_list f xs ys :
    Forall2 (fun x y => f x = Some y) xs ys -> eval_list_using f xs = Some ys.
  Proof. induction 1; simpl; auto; rewrite H, IHForall2; reflexivity. Qed.

  Lemma force_with_fuel_to_data n th r :
    force_with_fuel n th = Some r -> exists d, r = Data d.
  Proof.
    revert th r; induction n as [|n IH]; intros th r E; simpl in E; try discriminate.
    unfold force_with_fuel in E; simpl in E.
    destruct (think_fn (with_fuel n) th) as [h|] eqn:T; simpl in E; try discriminate.
    destruct h as [d|th'|e]; [inversion E; eauto|apply (IH th' r E)|].
    destruct e; eapply IH; exact E.
  Qed.
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
  Inductive value_tree : TreeName -> Prop :=
  | value_tree_intro : forall t, Forall value_handle (get_tree_raw t) -> value_tree t
  with value_handle : handle -> Prop :=
  | blob_obj_handle : forall b, value_handle (HBlobObj b)
  | tree_obj_handle : forall t, value_tree t -> value_handle (HTreeObj t)
  | ref_handle : forall r, value_handle (Data (Ref r))
  | thunk_handle : forall th, value_handle (Thunk th).

  Lemma value_handle_not_encode h : value_handle h -> ~(exists e, h = Encode e).
  Proof. intros V [e H]; inversion V; subst; discriminate. Qed.
  Lemma eval_list_using_output f xs ys :
    (forall h r, f h = Some r -> value_handle r) ->
    eval_list_using f xs = Some ys -> Forall value_handle ys.
  Proof.
    intros Safe Ev; apply eval_list_to_list_all in Ev.
    induction Ev; constructor; eauto.
  Qed.
  Lemma eval_with_fuel_to_value_handle n h r :
    eval_with_fuel n h = Some r -> value_handle r.
  Proof.
    revert h r; induction n as [|n IH]; intros h r Ev;
      destruct h as [d|th|e]; try (destruct d as [[b|t]|ref]).
    all: try solve [cbn [eval_with_fuel with_fuel next_evaluators eval_fn] in Ev;
      inversion Ev; subst; constructor].
    - change (omap HTreeObj (omap create_tree (eval_list_using (eval_with_fuel n) (get_tree_raw t))) = Some r) in Ev.
      destruct (eval_list_using (eval_with_fuel n) (get_tree_raw t)) as [ys|] eqn:List; simpl in Ev; try discriminate.
      inversion Ev; subst; apply tree_obj_handle, value_tree_intro.
      rewrite get_tree_raw_create_tree.
      eapply eval_list_using_output; [exact IH|exact List].
    - change (obind (execute_with_fuel n e) (eval_with_fuel n) = Some r) in Ev.
      destruct (execute_with_fuel n e) eqn:Ex; simpl in Ev; try discriminate.
      eapply IH; exact Ev.
  Qed.
End Evaluation.
