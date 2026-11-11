(** Proves that evaluating a Ref-backed coupon preserves its assertion
    and conditional authority. *)

From Stdlib Require Import List.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Collector Require Import Exchanges.
Import ListNotations.

Module CollectorPreservation (S : STORAGE) (P : PROGRAM S).
  Module X := CouponExchanges S P.
  Import X X.D X.D.Context.EC.E.EP X.D.Context.EC.E.EP.E X.D.Context.EC.E.EP.E.H Wire.

Load "fix/proof/collector/machinery/Preservation/evaluation_preserves_authority".
  Theorem evaluation_preserves_authority trusted tag claim evaluated kind author value result :
    let before := HTreeObj (create_tree [HTreeObj tag; HTreeObj claim]) in
    let after := HTreeObj (create_tree [HTreeObj evaluated; HTreeObj claim]) in
    Wire.decode before = Some (kind, author, value, result) ->
    eval (HTreeObj tag) = Some (HTreeObj evaluated) ->
    stable_conditional trusted before ->
    exists evaluated_author,
      Wire.decode after = Some (kind, evaluated_author, value, result) /\
      stable_conditional trusted after.
  Proof. apply evaluation_preserves_authority_proof. Qed.
End CollectorPreservation.
