From Stdlib Require Import List.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Equivalence Require Import Indistinguishability.
Import ListNotations.

Module EvaluationBoundary (S : STORAGE) (P : PROGRAM S).
  Module O := Indistinguishability S P.
  Import O O.EC.E O.EC.E.EP O.EC.E.EP.E O.EC.E.EP.E.H.

  Definition pending b := create_tree [Encode (Strict (Identification (BlobRef b)))].
  Definition ready b := create_tree [HBlobObj b].

  Lemma force_identification reference : force (Identification reference) = Some (Data (Ref reference)).
  Proof. rewrite force_unfold, think_identification; reflexivity. Qed.

  Lemma trees_evaluate b : eval (HTreeObj (pending b)) = Some (HTreeObj (ready b)).
  Proof.
    unfold pending, ready, create_tree.
    rewrite <- (tree_tag_create_tree_tagged false [Encode (Strict (Identification (BlobRef b)))]) at 2.
    apply evaluation_tree; rewrite get_tree_raw_create_tree_tagged; constructor; [|constructor].
    rewrite eval_encode, execute_unfold; cbn [encode_to_thunk]; rewrite force_identification.
    cbn [omap obind lift lift_data]; apply eval_blob.
  Qed.

  Theorem trees_equivalent b : O.Certified.computational_equiv (HTreeObj (pending b)) (HTreeObj (ready b)).
  Proof. apply O.Certified.eval_to_equiv, trees_evaluate. Qed.

  Lemma reference_comparability b :
    is_comparable (HTreeRef (pending b)) = false /\ is_comparable (HTreeRef (ready b)) = true.
  Proof.
    unfold HTreeRef, is_comparable, pending, ready, create_tree;
      rewrite !tree_comparable_create_tree_tagged; split; reflexivity.
  Qed.

  Theorem references_distinguished b :
    ~ O.Certified.computational_equiv (HTreeRef (pending b)) (HTreeRef (ready b)).
  Proof.
    intro Related; apply O.Certified.equivalent_references in Related.
    pose proof (observations_comparable _ _ (proj2 Related)) as Comparable.
    change (is_comparable (HTreeRef (pending b)) = is_comparable (HTreeRef (ready b))) in Comparable.
    destruct (reference_comparability b) as [Pending Ready]; rewrite Pending, Ready in Comparable; discriminate.
  Qed.

  Theorem thunks_distinguished b :
    ~ O.Certified.computational_equiv (Thunk (Identification (TreeRef (pending b))))
      (Thunk (Identification (TreeRef (ready b)))).
  Proof.
    intro Related; apply O.Certified.equivalent_force in Related.
    rewrite !force_identification in Related; cbn [omap lower lower_data rel_opt] in Related.
    pose proof (observations_comparable _ _ (proj2 Related)) as Comparable.
    change (is_comparable (HTreeRef (pending b)) = is_comparable (HTreeRef (ready b))) in Comparable.
    destruct (reference_comparability b) as [Pending Ready]; rewrite Pending, Ready in Comparable; discriminate.
  Qed.
End EvaluationBoundary.
