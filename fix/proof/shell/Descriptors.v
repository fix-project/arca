From Stdlib Require Import List Arith Lia.
From FixProof.Semantics Require Import Handle.
Import ListNotations.

Module Descriptors (S : STORAGE).
  Import S.

  Definition table := list (option handle).
  Definition lookup (slots : table) index :=
    match nth_error slots index with Some value => value | None => None end.

  Fixpoint insert (slots : table) (value : handle) : nat * table :=
    match slots with
    | [] => (0, [Some value])
    | None :: rest => (0, Some value :: rest)
    | Some previous :: rest =>
      let '(index, next) := insert rest value in
      (Datatypes.S index, Some previous :: next)
    end.

  Definition allocate capacity slots value :=
    let '(index, next) := insert slots value in
    if Nat.ltb index capacity then Some (index, next) else None.

  Fixpoint close (slots : table) index : option table :=
    match slots, index with
    | Some _ :: rest, 0 => Some (None :: rest)
    | previous :: rest, Datatypes.S offset =>
      option_map (cons previous) (close rest offset)
    | _, _ => None
    end.

  Lemma insert_lookup slots value index next :
    insert slots value = (index, next) -> lookup next index = Some value.
  Proof.
    revert index next; induction slots as [|[previous|] rest IH]; cbn;
      intros index next Insert; try (inversion Insert; reflexivity).
    destruct (insert rest value) as [offset tail] eqn:Inserted.
    inversion Insert; subst; cbn [lookup]; apply (IH _ _ eq_refl).
  Qed.

  Lemma insert_fresh slots value index next :
    insert slots value = (index, next) -> lookup slots index = None.
  Proof.
    revert index next; induction slots as [|[previous|] rest IH]; cbn;
      intros index next Insert; try (inversion Insert; reflexivity).
    destruct (insert rest value) as [offset tail] eqn:Inserted.
    inversion Insert; subst; cbn [lookup]; apply (IH _ _ eq_refl).
  Qed.

  Lemma insert_lowest slots value index next :
    insert slots value = (index, next) ->
    forall before, before < index -> exists previous, lookup slots before = Some previous.
  Proof.
    revert index next; induction slots as [|[previous|] rest IH]; cbn;
      intros index next Insert before Bound; try (inversion Insert; lia).
    destruct (insert rest value) as [offset tail] eqn:Inserted.
    inversion Insert; subst; destruct before as [|before].
    - exists previous; reflexivity.
    - cbn [lookup]; apply (IH _ _ eq_refl); lia.
  Qed.

  Lemma insert_other slots value index next other :
    insert slots value = (index, next) -> other <> index ->
    lookup next other = lookup slots other.
  Proof.
    revert index next other; induction slots as [|[previous|] rest IH]; cbn;
      intros index next other Insert Different.
    - inversion Insert; subst; destruct other; [contradiction|destruct other; reflexivity].
    - destruct (insert rest value) as [offset tail] eqn:Inserted.
      inversion Insert; subst; destruct other; [reflexivity|].
      cbn [lookup]; apply (IH _ _ _ eq_refl); lia.
    - inversion Insert; subst; destruct other; [contradiction|destruct other; reflexivity].
  Qed.

  Lemma insert_length slots value index next :
    insert slots value = (index, next) ->
    index <= length slots /\ length next = Nat.max (length slots) (Datatypes.S index).
  Proof.
    revert index next; induction slots as [|[previous|] rest IH]; cbn;
      intros index next Insert; try (inversion Insert; cbn; lia).
    destruct (insert rest value) as [offset tail] eqn:Inserted.
    inversion Insert; subst; specialize (IH _ _ eq_refl); cbn; lia.
  Qed.

  Lemma allocate_spec capacity slots value index next :
    allocate capacity slots value = Some (index, next) ->
    index < capacity /\ lookup slots index = None /\ lookup next index = Some value /\
    (forall before, before < index -> exists previous, lookup slots before = Some previous) /\
    (forall other, other <> index -> lookup next other = lookup slots other).
  Proof.
    unfold allocate; destruct (insert slots value) as [offset tail] eqn:Inserted.
    destruct (Nat.ltb offset capacity) eqn:Fits; [|discriminate].
    intro Allocated; inversion Allocated; subst.
    repeat split.
    - apply Nat.ltb_lt; exact Fits.
    - eapply insert_fresh; exact Inserted.
    - eapply insert_lookup; exact Inserted.
    - intros before Bound; exact (insert_lowest slots value index next Inserted before Bound).
    - intros other Different; exact (insert_other slots value index next other Inserted Different).
  Qed.

  Lemma close_lookup slots index next :
    close slots index = Some next -> lookup next index = None.
  Proof.
    revert index next; induction slots as [|[previous|] rest IH];
      intros [|index] next Closed; cbn in Closed; try discriminate.
    - inversion Closed; reflexivity.
    - destruct (close rest index) as [tail|] eqn:Tail; cbn in Closed; try discriminate.
      inversion Closed; subst; cbn [lookup]; apply (IH _ _ Tail).
    - destruct (close rest index) as [tail|] eqn:Tail; cbn in Closed; try discriminate.
      inversion Closed; subst; cbn [lookup]; apply (IH _ _ Tail).
  Qed.

  Lemma close_invalid slots index : lookup slots index = None -> close slots index = None.
  Proof.
    revert index; induction slots as [|[previous|] rest IH]; intros [|index] Missing;
      cbn [lookup] in Missing; cbn [close]; try reflexivity; try discriminate;
      rewrite IH by exact Missing; reflexivity.
  Qed.

  Lemma close_available slots index value : lookup slots index = Some value ->
    exists next, close slots index = Some next.
  Proof.
    revert index; induction slots as [|entry rest IH]; intros [|index] Present;
      cbn [lookup] in Present; try discriminate.
    - destruct entry as [previous|]; [exists (None :: rest); reflexivity|discriminate].
    - destruct (IH index Present) as [following Closed].
      cbn [close]; rewrite Closed; destruct entry; cbn; eexists; reflexivity.
  Qed.

  Lemma close_twice slots index next : close slots index = Some next -> close next index = None.
  Proof. intro Closed; apply close_invalid; eapply close_lookup; exact Closed. Qed.

  Lemma close_length slots index next : close slots index = Some next -> length next = length slots.
  Proof.
    revert index next; induction slots as [|[previous|] rest IH];
      intros [|index] next Closed; cbn in Closed; try discriminate.
    - inversion Closed; reflexivity.
    - destruct (close rest index) as [tail|] eqn:Tail; cbn in Closed; try discriminate.
      inversion Closed; subst; cbn; f_equal; apply (IH _ _ Tail).
    - destruct (close rest index) as [tail|] eqn:Tail; cbn in Closed; try discriminate.
      inversion Closed; subst; cbn; f_equal; apply (IH _ _ Tail).
  Qed.

  Lemma close_other slots index next other :
    close slots index = Some next -> other <> index -> lookup next other = lookup slots other.
  Proof.
    revert index next other; induction slots as [|[previous|] rest IH];
      intros [|index] next other Closed Different; cbn in Closed; try discriminate.
    - inversion Closed; subst; destruct other; [contradiction|destruct other; reflexivity].
    - destruct (close rest index) as [tail|] eqn:Tail; cbn in Closed; try discriminate.
      inversion Closed; subst; destruct other; [reflexivity|].
      cbn [lookup]; apply (IH _ _ _ Tail); lia.
    - destruct (close rest index) as [tail|] eqn:Tail; cbn in Closed; try discriminate.
      inversion Closed; subst; destruct other; [reflexivity|].
      cbn [lookup]; apply (IH _ _ _ Tail); lia.
  Qed.

  Fixpoint live_count (slots : table) := match slots with
    | [] => 0 | None :: rest => live_count rest
    | Some _ :: rest => Datatypes.S (live_count rest) end.

  Lemma insert_live slots value index next :
    insert slots value = (index, next) -> live_count next = Datatypes.S (live_count slots).
  Proof.
    revert index next; induction slots as [|[previous|] rest IH]; cbn;
      intros index next Insert; try (inversion Insert; reflexivity).
    destruct (insert rest value) as [offset tail] eqn:Inserted.
    inversion Insert; subst; cbn; f_equal; apply (IH _ _ eq_refl).
  Qed.

  Lemma insert_index_live slots value index next :
    insert slots value = (index, next) -> index <= live_count slots.
  Proof.
    revert index next; induction slots as [|[previous|] rest IH]; cbn;
      intros index next Insert; try (inversion Insert; lia).
    destruct (insert rest value) as [offset tail] eqn:Inserted.
    inversion Insert; subst; specialize (IH _ _ eq_refl); lia.
  Qed.

  Theorem allocate_available capacity slots value :
    live_count slots < capacity -> exists index next,
      allocate capacity slots value = Some (index, next).
  Proof.
    intro Room; unfold allocate; destruct (insert slots value) as [index next] eqn:Inserted.
    assert (Fits : Nat.ltb index capacity = true).
    { apply Nat.ltb_lt; pose proof (insert_index_live _ _ _ _ Inserted); lia. }
    rewrite Fits; eauto.
  Qed.

  Lemma close_live slots index next :
    close slots index = Some next -> live_count slots = Datatypes.S (live_count next).
  Proof.
    revert index next; induction slots as [|[previous|] rest IH];
      intros [|index] next Closed; cbn in Closed; try discriminate.
    - inversion Closed; reflexivity.
    - destruct (close rest index) as [tail|] eqn:Tail; cbn in Closed; try discriminate.
      inversion Closed; subst; cbn; f_equal; apply (IH _ _ Tail).
    - destruct (close rest index) as [tail|] eqn:Tail; cbn in Closed; try discriminate.
      inversion Closed; subst; cbn; apply (IH _ _ Tail).
  Qed.

  Definition empty_blob := Data (Object (BlobObj (create_blob []))).
  Definition contents slots := map (fun value => match value with
    | Some handle => handle | None => empty_blob end) slots.
  Definition detach slots := create_tree (contents slots).

  Lemma detach_contents slots : get_tree_raw (detach slots) = contents slots.
  Proof. apply get_tree_raw_create_tree. Qed.
End Descriptors.
