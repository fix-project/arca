(** Models issuance from primitive computation certificates and collector
    exchanges. Proves issued claims sound and every terminating evaluation
    certifiable through the public exchanges. *)

From Stdlib Require Import List Bool.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Collector Require Import Matching Encoding Exchanges.
From FixProof.Evaluation Require Import Adequacy.
Import ListNotations.

Module CertificationCompleteness (S : STORAGE) (P : PROGRAM S).
  Module X := CouponExchanges S P.
  Module EC := EvaluationAdequacy S P X.D.Context.EC.E.EP.
  Import X X.D X.D.Context.EC.E.EP X.D.Context.EC.E.EP.E X.D.Context.EC.E.EP.E.H.

  (** This records the certificates a scheduler could obtain: exact application and
      selection facts come from their trusted issuers, while other facts arise by accepted
      exchanges on certificates already obtained. An issuance derivation is finite
      evidence of availability; it does not require the collector to perform the
      computations being certified. *)
  Inductive issued issuing_author : handle -> Prop :=
  | IssuedApplication tree result :
      value_handle (HTreeObj tree) -> A.apply_tree tree = Some result ->
      issued issuing_author (Wire.witness (HTreeObj (create_tree [])) 2 (HTreeObj tree) result)
  | IssuedSelection tree result :
      Sl.select tree = Some result ->
      issued issuing_author (Wire.witness (HTreeObj (create_tree [])) 6 (HTreeObj tree) result)
  | IssuedExchange rule coupons lhs rhs kind output :
      Forall (issued issuing_author) coupons -> check rule coupons lhs rhs = Some kind ->
      X.issue issuing_author kind lhs rhs coupons output -> issued issuing_author output.

  Definition certifies issuing_author kind lhs rhs := exists witness,
    issued issuing_author witness /\ read kind witness = Some (lhs, rhs).

Load "fix/proof/collector/internal/Completeness/Issuance/read_encoded".
Load "fix/proof/collector/machinery/Completeness/SoundnessInduction".
  (** A scheduler-built issuance derivation establishes a genuine logical claim. The
      scheduler's choices of witnesses and exchanges add no inference power beyond the
      primitive certificates and checked collector rules. *)
  Corollary certification_sound issuing_author kind lhs rhs :
    certifies issuing_author kind lhs rhs -> relation_of kind lhs rhs.
  Proof. intros [witness [Issued Read]]; eauto using read_sound, issued_sound. Qed.

Load "fix/proof/collector/machinery/Completeness/Tactics".
  Section Certification.
    Variable issuing_author : handle.

Load "fix/proof/collector/internal/Completeness/Construction/certify_leaf".
Load "fix/proof/collector/machinery/Completeness/MutualInduction".
    (** Every successful Fix evaluation admits a finite schedule of primitive certificate
        requests and public collector exchanges. This is completeness for the exposed
        logic, rather than a theorem that every Fix program terminates. *)
Load "fix/proof/collector/machinery/Completeness/terminating_evaluation_is_certifiable".
    Theorem terminating_evaluation_is_certifiable value result :
      eval value = Some result -> certifies issuing_author Eval value result.
  Proof. apply terminating_evaluation_is_certifiable_proof. Qed.
  End Certification.
End CertificationCompleteness.
