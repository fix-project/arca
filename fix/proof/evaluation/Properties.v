(** Proves fuel monotonicity and determinism, defines unbounded evaluation,
    and establishes evaluator equations and congruence properties. *)

From Stdlib Require Import List Arith Lia ClassicalEpsilon.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle ApplyTree Evaluation.
Import ListNotations.

Module EvaluationProperties (S : STORAGE) (P : PROGRAM S).
  Module E := Evaluation S P.
  Import E E.H.
Load "fix/proof/evaluation/machinery/Properties/Fuel".
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
Load "fix/proof/evaluation/machinery/Properties/Choice".
  Definition think := unfuel think_with_fuel.
  Definition force := unfuel force_with_fuel.
  Definition execute := unfuel execute_with_fuel.
  Definition eval := unfuel eval_with_fuel.
  Definition eval_tree := unfuel eval_tree_with_fuel.
  Definition encode_to_thunk e := match e with Strict th | Shallow th => th end.
Load "fix/proof/evaluation/internal/Properties/Relations".
Load "fix/proof/evaluation/machinery/Properties/FuelCorrespondence".
Load "fix/proof/evaluation/internal/Properties/EvaluationEquations".
  (** Evaluation leaves leaves unchanged, evaluates Tree children, and executes exposed
      Encodes before continuing. *)
Load "fix/proof/evaluation/machinery/Properties/eval_unfold".
  Theorem eval_unfold h : eval h =
    match h with
    | Thunk _ | Data (Ref _) | Data (Object (BlobObj _)) => Some h
    | Data (Object (TreeObj t)) => omap HTreeObj (eval_tree t)
    | Encode e => obind (execute e) eval
    end.
  Proof. apply eval_unfold_proof. Qed.
Load "fix/proof/evaluation/machinery/Properties/ExecuteFuel".
  (** Executing an Encode forces its Thunk and then lifts a strict result or lowers a
      shallow result. *)
Load "fix/proof/evaluation/machinery/Properties/execute_unfold".
  Theorem execute_unfold e : execute e =
    omap (match e with Strict _ => lift | Shallow _ => lower end)
         (force (encode_to_thunk e)).
  Proof. apply execute_unfold_proof. Qed.

Load "fix/proof/evaluation/internal/Properties/Results".
  (** Successful evaluation returns a Value. *)
Load "fix/proof/evaluation/machinery/Properties/eval_to_value_handle".
  Lemma eval_to_value_handle h r : eval h = Some r -> value_handle r.
  Proof. apply eval_to_value_handle_proof. Qed.
Load "fix/proof/evaluation/machinery/Properties/ForceFuel".
  (** Force follows returned computations until it obtains Data, ignoring whether an
      intermediate reply was packaged as a bare Thunk or an Encode. The final consumer
      determines how that Data is exposed. *)
Load "fix/proof/evaluation/machinery/Properties/force_unfold".
  Theorem force_unfold th : force th = obind (think th) (fun h =>
    match h with
    | Data _ => Some h
    | Thunk th' => force th'
    | Encode e => force (encode_to_thunk e)
    end).
  Proof. apply force_unfold_proof. Qed.

Load "fix/proof/evaluation/internal/Properties/Thinking".
  (** Each Thunk performs its own Think operation, evaluating application and selection
      operand Trees first. *)
Load "fix/proof/evaluation/machinery/Properties/think_unfold".
  Theorem think_unfold th : think th = match th with
    | Application t => obind (eval_tree t) A.apply_tree
    | Identification d => I.identify d
    | Selection t => obind (eval_tree t) Sl.select
    end.
  Proof. apply think_unfold_proof. Qed.

Load "fix/proof/evaluation/internal/Properties/Trees".
  (** Evaluating any Value leaves it unchanged. *)
Load "fix/proof/evaluation/machinery/Properties/value_handle_eval_to_itself".
  Theorem value_handle_eval_to_itself h : value_handle h -> eval h = Some h.
  Proof. apply value_handle_eval_to_itself_proof. Qed.

Load "fix/proof/evaluation/internal/Properties/Values".
Load "fix/proof/evaluation/machinery/Properties/TreeTransport".
Load "fix/proof/evaluation/internal/Properties/Congruence".
  Section Relation.
Load "fix/proof/evaluation/internal/Properties/Transport/value_shapes".
Load "fix/proof/evaluation/machinery/Properties/TransportInduction".
Load "fix/proof/evaluation/internal/Properties/Transport/evals_X".
  End Relation.
End EvaluationProperties.
