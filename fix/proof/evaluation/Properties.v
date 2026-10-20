(** Proves fuel monotonicity and determinism, defines unbounded evaluation,
    and establishes evaluator equations and congruence properties. *)

From Stdlib Require Import List Arith Lia ClassicalEpsilon.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle ApplyTree Evaluation.
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

  (** Mapping a function over optional results preserves the order that permits additional
      successes. *)
  Lemma omap_le {A B} (f : A -> B) x y : option_le x y -> option_le (omap f x) (omap f y).
  Proof.
    intros L r R; destruct x as [x|]; simpl in R; try discriminate.
    rewrite (L x eq_refl); exact R.
  Qed.
  (** Optional binding preserves the success order when both the input and continuation
      grow monotonically. *)
  Lemma obind_le {A B} x y (f g : A -> option B) :
    option_le x y -> success_le f g -> option_le (obind x f) (obind y g).
  Proof.
    intros L F r R; destruct x as [x|]; simpl in R; try discriminate.
    rewrite (L x eq_refl); apply F; exact R.
  Qed.
  (** Every optional result is below itself in the success order. *)
  Lemma option_le_refl {A} (x : option A) : option_le x x.
  Proof. intros r R; exact R. Qed.
  (** Giving entry evaluation more successes cannot remove successful list evaluations. *)
  Lemma eval_list_using_le f g : success_le f g -> success_le (eval_list_using f) (eval_list_using g).
  Proof.
    intros F xs; induction xs; simpl; [apply option_le_refl|].
    apply obind_le; [apply F|].
    intro y; apply omap_le; exact IHxs.
  Qed.
  (** Giving entry evaluation more successes cannot remove successful Tree evaluations. *)
  Lemma eval_tree_using_le f g : success_le f g -> success_le (eval_tree_using f) (eval_tree_using g).
  Proof. intros F t; apply omap_le, eval_list_using_le; exact F. Qed.

  (** One unfolding of the mutually recursive evaluators preserves the success order. *)
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

  (** Adding one unit of fuel cannot change an already successful evaluator result. *)
  Lemma with_fuel_step n : evaluators_le (with_fuel n) (with_fuel (S n)).
  Proof.
    (* Only the absence of enough fuel can become a new success. A success already found
        has fixed all of the recursive replies it depends on; monotonicity of those
        replies therefore preserves it at the next bound. *)
    induction n as [|n IH]; [|apply next_evaluators_le; exact IH].
    repeat split; unfold success_le, option_le; intros x r R;
      cbn [with_fuel think_fn force_fn execute_fn] in R; try discriminate.
    destruct x as [d|th|e]; [destruct d as [[blob|tree]|ref]| |];
      cbn [with_fuel next_evaluators eval_fn] in *; try discriminate; exact R.
  Qed.

  (** Adding any amount of fuel preserves successful evaluator results. *)
  Lemma with_fuel_padding n k : evaluators_le (with_fuel n) (with_fuel (n+k)).
  Proof.
    (* A finite extension of the fuel bound is a chain of one-unit extensions. The
        previous result applies to every link, so a successful answer is unchanged
        throughout the chain. *)
    induction k as [|k IH].
    - rewrite Nat.add_0_r; repeat split; intros x r R; exact R.
    - rewrite Nat.add_succ_r.
      destruct IH as [T [F [Ex Ev]]].
      destruct (with_fuel_step (n+k)) as [T' [F' [Ex' Ev']]].
      repeat split; intros x r R; [apply T'|apply F'|apply Ex'|apply Ev'];
        [apply T|apply F|apply Ex|apply Ev]; exact R.
  Qed.

  (** Think, Force, Execute, and all evaluation variants preserve their results when given
      more fuel. *)
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

  (** Successful runs at different fuel bounds have the same result. This makes fuel an
      instrument for reasoning about finite computations rather than part of the
      semantics. *)
  Local Lemma fuel_deterministic {A B} (f : nat -> A -> option B)
      (Padding : forall n k x r, f n x = Some r -> f (n+k) x = Some r)
      x a b : (exists n, f n x = Some a) -> (exists n, f n x = Some b) -> a = b.
  Proof.
    (* Neither run initially has enough fuel to be compared directly with the other.
        Raise both bounds to their sum. Monotonicity keeps both answers unchanged, and at
        that common bound they are values of the same function. *)
    intros [n First] [m Second].
    pose proof (Padding n m x a First) as FirstPadded.
    pose proof (Padding m n x b Second) as SecondPadded.
    rewrite Nat.add_comm in SecondPadded; congruence.
  Qed.

  (** Thinking a Thunk has at most one successful result, regardless of fuel. *)
  Theorem think_deterministic th x y : thinks_to th x -> thinks_to th y -> x = y.
  Proof. apply (fuel_deterministic think_with_fuel); intros n k; pose proof (fuel_padding n k); tauto. Qed.
  (** Forcing a Thunk has at most one successful result, regardless of fuel. *)
  Theorem force_deterministic th x y : forces_to th x -> forces_to th y -> x = y.
  Proof. apply (fuel_deterministic force_with_fuel); intros n k; pose proof (fuel_padding n k); tauto. Qed.
  (** Executing an Encode has at most one successful result, regardless of fuel. *)
  Theorem execute_deterministic e x y : executes_to e x -> executes_to e y -> x = y.
  Proof. apply (fuel_deterministic execute_with_fuel); intros n k; pose proof (fuel_padding n k); tauto. Qed.
  (** Evaluating a handle has at most one successful result, regardless of fuel. *)
  Theorem eval_deterministic h x y : evals_to h x -> evals_to h y -> x = y.
  Proof. apply (fuel_deterministic eval_with_fuel); intros n k; pose proof (fuel_padding n k); tauto. Qed.
  (** Evaluating a Tree has at most one successful result, regardless of fuel. *)
  Theorem eval_tree_deterministic t x y : evals_tree_to t x -> evals_tree_to t y -> x = y.
  Proof. apply (fuel_deterministic eval_tree_with_fuel); intros n k; pose proof (fuel_padding n k); tauto. Qed.

  (** Isabelle's SOME/THE-based, unbounded option-valued evaluators become
      classical definite choice. No computable termination test is claimed. *)
  Definition unfuel {A B} (f : nat -> A -> option B) (x : A) : option B :=
    match excluded_middle_informative (exists r, exists n, f n x = Some r) with
    | left H => Some (proj1_sig (constructive_indefinite_description _ H))
    | right _ => None
    end.
  (** For a result-unique fuelled function, removing fuel succeeds exactly when some finite
      fuel succeeds. *)
  Lemma unfuel_some {A B} (f : nat -> A -> option B)
      (unique : forall x r1 r2, (exists n, f n x = Some r1) -> (exists n, f n x = Some r2) -> r1 = r2)
      x r : unfuel f x = Some r <-> exists n, f n x = Some r.
  Proof.
    (* The unfuelled evaluator chooses a successful result if one exists. Result
        uniqueness makes that choice immaterial: every finite successful run supplies the
        same answer, so removing the bound cannot introduce an additional result. *)
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
  (** Force can return Data as an Object or as a Ref, while a later consumer chooses how
      to expose it. The relation on replies must therefore compare their underlying
      Objects. The shallow-Encode case likewise moves to the strict request so that both
      sides are compared before lowering to Refs. *)
  Definition relaxed_X (X : handle -> handle -> Prop) h1 h2 :=
    match h1 with
    | Encode (Shallow th) => X (Encode (Strict th)) (lift h2)
    | _ => X (lift h1) (lift h2)
    end.
  (** A Think reply is not necessarily final Data: it can defer work as a Thunk or an
      Encode. The relation needed by the next Force step must account for that deferral.
      In particular, two Encode replies may be justified through related Thunks or through
      already related forces; the subsequent continuation argument needs both
      alternatives. *)
  Definition strengthen (X : handle -> handle -> Prop) h1 h2 :=
    match h1, h2 with
    | Encode e1, Encode e2 =>
        X (Thunk (encode_to_thunk e1)) (Thunk (encode_to_thunk e2)) \/
        rel_opt (relaxed_X X) (force (encode_to_thunk e1)) (force (encode_to_thunk e2))
    | Encode e, Thunk th => X (Thunk (encode_to_thunk e)) (Thunk th)
    | Thunk th, Encode e => X (Thunk th) (Thunk (encode_to_thunk e))
    | _, _ => relaxed_X X h1 h2
    end.
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
  (** Evaluation leaves leaves unchanged, evaluates Tree children, and executes exposed
      Encodes before continuing. *)
  Theorem eval_unfold h : eval h =
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
  (** Finite-fuel Encode execution forces its Thunk, then lifts or lowers the result
      according to strictness. *)
  Lemma execute_with_fuel_force n e : execute_with_fuel n e =
    omap (match e with Strict _ => lift | Shallow _ => lower end)
         (force_with_fuel n (encode_to_thunk e)).
  Proof. destruct n, e; reflexivity. Qed.
  (** Executing an Encode forces its Thunk and then lifts a strict result or lowers a
      shallow result. *)
  Theorem execute_unfold e : execute e =
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

  (** Successful forcing returns Data. *)
  Lemma force_data th r : force th = Some r -> exists d, r = Data d.
  Proof. rewrite force_some; intros [n F]; eapply force_with_fuel_to_data; exact F. Qed.
  (** Successful Encode execution returns Data. *)
  Lemma execute_data e h : execute e = Some h -> exists d, h = Data d.
  Proof.
    rewrite execute_unfold; destruct e; cbn; intro Exec; apply omap_some in Exec;
      destruct Exec as [v [Force ->]]; destruct (force_data _ _ Force) as [d ->];
      [exists (lift_data d)|exists (lower_data d)]; reflexivity.
  Qed.
  (** The strict version of a successful Encode returns the lifted result. *)
  Lemma execute_to_lift e h : execute e = Some h ->
    execute (Strict (encode_to_thunk e)) = Some (lift h).
  Proof.
    destruct e as [th|th]; intro Exec; rewrite execute_unfold in Exec;
      apply omap_some in Exec; destruct Exec as [r [Force ->]];
      cbn [encode_to_thunk] in Force;
      destruct (force_data _ _ Force) as [d ->];
      rewrite execute_unfold; cbn [encode_to_thunk]; rewrite Force;
      destruct d as [[b|t]|[b|t]]; reflexivity.
  Qed.
  (** Successful evaluation never returns an exposed Encode. *)
  Lemma eval_not_encode h r : eval h = Some r -> not_encode r.
  Proof.
    rewrite eval_some; intros [n Ev]; apply eval_with_fuel_not_encode in Ev;
      destruct Ev as [[d ->]|[th ->]]; exact I.
  Qed.
  (** Successful evaluation returns a Value. *)
  Lemma eval_to_value_handle h r : eval h = Some r -> value_handle r.
  Proof. rewrite eval_some; intros [n Ev]; eapply eval_with_fuel_to_value_handle; exact Ev. Qed.
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
  (** Force follows returned computations until it obtains Data, ignoring whether an
      intermediate reply was packaged as a bare Thunk or an Encode. The final consumer
      determines how that Data is exposed. *)
  Theorem force_unfold th : force th = obind (think th) (fun h =>
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
  (** Each Thunk performs its own Think operation, evaluating application and selection
      operand Trees first. *)
  Theorem think_unfold th : think th = match th with
    | Application t => obind (eval_tree t) A.apply_tree
    | Identification d => I.identify d
    | Selection t => obind (eval_tree t) Sl.select
    end.
  Proof. destruct th; [apply think_application|apply think_identification|apply think_selection]. Qed.

  (** Every successful finite forcing derivation returns Data. *)
  Lemma forces_to_data th r : forces_to th r -> exists d, r = Data d.
  Proof. intro Force; apply force_data with th; apply force_unique; exact Force. Qed.

  (** Every successful finite evaluation derivation returns Data or a bare Thunk. *)
  Lemma evals_to_not_encode h r : evals_to h r ->
    (exists d, r = Data d) \/ (exists th, r = Thunk th).
  Proof.
    intro Eval; pose proof (eval_not_encode h r (eval_unique h r Eval)) as Shape.
    destruct r; [left; eauto|right; eauto|contradiction].
  Qed.

  (** Forcing to a Blob Object also evaluates the corresponding strict Encode to that Blob. *)
  Lemma forces_to_implies_evals_to th b : forces_to th (HBlobObj b) ->
    evals_to (Encode (Strict th)) (HBlobObj b).
  Proof.
    intro Force; apply eval_some; rewrite eval_encode, execute_unfold.
    cbn [encode_to_thunk]; rewrite (force_unique th _ Force).
    change (eval (HBlobObj b) = Some (HBlobObj b)); apply eval_blob.
  Qed.

  (** Independent child evaluations assemble into evaluation of the containing Tree. The
      common Tag marker records that evaluating fields does not change which kind of Tree
      is being evaluated. *)
  Lemma evaluation_tree tree children :
    Forall2 (fun h result => eval h = Some result) (get_tree_raw tree) children ->
    eval (HTreeObj tree) = Some (HTreeObj (create_tree_tagged (tree_tag tree) children)).
  Proof.
    intro Entries; apply eval_unique.
    apply (A.Forall2_mono _ evals_to) in Entries;
      [|intros h result Evaluated; apply eval_some; exact Evaluated].
    destruct (evals_to_tree_to _ _ Entries) as [n Evaluated].
    exists (S n).
    change (omap HTreeObj (omap (create_tree_tagged (tree_tag tree))
      (eval_list_with_fuel n (get_tree_raw tree))) =
        Some (HTreeObj (create_tree_tagged (tree_tag tree) children))).
    rewrite Evaluated; reflexivity.
  Qed.

  (** Pointwise evaluation of children gives evaluation of their ordinary Trees. *)
  Lemma eval_entry_to_eval_tree xs ys : Forall2 evals_to xs ys ->
    eval (HTreeObj (create_tree xs)) = Some (HTreeObj (create_tree ys)).
  Proof.
    intro Entries; pose proof (evaluation_tree (create_tree xs) ys) as Evaluate.
    rewrite get_tree_raw_create_tree, tree_tag_create_tree in Evaluate.
    apply Evaluate; eapply A.Forall2_mono; [|exact Entries].
    intros h result Evaluated; now apply eval_unique.
  Qed.

  (** A successful Tree evaluation consists of successful child evaluations and preserves the
      Tag marker.

      Guarded Isabelle [the (eval x)] uses become explicit output witnesses. The Forall2
      conclusion specifies every output, including duplicates. *)
  Lemma eval_tree_to_eval_entry t r : eval (HTreeObj t) = Some r ->
    exists ys, r = HTreeObj (create_tree_tagged (tree_tag t) ys) /\
      Forall2 (fun x y => eval x = Some y) (get_tree_raw t) ys /\
      Forall (fun x => exists y, eval x = Some y) (get_tree_raw t).
  Proof.
    intro Eval; rewrite eval_tree_handle in Eval.
    apply omap_some in Eval as [t' [Tree ->]]; apply eval_tree_some in Tree as [n Tree].
    change (omap (create_tree_tagged (tree_tag t)) (eval_list_with_fuel n (get_tree_raw t)) = Some t') in Tree.
    apply omap_some in Tree as [ys [Entries ->]].
    apply eval_list_Forall2 in Entries.
    assert (Outputs : Forall2 (fun x y => eval x = Some y) (get_tree_raw t) ys).
    { eapply A.Forall2_mono; [|exact Entries].
      intros x y Entry; apply eval_unique; exists n; exact Entry. }
    exists ys; split; [reflexivity|]; split; [exact Outputs|].
    clear Entries; induction Outputs; constructor; eauto.
  Qed.

  (** One Tree evaluates to another exactly when their markers agree and their children
      evaluate pointwise. *)
  Lemma eval_tree_iff t u :
    eval (HTreeObj t) = Some (HTreeObj u) <->
    tree_tag t = tree_tag u /\
    Forall2 (fun h r => eval h = Some r) (get_tree_raw t) (get_tree_raw u).
  Proof.
    split.
    - intro Evaluated.
      destruct (eval_tree_to_eval_entry _ _ Evaluated) as (children & Output & Entries & _).
      injection Output as ->.
      rewrite tree_tag_create_tree_tagged, get_tree_raw_create_tree_tagged; auto.
    - intros (Tags & Entries).
      pose proof (evaluation_tree t (get_tree_raw u) Entries) as Evaluated.
      now rewrite Tags, create_tree_get_tree_raw in Evaluated.
  Qed.

  (** Evaluating a Tree/Object-and-Ref pair evaluates only the Object field and preserves
      the outer marker. *)
  Lemma eval_tree_ref_pair tagged t reference evaluated :
    eval (HTreeObj (create_tree_tagged tagged [HTreeObj t; HTreeRef reference])) =
      Some (HTreeObj evaluated) ->
    exists u, eval (HTreeObj t) = Some (HTreeObj u) /\
      evaluated = create_tree_tagged tagged [HTreeObj u; HTreeRef reference].
  Proof.
    intro Evaluated.
    sauto inv: Forall2 ctrs: - cases: -
      use: eval_tree_iff, (eval_tree_to_eval_entry t),
        (eval_ref (TreeRef reference)), (create_tree_get_tree_raw evaluated),
        get_tree_raw_create_tree_tagged, tree_tag_create_tree_tagged
      unfold!: HTreeObj, HTreeRef.
  Qed.

  (** Evaluating a Value Tree leaves it unchanged. *)
  Lemma value_tree_eval_to_itself t : value_tree t -> eval (HTreeObj t) = Some (HTreeObj t).
  Proof.
    (* Object containment is well founded, so the claim can be established from children
        to parents. Every child of a Value Tree is itself a Value and hence unchanged.
        Reconstructing the unchanged child list with the original marker gives the
        original Tree name. *)
    revert t; refine (well_founded_induction wfp_tree_child
      (fun t => value_tree t -> eval (HTreeObj t) = Some (HTreeObj t)) _).
    intros t IH Value; inversion Value as [tree Values]; subst.
    (* These child equalities justify using the original children as the whole evaluation
        result. The remaining step is the storage law identifying a Tree with
        reconstruction from its marker and children. *)
    assert (ChildrenUnchanged : Forall2 (fun child result => eval child = Some result)
      (get_tree_raw t) (get_tree_raw t)).
    (* The containment induction hypothesis applies to each Object-Tree child. Non-Tree
        Values are already leaves of evaluation, so no computation is introduced at those
        positions. *)
    { assert (EachChildUnchanged : forall child, In child (get_tree_raw t) ->
          eval child = Some child).
      { intros child Member.
        assert (ChildValue : value_handle child) by (eapply Forall_forall; eassumption).
        sauto inv: value_handle ctrs: - cases: - use: eval_blob, eval_ref, eval_thunk, IH. }
      apply Forall_forall in EachChildUnchanged.
      clear -EachChildUnchanged; induction EachChildUnchanged; constructor; assumption. }
    pose proof (evaluation_tree t (get_tree_raw t) ChildrenUnchanged) as TreeUnchanged.
    rewrite create_tree_get_tree_raw in TreeUnchanged; exact TreeUnchanged.
  Qed.
  (** Evaluating any Value leaves it unchanged. *)
  Theorem value_handle_eval_to_itself h : value_handle h -> eval h = Some h.
  Proof.
    intro V; inversion V; subst.
    - apply eval_blob.
    - apply value_tree_eval_to_itself; assumption.
    - apply eval_ref.
    - apply eval_thunk.
  Qed.

  (** Evaluating an author list cannot remove a requirement imposed by an original Value
      author: that author is unchanged and still occurs at the same position in the
      result. *)
  Lemma eval_preserves_stable_requirements (requirement : handle -> Prop) t u :
    eval (HTreeObj t) = Some (HTreeObj u) ->
    Forall (fun h => value_handle h -> requirement h) (get_tree_raw u) ->
    Forall (fun h => value_handle h -> requirement h) (get_tree_raw t).
  Proof.
    (* Match each original entry with its evaluated entry. Whenever the original is a
        Value, evaluation fixes it, so the requirement already known for that result
        applies to the original as well. Non-Value entries impose no stable-author
        requirement. *)
    intros Evaluated Requirements.
    apply eval_tree_iff in Evaluated as (_ & Children).
    revert Requirements; induction Children;
      sauto inv: Forall ctrs: Forall cases: - use: value_handle_eval_to_itself.
  Qed.

  (** The Tree evaluator leaves a Value Tree unchanged. *)
  Lemma value_tree_fixed_eval t : value_handle (HTreeObj t) -> eval_tree t = Some t.
  Proof.
    intro Value; pose proof (value_handle_eval_to_itself _ Value) as Ev.
    rewrite eval_tree_handle in Ev; apply omap_some in Ev; destruct Ev as [u [Tree E']].
    inversion E'; subst u; exact Tree.
  Qed.

  Definition same_shape h1 h2 := get_type h1 = get_type h2.
  (** Handles of the same shape have the same observable type. *)
  Lemma same_shape_get_type h1 h2 : same_shape h1 h2 -> get_type h1 = get_type h2.
  Proof. auto. Qed.
  (** A pointwise relation between Value lists can be strengthened using a rule for each
      corresponding child pair. *)
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
    (** Related Tree Objects have pointwise related children. *)
    Hypothesis tree_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> Forall2 X (get_tree_raw t1) (get_tree_raw t2).
    (** Related Tree Objects have the same Tag marker. *)
    Hypothesis tag_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> tree_tag t1 = tree_tag t2.
    (** Related non-Encode handles have the same shape. *)
    Hypothesis X_preserve_value_handle : forall h1 h2, X h1 h2 ->
      ~(exists e, h1 = Encode e) -> ~(exists e, h2 = Encode e) -> same_shape h1 h2.

    (** Related Value Trees have matching recursive handle types. *)
    Lemma value_tree_to_sametypedness t1 : value_tree t1 -> forall t2,
      X (HTreeObj t1) (HTreeObj t2) -> value_tree t2 -> same_typed_tree t1 t2.
    Proof.
      (* Program execution is allowed to inspect handle constructors and Tree positions.
          Before using relational congruence, related Value inputs must therefore have
          corresponding constructors, recursively through Object Trees. Well-founded
          Object containment lets the outer Tree fact be assembled from the child facts. *)
      revert t1; refine (well_founded_induction wfp_tree_child
        (fun t1 => value_tree t1 -> forall t2, X (HTreeObj t1) (HTreeObj t2) -> value_tree t2 -> same_typed_tree t1 t2) _).
      intros t1 IH V1 t2 R V2; apply tree; [apply tag_cong; exact R|].
      inversion V1 as [t Values1]; inversion V2 as [u Values2]; subst.
      eapply (Forall2_value_transform X same_typed_handle); [apply tree_cong; exact R| |exact Values1|exact Values2].
      intros x y Member HX VX VY.
      pose proof (same_shape_get_type x y
        (X_preserve_value_handle x y HX (value_handle_not_encode x VX) (value_handle_not_encode y VY))) as Types.
      destruct x as [[[b|t]|[b|t]]|th|e];
        destruct y as [[[b'|t']|[b'|t']]|th'|e'];
        cbn [get_type] in Types; try discriminate;
        try solve [constructor];
        try solve [exfalso; apply (value_handle_not_encode _ VX); eauto].
      apply tree_obj; apply IH; [exact Member| |exact HX|];
        inversion VX; inversion VY; subst; assumption.
    Qed.
    (** Related Values have matching handle types. *)
    Lemma value_handle_to_sametypedness h1 h2 :
      X h1 h2 -> value_handle h1 -> value_handle h2 -> same_typed_handle h1 h2.
    Proof.
      intros R V1 V2.
      pose proof (same_shape_get_type h1 h2
        (X_preserve_value_handle h1 h2 R (value_handle_not_encode h1 V1) (value_handle_not_encode h2 V2))) as Types.
      destruct h1 as [[[b|t]|[b|t]]|th|e];
        destruct h2 as [[[b'|t']|[b'|t']]|th'|e'];
        cbn [get_type] in Types; try discriminate;
        try solve [constructor];
        try solve [exfalso; apply (value_handle_not_encode _ V1); eauto].
      apply tree_obj; apply value_tree_to_sametypedness; [|exact R|];
        inversion V1; inversion V2; subst; assumption.
    Qed.
  End ValueTypes.

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

  Section ReferenceCongruence.
    Variable X : handle -> handle -> Prop.
    (** Related Blob Objects have related Refs. *)
    Hypothesis blob_ref_cong : forall b1 b2, X (HBlobObj b1) (HBlobObj b2) -> X (HBlobRef b1) (HBlobRef b2).
    (** Related Tree Objects have related Refs. *)
    Hypothesis tree_ref_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> X (HTreeRef t1) (HTreeRef t2).
    (** Related Blob Refs have related underlying Objects. *)
    Hypothesis blob_ref_complete : forall b1 b2, X (HBlobRef b1) (HBlobRef b2) -> X (HBlobObj b1) (HBlobObj b2).
    (** Related Tree Refs have related underlying Objects. *)
    Hypothesis tree_ref_complete : forall t1 t2, X (HTreeRef t1) (HTreeRef t2) -> X (HTreeObj t1) (HTreeObj t2).
    (** A handle related from a Tree Ref is also a Tree Ref. *)
    Hypothesis X_preserve_tree_ref : forall t h, X (HTreeRef t) h -> exists t', h = HTreeRef t'.
    (** A handle related from a Blob Ref is also a Blob Ref. *)
    Hypothesis X_preserve_blob_ref : forall b h, X (HBlobRef b) h -> exists b', h = HBlobRef b'.
    (** A handle related from a Tree Object is also a Tree Object. *)
    Hypothesis X_preserve_tree : forall t h, X (HTreeObj t) h -> exists t', h = HTreeObj t'.
    (** A handle related from a Blob Object is also a Blob Object. *)
    Hypothesis X_preserve_blob : forall b h, X (HBlobObj b) h -> exists b', h = HBlobObj b'.

    Ltac reject_shape R :=
      try solve [destruct (X_preserve_blob_ref _ _ R) as [? E]; discriminate E];
      try solve [destruct (X_preserve_tree_ref _ _ R) as [? E]; discriminate E];
      try solve [destruct (X_preserve_blob _ _ R) as [? E]; discriminate E];
      try solve [destruct (X_preserve_tree _ _ R) as [? E]; discriminate E].
    (** Related Data remains related when compared through its lifted Objects. *)
    Lemma data_to_relaxed d1 d2 : X (Data d1) (Data d2) -> relaxed_X X (Data d1) (Data d2).
    Proof.
      intro R; destruct d1 as [[b1|t1]|[b1|t1]], d2 as [[b2|t2]|[b2|t2]];
        cbv beta iota delta [relaxed_X lift lift_data];
        try exact R;
        try solve [eauto using blob_ref_complete, tree_ref_complete]; reject_shape R.
    Qed.
    (** Related lowered Data has related lifted Objects. *)
    Lemma lower_to_lift d1 d2 : X (lower (Data d1)) (lower (Data d2)) -> X (lift (Data d1)) (lift (Data d2)).
    Proof.
      intro R; destruct d1 as [[b1|t1]|[b1|t1]], d2 as [[b2|t2]|[b2|t2]];
        cbv beta iota delta [lift lower lift_data lower_data] in *;
        try solve [eauto using blob_ref_complete, tree_ref_complete]; reject_shape R.
    Qed.
    (** Related lifted Objects have related lowered Refs. *)
    Lemma lift_to_lower d1 d2 : X (lift (Data d1)) (lift (Data d2)) -> X (lower (Data d1)) (lower (Data d2)).
    Proof.
      intro R; destruct d1 as [[b1|t1]|[b1|t1]], d2 as [[b2|t2]|[b2|t2]];
        cbv beta iota delta [lift lower lift_data lower_data] in *;
        try solve [apply blob_ref_cong; exact R];
        try solve [apply tree_ref_cong; exact R]; reject_shape R.
    Qed.
    (** Related lowered forcing results give the relaxed relation between the original
        forcing results. *)
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
    (exists d1 d2, X (Data (Ref d1)) (Data (Ref d2)) /\ t1 = Identification d1 /\ t2 = Identification d2) \/
    (think t1 = None /\ think t2 = None) \/
    (exists r1 r2, think t1 = Some r1 /\ think t2 = Some r2 /\ strengthen X r1 r2) \/
    think t1 = Some (Thunk t2) \/ think t1 = Some (Encode (Strict t2)) \/ think t1 = Some (Encode (Shallow t2)).

  Record relation_properties (X : handle -> handle -> Prop) : Prop := {
    (** The relation satisfies the structural congruence laws. *)
    structural : congruence X;
    (** A handle related to a Tree Ref is a Tree Ref or a shallow Encode. *)
    preserve_tree_ref_rev : forall t h, X h (HTreeRef t) -> (exists t', h = HTreeRef t') \/ (exists th, h = Encode (Shallow th));
    (** A handle related to a Blob Ref is a Blob Ref or a shallow Encode. *)
    preserve_blob_ref_rev : forall b h, X h (HBlobRef b) -> (exists b', h = HBlobRef b') \/ (exists th, h = Encode (Shallow th));
    (** A handle related to a Tree Object is a Tree Object or a strict Encode. *)
    preserve_tree_rev : forall t h, X h (HTreeObj t) -> (exists t', h = HTreeObj t') \/ (exists th, h = Encode (Strict th));
    (** A handle related to a Blob Object is a Blob Object or a strict Encode. *)
    preserve_blob_rev : forall b h, X h (HBlobObj b) -> (exists b', h = HBlobObj b') \/ (exists th, h = Encode (Strict th));
    (** An Encode related to a non-Encode executes to that handle. *)
    encode_eval : forall e h, X (Encode e) h -> ~(exists e', h = Encode e') -> executes_to e h;
    (** Successful Encode execution relates the Encode to its result. *)
    execute_result : forall e h, execute e = Some h -> X (Encode e) h;
    (** Related strict Encodes have related Thunks or related execution results, including
        shared failure. *)
    strict_encode_reasons : forall t1 t2, X (Encode (Strict t1)) (Encode (Strict t2)) ->
      X (Thunk t1) (Thunk t2) \/ rel_opt X (execute (Strict t1)) (execute (Strict t2));
    (** Related shallow Encodes have related Thunks or related execution results, including
        shared failure. *)
    shallow_encode_reasons : forall t1 t2, X (Encode (Shallow t1)) (Encode (Shallow t2)) ->
      X (Thunk t1) (Thunk t2) \/ rel_opt X (execute (Shallow t1)) (execute (Shallow t2));
    (** A shallow Encode is not directly related to a strict Encode. *)
    not_shallow_strict : forall t1 t2, ~X (Encode (Shallow t1)) (Encode (Strict t2));
    (** A strict Encode is not directly related to a shallow Encode. *)
    not_strict_shallow : forall t1 t2, ~X (Encode (Strict t1)) (Encode (Shallow t2));
    (** A non-Encode is not directly related to an Encode in the forward direction. *)
    encode_reverse_absent : forall e h, X h (Encode e) -> ~(exists e', h = Encode e') -> False;
    (** Related bare Thunks satisfy the cases required by Think transport. *)
    related_thunk_reasons : forall t1 t2, X (Thunk t1) (Thunk t2) -> thunk_reasons X t1 t2;
  }.
  Coercion structural : relation_properties >-> congruence.

  Section Relation.
    Variable X : handle -> handle -> Prop.
    Variable Props : relation_properties X.

    (** Related handles that are not Encodes have the same shape. *)
    Lemma value_shapes x y : X x y -> ~(exists e, x = Encode e) -> ~(exists e, y = Encode e) -> same_shape x y.
    Proof.
      intros Related Nx Ny; unfold same_shape.
      eapply (Sl.X_preserves_type X Props); [exact Related| |].
      - destruct x; cbn [Sl.H.not_encode]; auto; exfalso; apply Nx; eexists; reflexivity.
      - destruct y; cbn [Sl.H.not_encode]; auto; exfalso; apply Ny; eexists; reflexivity.
    Qed.
    (** Related Values have matching handle types. *)
    Lemma values_same_type x y : X x y -> value_handle x -> value_handle y -> same_typed_handle x y.
    Proof. apply (value_handle_to_sametypedness X (tree_cong X Props) (tag_cong X Props) value_shapes). Qed.
    (** The strengthened relation holds for related Data. *)
    Lemma related_data_strengthen d1 d2 : X (Data d1) (Data d2) -> strengthen X (Data d1) (Data d2).
    Proof.
      apply (data_to_relaxed X (blob_ref_complete X Props) (tree_ref_complete X Props)
        (preserve_tree_ref X Props) (preserve_blob_ref X Props) (preserve_tree X Props) (preserve_blob X Props)).
    Qed.
    (** Related lifted forcing results give the relaxed relation between the forcing
        results. *)
    Lemma strict_force_to_relaxed t1 t2 :
      rel_opt X (omap lift (force t1)) (omap lift (force t2)) -> rel_opt (relaxed_X X) (force t1) (force t2).
    Proof.
      destruct (force t1) as [h1|] eqn:F1, (force t2) as [h2|] eqn:F2;
        cbn [rel_opt omap]; try contradiction; [|auto].
      intro R; destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->]; exact R.
    Qed.
    (** Related strict Encodes satisfy the strengthened relation. *)
    Lemma strict_strengthen t1 t2 : X (Encode (Strict t1)) (Encode (Strict t2)) -> strengthen X (Encode (Strict t1)) (Encode (Strict t2)).
    Proof.
      intro R; destruct (strict_encode_reasons X Props _ _ R) as [Thunks|Exec].
      - left; exact Thunks.
      - right; rewrite !execute_unfold in Exec; apply strict_force_to_relaxed; exact Exec.
    Qed.
    (** Related shallow Encodes satisfy the strengthened relation. *)
    Lemma shallow_strengthen t1 t2 : X (Encode (Shallow t1)) (Encode (Shallow t2)) -> strengthen X (Encode (Shallow t1)) (Encode (Shallow t2)).
    Proof.
      intro R; destruct (shallow_encode_reasons X Props _ _ R) as [Thunks|Exec].
      - left; exact Thunks.
      - right; rewrite !execute_unfold in Exec.
        eapply force_to_lift; [exact (blob_ref_complete X Props)|exact (tree_ref_complete X Props)|
          exact (preserve_tree_ref X Props)|exact (preserve_blob_ref X Props)|exact Exec].
    Qed.
    (** Related handles of matching types satisfy the strengthened relation. *)
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
    (** Applying related Value combinations preserves related results and matching types,
        or both applications fail. *)
    Lemma apply_tree_values t1 t2 : X (HTreeObj t1) (HTreeObj t2) ->
      value_handle (HTreeObj t1) -> value_handle (HTreeObj t2) ->
      rel_opt X (A.apply_tree t1) (A.apply_tree t2) /\ rel_opt same_typed_handle (A.apply_tree t1) (A.apply_tree t2).
    Proof.
      intros R V1 V2; eapply (A.apply_tree_X X Props).
      split; [exact R|apply values_same_type; assumption].
    Qed.
    (** Applying related Value combinations preserves the strengthened result relation and
        agreement on failure. *)
    Lemma apply_tree_strengthen t1 t2 : X (HTreeObj t1) (HTreeObj t2) ->
      value_handle (HTreeObj t1) -> value_handle (HTreeObj t2) ->
      rel_opt (strengthen X) (A.apply_tree t1) (A.apply_tree t2).
    Proof.
      intros R V1 V2; destruct (apply_tree_values _ _ R V1 V2) as [Related Typed].
      destruct (A.apply_tree t1), (A.apply_tree t2); simpl in *; try contradiction; [|exact I].
      apply related_typed_strengthen; assumption.
    Qed.

    (** Selection preserves related results when both operand Trees have no immediate
        Encodes. *)
    Lemma slice_values t1 t2 : X (HTreeObj t1) (HTreeObj t2) ->
      Forall not_encode (get_tree_raw t1) -> Forall not_encode (get_tree_raw t2) ->
      rel_opt X (Sl.select t1) (Sl.select t2).
    Proof.
      intros R N1 N2; eapply (Sl.select_X X Props).
      repeat split; assumption.
    Qed.

    (** Related computations can take different numbers of steps. Bound a successful run
        on one side by n and require only some finite successful run on the other; repeat
        in the opposite direction. This asymmetry of fuel bounds is what allows direct
        unfolding steps to be included in the relation. *)
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
    (** Every handle is strengthened-related to itself. *)
    Lemma strengthen_self h : strengthen X h h.
    Proof. destruct h; [apply (X_self X Props)|apply (X_self X Props)|left; apply (X_self X Props)]. Qed.
    (** Data cannot be related to a bare Thunk. *)
    Lemma data_not_thunk d th : ~X (Data d) (Thunk th).
    Proof.
      intro R; destruct (proj2 (preserve_thunk X Props _ _ R) (ex_intro _ th eq_refl)) as [u E]; discriminate.
    Qed.
    (** A bare Thunk cannot be related to Data. *)
    Lemma thunk_not_data th d : ~X (Thunk th) (Data d).
    Proof.
      intro R; destruct (proj1 (preserve_thunk X Props _ _ R) (ex_intro _ th eq_refl)) as [u E]; discriminate.
    Qed.
    (** Data cannot be related to an Encode in the forward direction. *)
    Lemma data_not_encode d e : ~X (Data d) (Encode e).
    Proof. intro R; apply (encode_reverse_absent X Props e (Data d) R); intros [e' E]; discriminate. Qed.

    (** The structural and evaluation laws strengthen every related pair of handles. *)
    Lemma related_strengthen h1 h2 : X h1 h2 -> strengthen X h1 h2.
    Proof.
      (* The ordinary relation describes complete handles. A Force continuation needs a
          relation between the work remaining in their replies. Constructor preservation
          rules out incompatible Data/Thunk cases; Encode execution facts connect the
          remaining mixed cases to the same underlying Object. *)
      intro Related; destruct h1 as [d1|th1|e1], h2 as [d2|th2|e2].
      - apply related_data_strengthen; exact Related.
      - exfalso; eapply data_not_thunk; exact Related.
      - exfalso; eapply data_not_encode; exact Related.
      - exfalso; eapply thunk_not_data; exact Related.
      - exact Related.
      - exfalso; destruct (proj1 (preserve_thunk X Props _ _ Related)
          (ex_intro _ th1 eq_refl)) as [th E]; discriminate.
      - pose proof (execute_unique _ _ (encode_eval X Props _ _ Related
          ltac:(intros [e E]; discriminate))) as Exec.
        pose proof (execute_to_lift _ _ Exec) as Lifted.
        destruct e1; apply (execute_result X Props); exact Lifted.
      - exfalso; destruct (proj2 (preserve_thunk X Props _ _ Related)
          (ex_intro _ th2 eq_refl)) as [th E]; discriminate.
      - destruct e1, e2.
        + apply strict_strengthen; exact Related.
        + exfalso; eapply (not_strict_shallow X Props); exact Related.
        + exfalso; eapply (not_shallow_strict X Props); exact Related.
        + apply shallow_strengthen; exact Related.
    Qed.

    (** A successful finite continuation of the left Think result can be matched on the
        right with strengthened-related Data. *)
    Lemma force_reply_forward n (IH : force_transport n) h1 h2 v1 :
      strengthen X h1 h2 -> force_after n h1 = Some v1 ->
      exists v2, force_reply h2 = Some v2 /\ strengthen X v1 v2.
    Proof.
      (* When both replies defer work, the Force induction hypothesis supplies the match.
          The less immediate case is an Encode related to final Data: the relation already
          proves execution to the corresponding lifted Object, so no new computation fact
          needs to be assumed. *)
      intros Related Fuel; destruct h1 as [d1|t1|e1], h2 as [d2|t2|e2].
      - inversion Fuel; subst; exists (Data d2); split; [reflexivity|exact Related].
      - exfalso; eapply data_not_thunk; exact Related.
      - exfalso; eapply data_not_encode; exact Related.
      - exfalso; eapply thunk_not_data; exact Related.
      - destruct (proj1 (IH t1 t2 Related) v1 Fuel) as [v2 [F2 Strength]].
        eauto using force_unique.
      - destruct (proj1 (IH t1 (encode_to_thunk e2) Related) v1 Fuel) as [v2 [F2 Strength]].
        eauto using force_unique.
      - assert (X (Encode (Strict (encode_to_thunk e1))) (lift (Data d2))) as ExecRel by
          (destruct e1; exact Related).
        pose proof (execute_unique _ _ (encode_eval X Props _ _ ExecRel ltac:(intros [e E]; discriminate))) as Exec.
        rewrite execute_unfold in Exec; simpl in Exec.
        assert (force (encode_to_thunk e1) = Some v1) as F1 by
          (apply force_unique; exists n; destruct e1; exact Fuel).
        rewrite F1 in Exec; inversion Exec as [Lift].
        destruct (force_data _ _ F1) as [d ->].
        exists (Data d2); split; [reflexivity|].
        change (X (Data (lift_data d)) (Data (lift_data d2))); rewrite <- Lift; apply (X_self X Props).
      - destruct (proj1 (IH (encode_to_thunk e1) t2 Related) v1 ltac:(destruct e1; exact Fuel)) as [v2 [F2 Strength]].
        eauto using force_unique.
      - destruct Related as [Thunks|Forces].
        + destruct (proj1 (IH (encode_to_thunk e1) (encode_to_thunk e2) Thunks) v1 ltac:(destruct e1; exact Fuel)) as [v2 [F2 Strength]].
          eauto using force_unique.
        + assert (force (encode_to_thunk e1) = Some v1) as F1 by
            (apply force_unique; exists n; destruct e1; exact Fuel).
          rewrite F1 in Forces; destruct (force (encode_to_thunk e2)) as [v2|] eqn:F2; [|contradiction].
          destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->].
          exists (Data d2); split; [exact F2|exact Forces].
    Qed.
    (** A successful finite continuation of the right Think result can be matched on the
        left with strengthened-related Data. *)
    Lemma force_reply_backward n (IH : force_transport n) h1 h2 v2 :
      strengthen X h1 h2 -> force_after n h2 = Some v2 ->
      exists v1, force_reply h1 = Some v1 /\ strengthen X v1 v2.
    Proof.
      (* The same argument must work starting from a right-hand success. In an
          Encode/Data pair, the Data is already final, while the relation supplies the
          left execution needed to reach it. For two deferred replies, reuse the finite
          forcing argument. *)
      intros Related Fuel; destruct h1 as [d1|t1|e1], h2 as [d2|t2|e2].
      - inversion Fuel; subst; exists (Data d1); split; [reflexivity|exact Related].
      - exfalso; eapply data_not_thunk; exact Related.
      - exfalso; eapply data_not_encode; exact Related.
      - exfalso; eapply thunk_not_data; exact Related.
      - destruct (proj2 (IH t1 t2 Related) v2 Fuel) as [v1 [F1 Strength]].
        eauto using force_unique.
      - destruct (proj2 (IH t1 (encode_to_thunk e2) Related) v2 ltac:(destruct e2; exact Fuel)) as [v1 [F1 Strength]].
        eauto using force_unique.
      - inversion Fuel; subst v2.
        assert (X (Encode (Strict (encode_to_thunk e1))) (lift (Data d2))) as ExecRel by
          (destruct e1; exact Related).
        pose proof (execute_unique _ _ (encode_eval X Props _ _ ExecRel ltac:(intros [e E]; discriminate))) as Exec.
        rewrite execute_unfold in Exec; simpl in Exec; apply omap_some in Exec.
        destruct Exec as [v1 [F1 Lift]].
        destruct (force_data _ _ F1) as [d ->].
        cbn [lift] in Lift.
        exists (Data d); split; [exact F1|].
        change (X (Data (lift_data d)) (Data (lift_data d2))); rewrite <- Lift; apply (X_self X Props).
      - destruct (proj2 (IH (encode_to_thunk e1) t2 Related) v2 Fuel) as [v1 [F1 Strength]].
        eauto using force_unique.
      - destruct Related as [Thunks|Forces].
        + destruct (proj2 (IH (encode_to_thunk e1) (encode_to_thunk e2) Thunks) v2 ltac:(destruct e2; exact Fuel)) as [v1 [F1 Strength]].
          eauto using force_unique.
        + assert (force (encode_to_thunk e2) = Some v2) as F2 by
            (apply force_unique; exists n; destruct e2; exact Fuel).
          rewrite F2 in Forces; destruct (force (encode_to_thunk e1)) as [v1|] eqn:F1; [|contradiction].
          destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->].
          exists (Data d1); split; [exact F1|exact Forces].
    Qed.
    (** A successful Think step followed to Data gives a successful force of the original
        Thunk. *)
    Lemma force_reply_some th h v : thinks_to th h -> force_reply h = Some v -> forces_to th v.
    Proof.
      intros Think Reply; apply force_some; rewrite force_unfold.
      rewrite (think_unique _ _ Think); exact Reply.
    Qed.
    (** Transport for Think and Force at one fuel level gives Force transport at the next
        level. *)
    Theorem force_step n : think_transport n -> force_transport n -> force_transport (S n).
    Proof.
      (* A successful force has a first Think reply and a finite continuation to Data. A
          matched pair of replies uses the continuation relation. If the first Think
          instead directly unfolds to the other Thunk, both forces share the same
          remaining computation, so that continuation can be reused. *)
      intros ThinkIH ForceIH t1 t2 R; split.
      - intros v1 Fuel.
        change (obind (think_with_fuel n t1) (force_after n) = Some v1) in Fuel.
        apply obind_some in Fuel; destruct Fuel as [r1 [Think1 Reply1]].
        destruct (proj1 (ThinkIH t1 t2 R) r1 Think1) as
          [[r2 [Think2 Strength]]|[OneThunk|[OneStrict|OneShallow]]].
        + destruct (force_reply_forward n ForceIH r1 r2 v1 Strength Reply1) as [v2 [Reply2 Result]].
          (* In the reverse argument, first fix the right force's unique result. A direct
            unfolding on the left can then prepend itself to this completed right
            computation. *)
        assert (SecondForced : forces_to t2 v2).
          { eapply force_reply_some; [exact Think2|exact Reply2]. }
          exists v2; split; assumption.
        (* Here the left Think step has reached the right source itself. There is no
            reason to demand another matching Think step; the already successful
            continuation is a witness for the right force. *)
        + subst r1; exists v1; split; [exists n; exact Reply1|apply strengthen_self].
        + subst r1; exists v1; split; [exists n; exact Reply1|apply strengthen_self].
        + subst r1; exists v1; split; [exists n; exact Reply1|apply strengthen_self].
      - intros v2 Fuel.
        assert (SecondForced : force t2 = Some v2).
        { apply force_unique; exists (S n); exact Fuel. }
        change (obind (think_with_fuel n t2) (force_after n) = Some v2) in Fuel.
        apply obind_some in Fuel; destruct Fuel as [r2 [Think2 Reply2]].
        destruct (proj2 (ThinkIH t1 t2 R) r2 Think2) as
          [[r1 [Think1 Strength]]|[OneThunk|[OneStrict|OneShallow]]].
        + destruct (force_reply_backward n ForceIH r1 r2 v2 Strength Reply2) as [v1 [Reply1 Result]].
          assert (FirstForced : forces_to t1 v1).
          { eapply force_reply_some; [exact Think1|exact Reply1]. }
          exists v1; split; assumption.
        + exists v2; split; [|apply strengthen_self].
          eapply force_reply_some; [exact OneThunk|exact SecondForced].
        + exists v2; split; [|apply strengthen_self].
          eapply force_reply_some; [exact OneStrict|exact SecondForced].
        + exists v2; split; [|apply strengthen_self].
          eapply force_reply_some; [exact OneShallow|exact SecondForced].
    Qed.
    Definition execute_transport n e1 e2 :=
      (forall v1, execute_with_fuel n e1 = Some v1 -> exists v2, executes_to e2 v2 /\ X v1 v2) /\
      (forall v2, execute_with_fuel n e2 = Some v2 -> exists v1, executes_to e1 v1 /\ X v1 v2).
    (** Finite-fuel strict execution lifts its Thunk's forcing result. *)
    Lemma execute_fuel_strict n th : execute_with_fuel n (Strict th) = omap lift (force_with_fuel n th).
    Proof. destruct n; reflexivity. Qed.
    (** Finite-fuel shallow execution lowers its Thunk's forcing result. *)
    Lemma execute_fuel_shallow n th : execute_with_fuel n (Shallow th) = omap lower (force_with_fuel n th).
    Proof. destruct n; reflexivity. Qed.
    (** Strengthened-related Data has both related lifted Objects and related lowered Refs. *)
    Lemma lifted_force_results h1 h2 :
      (exists d1, h1 = Data d1) -> (exists d2, h2 = Data d2) ->
      strengthen X h1 h2 -> X (lift h1) (lift h2) /\ X (lower h1) (lower h2).
    Proof.
      intros [d1 ->] [d2 ->] R; split; [exact R|].
      apply (lift_to_lower X (blob_ref_cong X Props) (tree_ref_cong X Props)
        (preserve_tree X Props) (preserve_blob X Props)); exact R.
    Qed.
    (** Related unfuelled execution results supply both directions of finite execution
        transport. *)
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
    (** Force transport yields Encode execution transport whenever the Encode's result
        mapping preserves the relation. *)
    Lemma execute_thunks_transport n (IH : force_transport n) wrap project
        (Fuel : forall k th, execute_with_fuel k (wrap th) = omap project (force_with_fuel k th))
        (Results : forall h1 h2, (exists d1, h1 = Data d1) -> (exists d2, h2 = Data d2) ->
          strengthen X h1 h2 -> X (project h1) (project h2)) t1 t2 :
      X (Thunk t1) (Thunk t2) -> execute_transport n (wrap t1) (wrap t2).
    Proof.
      (* Force supplies the common computational result; strictness only determines the
          observation made of that Data. Once the result mapping preserves the relation,
          the matching finite force can be reused to justify the whole Encode execution. *)
      intro Related; split; intros v Executed; rewrite Fuel in Executed;
        apply omap_some in Executed; destruct Executed as [h [Forced ->]].
      - destruct (proj1 (IH t1 t2 Related) h Forced) as [h2 [[k Forced2] Strength]].
        exists (project h2); split.
        + exists k; rewrite Fuel, Forced2; reflexivity.
        + apply Results; [eapply force_with_fuel_to_data; exact Forced|
            eapply force_with_fuel_to_data; exact Forced2|exact Strength].
      - destruct (proj2 (IH t1 t2 Related) h Forced) as [h1 [[k Forced1] Strength]].
        exists (project h1); split.
        + exists k; rewrite Fuel, Forced1; reflexivity.
        + apply Results; [eapply force_with_fuel_to_data; exact Forced1|
            eapply force_with_fuel_to_data; exact Forced|exact Strength].
    Qed.
    (** Related strict Encodes transport successful execution in both directions. *)
    Lemma execute_strict_transport n (IH : force_transport n) t1 t2 :
      X (Encode (Strict t1)) (Encode (Strict t2)) -> execute_transport n (Strict t1) (Strict t2).
    Proof.
      (* A relation between strict Encodes may come from their source Thunks or from
          their completed executions. The first justification needs Force transport; the
          second already gives the result relation. Requiring only one of these reasons
          would lose valid derivations. *)
      intro Related; destruct (strict_encode_reasons X Props _ _ Related) as [Thunks|Exec].
      - eapply (execute_thunks_transport n IH Strict lift execute_fuel_strict);
          [intros h1 h2 D1 D2 Strength; exact (proj1 (lifted_force_results h1 h2 D1 D2 Strength))|exact Thunks].
      - apply execute_related_options; exact Exec.
    Qed.
    (** Related shallow Encodes transport successful execution in both directions. *)
    Lemma execute_shallow_transport n (IH : force_transport n) t1 t2 :
      X (Encode (Shallow t1)) (Encode (Shallow t2)) -> execute_transport n (Shallow t1) (Shallow t2).
    Proof.
      (* The same two justifications occur for shallow Encodes. Completed forces are
          compared through Objects, but Ref congruence transfers those facts to the Refs
          that a shallow consumer actually receives. *)
      intro Related; destruct (shallow_encode_reasons X Props _ _ Related) as [Thunks|Exec].
      - eapply (execute_thunks_transport n IH Shallow lower execute_fuel_shallow);
          [intros h1 h2 D1 D2 Strength; exact (proj2 (lifted_force_results h1 h2 D1 D2 Strength))|exact Thunks].
      - apply execute_related_options; exact Exec.
    Qed.
    (** Related Encodes transport successful execution in both directions, with matching
        strictness. *)
    Lemma execute_encodes_transport n (IH : force_transport n) e1 e2 :
      X (Encode e1) (Encode e2) -> execute_transport n e1 e2.
    Proof.
      destruct e1 as [t1|t1], e2 as [t2|t2]; intro Related.
      - apply execute_strict_transport; assumption.
      - exfalso; exact (not_strict_shallow X Props t1 t2 Related).
      - exfalso; exact (not_shallow_strict X Props t1 t2 Related).
      - apply execute_shallow_transport; assumption.
    Qed.


    Section BoundedThink.
      Variable n : nat.
      (** Evaluation transports related inputs in both directions at the current fuel
          level. *)
      Hypothesis IH_eval : eval_transport n.
      (** Forcing transports related Thunks in both directions at the current fuel level. *)
      Hypothesis IH_force : force_transport n.

      (** A finite evaluation of the left Tree can be matched by evaluation of the related
          right Tree. *)
      Lemma tree_forward t1 t2 v1 : X (HTreeObj t1) (HTreeObj t2) ->
        eval_tree_with_fuel n t1 = Some v1 ->
        exists v2, eval_tree t2 = Some v2 /\ X (HTreeObj v1) (HTreeObj v2).
      Proof.
        intros R Ev; destruct (proj1 (eq_tree_eval_fuel_n X n (tree_cong X Props)
          (tag_cong X Props) (tree_complete X Props) IH_eval t1 t2 R) v1 Ev) as [v2 [Ev2 Related]].
        exists v2; split; [apply eval_tree_unique; exact Ev2|exact Related].
      Qed.
      (** A finite evaluation of the right Tree can be matched by evaluation of the related
          left Tree. *)
      Lemma tree_backward t1 t2 v2 : X (HTreeObj t1) (HTreeObj t2) ->
        eval_tree_with_fuel n t2 = Some v2 ->
        exists v1, eval_tree t1 = Some v1 /\ X (HTreeObj v1) (HTreeObj v2).
      Proof.
        intros R Ev; destruct (proj2 (eq_tree_eval_fuel_n X n (tree_cong X Props)
          (tag_cong X Props) (tree_complete X Props) IH_eval t1 t2 R) v2 Ev) as [v1 [Ev1 Related]].
        exists v1; split; [apply eval_tree_unique; exact Ev1|exact Related].
      Qed.
      (** A finite application Think step on the left has a strengthened-related matching
          step on the right. *)
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
      (** A finite selection Think step on the left has a strengthened-related matching
          step on the right. *)
      Lemma selection_think_forward t1 t2 r1 : X (HTreeObj t1) (HTreeObj t2) ->
        think_with_fuel (S n) (Selection t1) = Some r1 ->
        exists r2, thinks_to (Selection t2) r2 /\ strengthen X r1 r2.
      Proof.
        intros R Ev; change (obind (eval_tree_with_fuel n t1) Sl.select = Some r1) in Ev.
        apply obind_some in Ev; destruct Ev as [v1 [Fuel1 Selected1]].
        destruct (tree_forward t1 t2 v1 R Fuel1) as [v2 [Tree2 Related]].
        assert (eval_tree t1 = Some v1) as Tree1 by (apply eval_tree_unique; exists n; exact Fuel1).
        pose proof (slice_values v1 v2 Related
          (eval_tree_to_not_encode _ _ Tree1) (eval_tree_to_not_encode _ _ Tree2)) as Selected.
        rewrite Selected1 in Selected; destruct (Sl.select v2) as [r2|] eqn:Selected2; [|contradiction].
        exists r2; split; [|apply related_strengthen; exact Selected].
        apply think_some; rewrite think_selection, Tree2; exact Selected2.
      Qed.
      (** A finite identification Think step on the left has a strengthened-related
          matching step on the right. *)
      Lemma identification_think_forward d1 d2 r1 : X (Data (Ref d1)) (Data (Ref d2)) ->
        think_with_fuel (S n) (Identification d1) = Some r1 ->
        exists r2, thinks_to (Identification d2) r2 /\ strengthen X r1 r2.
      Proof.
        intros R Ev; change (Some (Data (Ref d1)) = Some r1) in Ev; inversion Ev; subst.
        exists (Data (Ref d2)); split; [exists 1; reflexivity|apply related_data_strengthen; exact R].
      Qed.
      (** A finite application Think step on the right has a strengthened-related matching
          step on the left. *)
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
      (** A finite selection Think step on the right has a strengthened-related matching
          step on the left. *)
      Lemma selection_think_backward t1 t2 r2 : X (HTreeObj t1) (HTreeObj t2) ->
        think_with_fuel (S n) (Selection t2) = Some r2 ->
        exists r1, thinks_to (Selection t1) r1 /\ strengthen X r1 r2.
      Proof.
        intros R Ev; change (obind (eval_tree_with_fuel n t2) Sl.select = Some r2) in Ev.
        apply obind_some in Ev; destruct Ev as [v2 [Fuel2 Selected2]].
        destruct (tree_backward t1 t2 v2 R Fuel2) as [v1 [Tree1 Related]].
        assert (eval_tree t2 = Some v2) as Tree2 by (apply eval_tree_unique; exists n; exact Fuel2).
        pose proof (slice_values v1 v2 Related
          (eval_tree_to_not_encode _ _ Tree1) (eval_tree_to_not_encode _ _ Tree2)) as Selected.
        rewrite Selected2 in Selected; destruct (Sl.select v1) as [r1|] eqn:Selected1; [|contradiction].
        exists r1; split; [|apply related_strengthen; exact Selected].
        apply think_some; rewrite think_selection, Tree1; exact Selected1.
      Qed.
      (** A finite identification Think step on the right has a strengthened-related
          matching step on the left. *)
      Lemma identification_think_backward d1 d2 r2 : X (Data (Ref d1)) (Data (Ref d2)) ->
        think_with_fuel (S n) (Identification d2) = Some r2 ->
        exists r1, thinks_to (Identification d1) r1 /\ strengthen X r1 r2.
      Proof.
        intros R Ev; change (Some (Data (Ref d2)) = Some r2) in Ev; inversion Ev; subst.
        exists (Data (Ref d1)); split; [exists 1; reflexivity|apply related_data_strengthen; exact R].
      Qed.
      (** A left Think step has a related right step, or directly returns the right Thunk
          or one of its Encodes. *)
      Theorem think_forward_from_eval t1 t2 r1 : X (Thunk t1) (Thunk t2) ->
        think_with_fuel (S n) t1 = Some r1 ->
        (exists r2, thinks_to t2 r2 /\ strengthen X r1 r2) \/
        r1 = Thunk t2 \/ r1 = Encode (Strict t2) \/ r1 = Encode (Shallow t2).
      Proof.
        (* An application or selection can compare its next replies once the operand
            Trees have been evaluated. Direct-unfolding constructors are different: the
            left reply may already be the right source Thunk, so the conclusion retains
            that possibility instead of demanding an extra right-hand computation. *)
        intros Related Fuel.
        assert (Computed : think t1 = Some r1)
          by (apply think_unique; exists (S n); exact Fuel).
        assert (Reasons : thunk_reasons X t1 t2)
          by (exact (related_thunk_reasons X Props _ _ Related)).
        hauto cases: - use: application_think_forward, selection_think_forward,
          identification_think_forward, think_some unfold!: thunk_reasons.
      Qed.
      (** A right Think step has a related left step, or the left Thunk directly reaches
          the right Thunk or one of its Encodes. *)
      Theorem think_backward_from_eval t1 t2 r2 : X (Thunk t1) (Thunk t2) ->
        think_with_fuel (S n) t2 = Some r2 ->
        (exists r1, thinks_to t1 r1 /\ strengthen X r1 r2) \/
        thinks_to t1 (Thunk t2) \/ thinks_to t1 (Encode (Strict t2)) \/ thinks_to t1 (Encode (Shallow t2)).
      Proof.
        (* A successful right Think step can normally be matched on the left. If the
            relation instead says that the left Thunk unfolds directly to the right one,
            preserve that left step as evidence; its subsequent continuation can consume
            the already known right result. *)
        intros Related Fuel.
        assert (Computed : think t2 = Some r2)
          by (apply think_unique; exists (S n); exact Fuel).
        assert (Reasons : thunk_reasons X t1 t2)
          by (exact (related_thunk_reasons X Props _ _ Related)).
        hauto cases: - use: application_think_backward, selection_think_backward,
          identification_think_backward, think_some unfold!: thunk_reasons.
      Qed.
    End BoundedThink.
    (** Evaluation and Force transport at one fuel level give Think transport at the next
        level. *)
    Theorem think_step n : eval_transport n -> force_transport n -> think_transport (S n).
    Proof.
      intros Eval Force t1 t2 Related; split.
      - intros r Fuel; exact (think_forward_from_eval n Eval t1 t2 r Related Fuel).
      - intros r Fuel; exact (think_backward_from_eval n Eval t1 t2 r Related Fuel).
    Qed.
    (** Executing an Encode and evaluating its result gives evaluation of the Encode. *)
    Lemma eval_after_execute e h v : executes_to e h -> evals_to h v -> evals_to (Encode e) v.
    Proof.
      intros Exec Ev; apply eval_some; rewrite eval_encode, (execute_unique _ _ Exec).
      exact (eval_unique _ _ Ev).
    Qed.
    (** Force and evaluation transport extend evaluation transport to pairs of Encodes. *)
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
    (** Related handles fixed by evaluation transport their evaluations in both directions. *)
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
    (** Evaluation transport extends one fuel step for related inputs whose left handle is
        not an Encode. *)
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
    (** A related Encode and non-Encode transport evaluations by the Encode's already-known
        execution result. *)
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
    (** Force and evaluation transport extend evaluation transport to the next fuel level. *)
    Theorem eval_step n : force_transport n -> eval_transport n -> eval_transport (S n).
    Proof.
      (* Leaf Values need no computation, and Trees use the argument for their children.
          Encodes require execution followed by evaluation of the produced Data. Because
          Think, Force, and evaluation call one another, these cases must advance their
          shared fuel induction together. *)
      intros IH_force IH_eval h1 h2 R; destruct h1 as [d|th|e].
      - apply eval_nonencode_step; [exact IH_eval|exact R|intros [e E]; discriminate].
      - apply eval_nonencode_step; [exact IH_eval|exact R|intros [e E]; discriminate].
      - destruct h2 as [d|th|e2].
        + apply eval_encode_nonencode; [exact R|intros [e' E]; discriminate].
        + apply eval_encode_nonencode; [exact R|intros [e' E]; discriminate].
        + apply eval_encodes_transport; assumption.
    Qed.
    (** Think transport holds at zero fuel, where no Think step can succeed. *)
    Lemma think_zero : think_transport 0.
    Proof. intros t1 t2 R; split; intros v Fuel; discriminate. Qed.
    (** Force transport holds at zero fuel, where no force can succeed. *)
    Lemma force_zero : force_transport 0.
    Proof. intros t1 t2 R; split; intros v Fuel; discriminate. Qed.
    (** Evaluation transport holds at zero fuel for the forms evaluation can already return
        unchanged. *)
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
    (** The three mutually dependent preservation statements hold at every finite fuel
        bound. Their joint proof breaks the apparent cycle between evaluating operands,
        thinking a Thunk, and forcing its returned computation. *)
    Theorem eq_forces_to_induct n : think_transport n /\ force_transport n /\ eval_transport n.
    Proof.
      (* At a successor bound, each evaluator depends only on calls from the preceding
          bound. Prove all three successor statements from that common stock of
          hypotheses; no evaluator-preservation theorem is assumed in order to prove
          itself. *)
      induction n as [|n [Think [Force Eval]]].
      - split; [apply think_zero|split; [apply force_zero|apply eval_zero]].
      - assert (NextThink : think_transport (S n)) by (apply think_step; assumption).
        assert (NextForce : force_transport (S n)) by (apply force_step; assumption).
        assert (NextEval : eval_transport (S n)) by (apply eval_step; assumption).
        auto.
    Qed.
    (** Two-way transport of successful payloads gives the optional relation, including
        agreement on failure. *)
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
    (** Evaluation preserves the handle relation and agrees on whether it succeeds. *)
    Theorem evals_X h1 h2 : X h1 h2 -> rel_opt X (eval h1) (eval h2).
    Proof.
      (* Every actual successful run has some finite witness. The joint induction
          supplies a matching finite run on the other side at that witness bound. Applying
          this in both directions rules out a successful result on just one side. *)
      intro Related.
      assert (FiniteEvaluations : forall n, eval_transport n)
        by (hauto cases: - use: eq_forces_to_induct).
      apply option_transport;
        hauto cases: - use: eval_some, eval_unique unfold!: eval_transport.
    Qed.
    (** Forcing related Thunks preserves the strengthened relation and agrees on success. *)
    Lemma forces_strengthen t1 t2 : X (Thunk t1) (Thunk t2) ->
      rel_opt (strengthen X) (force t1) (force t2).
    Proof.
      intro Related.
      assert (FiniteForcing : forall n, force_transport n)
        by (hauto cases: - use: eq_forces_to_induct).
      apply option_transport;
        hauto cases: - use: force_some, force_unique unfold!: force_transport.
    Qed.
    (** Forcing related Thunks preserves the relaxed relation and agrees on success. *)
    Theorem forces_X t1 t2 : X (Thunk t1) (Thunk t2) ->
      rel_opt (relaxed_X X) (force t1) (force t2).
    Proof.
      intro Related; pose proof (forces_strengthen t1 t2 Related) as Result.
      destruct (force t1) as [h1|] eqn:F1, (force t2) as [h2|] eqn:F2;
        cbn [rel_opt] in *; try contradiction; [|exact I].
      destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->]; exact Result.
    Qed.
    (** Thinking related Thunks either returns strengthened-related results or directly
        unfolds the left Thunk to the right one. *)
    Theorem think_X t1 t2 : X (Thunk t1) (Thunk t2) ->
      rel_opt (strengthen X) (think t1) (think t2) \/
      think t1 = Some (Thunk t2) \/ think t1 = Some (Encode (Strict t2)) \/ think t1 = Some (Encode (Shallow t2)).
    Proof.
      intro Related; destruct (think t1) as [v1|] eqn:T1.
      - destruct (proj1 (think_some _ _) T1) as [n Fuel].
        destruct (eq_forces_to_induct n) as [Thinking _].
        destruct (Thinking t1 t2 Related) as [Forward _].
        destruct (Forward v1 Fuel) as
          [[v2 [T2 Strength]]|[OneThunk|[OneStrict|OneShallow]]].
        + left; rewrite (think_unique _ _ T2); exact Strength.
        + right; left; congruence.
        + right; right; left; congruence.
        + right; right; right; congruence.
      - destruct (think t2) as [v2|] eqn:T2; [|left; exact I].
        destruct (proj1 (think_some _ _) T2) as [n Fuel].
        destruct (eq_forces_to_induct n) as [Thinking _].
        destruct (Thinking t1 t2 Related) as [_ Backward].
        destruct (Backward v2 Fuel) as
          [[v1 [T1' Strength]]|[OneThunk|[OneStrict|OneShallow]]];
          exfalso; first [pose proof (think_unique _ _ T1')|pose proof (think_unique _ _ OneThunk)|
            pose proof (think_unique _ _ OneStrict)|pose proof (think_unique _ _ OneShallow)]; congruence.
    Qed.
  End Relation.
End EvaluationProperties.
