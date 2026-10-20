(** Proves stability of selected authors, preservation of Tag markers under
    evaluation, and the distinction between Tags and ordinary Trees. *)

From Stdlib Require Import List.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Equivalence Require Import Closure.
Import ListNotations.

Module TagProperties (S : STORAGE) (P : PROGRAM S).
  Module EC := EquivalenceClosure S P.
  Import EC EC.E.EP EC.E.EP.E EC.E.EP.E.H P.

  (** An author selected from a Value is also a Value. *)
  Lemma author_at_value depth h author :
    value_handle h -> A.author_at depth h = Some author -> value_handle author.
  Proof.
    revert h; induction depth as [|depth IH]; intros h Value Selected.
    - inversion Selected; subst; exact Value.
    - destruct h as [[[b|t]|ref]|th|e]; cbn [A.author_at] in Selected;
        try discriminate.
      inversion Value; subst.
      match goal with V : value_tree _ |- _ => inversion V; subst end.
      destruct (nth_error (get_tree_raw t) 1) as [child|] eqn:Field; [|discriminate].
      eapply IH; [|exact Selected].
      match goal with V : Forall value_handle _ |- _ =>
        rewrite Forall_forall in V; apply V; eapply nth_error_In; exact Field
      end.
  Qed.

  (** An author selected from an evaluated combination is fixed by evaluation. *)
  Theorem selected_author_stable input evaluated depth author :
    eval input = Some evaluated -> A.author_at depth evaluated = Some author ->
    eval author = Some author.
  Proof.
    (* Evaluation has already removed exposed Encodes from the combination. Selecting the
        issuer follows only Object-Tree fields, whose children are Values too. The issuer
        therefore carries no exposed computation that could later change its authority; no
        stability assumption on arbitrary extra authors is needed here. *)
    intros Evaluated Selected.
    assert (AuthorIsValue : value_handle author)
      by (hauto cases: - use: eval_to_value_handle, author_at_value).
    hauto cases: - use: value_handle_eval_to_itself.
  Qed.

  (** Tree evaluation preserves the Tag marker. *)
  Theorem tag_marker_preserved input result :
    eval (HTreeObj input) = Some (HTreeObj result) ->
    tree_tag input = tree_tag result.
  Proof.
    hauto cases: - use: eval_tree_iff.
  Qed.

  (** A Tag and an ordinary Tree with identical fields are not computationally equivalent. *)
  Theorem tag_distinct_from_tree fields :
    ~ computational_equiv (HTreeObj (create_tree_tagged true fields)) (HTreeObj (create_tree fields)).
  Proof.
    intro Related; apply equiv_tree_tag in Related.
    rewrite tree_tag_create_tree_tagged, tree_tag_create_tree in Related; discriminate.
  Qed.

End TagProperties.
