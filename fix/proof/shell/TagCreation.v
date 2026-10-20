(** Models Tag creation with a selected issuing author. Proves preservation
    of supplied authors, inclusion of the issuer, and observational congruence. *)

From Stdlib Require Import List Bool.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Equivalence Require Import Indistinguishability.
Import ListNotations.

Module TagCreation (S : STORAGE) (P : PROGRAM S).
  Module O := Indistinguishability S P.
  Module A := ApplyTree S P.
  Import O.EC.E.EP.E.H.

  Definition author_matches issuing_author existing :=
    match O.compare_handles issuing_author existing with
    | Some true => true
    | _ => false
    end.

  Definition add_author authors issuing_author :=
    if existsb (author_matches issuing_author) (get_tree_raw authors) then authors
    else create_tree (get_tree_raw authors ++ [issuing_author]).

  Definition create_tag combination depth fields :=
    match A.author_at depth (HTreeObj combination), fields with
    | Some issuing_author, Data (Object (TreeObj authors)) :: body =>
      Some (create_tree_tagged true (HTreeObj (add_author authors issuing_author) :: body))
    | _, _ => None
    end.

  (** Author deduplication is justified only by a successful public handle comparison.
      Non-Comparable authors remain in the list even if a stronger, unavailable equality
      could identify them; Tag creation must not reveal such equality through the choice
      to append. *)
  Lemma author_matches_spec issuing_author existing : author_matches issuing_author existing = true <->
    is_comparable issuing_author = true /\ is_comparable existing = true /\
    O.indistinguishable issuing_author existing.
  Proof.
    rewrite <- O.compare_handles_equal; unfold author_matches.
    destruct (O.compare_handles issuing_author existing) as [[|]|]; intuition discriminate.
  Qed.

  (** Replacing author operands by indistinguishable handles preserves the deduplication
      test. *)
  Lemma author_matches_congruence issuing_author issuing_author' existing existing' :
    O.indistinguishable issuing_author issuing_author' -> O.indistinguishable existing existing' ->
    author_matches issuing_author existing = author_matches issuing_author' existing'.
  Proof.
    intros Actual Existing; unfold author_matches.
    rewrite (O.compare_handles_congruence _ _ _ _ Actual Existing); reflexivity.
  Qed.

  (** Indistinguishable issuers and pointwise indistinguishable author lists give the same
      membership decision. *)
  Lemma authors_match_congruence issuing_author issuing_author' authors authors' :
    O.indistinguishable issuing_author issuing_author' -> Forall2 O.indistinguishable authors authors' ->
    existsb (author_matches issuing_author) authors = existsb (author_matches issuing_author') authors'.
  Proof.
    intros Issuer Authors; induction Authors; cbn;
      f_equal; eauto using author_matches_congruence.
  Qed.

  (** Adding indistinguishable issuing authors to indistinguishable author Trees preserves
      indistinguishability. *)
  Theorem add_author_congruence authors authors' issuing_author issuing_author' :
    O.indistinguishable (HTreeObj authors) (HTreeObj authors') ->
    O.indistinguishable issuing_author issuing_author' ->
    O.indistinguishable (HTreeObj (add_author authors issuing_author))
      (HTreeObj (add_author authors' issuing_author')).
  Proof.
    intros Authors Actual.
    pose proof (proj1 (O.indistinguishable_trees _ _) Authors) as [_ Fields].
    unfold add_author; rewrite (authors_match_congruence _ _ _ _ Actual Fields).
    destruct (existsb _ _); [exact Authors|].
    apply O.indistinguishable_trees; split; [rewrite !tree_tag_create_tree; reflexivity|].
    rewrite !get_tree_raw_create_tree; apply Forall2_app; [exact Fields|].
    constructor; [exact Actual|constructor].
  Qed.

  (** Selecting an author at the same callable depth preserves indistinguishability and
      agreement on failure. *)
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

  (** Tag creation from indistinguishable combinations and fields gives indistinguishable
      Tags or shared failure. *)
  Theorem create_tag_congruence combination combination' depth fields fields' :
    O.indistinguishable (HTreeObj combination) (HTreeObj combination') ->
    Forall2 O.indistinguishable fields fields' ->
    rel_opt (fun tag tag' => O.indistinguishable (HTreeObj tag) (HTreeObj tag'))
      (create_tag combination depth fields) (create_tag combination' depth fields').
  Proof.
    intros Combination Fields.
    pose proof (author_at_congruence depth _ _ Combination) as Selected.
    unfold create_tag.
    destruct (A.author_at depth (HTreeObj combination)),
      (A.author_at depth (HTreeObj combination'));
      cbn [rel_opt] in Selected |- *; try contradiction.
    - destruct Fields as [|head head' body body' Head Body]; [exact I|].
      pose proof Head as [_ Observed]; destruct Observed;
        cbn [HBlobObj HBlobRef HTreeObj HTreeRef]; try exact I.
      apply O.indistinguishable_trees; split.
      + rewrite !tree_tag_create_tree_tagged; reflexivity.
      + rewrite !get_tree_raw_create_tree_tagged; constructor;
          [apply add_author_congruence; assumption|exact Body].
    - exact I.
  Qed.

  (** Adding an author either reuses the author list or appends exactly the issuing author. *)
  Lemma add_author_contents authors issuing_author :
    get_tree_raw (add_author authors issuing_author) = get_tree_raw authors \/
    get_tree_raw (add_author authors issuing_author) = get_tree_raw authors ++ [issuing_author].
  Proof.
    unfold add_author; destruct (existsb _ _); [left; reflexivity|right].
    apply get_tree_raw_create_tree.
  Qed.

  (** Adding the issuer never drops a supplied authority requirement. This inclusion is
      the direction needed for conditional claims: trusting every resulting author must
      still entail trusting every original author. *)
  Lemma add_author_preserves_authors authors issuing_author :
    incl (get_tree_raw authors) (get_tree_raw (add_author authors issuing_author)).
  Proof.
    destruct (add_author_contents authors issuing_author) as [Unchanged|Appended]; rewrite ?Unchanged, ?Appended;
      intros author Member; [exact Member|apply in_or_app; left; exact Member].
  Qed.

  (** The updated author list contains an author indistinguishable from the issuer. *)
  Lemma add_author_contains_issuer authors issuing_author :
    exists author, In author (get_tree_raw (add_author authors issuing_author)) /\
      O.indistinguishable issuing_author author.
  Proof.
    unfold add_author; destruct (existsb (author_matches issuing_author) (get_tree_raw authors)) eqn:Present.
    - apply existsb_exists in Present as [author [Member Matches]].
      exists author; split; [exact Member|apply author_matches_spec in Matches; tauto].
    - exists issuing_author; rewrite get_tree_raw_create_tree; split.
      + apply in_or_app; right; simpl; auto.
      + apply O.indistinguishable_refl.
  Qed.

  (** Successful Tag creation preserves supplied authors and body and includes an author
      indistinguishable from the selected issuer. *)
  Theorem create_tag_spec combination depth fields tag :
    create_tag combination depth fields = Some tag ->
    exists issuing_author authors updated body,
      A.author_at depth (HTreeObj combination) = Some issuing_author /\
      fields = HTreeObj authors :: body /\
      tree_tag tag = true /\ get_tree_raw tag = HTreeObj updated :: body /\
      incl (get_tree_raw authors) (get_tree_raw updated) /\
      exists author, In author (get_tree_raw updated) /\ O.indistinguishable issuing_author author.
  Proof.
    unfold create_tag; destruct (A.author_at depth (HTreeObj combination)) as [issuing_author|] eqn:Selected;
      destruct fields as [|[[[b|authors]|reference]|thunk|encode] body]; try discriminate.
    intro Created; inversion Created; subst tag.
    exists issuing_author, authors, (add_author authors issuing_author), body.
    repeat split; try reflexivity; try assumption.
    - apply tree_tag_create_tree_tagged.
    - apply get_tree_raw_create_tree_tagged.
    - apply add_author_preserves_authors.
    - apply add_author_contains_issuer.
  Qed.

End TagCreation.
