(** Defines finite evaluation derivations and proves that they describe
    exactly the successful evaluations. *)

From Stdlib Require Import List.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Evaluation Require Import Properties.
Import ListNotations.

Module Type EVALUATION_MODEL (S : STORAGE) (P : PROGRAM S).
  Include EvaluationProperties S P.
End EVALUATION_MODEL.

Module EvaluationAdequacy (S : STORAGE) (P : PROGRAM S) (EP : EVALUATION_MODEL S P).
  Import EP EP.E EP.E.H.

Load "fix/proof/evaluation/internal/Adequacy/Lowering".
  (** Evaluating a shallow Encode is exactly executing it; its resulting Ref needs no
      further evaluation. *)
Load "fix/proof/evaluation/machinery/Adequacy/shallow_evaluation".
  Theorem shallow_evaluation th :
    eval (Encode (Shallow th)) = execute (Shallow th).
  Proof. apply shallow_evaluation_proof. Qed.

  (** A strict Encode evaluates by shallow evaluation to a Ref, followed by evaluation of
      the lifted Object. *)
Load "fix/proof/evaluation/machinery/Adequacy/strict_evaluation".
  Theorem strict_evaluation th result :
    eval (Encode (Strict th)) = Some result <->
    exists reference,
      eval (Encode (Shallow th)) = Some (Data (Ref reference)) /\
      eval (lift (Data (Ref reference))) = Some result.
  Proof. apply strict_evaluation_proof. Qed.

  Definition follow_result h result := match h with
    | Data _ => result = lower h
    | Thunk th | Encode (Strict th) | Encode (Shallow th) =>
      eval (Encode (Shallow th)) = Some result
    end.

  (** After one Think step, shallow evaluation follows the returned Data, Thunk, or Encode
      according to its form. *)
Load "fix/proof/evaluation/machinery/Adequacy/shallow_step".
  Theorem shallow_step th intermediate result :
    think th = Some intermediate ->
    (eval (Encode (Shallow th)) = Some result <-> follow_result intermediate result).
  Proof. apply shallow_step_proof. Qed.

  Definition next_thunk h := match h with
    | Thunk th | Encode (Strict th) | Encode (Shallow th) => Some th
    | _ => None end.

  Definition leaf h := match h with
    | Data (Object (BlobObj _)) | Data (Ref _) | Thunk _ => True
    | _ => False end.

  Inductive eval_derivation : handle -> handle -> Prop :=
  | EvalLeaf h : leaf h -> eval_derivation h h
  | EvalTree tree children :
      Forall2 eval_derivation (get_tree_raw tree) children ->
      eval_derivation (HTreeObj tree)
        (HTreeObj (create_tree_tagged (tree_tag tree) children))
  | EvalStrict th reference result :
      eval_derivation (Encode (Shallow th)) (Data (Ref reference)) ->
      eval_derivation (lift (Data (Ref reference))) result ->
      eval_derivation (Encode (Strict th)) result
  | EvalIdentification d :
      eval_derivation (Encode (Shallow (Identification d))) (Data (Ref d))
  | EvalApplication tree ready intermediate result :
      eval_derivation (HTreeObj tree) (HTreeObj ready) ->
      A.apply_tree ready = Some intermediate ->
      follow_derivation intermediate result ->
      eval_derivation (Encode (Shallow (Application tree))) result
  | EvalSelection tree ready selected result :
      eval_derivation (HTreeObj tree) (HTreeObj ready) ->
      Sl.select ready = Some selected ->
      follow_derivation selected result ->
      eval_derivation (Encode (Shallow (Selection tree))) result
  with follow_derivation : handle -> handle -> Prop :=
  | FollowData d : follow_derivation (Data d) (lower (Data d))
  | FollowThunk h th result : next_thunk h = Some th ->
      eval_derivation (Encode (Shallow th)) result -> follow_derivation h result.

Load "fix/proof/evaluation/machinery/Adequacy/Correspondence".
  (** The evaluation rules derive exactly the successful runs of the evaluator.

      Each derivation follows the evaluator equations. Conversely, any successful run has
      finite fuel and therefore a finite derivation. *)
  Corollary eval_adequacy h result : eval_derivation h result <-> eval h = Some result.
  Proof. split; auto using eval_derivation_sound, eval_derivation_complete. Qed.
End EvaluationAdequacy.
