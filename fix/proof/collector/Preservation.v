(** Proves that evaluating a Ref-backed coupon preserves its assertion
    and conditional authority. *)

From Stdlib Require Import List.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Collector Require Import Exchanges.
Import ListNotations.

Module CollectorPreservation (S : STORAGE) (P : PROGRAM S).
  Module X := CouponExchanges S P.
  Import X X.D X.D.EC.E.EP X.D.EC.E.EP.E X.D.EC.E.EP.E.H Wire.

  Theorem evaluation_preserves_authority trusted tag claim evaluated kind author value result :
    let before := HTreeObj (create_tree [HTreeObj tag; HTreeObj claim]) in
    let after := HTreeObj (create_tree [HTreeObj evaluated; HTreeObj claim]) in
    Wire.decode before = Some (kind, author, value, result) ->
    eval (HTreeObj tag) = Some (HTreeObj evaluated) ->
    stable_conditional trusted before ->
    exists evaluated_author,
      Wire.decode after = Some (kind, evaluated_author, value, result) /\
      stable_conditional trusted after.
  Proof.
    intros before after Before Evaluated Valid.
    assert (TagShape : tag = create_tree_tagged true [author; HTreeRef claim])
      by (hauto cases: - use: Wire.decode_tag_shape).
    assert (AuthorEvaluates : exists evaluated_author,
      eval author = Some evaluated_author /\
      evaluated = create_tree_tagged true [evaluated_author; HTreeRef claim]).
    { rewrite TagShape in Evaluated; eapply eval_handle_ref_pair; exact Evaluated. }
    destruct AuthorEvaluates as [evaluated_author [AuthorEvaluated Shape]].
    exists evaluated_author; split.
    - unfold after; rewrite Shape; eapply Wire.decode_replace_author; exact Before.
    - intros operation output_author lhs rhs Decoded Trusted.
      assert (OutputDecoded : Wire.decode after = Some (kind, evaluated_author, value, result)).
      { unfold after; rewrite Shape; eapply Wire.decode_replace_author; exact Before. }
      rewrite OutputDecoded in Decoded; inversion Decoded; subst operation output_author lhs rhs.
      apply (Valid _ _ _ _ Before).
      intros Value.
      pose proof (value_handle_eval_to_itself _ Value) as Unchanged.
      rewrite Unchanged in AuthorEvaluated; inversion AuthorEvaluated; subst evaluated_author.
      exact (Trusted Value).
  Qed.
End CollectorPreservation.
