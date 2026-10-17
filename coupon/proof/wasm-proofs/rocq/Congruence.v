From FixProof Require Import Handle ApplyTree EvaluationProperties.

(** Compatibility entry point for the coupled congruence proof, which lives
    with the evaluator properties as in the original Isabelle theory. *)
Module Congruence (S : STORAGE) (P : PROGRAM S).
  Include EvaluationProperties S P.
End Congruence.
