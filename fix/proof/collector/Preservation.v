(** Proves that evaluating a Ref-backed coupon Tag preserves its assertion
    and conditional validity, using preservation of its stable authors. *)

From Stdlib Require Import List.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Collector Require Import Exchanges.
Import ListNotations.

Module CollectorPreservation (S : STORAGE) (P : PROGRAM S) (T : TAG_AUTHORS S).
  Module X := CouponExchanges S P T.
  Import X X.D X.D.EC.E.EP X.D.EC.E.EP.E X.D.EC.E.EP.E.H Wire.

  Local Notation "h '⇓' r" := (eval h = Some r) (at level 70).
  Local Notation "'trusts_stable_authors' trusted authors" :=
    (Forall (fun author => value_handle author -> trusted author) (get_tree_raw authors))
    (at level 10, trusted at next level, authors at next level).

  (** Evaluating an authenticated certificate preserves both the fact it asserts and the
      trust condition sufficient to use that fact. This is stronger than merely preserving
      a well-formed Tag. *)
  Theorem evaluation_preserves_authority trusted tag claim evaluated kind authors value result :
    let before := HTreeObj (create_tree [HTreeObj tag; HTreeObj claim]) in
    let after := HTreeObj (create_tree [HTreeObj evaluated; HTreeObj claim]) in
    Wire.decode before = Some (kind, authors, value, result) ->
    eval (HTreeObj tag) = Some (HTreeObj evaluated) ->
    stable_conditional trusted before ->
    exists evaluated_authors,
      Wire.decode after = Some (kind, evaluated_authors, value, result) /\
      stable_conditional trusted after.
  Proof.
    (* There are two independent hazards: evaluation could change the assertion, or it
        could remove an authority on which the assertion depends. The assertion Ref
        prevents the first. The stable-author condition prevents the second, because every
        original Value author remains unchanged in the evaluated author list. *)
    intros before after Before Evaluated Valid.
    (* Authentication fixes which assertion this Tag is allowed to speak about. Recover
        that fixed Ref before reasoning about evaluation of its other fields. *)
    assert (TagShape : tag = create_tree_tagged true [HTreeObj authors; HTreeRef claim])
      by (hauto cases: - use: Wire.decode_tag_shape).
    (* A Ref is a stopping point for evaluation, so the claim's endpoints never enter the
        evaluation traversal. All possible change is confined to the authors field. *)
    assert (AuthorsEvaluate : exists evaluated_authors,
      HTreeObj authors ⇓ HTreeObj evaluated_authors /\
      evaluated = create_tree_tagged true [HTreeObj evaluated_authors; HTreeRef claim])
      by (hauto cases: - use: eval_tree_ref_pair).
    (* The evaluated Tag still refers to the same supplied assertion Object. Its claim
        can therefore be decoded with identical endpoints without loading or evaluating
        that Object. *)
    assert (ClaimPreserved : forall evaluated_authors,
      evaluated = create_tree_tagged true [HTreeObj evaluated_authors; HTreeRef claim] ->
      Wire.decode after = Some (kind, evaluated_authors, value, result))
      by (hauto cases: - use: Wire.decode_replace_authors).
    (* Reason backward from trust of the output authorities. Every original stable
        authority is still present there, so the output assumption implies the old
        assumption needed to establish the fact. *)
    assert (TrustPreserved : forall evaluated_authors,
      HTreeObj authors ⇓ HTreeObj evaluated_authors ->
      trusts_stable_authors trusted evaluated_authors ->
      trusts_stable_authors trusted authors)
      by (hauto cases: - use: eval_preserves_stable_requirements).
    hauto cases: - unfold!: stable_conditional.
  Qed.

End CollectorPreservation.
