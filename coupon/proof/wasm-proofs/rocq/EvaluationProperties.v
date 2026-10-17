From Stdlib Require Import List Arith Lia ClassicalEpsilon.
From FixProof Require Import Handle ApplyTree Evaluation.
Import ListNotations.

Module EvaluationProperties (S : STORAGE) (P : PROGRAM S).
  Module E := Evaluation S P.
  Import E E.H.
  Definition option_le {A} (x y : option A) := forall r, x = Some r -> y = Some r.
  Definition success_le {A B} (f g : A -> option B) := forall x, option_le (f x) (g x).
  Definition evaluators_le a b :=
    success_le (think_fn a) (think_fn b) /\
    success_le (force_fn a) (force_fn b) /\
    success_le (execute_fn a) (execute_fn b) /\
    success_le (eval_fn a) (eval_fn b).

  Lemma omap_le {A B} (f : A -> B) x y : option_le x y -> option_le (omap f x) (omap f y).
  Proof.
    intros L r R; destruct x as [x|]; simpl in R; try discriminate.
    rewrite (L x eq_refl); exact R.
  Qed.
  Lemma obind_le {A B} x y (f g : A -> option B) :
    option_le x y -> success_le f g -> option_le (obind x f) (obind y g).
  Proof.
    intros L F r R; destruct x as [x|]; simpl in R; try discriminate.
    rewrite (L x eq_refl); apply F; exact R.
  Qed.
  Lemma option_le_refl {A} (x : option A) : option_le x x.
  Proof. intros r R; exact R. Qed.
  Lemma eval_list_using_le f g : success_le f g -> success_le (eval_list_using f) (eval_list_using g).
  Proof.
    intros F xs; induction xs; simpl; [apply option_le_refl|].
    apply obind_le; [apply F|].
    intro y; apply omap_le; exact IHxs.
  Qed.
  Lemma eval_tree_using_le f g : success_le f g -> success_le (eval_tree_using f) (eval_tree_using g).
  Proof. intros F t; apply omap_le, eval_list_using_le; exact F. Qed.

  Lemma next_evaluators_le a b : evaluators_le a b -> evaluators_le (next_evaluators a) (next_evaluators b).
  Proof.
    intros [T [F [Ex Ev]]].
    assert (success_le (think_fn (next_evaluators a)) (think_fn (next_evaluators b))) as NT.
    { intro th; destruct th; cbn [next_evaluators think_fn].
      - apply obind_le; [apply eval_tree_using_le; exact Ev|].
        intro t0; apply option_le_refl.
      - apply option_le_refl.
      - apply obind_le; [apply eval_tree_using_le; exact Ev|].
        intro t0; apply option_le_refl.
      - apply obind_le; [apply eval_tree_using_le; exact Ev|].
        intro t0; apply obind_le; [apply option_le_refl|].
        intros [b'|t']; [apply option_le_refl|].
        apply omap_le, eval_tree_using_le; exact Ev.
    }
    assert (success_le (force_fn (next_evaluators a)) (force_fn (next_evaluators b))) as NF.
    { intro th; cbn [next_evaluators force_fn].
      apply obind_le; [apply T|].
      intros [d|th'|[th'|th']]; [apply option_le_refl|apply F|apply F|apply F].
    }
    repeat split.
    - exact NT.
    - exact NF.
    - intros [th|th]; cbn [next_evaluators execute_fn]; apply omap_le; apply NF.
    - intros [d|th|e]; [destruct d as [[blob|tree]|ref]| |]; cbn [next_evaluators eval_fn];
        try apply option_le_refl.
      + apply omap_le, eval_tree_using_le; exact Ev.
      + apply obind_le; [apply Ex|exact Ev].
  Qed.

  Lemma with_fuel_step n : evaluators_le (with_fuel n) (with_fuel (S n)).
  Proof.
    induction n as [|n IH]; [|apply next_evaluators_le; exact IH].
    repeat split; unfold success_le, option_le; intros x r R;
      cbn [with_fuel think_fn force_fn execute_fn] in R; try discriminate.
    destruct x as [d|th|e]; [destruct d as [[blob|tree]|ref]| |];
      cbn [with_fuel next_evaluators eval_fn] in *; try discriminate; exact R.
  Qed.

  Lemma with_fuel_padding n k : evaluators_le (with_fuel n) (with_fuel (n+k)).
  Proof.
    induction k as [|k IH].
    - rewrite Nat.add_0_r; repeat split; intros x r R; exact R.
    - rewrite Nat.add_succ_r.
      destruct IH as [T [F [Ex Ev]]].
      destruct (with_fuel_step (n+k)) as [T' [F' [Ex' Ev']]].
      repeat split; intros x r R; [apply T'|apply F'|apply Ex'|apply Ev'];
        [apply T|apply F|apply Ex|apply Ev]; exact R.
  Qed.

  Theorem fuel_padding n k :
    (forall th r, think_with_fuel n th = Some r -> think_with_fuel (n+k) th = Some r) /\
    (forall th r, force_with_fuel n th = Some r -> force_with_fuel (n+k) th = Some r) /\
    (forall e r, execute_with_fuel n e = Some r -> execute_with_fuel (n+k) e = Some r) /\
    (forall xs ys, eval_list_with_fuel n xs = Some ys -> eval_list_with_fuel (n+k) xs = Some ys) /\
    (forall t r, eval_tree_with_fuel n t = Some r -> eval_tree_with_fuel (n+k) t = Some r) /\
    (forall h r, eval_with_fuel n h = Some r -> eval_with_fuel (n+k) h = Some r).
  Proof.
    destruct (with_fuel_padding n k) as [T [F [Ex Ev]]].
    repeat split; try assumption.
    - apply eval_list_using_le; exact Ev.
    - apply eval_tree_using_le; exact Ev.
  Qed.

  Theorem think_deterministic th x y : thinks_to th x -> thinks_to th y -> x = y.
  Proof.
    intros [n N] [m M].
    destruct (fuel_padding n m) as [Pad _].
    destruct (fuel_padding m n) as [Pad' _].
    specialize (Pad th x N); specialize (Pad' th y M).
    rewrite Nat.add_comm in Pad'; congruence.
  Qed.
  Theorem force_deterministic th x y : forces_to th x -> forces_to th y -> x = y.
  Proof.
    intros [n N] [m M].
    destruct (fuel_padding n m) as [_ [Pad _]].
    destruct (fuel_padding m n) as [_ [Pad' _]].
    specialize (Pad th x N); specialize (Pad' th y M).
    rewrite Nat.add_comm in Pad'; congruence.
  Qed.
  Theorem execute_deterministic e x y : executes_to e x -> executes_to e y -> x = y.
  Proof.
    intros [n N] [m M].
    destruct (fuel_padding n m) as [_ [_ [Pad _]]].
    destruct (fuel_padding m n) as [_ [_ [Pad' _]]].
    specialize (Pad e x N); specialize (Pad' e y M).
    rewrite Nat.add_comm in Pad'; congruence.
  Qed.
  Theorem eval_deterministic h x y : evals_to h x -> evals_to h y -> x = y.
  Proof.
    intros [n N] [m M].
    destruct (fuel_padding n m) as [_ [_ [_ [_ [_ Pad]]]]].
    destruct (fuel_padding m n) as [_ [_ [_ [_ [_ Pad']]]]].
    specialize (Pad h x N); specialize (Pad' h y M).
    rewrite Nat.add_comm in Pad'; congruence.
  Qed.
  Theorem eval_tree_deterministic t x y : evals_tree_to t x -> evals_tree_to t y -> x = y.
  Proof.
    intros [n N] [m M].
    destruct (fuel_padding n m) as [_ [_ [_ [_ [Pad _]]]]].
    destruct (fuel_padding m n) as [_ [_ [_ [_ [Pad' _]]]]].
    specialize (Pad t x N); specialize (Pad' t y M).
    rewrite Nat.add_comm in Pad'; congruence.
  Qed.

  (** Isabelle's SOME/THE-based, unbounded option-valued evaluators become
      classical definite choice. No computable termination test is claimed. *)
  Definition unfuel {A B} (f : nat -> A -> option B) (x : A) : option B :=
    match excluded_middle_informative (exists r, exists n, f n x = Some r) with
    | left H => Some (proj1_sig (constructive_indefinite_description _ H))
    | right _ => None
    end.
  Lemma unfuel_some {A B} (f : nat -> A -> option B)
      (unique : forall x r1 r2, (exists n, f n x = Some r1) -> (exists n, f n x = Some r2) -> r1 = r2)
      x r : unfuel f x = Some r <-> exists n, f n x = Some r.
  Proof.
    unfold unfuel; destruct (excluded_middle_informative _) as [H|H].
    - destruct (constructive_indefinite_description _ H) as [chosen C]; simpl.
      split; [intro R; inversion R; subst; exact C|].
      intro R; f_equal; apply (unique x chosen r C R).
    - split; [discriminate|intro R; exfalso; apply H; exists r; exact R].
  Qed.
  Definition think := unfuel think_with_fuel.
  Definition force := unfuel force_with_fuel.
  Definition execute := unfuel execute_with_fuel.
  Definition eval := unfuel eval_with_fuel.
  Definition eval_tree := unfuel eval_tree_with_fuel.
  Definition encode_to_thunk e := match e with Strict th | Shallow th => th end.
  Definition relaxed_X (X : handle -> handle -> Prop) h1 h2 :=
    match h1 with
    | Encode (Shallow th) => X (Encode (Strict th)) (lift h2)
    | _ => X (lift h1) (lift h2)
    end.
  Definition strengthen (X : handle -> handle -> Prop) h1 h2 :=
    match h1, h2 with
    | Encode e1, Encode e2 =>
        X (Thunk (encode_to_thunk e1)) (Thunk (encode_to_thunk e2)) \/
        rel_opt (relaxed_X X) (force (encode_to_thunk e1)) (force (encode_to_thunk e2))
    | Encode e, Thunk th => X (Thunk (encode_to_thunk e)) (Thunk th)
    | Thunk th, Encode e => X (Thunk th) (Thunk (encode_to_thunk e))
    | _, _ => relaxed_X X h1 h2
    end.
  Lemma think_some th r : think th = Some r <-> thinks_to th r.
  Proof. apply unfuel_some; exact think_deterministic. Qed.
  Lemma force_some th r : force th = Some r <-> forces_to th r.
  Proof. apply unfuel_some; exact force_deterministic. Qed.
  Lemma execute_some e r : execute e = Some r <-> executes_to e r.
  Proof. apply unfuel_some; exact execute_deterministic. Qed.
  Lemma eval_some h r : eval h = Some r <-> evals_to h r.
  Proof. apply unfuel_some; exact eval_deterministic. Qed.
  Lemma eval_tree_some t r : eval_tree t = Some r <-> evals_tree_to t r.
  Proof. apply unfuel_some; exact eval_tree_deterministic. Qed.
  Lemma think_unique th r : thinks_to th r -> think th = Some r.
  Proof. apply think_some. Qed.
  Lemma force_unique th r : forces_to th r -> force th = Some r.
  Proof. apply force_some. Qed.
  Lemma execute_unique e r : executes_to e r -> execute e = Some r.
  Proof. apply execute_some. Qed.
  Lemma eval_unique h r : evals_to h r -> eval h = Some r.
  Proof. apply eval_some. Qed.
  Lemma eval_tree_unique t r : evals_tree_to t r -> eval_tree t = Some r.
  Proof. apply eval_tree_some. Qed.

  Lemma think_padding n k th r : think_with_fuel n th = Some r -> think_with_fuel (n+k) th = Some r.
  Proof. apply (proj1 (fuel_padding n k)). Qed.
  Lemma force_padding n k th r : force_with_fuel n th = Some r -> force_with_fuel (n+k) th = Some r.
  Proof. apply (proj1 (proj2 (fuel_padding n k))). Qed.
  Lemma execute_padding n k e r : execute_with_fuel n e = Some r -> execute_with_fuel (n+k) e = Some r.
  Proof. apply (proj1 (proj2 (proj2 (fuel_padding n k)))). Qed.
  Lemma eval_padding n k h r : eval_with_fuel n h = Some r -> eval_with_fuel (n+k) h = Some r.
  Proof. apply (proj2 (proj2 (proj2 (proj2 (proj2 (fuel_padding n k)))))). Qed.
  Lemma eval_tree_padding n k t r : eval_tree_with_fuel n t = Some r -> eval_tree_with_fuel (n+k) t = Some r.
  Proof. apply (proj1 (proj2 (proj2 (proj2 (proj2 (fuel_padding n k)))))). Qed.

  Lemma eval_list_with_fuel_padding n k xs ys : eval_list_with_fuel n xs = Some ys ->
    eval_list_with_fuel (n+k) xs = Some ys.
  Proof. destruct (fuel_padding n k) as [_ [_ [_ [Pad _]]]]; apply Pad. Qed.

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

  Lemma evals_to_tree_to_exists xs : Forall (fun x => exists y, evals_to x y) xs ->
    exists n ys, eval_list_with_fuel n xs = Some ys.
  Proof.
    intro Entries; induction Entries as [|x xs [y Head] Entries IH].
    - exists 0, []; reflexivity.
    - destruct Head as [n Head], IH as [m [ys Tail]].
      exists (n+m), (y::ys); change (obind (eval_with_fuel (n+m) x)
        (fun v => omap (cons v) (eval_list_with_fuel (n+m) xs)) = Some (y::ys)).
      rewrite (eval_padding n m x y Head); simpl.
      rewrite Nat.add_comm, (eval_list_with_fuel_padding m n xs ys Tail); reflexivity.
  Qed.

  Lemma option_ext_some {A} (x y : option A) :
    (forall r, x = Some r <-> y = Some r) -> x = y.
  Proof.
    intro H; destruct x as [x|], y as [y|]; auto.
    - specialize (proj1 (H x) eq_refl); congruence.
    - specialize (proj1 (H x) eq_refl); discriminate.
    - specialize (proj2 (H y) eq_refl); discriminate.
  Qed.
  Lemma omap_some {A B} (f : A -> B) x r :
    omap f x = Some r <-> exists v, x = Some v /\ r = f v.
  Proof. destruct x; simpl; split; intro H; try discriminate; inversion H; subst; eauto; firstorder congruence. Qed.
  Lemma obind_some {A B} x (f : A -> option B) r :
    obind x f = Some r <-> exists v, x = Some v /\ f v = Some r.
  Proof. destruct x; simpl; split; intro H; try discriminate; eauto; firstorder congruence. Qed.

  Lemma eval_blob b : eval (HBlobObj b) = Some (HBlobObj b).
  Proof. apply eval_unique; exists 0; reflexivity. Qed.
  Lemma eval_ref r : eval (Data (Ref r)) = Some (Data (Ref r)).
  Proof. apply eval_unique; exists 0; reflexivity. Qed.
  Lemma eval_thunk th : eval (Thunk th) = Some (Thunk th).
  Proof. apply eval_unique; exists 0; reflexivity. Qed.
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
  Lemma eval_encode e : eval (Encode e) = obind (execute e) eval.
  Proof.
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
  Theorem eval_hs h : eval h =
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
  Lemma execute_with_fuel_force n e : execute_with_fuel n e =
    omap (match e with Strict _ => lift | Shallow _ => lower end)
         (force_with_fuel n (encode_to_thunk e)).
  Proof. destruct n, e; reflexivity. Qed.
  Theorem execute_hs e : execute e =
    omap (match e with Strict _ => lift | Shallow _ => lower end)
         (force (encode_to_thunk e)).
  Proof.
    apply option_ext_some; intro r; rewrite omap_some; split.
    - intro Ex; apply execute_some in Ex; destruct Ex as [n Ex].
      rewrite execute_with_fuel_force in Ex; apply omap_some in Ex.
      destruct Ex as [v [F ->]]; exists v; split; [apply force_unique; exists n; exact F|reflexivity].
    - intros [v [F ->]]; apply force_some in F; destruct F as [n F].
      apply execute_unique; exists n; rewrite execute_with_fuel_force, F; reflexivity.
  Qed.

  Lemma force_data th r : force th = Some r -> exists d, r = Data d.
  Proof. rewrite force_some; intros [n F]; eapply force_with_fuel_to_data; exact F. Qed.
  Lemma eval_not_encode h r : eval h = Some r -> not_encode r.
  Proof.
    rewrite eval_some; intros [n Ev]; apply eval_with_fuel_not_encode in Ev;
      destruct Ev as [[d ->]|[th ->]]; exact I.
  Qed.
  Lemma eval_to_value_handle h r : eval h = Some r -> value_handle r.
  Proof. rewrite eval_some; intros [n Ev]; eapply eval_with_fuel_to_value_handle; exact Ev. Qed.
  Lemma eval_tree_to_value_handle t1 t2 : eval_tree t1 = Some t2 -> value_handle (HTreeObj t2).
  Proof.
    intro Ev; apply (eval_to_value_handle (HTreeObj t1)).
    rewrite eval_tree_handle, Ev; reflexivity.
  Qed.
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
  Theorem force_hs th : force th = obind (think th) (fun h =>
    match h with
    | Data _ => Some h
    | Thunk th' => force th'
    | Encode e => force (encode_to_thunk e)
    end).
  Proof.
    apply option_ext_some; intro r; rewrite obind_some; split.
    - intro F; apply force_some in F; destruct F as [n F].
      destruct n as [|n]; [discriminate|].
      change (obind (think_with_fuel n th) (fun h =>
        match h with
        | Data _ => Some h
        | Thunk th' | Encode (Strict th') | Encode (Shallow th') => force_with_fuel n th'
        end) = Some r) in F.
      apply obind_some in F; destruct F as [h [T F]].
      exists h; split; [apply think_unique; exists n; exact T|].
      destruct h as [d|th'|[th'|th']]; [exact F| | |];
        apply force_unique; exists n; exact F.
    - intros [h [T F]]; apply think_some in T; destruct T as [n T].
      destruct h as [d|th'|[th'|th']].
      + inversion F; subst; apply force_unique; exists (S n).
        change (obind (think_with_fuel n th) (force_after n) = Some (Data d)); rewrite T; reflexivity.
      + apply force_some in F; destruct F as [m F].
        apply force_unique; exists (S (n+m)).
        change (obind (think_with_fuel (n+m) th) (force_after (n+m)) = Some r).
        rewrite (think_padding n m th _ T); simpl.
        rewrite Nat.add_comm; apply force_padding; exact F.
      + apply force_some in F; destruct F as [m F].
        apply force_unique; exists (S (n+m)).
        change (obind (think_with_fuel (n+m) th) (force_after (n+m)) = Some r).
        rewrite (think_padding n m th _ T); simpl.
        rewrite Nat.add_comm; apply force_padding; exact F.
      + apply force_some in F; destruct F as [m F].
        apply force_unique; exists (S (n+m)).
        change (obind (think_with_fuel (n+m) th) (force_after (n+m)) = Some r).
        rewrite (think_padding n m th _ T); simpl.
        rewrite Nat.add_comm; apply force_padding; exact F.
  Qed.

  Lemma think_identification d : think (Identification d) = I.identify d.
  Proof. apply think_unique; exists 1; reflexivity. Qed.
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
  Lemma think_selection t : think (Selection t) =
    obind (eval_tree t) (fun t' => omap (fun ref => Data (Ref ref)) (Sl.slice t')).
  Proof.
    apply option_ext_some; intro r; rewrite obind_some; split.
    - intro T; apply think_some in T; destruct T as [n T].
      destruct n as [|n]; [discriminate|].
      change (obind (eval_tree_with_fuel n t) (fun t' => omap (fun ref => Data (Ref ref)) (Sl.slice t')) = Some r) in T.
      apply obind_some in T; destruct T as [t' [Ev Slice]].
      exists t'; split; [apply eval_tree_unique; exists n; exact Ev|exact Slice].
    - intros [t' [Ev Slice]]; apply eval_tree_some in Ev; destruct Ev as [n Ev].
      apply think_unique; exists (S n).
      change (obind (eval_tree_with_fuel n t) (fun t' => omap (fun ref => Data (Ref ref)) (Sl.slice t')) = Some r).
      rewrite Ev; exact Slice.
  Qed.
  Definition digest_after (ev : TreeName -> option TreeName) t :=
    obind (Sl.slice t) (fun ref => match ref with
      | BlobRef _ => None
      | TreeRef t' => omap (fun t'' => HTreeObj (D.digest t'')) (ev t')
      end).
  Lemma digest_after_some ev t r : digest_after ev t = Some r <->
    exists t' t'', Sl.slice t = Some (TreeRef t') /\ ev t' = Some t'' /\ r = HTreeObj (D.digest t'').
  Proof.
    unfold digest_after; rewrite obind_some; split.
    - intros [[b|t'] [Slice Rest]]; [discriminate|].
      apply omap_some in Rest; destruct Rest as [t'' [Ev ->]]; eauto.
    - intros [t' [t'' [Slice [Ev ->]]]]; exists (TreeRef t'); split; [exact Slice|].
      simpl; rewrite Ev; reflexivity.
  Qed.
  Lemma think_digestion t : think (Digestion t) = obind (eval_tree t) (digest_after eval_tree).
  Proof.
    apply option_ext_some; intro r; rewrite obind_some; split.
    - intro T; apply think_some in T; destruct T as [n T].
      destruct n as [|n]; [discriminate|].
      change (obind (eval_tree_with_fuel n t) (digest_after (eval_tree_with_fuel n)) = Some r) in T.
      apply obind_some in T; destruct T as [t' [Ev Rest]].
      apply digest_after_some in Rest; destruct Rest as [t'' [t''' [Slice [Ev' ->]]]].
      exists t'; split; [apply eval_tree_unique; exists n; exact Ev|].
      apply digest_after_some; exists t'', t'''; repeat split; auto.
      apply eval_tree_unique; exists n; exact Ev'.
    - intros [t' [Ev Rest]]; apply digest_after_some in Rest.
      destruct Rest as [t'' [t''' [Slice [Ev' ->]]]].
      apply eval_tree_some in Ev; apply eval_tree_some in Ev'.
      destruct Ev as [n Ev]; destruct Ev' as [m Ev'].
      apply think_unique; exists (S (n+m)).
      change (obind (eval_tree_with_fuel (n+m) t) (digest_after (eval_tree_with_fuel (n+m))) = Some (HTreeObj (D.digest t'''))).
      rewrite (eval_tree_padding n m t t' Ev); simpl.
      apply digest_after_some; exists t'', t'''; repeat split; auto.
      rewrite Nat.add_comm; apply eval_tree_padding; exact Ev'.
  Qed.
  Theorem think_hs th : think th = match th with
    | Application t => obind (eval_tree t) A.apply_tree
    | Identification d => I.identify d
    | Selection t => obind (eval_tree t) (fun t' => omap (fun ref => Data (Ref ref)) (Sl.slice t'))
    | Digestion t => obind (eval_tree t) (digest_after eval_tree)
    end.
  Proof. destruct th; [apply think_application|apply think_identification|apply think_selection|apply think_digestion]. Qed.

  Lemma forces_to_data th r : forces_to th r -> exists d, r = Data d.
  Proof. intro Force; apply force_data with th; apply force_unique; exact Force. Qed.

  Lemma evals_to_not_encode h r : evals_to h r ->
    (exists d, r = Data d) \/ (exists th, r = Thunk th).
  Proof.
    intro Eval; pose proof (eval_not_encode h r (eval_unique h r Eval)) as Shape.
    destruct r; [left; eauto|right; eauto|contradiction].
  Qed.

  Lemma forces_to_implies_evals_to th b : forces_to th (HBlobObj b) ->
    evals_to (Encode (Strict th)) (HBlobObj b).
  Proof.
    intro Force; apply eval_some; rewrite eval_encode, execute_hs.
    cbn [encode_to_thunk]; rewrite (force_unique th _ Force).
    change (eval (HBlobObj b) = Some (HBlobObj b)); apply eval_blob.
  Qed.

  Lemma eval_entry_to_eval_tree xs ys : Forall2 evals_to xs ys ->
    eval (HTreeObj (create_tree xs)) = Some (HTreeObj (create_tree ys)).
  Proof.
    intro Entries; destruct (evals_to_tree_to xs ys Entries) as [n List].
    apply eval_unique; exists (S n).
    change (omap HTreeObj (omap create_tree
      (eval_list_with_fuel n (get_tree_raw (create_tree xs)))) = Some (HTreeObj (create_tree ys))).
    rewrite get_tree_raw_create_tree, List; reflexivity.
  Qed.

  (** Guarded Isabelle [the (eval x)] uses become explicit output witnesses.
      The Forall2 conclusion specifies every output, including duplicates. *)
  Lemma eval_tree_to_eval_entry t r : eval (HTreeObj t) = Some r ->
    exists ys, r = HTreeObj (create_tree ys) /\
      Forall2 (fun x y => eval x = Some y) (get_tree_raw t) ys /\
      Forall (fun x => exists y, eval x = Some y) (get_tree_raw t).
  Proof.
    intro Eval; rewrite eval_tree_handle in Eval.
    apply omap_some in Eval as [t' [Tree ->]]; apply eval_tree_some in Tree as [n Tree].
    change (omap create_tree (eval_list_with_fuel n (get_tree_raw t)) = Some t') in Tree.
    apply omap_some in Tree as [ys [Entries ->]].
    apply eval_list_to_list_all in Entries.
    assert (Outputs : Forall2 (fun x y => eval x = Some y) (get_tree_raw t) ys).
    { eapply A.Forall2_mono; [|exact Entries].
      intros x y Entry; apply eval_unique; exists n; exact Entry. }
    exists ys; split; [reflexivity|]; split; [exact Outputs|].
    clear Entries; induction Outputs; constructor; eauto.
  Qed.

  Lemma eval_list_self_fuel xs : Forall (fun h => eval h = Some h) xs ->
    exists n, eval_list_with_fuel n xs = Some xs.
  Proof.
    intro V; induction V as [|h xs H V IH].
    - exists 0; reflexivity.
    - apply eval_some in H; destruct H as [n H]; destruct IH as [m Tail].
      exists (n+m); change (obind (eval_with_fuel (n+m) h)
        (fun h' => omap (cons h') (eval_list_with_fuel (n+m) xs)) = Some (h::xs)).
      rewrite (eval_padding n m h h H); simpl.
      destruct (fuel_padding m n) as [_ [_ [_ [Pad _]]]].
      rewrite Nat.add_comm, (Pad xs xs Tail); reflexivity.
  Qed.
  Lemma value_tree_eval_to_itself t : value_tree t -> eval (HTreeObj t) = Some (HTreeObj t).
  Proof.
    revert t; refine (well_founded_induction wfp_tree_child
      (fun t => value_tree t -> eval (HTreeObj t) = Some (HTreeObj t)) _).
    intros t IH V; inversion V; subst.
    assert (Forall (fun h => eval h = Some h) (get_tree_raw t)) as Self.
    { apply Forall_forall; intros h InTree.
      assert (value_handle h) as HV by (rewrite Forall_forall in H; auto).
      destruct h as [d|th|e]; [destruct d as [[b|u]|ref]| |].
      - exact (eval_blob b).
      - apply IH.
        + exact InTree.
        + inversion HV; subst; assumption.
      - exact (eval_ref ref).
      - exact (eval_thunk th).
      - exfalso; apply (value_handle_not_encode _ HV); eauto.
    }
    destruct (eval_list_self_fuel _ Self) as [n Fuel].
    assert (eval_tree t = Some t) as Tree.
    { apply eval_tree_unique; exists n; unfold eval_tree_with_fuel, eval_tree_using.
      change (omap create_tree (eval_list_with_fuel n (get_tree_raw t)) = Some t).
      rewrite Fuel; simpl; rewrite create_tree_get_tree_raw; reflexivity. }
    rewrite eval_tree_handle, Tree; reflexivity.
  Qed.
  Theorem value_handle_eval_to_itself h : value_handle h -> eval h = Some h.
  Proof.
    intro V; inversion V; subst.
    - apply eval_blob.
    - apply value_tree_eval_to_itself; assumption.
    - apply eval_ref.
    - apply eval_thunk.
  Qed.

  Definition same_shape h1 h2 :=
    ((exists b, h1 = HBlobObj b) <-> (exists b, h2 = HBlobObj b)) /\
    ((exists t, h1 = HTreeObj t) <-> (exists t, h2 = HTreeObj t)) /\
    ((exists b, h1 = HBlobRef b) <-> (exists b, h2 = HBlobRef b)) /\
    ((exists t, h1 = HTreeRef t) <-> (exists t, h2 = HTreeRef t)) /\
    ((exists th, h1 = Thunk th) <-> (exists th, h2 = Thunk th)).
  Lemma same_shape_get_type h1 h2 : same_shape h1 h2 ->
    ~(exists e, h1 = Encode e) -> ~(exists e, h2 = Encode e) -> get_type h1 = get_type h2.
  Proof.
    intros [Blob [Tree [BRef [TRef Th]]]] N1 N2.
    destruct h1 as [[[b|t]|[b|t]]|th|e].
    - destruct (proj1 Blob (ex_intro _ b eq_refl)) as [b' ->]; reflexivity.
    - destruct (proj1 Tree (ex_intro _ t eq_refl)) as [t' ->]; reflexivity.
    - destruct (proj1 BRef (ex_intro _ b eq_refl)) as [b' ->]; reflexivity.
    - destruct (proj1 TRef (ex_intro _ t eq_refl)) as [t' ->]; reflexivity.
    - destruct (proj1 Th (ex_intro _ th eq_refl)) as [th' ->]; reflexivity.
    - exfalso; apply N1; exists e; reflexivity.
  Qed.
  Lemma Forall2_value_transform (X Y : handle -> handle -> Prop) xs ys :
    Forall2 X xs ys ->
    (forall x y, In x xs -> X x y -> value_handle x -> value_handle y -> Y x y) ->
    Forall value_handle xs -> Forall value_handle ys -> Forall2 Y xs ys.
  Proof.
    intro R; induction R; intros Safe V1 V2; [constructor|].
    inversion V1; inversion V2; subst; constructor.
    - apply Safe; simpl; auto.
    - apply IHR; auto. intros a b InTail; apply Safe; simpl; auto.
  Qed.

  Section ValueTypes.
    Variable X : handle -> handle -> Prop.
    Hypothesis tree_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> Forall2 X (get_tree_raw t1) (get_tree_raw t2).
    Hypothesis X_preserve_value_handle : forall h1 h2, X h1 h2 ->
      ~(exists e, h1 = Encode e) -> ~(exists e, h2 = Encode e) -> same_shape h1 h2.

    Lemma value_tree_to_sametypedness t1 : value_tree t1 -> forall t2,
      X (HTreeObj t1) (HTreeObj t2) -> value_tree t2 -> same_typed_tree t1 t2.
    Proof.
      revert t1; refine (well_founded_induction wfp_tree_child
        (fun t1 => value_tree t1 -> forall t2, X (HTreeObj t1) (HTreeObj t2) -> value_tree t2 -> same_typed_tree t1 t2) _).
      intros t1 IH V1 t2 R V2; apply tree.
      inversion V1 as [t Values1]; inversion V2 as [u Values2]; subst.
      eapply (Forall2_value_transform X same_typed_handle); [apply tree_cong; exact R| |exact Values1|exact Values2].
      intros x y Member HX VX VY.
      pose proof (same_shape_get_type x y
        (X_preserve_value_handle x y HX (value_handle_not_encode x VX) (value_handle_not_encode y VY))
        (value_handle_not_encode x VX) (value_handle_not_encode y VY)) as Types.
      destruct x as [[[b|t]|[b|t]]|th|e];
        destruct y as [[[b'|t']|[b'|t']]|th'|e'];
        cbn [get_type] in Types; try discriminate;
        try solve [constructor];
        try solve [exfalso; apply (value_handle_not_encode _ VX); eauto].
      apply tree_obj; apply IH; [exact Member| |exact HX|];
        inversion VX; inversion VY; subst; assumption.
    Qed.
    Lemma value_handle_to_sametypedness h1 h2 :
      X h1 h2 -> value_handle h1 -> value_handle h2 -> same_typed_handle h1 h2.
    Proof.
      intros R V1 V2.
      pose proof (same_shape_get_type h1 h2
        (X_preserve_value_handle h1 h2 R (value_handle_not_encode h1 V1) (value_handle_not_encode h2 V2))
        (value_handle_not_encode h1 V1) (value_handle_not_encode h2 V2)) as Types.
      destruct h1 as [[[b|t]|[b|t]]|th|e];
        destruct h2 as [[[b'|t']|[b'|t']]|th'|e'];
        cbn [get_type] in Types; try discriminate;
        try solve [constructor];
        try solve [exfalso; apply (value_handle_not_encode _ V1); eauto].
      apply tree_obj; apply value_tree_to_sametypedness; [|exact R|];
        inversion V1; inversion V2; subst; assumption.
    Qed.
  End ValueTypes.

  Lemma Forall2_flip (X : handle -> handle -> Prop) xs ys :
    Forall2 X xs ys -> Forall2 (fun y x => X x y) ys xs.
  Proof. induction 1; constructor; auto. Qed.
  Lemma eval_list_transport (X : handle -> handle -> Prop) n
      (eval_cong_n : forall h1 h2, X h1 h2 -> forall v1,
        eval_with_fuel n h1 = Some v1 -> exists v2, evals_to h2 v2 /\ X v1 v2)
      xs ys : Forall2 X xs ys -> forall out1,
      eval_list_with_fuel n xs = Some out1 ->
      exists m out2, eval_list_with_fuel m ys = Some out2 /\ Forall2 X out1 out2.
  Proof.
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
  Lemma eval_tree_transport (X : handle -> handle -> Prop) n
      (tree_complete : forall xs ys, Forall2 X xs ys -> X (HTreeObj (create_tree xs)) (HTreeObj (create_tree ys)))
      (eval_cong_n : forall h1 h2, X h1 h2 -> forall v1,
        eval_with_fuel n h1 = Some v1 -> exists v2, evals_to h2 v2 /\ X v1 v2)
      t1 t2 : Forall2 X (get_tree_raw t1) (get_tree_raw t2) -> forall v1,
      eval_tree_with_fuel n t1 = Some v1 ->
      exists v2, evals_tree_to t2 v2 /\ X (HTreeObj v1) (HTreeObj v2).
  Proof.
    intros Raw v1 Ev; unfold eval_tree_with_fuel, eval_tree_using in Ev.
    change (omap create_tree (eval_list_with_fuel n (get_tree_raw t1)) = Some v1) in Ev.
    apply omap_some in Ev; destruct Ev as [xs [List ->]].
    destruct (eval_list_transport X n eval_cong_n _ _ Raw xs List) as [m [ys [List2 Related]]].
    exists (create_tree ys); split; [|apply tree_complete; exact Related].
    exists m; change (omap create_tree (eval_list_with_fuel m (get_tree_raw t2)) = Some (create_tree ys)).
    rewrite List2; reflexivity.
  Qed.
  Lemma eq_tree_eval_fuel_n (X : handle -> handle -> Prop) n
      (tree_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> Forall2 X (get_tree_raw t1) (get_tree_raw t2))
      (tree_complete : forall xs ys, Forall2 X xs ys -> X (HTreeObj (create_tree xs)) (HTreeObj (create_tree ys)))
      (eval_cong_n : forall h1 h2, X h1 h2 ->
        (forall v1, eval_with_fuel n h1 = Some v1 -> exists v2, evals_to h2 v2 /\ X v1 v2) /\
        (forall v2, eval_with_fuel n h2 = Some v2 -> exists v1, evals_to h1 v1 /\ X v1 v2))
      t1 t2 : X (HTreeObj t1) (HTreeObj t2) ->
      (forall v1, eval_tree_with_fuel n t1 = Some v1 -> exists v2, evals_tree_to t2 v2 /\ X (HTreeObj v1) (HTreeObj v2)) /\
      (forall v2, eval_tree_with_fuel n t2 = Some v2 -> exists v1, evals_tree_to t1 v1 /\ X (HTreeObj v1) (HTreeObj v2)).
  Proof.
    intro R; split.
    - apply (eval_tree_transport X n tree_complete); [|apply tree_cong; exact R].
      intros h1 h2 H; exact (proj1 (eval_cong_n h1 h2 H)).
    - apply (eval_tree_transport (fun x y => X y x) n).
      + intros xs ys Related; apply tree_complete.
        apply Forall2_flip in Related; exact Related.
      + intros h2 h1 H; exact (proj2 (eval_cong_n h1 h2 H)).
      + apply Forall2_flip, tree_cong; exact R.
  Qed.

  Section ReferenceCongruence.
    Variable X : handle -> handle -> Prop.
    Hypothesis blob_ref_cong : forall b1 b2, X (HBlobObj b1) (HBlobObj b2) -> X (HBlobRef b1) (HBlobRef b2).
    Hypothesis tree_ref_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> X (HTreeRef t1) (HTreeRef t2).
    Hypothesis blob_ref_complete : forall b1 b2, X (HBlobRef b1) (HBlobRef b2) -> X (HBlobObj b1) (HBlobObj b2).
    Hypothesis tree_ref_complete : forall t1 t2, X (HTreeRef t1) (HTreeRef t2) -> X (HTreeObj t1) (HTreeObj t2).
    Hypothesis X_preserve_tree_ref : forall t h, X (HTreeRef t) h -> exists t', h = HTreeRef t'.
    Hypothesis X_preserve_blob_ref : forall b h, X (HBlobRef b) h -> exists b', h = HBlobRef b'.
    Hypothesis X_preserve_tree : forall t h, X (HTreeObj t) h -> exists t', h = HTreeObj t'.
    Hypothesis X_preserve_blob : forall b h, X (HBlobObj b) h -> exists b', h = HBlobObj b'.

    Ltac reject_shape R :=
      try solve [destruct (X_preserve_blob_ref _ _ R) as [? E]; discriminate E];
      try solve [destruct (X_preserve_tree_ref _ _ R) as [? E]; discriminate E];
      try solve [destruct (X_preserve_blob _ _ R) as [? E]; discriminate E];
      try solve [destruct (X_preserve_tree _ _ R) as [? E]; discriminate E].
    Lemma ref_to_relaxed r1 r2 : X (Data (Ref r1)) (Data (Ref r2)) -> relaxed_X X (Data (Ref r1)) (Data (Ref r2)).
    Proof.
      intro R; destruct r1, r2; cbv beta iota delta [relaxed_X lift lift_data];
        try solve [apply blob_ref_complete; exact R];
        try solve [apply tree_ref_complete; exact R]; reject_shape R.
    Qed.
    Lemma data_to_relaxed d1 d2 : X (Data d1) (Data d2) -> relaxed_X X (Data d1) (Data d2).
    Proof.
      intro R; destruct d1 as [[b1|t1]|[b1|t1]], d2 as [[b2|t2]|[b2|t2]];
        cbv beta iota delta [relaxed_X lift lift_data];
        try exact R;
        try solve [apply blob_ref_complete; exact R];
        try solve [apply tree_ref_complete; exact R]; reject_shape R.
    Qed.
    Lemma lower_to_lift d1 d2 : X (lower (Data d1)) (lower (Data d2)) -> X (lift (Data d1)) (lift (Data d2)).
    Proof.
      intro R; destruct d1 as [[b1|t1]|[b1|t1]], d2 as [[b2|t2]|[b2|t2]];
        cbv beta iota delta [lift lower lift_data lower_data] in *;
        try solve [apply blob_ref_complete; exact R];
        try solve [apply tree_ref_complete; exact R]; reject_shape R.
    Qed.
    Lemma lift_to_lower d1 d2 : X (lift (Data d1)) (lift (Data d2)) -> X (lower (Data d1)) (lower (Data d2)).
    Proof.
      intro R; destruct d1 as [[b1|t1]|[b1|t1]], d2 as [[b2|t2]|[b2|t2]];
        cbv beta iota delta [lift lower lift_data lower_data] in *;
        try solve [apply blob_ref_cong; exact R];
        try solve [apply tree_ref_cong; exact R]; reject_shape R.
    Qed.
    Lemma force_to_lift e1 e2 :
      rel_opt X (omap lower (force e1)) (omap lower (force e2)) ->
      rel_opt (relaxed_X X) (force e1) (force e2).
    Proof.
      destruct (force e1) as [h1|] eqn:F1, (force e2) as [h2|] eqn:F2;
        cbn [rel_opt omap]; try contradiction; [|auto].
      intro R; destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->].
      change (X (lift (Data d1)) (lift (Data d2))); apply lower_to_lift; exact R.
    Qed.
  End ReferenceCongruence.

  (** The original eq_forces_to_induct hypotheses, grouped for reuse by R',
      R and their equivalence closure. These are premises, not backend axioms. *)
  Definition thunk_reasons (X : handle -> handle -> Prop) t1 t2 :=
    (exists tree1 tree2, X (HTreeObj tree1) (HTreeObj tree2) /\ t1 = Application tree1 /\ t2 = Application tree2) \/
    (exists tree1 tree2, X (HTreeObj tree1) (HTreeObj tree2) /\ t1 = Selection tree1 /\ t2 = Selection tree2) \/
    (exists tree1 tree2, X (HTreeObj tree1) (HTreeObj tree2) /\ t1 = Digestion tree1 /\ t2 = Digestion tree2) \/
    (exists d1 d2, X (Data d1) (Data d2) /\ t1 = Identification d1 /\ t2 = Identification d2) \/
    (think t1 = None /\ think t2 = None) \/
    (exists r1 r2, think t1 = Some r1 /\ think t2 = Some r2 /\ strengthen X r1 r2) \/
    think t1 = Some (Thunk t2) \/ think t1 = Some (Encode (Strict t2)) \/ think t1 = Some (Encode (Shallow t2)).

  Record relation_properties (X : handle -> handle -> Prop) : Prop := {
    blob_cong : forall b1 b2, X (HBlobObj b1) (HBlobObj b2) -> get_blob_data b1 = get_blob_data b2;
    tree_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> Forall2 X (get_tree_raw t1) (get_tree_raw t2);
    blob_complete : forall d1 d2, d1 = d2 -> X (HBlobObj (create_blob d1)) (HBlobObj (create_blob d2));
    tree_complete : forall xs ys, Forall2 X xs ys -> X (HTreeObj (create_tree xs)) (HTreeObj (create_tree ys));
    blob_ref_cong : forall b1 b2, X (HBlobObj b1) (HBlobObj b2) -> X (HBlobRef b1) (HBlobRef b2);
    tree_ref_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> X (HTreeRef t1) (HTreeRef t2);
    blob_ref_complete : forall b1 b2, X (HBlobRef b1) (HBlobRef b2) -> X (HBlobObj b1) (HBlobObj b2);
    tree_ref_complete : forall t1 t2, X (HTreeRef t1) (HTreeRef t2) -> X (HTreeObj t1) (HTreeObj t2);
    strict_encode_cong : forall t1 t2, X (Thunk t1) (Thunk t2) -> X (Encode (Strict t1)) (Encode (Strict t2));
    shallow_encode_cong : forall t1 t2, X (Thunk t1) (Thunk t2) -> X (Encode (Shallow t1)) (Encode (Shallow t2));
    application_thunk_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> X (Thunk (Application t1)) (Thunk (Application t2));
    selection_thunk_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> X (Thunk (Selection t1)) (Thunk (Selection t2));
    digestion_thunk_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> X (Thunk (Digestion t1)) (Thunk (Digestion t2));
    identification_thunk_cong : forall d1 d2, X (Data d1) (Data d2) -> X (Thunk (Identification d1)) (Thunk (Identification d2));
    preserve_tree_ref : forall t h, X (HTreeRef t) h -> exists t', h = HTreeRef t';
    preserve_tree_ref_rev : forall t h, X h (HTreeRef t) -> (exists t', h = HTreeRef t') \/ (exists th, h = Encode (Shallow th));
    preserve_blob_ref : forall b h, X (HBlobRef b) h -> exists b', h = HBlobRef b';
    preserve_blob_ref_rev : forall b h, X h (HBlobRef b) -> (exists b', h = HBlobRef b') \/ (exists th, h = Encode (Shallow th));
    preserve_tree : forall t h, X (HTreeObj t) h -> exists t', h = HTreeObj t';
    preserve_tree_rev : forall t h, X h (HTreeObj t) -> (exists t', h = HTreeObj t') \/ (exists th, h = Encode (Strict th));
    preserve_blob : forall b h, X (HBlobObj b) h -> exists b', h = HBlobObj b';
    preserve_blob_rev : forall b h, X h (HBlobObj b) -> (exists b', h = HBlobObj b') \/ (exists th, h = Encode (Strict th));
    preserve_thunk : forall h1 h2, X h1 h2 -> ((exists t1, h1 = Thunk t1) <-> (exists t2, h2 = Thunk t2));
    encode_eval : forall e h, X (Encode e) h -> ~(exists e', h = Encode e') -> executes_to e h;
    strict_encode_reasons : forall t1 t2, X (Encode (Strict t1)) (Encode (Strict t2)) ->
      X (Thunk t1) (Thunk t2) \/ rel_opt X (execute (Strict t1)) (execute (Strict t2));
    shallow_encode_reasons : forall t1 t2, X (Encode (Shallow t1)) (Encode (Shallow t2)) ->
      X (Thunk t1) (Thunk t2) \/ rel_opt X (execute (Shallow t1)) (execute (Shallow t2));
    not_shallow_strict : forall t1 t2, ~X (Encode (Shallow t1)) (Encode (Strict t2));
    not_strict_shallow : forall t1 t2, ~X (Encode (Strict t1)) (Encode (Shallow t2));
    encode_reverse_absent : forall e h, X h (Encode e) -> ~(exists e', h = Encode e') -> False;
    related_thunk_reasons : forall t1 t2, X (Thunk t1) (Thunk t2) -> thunk_reasons X t1 t2;
    X_self : forall h, X h h
  }.

  Section Relation.
    Variable X : handle -> handle -> Prop.
    Variable Props : relation_properties X.
    Ltac property := first [
      exact (blob_cong X Props) |
      exact (tree_cong X Props) |
      exact (blob_complete X Props) |
      exact (tree_complete X Props) |
      exact (blob_ref_cong X Props) |
      exact (tree_ref_cong X Props) |
      exact (blob_ref_complete X Props) |
      exact (tree_ref_complete X Props) |
      exact (strict_encode_cong X Props) |
      exact (shallow_encode_cong X Props) |
      exact (application_thunk_cong X Props) |
      exact (selection_thunk_cong X Props) |
      exact (digestion_thunk_cong X Props) |
      exact (identification_thunk_cong X Props) |
      exact (preserve_tree_ref X Props) |
      exact (preserve_blob_ref X Props) |
      exact (preserve_tree X Props) |
      exact (preserve_blob X Props) |
      exact (preserve_thunk X Props) |
      exact (X_self X Props) ].

    Lemma value_shapes x y : X x y -> ~(exists e, x = Encode e) -> ~(exists e, y = Encode e) -> same_shape x y.
    Proof.
      intros R Nx Ny; unfold same_shape.
      repeat split; intros [v E]; subst.
      - eapply preserve_blob; eauto.
      - destruct (preserve_blob_rev X Props _ _ R) as [H|[th E]]; [exact H|exfalso; apply Nx; eauto].
      - eapply preserve_tree; eauto.
      - destruct (preserve_tree_rev X Props _ _ R) as [H|[th E]]; [exact H|exfalso; apply Nx; eauto].
      - eapply preserve_blob_ref; eauto.
      - destruct (preserve_blob_ref_rev X Props _ _ R) as [H|[th E]]; [exact H|exfalso; apply Nx; eauto].
      - eapply preserve_tree_ref; eauto.
      - destruct (preserve_tree_ref_rev X Props _ _ R) as [H|[th E]]; [exact H|exfalso; apply Nx; eauto].
      - apply (proj1 (preserve_thunk X Props _ _ R)); eauto.
      - apply (proj2 (preserve_thunk X Props _ _ R)); eauto.
    Qed.
    Lemma values_same_type x y : X x y -> value_handle x -> value_handle y -> same_typed_handle x y.
    Proof. apply (value_handle_to_sametypedness X (tree_cong X Props) value_shapes). Qed.
    Lemma related_data_strengthen d1 d2 : X (Data d1) (Data d2) -> strengthen X (Data d1) (Data d2).
    Proof.
      apply (data_to_relaxed X (blob_ref_complete X Props) (tree_ref_complete X Props)
        (preserve_tree_ref X Props) (preserve_blob_ref X Props) (preserve_tree X Props) (preserve_blob X Props)).
    Qed.
    Lemma strict_force_to_relaxed t1 t2 :
      rel_opt X (omap lift (force t1)) (omap lift (force t2)) -> rel_opt (relaxed_X X) (force t1) (force t2).
    Proof.
      destruct (force t1) as [h1|] eqn:F1, (force t2) as [h2|] eqn:F2;
        cbn [rel_opt omap]; try contradiction; [|auto].
      intro R; destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->]; exact R.
    Qed.
    Lemma strict_strengthen t1 t2 : X (Encode (Strict t1)) (Encode (Strict t2)) -> strengthen X (Encode (Strict t1)) (Encode (Strict t2)).
    Proof.
      intro R; destruct (strict_encode_reasons X Props _ _ R) as [Thunks|Exec].
      - left; exact Thunks.
      - right; rewrite !execute_hs in Exec; apply strict_force_to_relaxed; exact Exec.
    Qed.
    Lemma shallow_strengthen t1 t2 : X (Encode (Shallow t1)) (Encode (Shallow t2)) -> strengthen X (Encode (Shallow t1)) (Encode (Shallow t2)).
    Proof.
      intro R; destruct (shallow_encode_reasons X Props _ _ R) as [Thunks|Exec].
      - left; exact Thunks.
      - right; rewrite !execute_hs in Exec.
        eapply force_to_lift; [exact (blob_ref_complete X Props)|exact (tree_ref_complete X Props)|
          exact (preserve_tree_ref X Props)|exact (preserve_blob_ref X Props)|exact Exec].
    Qed.
    Lemma related_typed_strengthen h1 h2 : X h1 h2 -> same_typed_handle h1 h2 -> strengthen X h1 h2.
    Proof.
      intros R Typed; inversion Typed; subst.
      - apply related_data_strengthen; exact R.
      - apply related_data_strengthen; exact R.
      - apply related_data_strengthen; exact R.
      - apply related_data_strengthen; exact R.
      - exact R.
      - apply shallow_strengthen; exact R.
      - apply strict_strengthen; exact R.
    Qed.
    Lemma apply_tree_values t1 t2 : X (HTreeObj t1) (HTreeObj t2) ->
      value_handle (HTreeObj t1) -> value_handle (HTreeObj t2) ->
      rel_opt X (A.apply_tree t1) (A.apply_tree t2) /\ rel_opt same_typed_handle (A.apply_tree t1) (A.apply_tree t2).
    Proof.
      intros R V1 V2; eapply (A.apply_tree_X X); try solve [property].
      split; [exact R|apply values_same_type; assumption].
    Qed.
    Lemma apply_tree_strengthen t1 t2 : X (HTreeObj t1) (HTreeObj t2) ->
      value_handle (HTreeObj t1) -> value_handle (HTreeObj t2) ->
      rel_opt (strengthen X) (A.apply_tree t1) (A.apply_tree t2).
    Proof.
      intros R V1 V2; destruct (apply_tree_values _ _ R V1 V2) as [Related Typed].
      destruct (A.apply_tree t1), (A.apply_tree t2); simpl in *; try contradiction; [|exact I].
      apply related_typed_strengthen; assumption.
    Qed.

    Lemma slice_values t1 t2 : X (HTreeObj t1) (HTreeObj t2) ->
      Forall not_encode (get_tree_raw t1) -> Forall not_encode (get_tree_raw t2) ->
      rel_opt (fun r1 r2 => X (Data (Ref r1)) (Data (Ref r2))) (Sl.slice t1) (Sl.slice t2).
    Proof.
      intros R N1 N2; eapply (Sl.slice_X X); try solve [property].
      repeat split; assumption.
    Qed.
    Lemma digest_values t1 t2 : X (HTreeObj t1) (HTreeObj t2) ->
      Forall not_encode (get_tree_raw t1) -> Forall not_encode (get_tree_raw t2) ->
      X (HTreeObj (D.digest t1)) (HTreeObj (D.digest t2)).
    Proof.
      intros R N1 N2; eapply (D.digest_X X); try solve [property].
      repeat split; assumption.
    Qed.

    Definition eval_transport n := forall h1 h2, X h1 h2 ->
      (forall v1, eval_with_fuel n h1 = Some v1 -> exists v2, evals_to h2 v2 /\ X v1 v2) /\
      (forall v2, eval_with_fuel n h2 = Some v2 -> exists v1, evals_to h1 v1 /\ X v1 v2).
    Definition force_transport n := forall t1 t2, X (Thunk t1) (Thunk t2) ->
      (forall v1, force_with_fuel n t1 = Some v1 -> exists v2, forces_to t2 v2 /\ strengthen X v1 v2) /\
      (forall v2, force_with_fuel n t2 = Some v2 -> exists v1, forces_to t1 v1 /\ strengthen X v1 v2).
    Definition think_transport n := forall t1 t2, X (Thunk t1) (Thunk t2) ->
      (forall v1, think_with_fuel n t1 = Some v1 ->
        (exists v2, thinks_to t2 v2 /\ strengthen X v1 v2) \/ v1 = Thunk t2 \/ v1 = Encode (Strict t2) \/ v1 = Encode (Shallow t2)) /\
      (forall v2, think_with_fuel n t2 = Some v2 ->
        (exists v1, thinks_to t1 v1 /\ strengthen X v1 v2) \/ thinks_to t1 (Thunk t2) \/ thinks_to t1 (Encode (Strict t2)) \/ thinks_to t1 (Encode (Shallow t2))).
    Definition force_reply h := match h with
      | Data _ => Some h | Thunk th => force th | Encode e => force (encode_to_thunk e) end.
    Lemma strengthen_self h : strengthen X h h.
    Proof. destruct h; [apply (X_self X Props)|apply (X_self X Props)|left; apply (X_self X Props)]. Qed.
    Lemma data_not_thunk d th : ~X (Data d) (Thunk th).
    Proof.
      intro R; destruct (proj2 (preserve_thunk X Props _ _ R) (ex_intro _ th eq_refl)) as [u E]; discriminate.
    Qed.
    Lemma thunk_not_data th d : ~X (Thunk th) (Data d).
    Proof.
      intro R; destruct (proj1 (preserve_thunk X Props _ _ R) (ex_intro _ th eq_refl)) as [u E]; discriminate.
    Qed.
    Lemma data_not_encode d e : ~X (Data d) (Encode e).
    Proof. intro R; apply (encode_reverse_absent X Props e (Data d) R); intros [e' E]; discriminate. Qed.

    Lemma force_reply_forward n (IH : force_transport n) h1 h2 v1 :
      strengthen X h1 h2 -> force_after n h1 = Some v1 ->
      exists v2, force_reply h2 = Some v2 /\ strengthen X v1 v2.
    Proof.
      intros Related Fuel; destruct h1 as [d1|t1|e1], h2 as [d2|t2|e2].
      - inversion Fuel; subst; exists (Data d2); split; [reflexivity|exact Related].
      - exfalso; eapply data_not_thunk; exact Related.
      - exfalso; eapply data_not_encode; exact Related.
      - exfalso; eapply thunk_not_data; exact Related.
      - destruct (proj1 (IH t1 t2 Related) v1 Fuel) as [v2 [F2 Strength]].
        exists v2; split; [apply force_unique; exact F2|exact Strength].
      - destruct (proj1 (IH t1 (encode_to_thunk e2) Related) v1 Fuel) as [v2 [F2 Strength]].
        exists v2; split; [apply force_unique; exact F2|exact Strength].
      - assert (X (Encode (Strict (encode_to_thunk e1))) (lift (Data d2))) as ExecRel by
          (destruct e1; exact Related).
        pose proof (execute_unique _ _ (encode_eval X Props _ _ ExecRel ltac:(intros [e E]; discriminate))) as Exec.
        rewrite execute_hs in Exec; simpl in Exec.
        assert (force (encode_to_thunk e1) = Some v1) as F1 by
          (apply force_unique; exists n; destruct e1; exact Fuel).
        rewrite F1 in Exec; inversion Exec as [Lift].
        destruct (force_data _ _ F1) as [d ->].
        exists (Data d2); split; [reflexivity|].
        change (X (Data (lift_data d)) (Data (lift_data d2))); rewrite <- Lift; apply (X_self X Props).
      - destruct (proj1 (IH (encode_to_thunk e1) t2 Related) v1 ltac:(destruct e1; exact Fuel)) as [v2 [F2 Strength]].
        exists v2; split; [apply force_unique; exact F2|exact Strength].
      - destruct Related as [Thunks|Forces].
        + destruct (proj1 (IH (encode_to_thunk e1) (encode_to_thunk e2) Thunks) v1 ltac:(destruct e1; exact Fuel)) as [v2 [F2 Strength]].
          exists v2; split; [apply force_unique; exact F2|exact Strength].
        + assert (force (encode_to_thunk e1) = Some v1) as F1 by
            (apply force_unique; exists n; destruct e1; exact Fuel).
          rewrite F1 in Forces; destruct (force (encode_to_thunk e2)) as [v2|] eqn:F2; [|contradiction].
          destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->].
          exists (Data d2); split; [exact F2|exact Forces].
    Qed.
    Lemma force_reply_backward n (IH : force_transport n) h1 h2 v2 :
      strengthen X h1 h2 -> force_after n h2 = Some v2 ->
      exists v1, force_reply h1 = Some v1 /\ strengthen X v1 v2.
    Proof.
      intros Related Fuel; destruct h1 as [d1|t1|e1], h2 as [d2|t2|e2].
      - inversion Fuel; subst; exists (Data d1); split; [reflexivity|exact Related].
      - exfalso; eapply data_not_thunk; exact Related.
      - exfalso; eapply data_not_encode; exact Related.
      - exfalso; eapply thunk_not_data; exact Related.
      - destruct (proj2 (IH t1 t2 Related) v2 Fuel) as [v1 [F1 Strength]].
        exists v1; split; [apply force_unique; exact F1|exact Strength].
      - destruct (proj2 (IH t1 (encode_to_thunk e2) Related) v2 ltac:(destruct e2; exact Fuel)) as [v1 [F1 Strength]].
        exists v1; split; [apply force_unique; exact F1|exact Strength].
      - inversion Fuel; subst v2.
        assert (X (Encode (Strict (encode_to_thunk e1))) (lift (Data d2))) as ExecRel by
          (destruct e1; exact Related).
        pose proof (execute_unique _ _ (encode_eval X Props _ _ ExecRel ltac:(intros [e E]; discriminate))) as Exec.
        rewrite execute_hs in Exec; simpl in Exec; apply omap_some in Exec.
        destruct Exec as [v1 [F1 Lift]].
        destruct (force_data _ _ F1) as [d ->].
        cbn [lift] in Lift.
        exists (Data d); split; [exact F1|].
        change (X (Data (lift_data d)) (Data (lift_data d2))); rewrite <- Lift; apply (X_self X Props).
      - destruct (proj2 (IH (encode_to_thunk e1) t2 Related) v2 Fuel) as [v1 [F1 Strength]].
        exists v1; split; [apply force_unique; exact F1|exact Strength].
      - destruct Related as [Thunks|Forces].
        + destruct (proj2 (IH (encode_to_thunk e1) (encode_to_thunk e2) Thunks) v2 ltac:(destruct e2; exact Fuel)) as [v1 [F1 Strength]].
          exists v1; split; [apply force_unique; exact F1|exact Strength].
        + assert (force (encode_to_thunk e2) = Some v2) as F2 by
            (apply force_unique; exists n; destruct e2; exact Fuel).
          rewrite F2 in Forces; destruct (force (encode_to_thunk e1)) as [v1|] eqn:F1; [|contradiction].
          destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->].
          exists (Data d1); split; [exact F1|exact Forces].
    Qed.
    Lemma force_reply_some th h v : thinks_to th h -> force_reply h = Some v -> forces_to th v.
    Proof.
      intros Think Reply; apply force_some; rewrite force_hs.
      rewrite (think_unique _ _ Think); exact Reply.
    Qed.
    Theorem force_step n : think_transport n -> force_transport n -> force_transport (S n).
    Proof.
      intros ThinkIH ForceIH t1 t2 R; split.
      - intros v1 Fuel.
        change (obind (think_with_fuel n t1) (force_after n) = Some v1) in Fuel.
        apply obind_some in Fuel; destruct Fuel as [r1 [Think1 Reply1]].
        destruct (proj1 (ThinkIH t1 t2 R) r1 Think1) as
          [[r2 [Think2 Strength]]|[OneThunk|[OneStrict|OneShallow]]].
        + destruct (force_reply_forward n ForceIH r1 r2 v1 Strength Reply1) as [v2 [Reply2 Result]].
          exists v2; split; [eapply force_reply_some; eauto|exact Result].
        + subst r1; exists v1; split; [exists n; exact Reply1|apply strengthen_self].
        + subst r1; exists v1; split; [exists n; exact Reply1|apply strengthen_self].
        + subst r1; exists v1; split; [exists n; exact Reply1|apply strengthen_self].
      - intros v2 Fuel.
        change (obind (think_with_fuel n t2) (force_after n) = Some v2) in Fuel.
        apply obind_some in Fuel; destruct Fuel as [r2 [Think2 Reply2]].
        destruct (proj2 (ThinkIH t1 t2 R) r2 Think2) as
          [[r1 [Think1 Strength]]|[OneThunk|[OneStrict|OneShallow]]].
        + destruct (force_reply_backward n ForceIH r1 r2 v2 Strength Reply2) as [v1 [Reply1 Result]].
          exists v1; split; [eapply force_reply_some; eauto|exact Result].
        + exists v2; split; [|apply strengthen_self].
          eapply force_reply_some; [exact OneThunk|].
          apply force_unique; exists (S n).
          change (obind (think_with_fuel n t2) (force_after n) = Some v2); rewrite Think2; exact Reply2.
        + exists v2; split; [|apply strengthen_self].
          eapply force_reply_some; [exact OneStrict|].
          apply force_unique; exists (S n).
          change (obind (think_with_fuel n t2) (force_after n) = Some v2); rewrite Think2; exact Reply2.
        + exists v2; split; [|apply strengthen_self].
          eapply force_reply_some; [exact OneShallow|].
          apply force_unique; exists (S n).
          change (obind (think_with_fuel n t2) (force_after n) = Some v2); rewrite Think2; exact Reply2.
    Qed.
    Section BoundedThink.
      Variable n : nat.
      Hypothesis IH_eval : eval_transport n.

      Lemma tree_forward t1 t2 v1 : X (HTreeObj t1) (HTreeObj t2) ->
        eval_tree_with_fuel n t1 = Some v1 ->
        exists v2, eval_tree t2 = Some v2 /\ X (HTreeObj v1) (HTreeObj v2).
      Proof.
        intros R Ev; destruct (proj1 (eq_tree_eval_fuel_n X n (tree_cong X Props)
          (tree_complete X Props) IH_eval t1 t2 R) v1 Ev) as [v2 [Ev2 Related]].
        exists v2; split; [apply eval_tree_unique; exact Ev2|exact Related].
      Qed.
      Lemma tree_backward t1 t2 v2 : X (HTreeObj t1) (HTreeObj t2) ->
        eval_tree_with_fuel n t2 = Some v2 ->
        exists v1, eval_tree t1 = Some v1 /\ X (HTreeObj v1) (HTreeObj v2).
      Proof.
        intros R Ev; destruct (proj2 (eq_tree_eval_fuel_n X n (tree_cong X Props)
          (tree_complete X Props) IH_eval t1 t2 R) v2 Ev) as [v1 [Ev1 Related]].
        exists v1; split; [apply eval_tree_unique; exact Ev1|exact Related].
      Qed.
      Lemma application_think_forward t1 t2 r1 : X (HTreeObj t1) (HTreeObj t2) ->
        think_with_fuel (S n) (Application t1) = Some r1 ->
        exists r2, thinks_to (Application t2) r2 /\ strengthen X r1 r2.
      Proof.
        intros R Ev; change (obind (eval_tree_with_fuel n t1) A.apply_tree = Some r1) in Ev.
        apply obind_some in Ev; destruct Ev as [v1 [Fuel1 App1]].
        destruct (tree_forward t1 t2 v1 R Fuel1) as [v2 [Tree2 Related]].
        assert (eval_tree t1 = Some v1) as Tree1 by (apply eval_tree_unique; exists n; exact Fuel1).
        pose proof (apply_tree_strengthen v1 v2 Related
          (eval_tree_to_value_handle _ _ Tree1) (eval_tree_to_value_handle _ _ Tree2)) as App.
        rewrite App1 in App; destruct (A.apply_tree v2) as [r2|] eqn:App2; [|contradiction].
        exists r2; split; [|exact App].
        apply think_some; rewrite think_application, Tree2; exact App2.
      Qed.
      Lemma selection_think_forward t1 t2 r1 : X (HTreeObj t1) (HTreeObj t2) ->
        think_with_fuel (S n) (Selection t1) = Some r1 ->
        exists r2, thinks_to (Selection t2) r2 /\ strengthen X r1 r2.
      Proof.
        intros R Ev; change (obind (eval_tree_with_fuel n t1)
          (fun t' => omap (fun ref => Data (Ref ref)) (Sl.slice t')) = Some r1) in Ev.
        apply obind_some in Ev; destruct Ev as [v1 [Fuel1 Slice1]].
        apply omap_some in Slice1; destruct Slice1 as [ref1 [Slice1 ->]].
        destruct (tree_forward t1 t2 v1 R Fuel1) as [v2 [Tree2 Related]].
        assert (eval_tree t1 = Some v1) as Tree1 by (apply eval_tree_unique; exists n; exact Fuel1).
        pose proof (slice_values v1 v2 Related (eval_tree_to_not_encode _ _ Tree1)
          (eval_tree_to_not_encode _ _ Tree2)) as Slices.
        rewrite Slice1 in Slices; destruct (Sl.slice v2) as [ref2|] eqn:Slice2; [|contradiction].
        exists (Data (Ref ref2)); split.
        - apply think_some; rewrite think_selection, Tree2; simpl; rewrite Slice2; reflexivity.
        - apply related_data_strengthen; exact Slices.
      Qed.
      Lemma identification_think_forward d1 d2 r1 : X (Data d1) (Data d2) ->
        think_with_fuel (S n) (Identification d1) = Some r1 ->
        exists r2, thinks_to (Identification d2) r2 /\ strengthen X r1 r2.
      Proof.
        intros R Ev; change (Some (Data d1) = Some r1) in Ev; inversion Ev; subst.
        exists (Data d2); split; [exists 1; reflexivity|apply related_data_strengthen; exact R].
      Qed.
      Lemma application_think_backward t1 t2 r2 : X (HTreeObj t1) (HTreeObj t2) ->
        think_with_fuel (S n) (Application t2) = Some r2 ->
        exists r1, thinks_to (Application t1) r1 /\ strengthen X r1 r2.
      Proof.
        intros R Ev; change (obind (eval_tree_with_fuel n t2) A.apply_tree = Some r2) in Ev.
        apply obind_some in Ev; destruct Ev as [v2 [Fuel2 App2]].
        destruct (tree_backward t1 t2 v2 R Fuel2) as [v1 [Tree1 Related]].
        assert (eval_tree t2 = Some v2) as Tree2 by (apply eval_tree_unique; exists n; exact Fuel2).
        pose proof (apply_tree_strengthen v1 v2 Related
          (eval_tree_to_value_handle _ _ Tree1) (eval_tree_to_value_handle _ _ Tree2)) as App.
        rewrite App2 in App; destruct (A.apply_tree v1) as [r1|] eqn:App1; [|contradiction].
        exists r1; split; [|exact App].
        apply think_some; rewrite think_application, Tree1; exact App1.
      Qed.
      Lemma selection_think_backward t1 t2 r2 : X (HTreeObj t1) (HTreeObj t2) ->
        think_with_fuel (S n) (Selection t2) = Some r2 ->
        exists r1, thinks_to (Selection t1) r1 /\ strengthen X r1 r2.
      Proof.
        intros R Ev; change (obind (eval_tree_with_fuel n t2)
          (fun t' => omap (fun ref => Data (Ref ref)) (Sl.slice t')) = Some r2) in Ev.
        apply obind_some in Ev; destruct Ev as [v2 [Fuel2 Slice2]].
        apply omap_some in Slice2; destruct Slice2 as [ref2 [Slice2 ->]].
        destruct (tree_backward t1 t2 v2 R Fuel2) as [v1 [Tree1 Related]].
        assert (eval_tree t2 = Some v2) as Tree2 by (apply eval_tree_unique; exists n; exact Fuel2).
        pose proof (slice_values v1 v2 Related (eval_tree_to_not_encode _ _ Tree1)
          (eval_tree_to_not_encode _ _ Tree2)) as Slices.
        rewrite Slice2 in Slices; destruct (Sl.slice v1) as [ref1|] eqn:Slice1; [|contradiction].
        exists (Data (Ref ref1)); split.
        - apply think_some; rewrite think_selection, Tree1; simpl; rewrite Slice1; reflexivity.
        - apply related_data_strengthen; exact Slices.
      Qed.
      Lemma identification_think_backward d1 d2 r2 : X (Data d1) (Data d2) ->
        think_with_fuel (S n) (Identification d2) = Some r2 ->
        exists r1, thinks_to (Identification d1) r1 /\ strengthen X r1 r2.
      Proof.
        intros R Ev; change (Some (Data d2) = Some r2) in Ev; inversion Ev; subst.
        exists (Data d1); split; [exists 1; reflexivity|apply related_data_strengthen; exact R].
      Qed.
      Lemma digestion_think_forward t1 t2 r1 : X (HTreeObj t1) (HTreeObj t2) ->
        think_with_fuel (S n) (Digestion t1) = Some r1 ->
        exists r2, thinks_to (Digestion t2) r2 /\ strengthen X r1 r2.
      Proof.
        intros R Ev; change (obind (eval_tree_with_fuel n t1) (digest_after (eval_tree_with_fuel n)) = Some r1) in Ev.
        apply obind_some in Ev; destruct Ev as [v1 [Fuel1 Rest]].
        apply digest_after_some in Rest; destruct Rest as [u1 [w1 [Slice1 [FuelW1 ->]]]].
        destruct (tree_forward t1 t2 v1 R Fuel1) as [v2 [Tree2 Related]].
        assert (eval_tree t1 = Some v1) as Tree1 by (apply eval_tree_unique; exists n; exact Fuel1).
        pose proof (slice_values v1 v2 Related (eval_tree_to_not_encode _ _ Tree1)
          (eval_tree_to_not_encode _ _ Tree2)) as Slices.
        rewrite Slice1 in Slices; destruct (Sl.slice v2) as [ref2|] eqn:Slice2; [|contradiction].
        destruct (preserve_tree_ref X Props u1 (Data (Ref ref2)) Slices) as [u2 EqRef].
        inversion EqRef; subst ref2.
        destruct (tree_forward u1 u2 w1 (tree_ref_complete X Props u1 u2 Slices) FuelW1) as [w2 [EvalW2 RelatedW]].
        assert (eval_tree u1 = Some w1) as EvalW1 by (apply eval_tree_unique; exists n; exact FuelW1).
        exists (HTreeObj (D.digest w2)); split.
        - apply think_some; rewrite think_digestion, Tree2.
          apply digest_after_some; exists u2, w2; repeat split; auto.
        - apply related_data_strengthen, digest_values; [exact RelatedW|
            exact (eval_tree_to_not_encode _ _ EvalW1)|exact (eval_tree_to_not_encode _ _ EvalW2)].
      Qed.
      Lemma digestion_think_backward t1 t2 r2 : X (HTreeObj t1) (HTreeObj t2) ->
        think_with_fuel (S n) (Digestion t2) = Some r2 ->
        exists r1, thinks_to (Digestion t1) r1 /\ strengthen X r1 r2.
      Proof.
        intros R Ev; change (obind (eval_tree_with_fuel n t2) (digest_after (eval_tree_with_fuel n)) = Some r2) in Ev.
        apply obind_some in Ev; destruct Ev as [v2 [Fuel2 Rest]].
        apply digest_after_some in Rest; destruct Rest as [u2 [w2 [Slice2 [FuelW2 ->]]]].
        destruct (tree_backward t1 t2 v2 R Fuel2) as [v1 [Tree1 Related]].
        assert (eval_tree t2 = Some v2) as Tree2 by (apply eval_tree_unique; exists n; exact Fuel2).
        pose proof (slice_values v1 v2 Related (eval_tree_to_not_encode _ _ Tree1)
          (eval_tree_to_not_encode _ _ Tree2)) as Slices.
        rewrite Slice2 in Slices; destruct (Sl.slice v1) as [ref1|] eqn:Slice1; [|contradiction].
        destruct (preserve_tree_ref_rev X Props u2 (Data (Ref ref1)) Slices) as [[u1 EqRef]|[th Impossible]];
          [inversion EqRef; subst ref1|discriminate Impossible].
        destruct (tree_backward u1 u2 w2 (tree_ref_complete X Props u1 u2 Slices) FuelW2) as [w1 [EvalW1 RelatedW]].
        assert (eval_tree u2 = Some w2) as EvalW2 by (apply eval_tree_unique; exists n; exact FuelW2).
        exists (HTreeObj (D.digest w1)); split.
        - apply think_some; rewrite think_digestion, Tree1.
          apply digest_after_some; exists u1, w1; repeat split; auto.
        - apply related_data_strengthen, digest_values; [exact RelatedW|
            exact (eval_tree_to_not_encode _ _ EvalW1)|exact (eval_tree_to_not_encode _ _ EvalW2)].
      Qed.
      Theorem think_forward_from_eval t1 t2 r1 : X (Thunk t1) (Thunk t2) ->
        think_with_fuel (S n) t1 = Some r1 ->
        (exists r2, thinks_to t2 r2 /\ strengthen X r1 r2) \/
        r1 = Thunk t2 \/ r1 = Encode (Strict t2) \/ r1 = Encode (Shallow t2).
      Proof.
        intros R Fuel.
        destruct (related_thunk_reasons X Props t1 t2 R) as
          [App|[Sel|[Dig|[Ident|[None|[Related|[OneThunk|[OneStrict|OneShallow]]]]]]]].
        - destruct App as [a [b [Trees [-> ->]]]]; left; eapply application_think_forward; eauto.
        - destruct Sel as [a [b [Trees [-> ->]]]]; left; eapply selection_think_forward; eauto.
        - destruct Dig as [a [b [Trees [-> ->]]]]; left; eapply digestion_think_forward; eauto.
        - destruct Ident as [a [b [Data [-> ->]]]]; left; eapply identification_think_forward; eauto.
        - destruct None as [T1 T2].
          pose proof (think_unique t1 r1 (ex_intro _ (S n) Fuel)); congruence.
        - destruct Related as [v1 [v2 [T1 [T2 Strength]]]].
          pose proof (think_unique t1 r1 (ex_intro _ (S n) Fuel)) as Actual.
          rewrite T1 in Actual; inversion Actual; subst v1.
          left; exists v2; split; [apply think_some; exact T2|exact Strength].
        - pose proof (think_unique t1 r1 (ex_intro _ (S n) Fuel)); right; left; congruence.
        - pose proof (think_unique t1 r1 (ex_intro _ (S n) Fuel)); right; right; left; congruence.
        - pose proof (think_unique t1 r1 (ex_intro _ (S n) Fuel)); right; right; right; congruence.
      Qed.
      Theorem think_backward_from_eval t1 t2 r2 : X (Thunk t1) (Thunk t2) ->
        think_with_fuel (S n) t2 = Some r2 ->
        (exists r1, thinks_to t1 r1 /\ strengthen X r1 r2) \/
        thinks_to t1 (Thunk t2) \/ thinks_to t1 (Encode (Strict t2)) \/ thinks_to t1 (Encode (Shallow t2)).
      Proof.
        intros R Fuel.
        destruct (related_thunk_reasons X Props t1 t2 R) as
          [App|[Sel|[Dig|[Ident|[None|[Related|[OneThunk|[OneStrict|OneShallow]]]]]]]].
        - destruct App as [a [b [Trees [-> ->]]]]; left; eapply application_think_backward; eauto.
        - destruct Sel as [a [b [Trees [-> ->]]]]; left; eapply selection_think_backward; eauto.
        - destruct Dig as [a [b [Trees [-> ->]]]]; left; eapply digestion_think_backward; eauto.
        - destruct Ident as [a [b [Data [-> ->]]]]; left; eapply identification_think_backward; eauto.
        - destruct None as [T1 T2].
          pose proof (think_unique t2 r2 (ex_intro _ (S n) Fuel)); congruence.
        - destruct Related as [v1 [v2 [T1 [T2 Strength]]]].
          pose proof (think_unique t2 r2 (ex_intro _ (S n) Fuel)) as Actual.
          rewrite T2 in Actual; inversion Actual; subst v2.
          left; exists v1; split; [apply think_some; exact T1|exact Strength].
        - right; left; apply think_some; exact OneThunk.
        - right; right; left; apply think_some; exact OneStrict.
        - right; right; right; apply think_some; exact OneShallow.
      Qed.
    End BoundedThink.
    Theorem think_step n : eval_transport n -> think_transport (S n).
    Proof.
      intros IH t1 t2 Related; split.
      - intros r Fuel; exact (think_forward_from_eval n IH t1 t2 r Related Fuel).
      - intros r Fuel; exact (think_backward_from_eval n IH t1 t2 r Related Fuel).
    Qed.
    Definition execute_transport n e1 e2 :=
      (forall v1, execute_with_fuel n e1 = Some v1 -> exists v2, executes_to e2 v2 /\ X v1 v2) /\
      (forall v2, execute_with_fuel n e2 = Some v2 -> exists v1, executes_to e1 v1 /\ X v1 v2).
    Lemma execute_fuel_strict n th : execute_with_fuel n (Strict th) = omap lift (force_with_fuel n th).
    Proof. destruct n; reflexivity. Qed.
    Lemma execute_fuel_shallow n th : execute_with_fuel n (Shallow th) = omap lower (force_with_fuel n th).
    Proof. destruct n; reflexivity. Qed.
    Lemma lifted_force_results h1 h2 :
      (exists d1, h1 = Data d1) -> (exists d2, h2 = Data d2) ->
      strengthen X h1 h2 -> X (lift h1) (lift h2) /\ X (lower h1) (lower h2).
    Proof.
      intros [d1 ->] [d2 ->] R; split; [exact R|].
      apply (lift_to_lower X (blob_ref_cong X Props) (tree_ref_cong X Props)
        (preserve_tree X Props) (preserve_blob X Props)); exact R.
    Qed.
    Lemma execute_related_options n e1 e2 :
      rel_opt X (execute e1) (execute e2) -> execute_transport n e1 e2.
    Proof.
      intro Related; split; intros v Fuel.
      - pose proof (execute_unique _ _ (ex_intro _ n Fuel)) as Exec.
        rewrite Exec in Related; destruct (execute e2) as [v2|] eqn:Exec2; [|contradiction].
        exists v2; split; [apply execute_some; exact Exec2|exact Related].
      - pose proof (execute_unique _ _ (ex_intro _ n Fuel)) as Exec.
        rewrite Exec in Related; destruct (execute e1) as [v1|] eqn:Exec1; [|contradiction].
        exists v1; split; [apply execute_some; exact Exec1|exact Related].
    Qed.
    Lemma execute_strict_transport n (IH : force_transport n) t1 t2 :
      X (Encode (Strict t1)) (Encode (Strict t2)) -> execute_transport n (Strict t1) (Strict t2).
    Proof.
      intro Related; destruct (strict_encode_reasons X Props _ _ Related) as [Thunks|Exec].
      2: apply execute_related_options; exact Exec.
      split; intros v Fuel; rewrite execute_fuel_strict in Fuel; apply omap_some in Fuel;
        destruct Fuel as [h [Force ->]].
      - destruct (proj1 (IH t1 t2 Thunks) h Force) as [h2 [F2 Strength]].
        pose proof (proj1 (lifted_force_results h h2 (force_with_fuel_to_data _ _ _ Force)
          (force_data _ _ (force_unique _ _ F2)) Strength)) as Result.
        exists (lift h2); split; [apply execute_some; rewrite execute_hs; cbn; rewrite (force_unique _ _ F2); reflexivity|exact Result].
      - destruct (proj2 (IH t1 t2 Thunks) h Force) as [h1 [F1 Strength]].
        destruct F1 as [k F1].
        pose proof (proj1 (lifted_force_results h1 h (force_with_fuel_to_data _ _ _ F1)
          (force_with_fuel_to_data _ _ _ Force) Strength)) as Result.
        exists (lift h1); split; [exists k; rewrite execute_fuel_strict, F1; reflexivity|exact Result].
    Qed.
    Lemma execute_shallow_transport n (IH : force_transport n) t1 t2 :
      X (Encode (Shallow t1)) (Encode (Shallow t2)) -> execute_transport n (Shallow t1) (Shallow t2).
    Proof.
      intro Related; destruct (shallow_encode_reasons X Props _ _ Related) as [Thunks|Exec].
      2: apply execute_related_options; exact Exec.
      split; intros v Fuel; rewrite execute_fuel_shallow in Fuel; apply omap_some in Fuel;
        destruct Fuel as [h [Force ->]].
      - destruct (proj1 (IH t1 t2 Thunks) h Force) as [h2 [F2 Strength]].
        pose proof (proj2 (lifted_force_results h h2 (force_with_fuel_to_data _ _ _ Force)
          (force_data _ _ (force_unique _ _ F2)) Strength)) as Result.
        exists (lower h2); split; [apply execute_some; rewrite execute_hs; cbn; rewrite (force_unique _ _ F2); reflexivity|exact Result].
      - destruct (proj2 (IH t1 t2 Thunks) h Force) as [h1 [F1 Strength]].
        destruct F1 as [k F1].
        pose proof (proj2 (lifted_force_results h1 h (force_with_fuel_to_data _ _ _ F1)
          (force_with_fuel_to_data _ _ _ Force) Strength)) as Result.
        exists (lower h1); split; [exists k; rewrite execute_fuel_shallow, F1; reflexivity|exact Result].
    Qed.
    Lemma execute_encodes_transport n (IH : force_transport n) e1 e2 :
      X (Encode e1) (Encode e2) -> execute_transport n e1 e2.
    Proof.
      destruct e1 as [t1|t1], e2 as [t2|t2]; intro Related.
      - apply execute_strict_transport; assumption.
      - exfalso; exact (not_strict_shallow X Props t1 t2 Related).
      - exfalso; exact (not_shallow_strict X Props t1 t2 Related).
      - apply execute_shallow_transport; assumption.
    Qed.
    Lemma eval_after_execute e h v : executes_to e h -> evals_to h v -> evals_to (Encode e) v.
    Proof.
      intros Exec Ev; apply eval_some; rewrite eval_encode, (execute_unique _ _ Exec).
      exact (eval_unique _ _ Ev).
    Qed.
    Lemma eval_encodes_transport n (IH_force : force_transport n) (IH_eval : eval_transport n) e1 e2 :
      X (Encode e1) (Encode e2) ->
      (forall v1, eval_with_fuel (S n) (Encode e1) = Some v1 -> exists v2, evals_to (Encode e2) v2 /\ X v1 v2) /\
      (forall v2, eval_with_fuel (S n) (Encode e2) = Some v2 -> exists v1, evals_to (Encode e1) v1 /\ X v1 v2).
    Proof.
      intro Related; pose proof (execute_encodes_transport n IH_force e1 e2 Related) as Execs.
      split; intros v Fuel.
      - change (obind (execute_with_fuel n e1) (eval_with_fuel n) = Some v) in Fuel.
        apply obind_some in Fuel; destruct Fuel as [h1 [Exec1 Ev1]].
        destruct (proj1 Execs h1 Exec1) as [h2 [Exec2 R]].
        destruct (proj1 (IH_eval h1 h2 R) v Ev1) as [v2 [Ev2 Result]].
        exists v2; split; [eapply eval_after_execute; eauto|exact Result].
      - change (obind (execute_with_fuel n e2) (eval_with_fuel n) = Some v) in Fuel.
        apply obind_some in Fuel; destruct Fuel as [h2 [Exec2 Ev2]].
        destruct (proj2 Execs h2 Exec2) as [h1 [Exec1 R]].
        destruct (proj2 (IH_eval h1 h2 R) v Ev2) as [v1 [Ev1 Result]].
        exists v1; split; [eapply eval_after_execute; eauto|exact Result].
    Qed.
    Lemma eval_fixed_transport k h1 h2 : X h1 h2 ->
      (forall n, eval_with_fuel n h1 = Some h1) ->
      (forall n, eval_with_fuel n h2 = Some h2) ->
      (forall v1, eval_with_fuel k h1 = Some v1 -> exists v2, evals_to h2 v2 /\ X v1 v2) /\
      (forall v2, eval_with_fuel k h2 = Some v2 -> exists v1, evals_to h1 v1 /\ X v1 v2).
    Proof.
      intros R F1 F2; split; intros v Fuel.
      - rewrite F1 in Fuel; inversion Fuel; subst; exists h2; split; [exists 0; apply F2|exact R].
      - rewrite F2 in Fuel; inversion Fuel; subst; exists h1; split; [exists 0; apply F1|exact R].
    Qed.
    Lemma eval_nonencode_step n (IH : eval_transport n) h1 h2 :
      X h1 h2 -> ~(exists e, h1 = Encode e) ->
      (forall v1, eval_with_fuel (S n) h1 = Some v1 -> exists v2, evals_to h2 v2 /\ X v1 v2) /\
      (forall v2, eval_with_fuel (S n) h2 = Some v2 -> exists v1, evals_to h1 v1 /\ X v1 v2).
    Proof.
      intros R N1; destruct h1 as [[[b1|t1]|[b1|t1]]|th1|e1].
      - destruct (preserve_blob X Props b1 h2 R) as [b2 ->].
        apply eval_fixed_transport; [exact R|intros [|k]; reflexivity|intros [|k]; reflexivity].
      - destruct (preserve_tree X Props t1 h2 R) as [t2 ->].
        split; intros v Fuel;
          first [change (omap HTreeObj (eval_tree_with_fuel n t1) = Some v) in Fuel |
                 change (omap HTreeObj (eval_tree_with_fuel n t2) = Some v) in Fuel];
          apply omap_some in Fuel; destruct Fuel as [t [Tree ->]].
        + destruct (tree_forward n IH t1 t2 t R Tree) as [t' [Ev Result]].
          exists (HTreeObj t'); split; [apply eval_some; change (eval (HTreeObj t2) = Some (HTreeObj t')); rewrite eval_tree_handle, Ev; reflexivity|exact Result].
        + destruct (tree_backward n IH t1 t2 t R Tree) as [t' [Ev Result]].
          exists (HTreeObj t'); split; [apply eval_some; change (eval (HTreeObj t1) = Some (HTreeObj t')); rewrite eval_tree_handle, Ev; reflexivity|exact Result].
      - destruct (preserve_blob_ref X Props b1 h2 R) as [b2 ->].
        apply eval_fixed_transport; [exact R|intros [|k]; reflexivity|intros [|k]; reflexivity].
      - destruct (preserve_tree_ref X Props t1 h2 R) as [t2 ->].
        apply eval_fixed_transport; [exact R|intros [|k]; reflexivity|intros [|k]; reflexivity].
      - destruct (proj1 (preserve_thunk X Props _ _ R) (ex_intro _ th1 eq_refl)) as [th2 ->].
        apply eval_fixed_transport; [exact R|intros [|k]; reflexivity|intros [|k]; reflexivity].
      - exfalso; apply N1; eauto.
    Qed.
    Lemma eval_encode_nonencode n e h :
      X (Encode e) h -> ~(exists e', h = Encode e') ->
      (forall v1, eval_with_fuel (S n) (Encode e) = Some v1 -> exists v2, evals_to h v2 /\ X v1 v2) /\
      (forall v2, eval_with_fuel (S n) h = Some v2 -> exists v1, evals_to (Encode e) v1 /\ X v1 v2).
    Proof.
      intros R Nh; pose proof (encode_eval X Props e h R Nh) as Exec.
      split; intros v Fuel; exists v; split; try apply (X_self X Props).
      - change (obind (execute_with_fuel n e) (eval_with_fuel n) = Some v) in Fuel.
        apply obind_some in Fuel; destruct Fuel as [h' [Exec' Ev]].
        pose proof (execute_deterministic e h' h (ex_intro _ n Exec') Exec) as ->.
        exists n; exact Ev.
      - eapply eval_after_execute; [exact Exec|exists (S n); exact Fuel].
    Qed.
    Theorem eval_step n : force_transport n -> eval_transport n -> eval_transport (S n).
    Proof.
      intros IH_force IH_eval h1 h2 R; destruct h1 as [d|th|e].
      - apply eval_nonencode_step; [exact IH_eval|exact R|intros [e E]; discriminate].
      - apply eval_nonencode_step; [exact IH_eval|exact R|intros [e E]; discriminate].
      - destruct h2 as [d|th|e2].
        + apply eval_encode_nonencode; [exact R|intros [e' E]; discriminate].
        + apply eval_encode_nonencode; [exact R|intros [e' E]; discriminate].
        + apply eval_encodes_transport; assumption.
    Qed.
    Lemma think_zero : think_transport 0.
    Proof. intros t1 t2 R; split; intros v Fuel; discriminate. Qed.
    Lemma force_zero : force_transport 0.
    Proof. intros t1 t2 R; split; intros v Fuel; discriminate. Qed.
    Lemma eval_zero : eval_transport 0.
    Proof.
      intros h1 h2 R; destruct h1 as [[[b1|t1]|[b1|t1]]|th1|e1].
      - destruct (preserve_blob X Props b1 h2 R) as [b2 ->].
        apply eval_fixed_transport; [exact R|intros [|k]; reflexivity|intros [|k]; reflexivity].
      - destruct (preserve_tree X Props t1 h2 R) as [t2 ->].
        split; intros v Fuel; discriminate.
      - destruct (preserve_blob_ref X Props b1 h2 R) as [b2 ->].
        apply eval_fixed_transport; [exact R|intros [|k]; reflexivity|intros [|k]; reflexivity].
      - destruct (preserve_tree_ref X Props t1 h2 R) as [t2 ->].
        apply eval_fixed_transport; [exact R|intros [|k]; reflexivity|intros [|k]; reflexivity].
      - destruct (proj1 (preserve_thunk X Props _ _ R) (ex_intro _ th1 eq_refl)) as [th2 ->].
        apply eval_fixed_transport; [exact R|intros [|k]; reflexivity|intros [|k]; reflexivity].
      - split; intros v Fuel; [discriminate|].
        destruct h2 as [d2|th2|e2]; [| |discriminate].
        + exists v; split; [|apply (X_self X Props)].
          eapply eval_after_execute; [eapply (encode_eval X Props); [exact R|intros [e E]; discriminate]|exists 0; exact Fuel].
        + exists v; split; [|apply (X_self X Props)].
          eapply eval_after_execute; [eapply (encode_eval X Props); [exact R|intros [e E]; discriminate]|exists 0; exact Fuel].
    Qed.
    Theorem eq_forces_to_induct n : think_transport n /\ force_transport n /\ eval_transport n.
    Proof.
      induction n as [|n [Think [Force Eval]]].
      - split; [apply think_zero|split; [apply force_zero|apply eval_zero]].
      - split; [apply think_step; exact Eval|].
        split; [apply force_step; assumption|apply eval_step; assumption].
    Qed.
    Lemma option_transport {B} (R : B -> B -> Prop) (x y : option B) :
      (forall v, x = Some v -> exists w, y = Some w /\ R v w) ->
      (forall w, y = Some w -> exists v, x = Some v /\ R v w) -> rel_opt R x y.
    Proof.
      intros Forward Backward; destruct x as [v|], y as [w|]; cbn [rel_opt].
      - destruct (Forward v eq_refl) as [w' [E Related]]; inversion E; subst; exact Related.
      - destruct (Forward v eq_refl) as [w' [E Related]]; discriminate.
      - destruct (Backward w eq_refl) as [v' [E Related]]; discriminate.
      - exact I.
    Qed.
    Theorem evals_X h1 h2 : X h1 h2 -> rel_opt X (eval h1) (eval h2).
    Proof.
      intro Related; apply option_transport; intros v Ev; apply eval_some in Ev; destruct Ev as [n Fuel].
      - destruct (proj1 (proj2 (proj2 (eq_forces_to_induct n)) h1 h2 Related) v Fuel) as [w [Ev Related']].
        exists w; split; [apply eval_unique; exact Ev|exact Related'].
      - destruct (proj2 (proj2 (proj2 (eq_forces_to_induct n)) h1 h2 Related) v Fuel) as [w [Ev Related']].
        exists w; split; [apply eval_unique; exact Ev|exact Related'].
    Qed.
    Lemma forces_strengthen t1 t2 : X (Thunk t1) (Thunk t2) ->
      rel_opt (strengthen X) (force t1) (force t2).
    Proof.
      intro Related; apply option_transport; intros v F; apply force_some in F; destruct F as [n Fuel].
      - destruct (proj1 (proj1 (proj2 (eq_forces_to_induct n)) t1 t2 Related) v Fuel) as [w [F Related']].
        exists w; split; [apply force_unique; exact F|exact Related'].
      - destruct (proj2 (proj1 (proj2 (eq_forces_to_induct n)) t1 t2 Related) v Fuel) as [w [F Related']].
        exists w; split; [apply force_unique; exact F|exact Related'].
    Qed.
    Theorem forces_X t1 t2 : X (Thunk t1) (Thunk t2) ->
      rel_opt (relaxed_X X) (force t1) (force t2).
    Proof.
      intro Related; pose proof (forces_strengthen t1 t2 Related) as Result.
      destruct (force t1) as [h1|] eqn:F1, (force t2) as [h2|] eqn:F2;
        cbn [rel_opt] in *; try contradiction; [|exact I].
      destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->]; exact Result.
    Qed.
    Theorem think_X t1 t2 : X (Thunk t1) (Thunk t2) ->
      rel_opt (strengthen X) (think t1) (think t2) \/
      think t1 = Some (Thunk t2) \/ think t1 = Some (Encode (Strict t2)) \/ think t1 = Some (Encode (Shallow t2)).
    Proof.
      intro Related; destruct (think t1) as [v1|] eqn:T1.
      - destruct (proj1 (think_some _ _) T1) as [n Fuel].
        destruct (proj1 (proj1 (eq_forces_to_induct n) t1 t2 Related) v1 Fuel) as
          [[v2 [T2 Strength]]|[OneThunk|[OneStrict|OneShallow]]].
        + left; rewrite (think_unique _ _ T2); exact Strength.
        + right; left; congruence.
        + right; right; left; congruence.
        + right; right; right; congruence.
      - destruct (think t2) as [v2|] eqn:T2; [|left; exact I].
        destruct (proj1 (think_some _ _) T2) as [n Fuel].
        destruct (proj2 (proj1 (eq_forces_to_induct n) t1 t2 Related) v2 Fuel) as
          [[v1 [T1' Strength]]|[OneThunk|[OneStrict|OneShallow]]];
          exfalso; first [pose proof (think_unique _ _ T1')|pose proof (think_unique _ _ OneThunk)|
            pose proof (think_unique _ _ OneStrict)|pose proof (think_unique _ _ OneShallow)]; congruence.
    Qed.
  End Relation.
End EvaluationProperties.
