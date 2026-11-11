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
