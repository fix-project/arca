(** Closes the underlying relation under reflexivity, symmetry, and
    transitivity, and proves that evaluation preserves this equivalence. *)

From Stdlib Require Import List Relations Relation_Operators.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Equivalence Require Import Equivalence.
Import ListNotations.

Module EquivalenceClosure (S : STORAGE) (P : PROGRAM S).
  Module E := Equivalence S P.
  Import E E.EP E.EP.E E.EP.E.H.
  (** R gives local justifications for replacing one computation by another, including a
      directed step from an Encode to its executed result. A finite chain of such
      justifications may reverse steps or pass through intermediate handles. Closing R
      captures exactly that freedom to compose derivations. *)
  Definition computational_equiv := clos_refl_sym_trans handle R.
Load "fix/proof/equivalence/internal/Closure/Constructors".
  (** Computational equivalence is compatible with evaluation: replacing an input by an
      equivalent one preserves existence of a successful result and preserves the result
      up to equivalence. *)
Load "fix/proof/equivalence/machinery/Closure/equiv_eval".
  Theorem equiv_eval h1 h2 : computational_equiv h1 h2 -> rel_opt computational_equiv (eval h1) (eval h2).
  Proof. apply equiv_eval_proof. Qed.
Load "fix/proof/equivalence/machinery/Closure/EvaluationFuel".
Load "fix/proof/equivalence/internal/Closure/Computation".
  (** Computational equivalence cannot change Blob contents. An equivalent endpoint may
      still be an explicit strict computation, but its evaluated Blob must contain exactly
      the original bytes. *)
Load "fix/proof/equivalence/machinery/Closure/equiv_blob_same_data".
  Theorem equiv_blob_same_data h1 h2 : computational_equiv h1 h2 -> forall b1, h1 = HBlobObj b1 ->
    (exists b2, h2 = HBlobObj b2 /\ get_blob_data b1 = get_blob_data b2) \/
    (exists th b2, h2 = Encode (Strict th) /\ eval h2 = Some (HBlobObj b2) /\ get_blob_data b1 = get_blob_data b2).
  Proof. apply equiv_blob_same_data_proof. Qed.
Load "fix/proof/equivalence/internal/Closure/References".
End EquivalenceClosure.
