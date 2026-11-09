(** Models checked Tag authorship and proves observational congruence. *)

From Stdlib Require Import List Bool ClassicalDescription.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Equivalence Require Import Indistinguishability.
Import ListNotations.

Module TagCreation (S : STORAGE) (P : PROGRAM S).
  Module O := Indistinguishability S P.
  Module A := ApplyTree S P.
  Import O.EC.E.EP.E O.EC.E.EP.E.H.

  Definition authorizes combination author :=
    exists depth selected,
      A.author_at depth combination = Some selected /\
      O.compare_handles (lift author) (lift selected) = Some true.

  Definition create_tag combination fields :=
    match fields with
    | author :: _ =>
      if excluded_middle_informative (authorizes (HTreeObj combination) author)
      then Some (create_tree_tagged true fields) else None
    | [] => None
    end.

  Lemma author_at_congruence depth h h' :
    O.indistinguishable h h' ->
    rel_opt O.indistinguishable (A.author_at depth h) (A.author_at depth h').
  Proof.
    revert h h'; induction depth as [|depth IH]; intros h h' Related;
      [exact Related|].
    pose proof Related as [_ Observed]; destruct Observed;
      cbn [A.author_at HBlobObj HBlobRef HTreeObj HTreeRef]; try exact I.
    pose proof (proj1 (O.indistinguishable_trees _ _) Related) as [_ Fields].
    pose proof (Forall2_nth_error O.indistinguishable _ _ 1 Fields) as Selected.
    destruct (nth_error (get_tree_raw t1) 1), (nth_error (get_tree_raw t2) 1);
      cbn [rel_opt] in Selected |- *; try contradiction; [apply IH; exact Selected|exact I].
  Qed.

  Lemma lift_congruence left right : O.indistinguishable left right ->
    O.indistinguishable (lift left) (lift right).
  Proof.
    intro Related; pose proof Related as [_ Observed]; destruct Observed;
      try exact Related; apply O.indistinguishable_lift_data; exact Related.
  Qed.

  Lemma lift_comparable h : is_comparable (lift h) = is_comparable h.
  Proof. destruct h as [[[b|t]|[b|t]]|th|[th|th]]; reflexivity. Qed.

  Lemma authorizes_transport combination combination' author author' :
    O.indistinguishable combination combination' ->
    O.indistinguishable author author' ->
    authorizes combination author -> authorizes combination' author'.
  Proof.
    intros Combination Author [depth [selected [Selected Compared]]].
    pose proof (author_at_congruence depth _ _ Combination) as Related.
    rewrite Selected in Related.
    destruct (A.author_at depth combination') as [selected'|] eqn:Other;
      cbn [rel_opt] in Related; [|contradiction].
    exists depth, selected'; split; [exact Other|].
    rewrite <- (O.compare_handles_congruence _ _ _ _
      (lift_congruence _ _ Author) (lift_congruence _ _ Related)); exact Compared.
  Qed.

  Theorem create_tag_congruence combination combination' fields fields' :
    O.indistinguishable (HTreeObj combination) (HTreeObj combination') ->
    Forall2 O.indistinguishable fields fields' ->
    rel_opt (fun tag tag' => O.indistinguishable (HTreeObj tag) (HTreeObj tag'))
      (create_tag combination fields) (create_tag combination' fields').
  Proof.
    intros Combination Fields.
    destruct Fields as [|author author' body body' Author Body]; [exact I|].
    assert (Allowed : authorizes (HTreeObj combination) author <->
      authorizes (HTreeObj combination') author').
    { split; eapply authorizes_transport; eauto using O.indistinguishable_sym. }
    unfold create_tag.
    destruct (excluded_middle_informative (authorizes (HTreeObj combination) author)),
      (excluded_middle_informative (authorizes (HTreeObj combination') author'));
      cbn [rel_opt]; try tauto.
    apply O.indistinguishable_trees; split.
    - rewrite !tree_tag_create_tree_tagged; reflexivity.
    - rewrite !get_tree_raw_create_tree_tagged; constructor; assumption.
  Qed.

  Lemma create_tag_authorized combination author body :
    authorizes (HTreeObj combination) author ->
    create_tag combination (author :: body) =
      Some (create_tree_tagged true (author :: body)).
  Proof.
    intro Allowed; unfold create_tag;
      destruct (excluded_middle_informative _); [reflexivity|contradiction].
  Qed.

  Lemma create_tag_rejects_noncomparable combination author body :
    is_comparable author = false -> create_tag combination (author :: body) = None.
  Proof.
    intro Comparable; unfold create_tag.
    destruct (excluded_middle_informative _) as [Allowed|]; [|reflexivity].
    destruct Allowed as [depth [selected [_ Compared]]].
    apply O.compare_handles_equal in Compared as [Required _].
    rewrite lift_comparable, Comparable in Required; discriminate.
  Qed.

  Lemma authorizes_selected combination depth author :
    A.author_at depth (HTreeObj combination) = Some author ->
    is_comparable author = true -> authorizes (HTreeObj combination) author.
  Proof.
    intros Selected Comparable; exists depth, author; split; [exact Selected|].
    apply O.compare_handles_equal; rewrite lift_comparable.
    split; [exact Comparable|].
    split; [exact Comparable|apply O.indistinguishable_refl].
  Qed.

  Theorem create_tag_spec combination fields tag :
    create_tag combination fields = Some tag ->
    exists author body,
      fields = author :: body /\ authorizes (HTreeObj combination) author /\
      is_comparable author = true /\ tree_tag tag = true /\ get_tree_raw tag = fields.
  Proof.
    unfold create_tag; destruct fields as [|author body]; [discriminate|].
    destruct (excluded_middle_informative _) as [Allowed|]; [|discriminate].
    intro Created; inversion Created; subst tag.
    exists author, body; repeat split; try reflexivity; try assumption.
    - destruct Allowed as [depth [selected [_ Compared]]].
      apply O.compare_handles_equal in Compared as [Comparable _].
      now rewrite lift_comparable in Comparable.
    - apply tree_tag_create_tree_tagged.
    - apply get_tree_raw_create_tree_tagged.
  Qed.
End TagCreation.
