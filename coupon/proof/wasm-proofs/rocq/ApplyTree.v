From Stdlib Require Import List Arith Bool Lia.
From FixProof Require Import Handle.
Import ListNotations.

Module Type PROGRAM (S : STORAGE).
  Inductive op : Set :=
  | OGetBlobData : nat -> op
  | OGetTreeData : nat -> nat -> op
  | OCreateBlob : nat -> op
  | OCreateTree : list nat -> op
  | OCreateBlobRef : nat -> op
  | OCreateTreeRef : nat -> op
  | OCreateApplicationThunk : nat -> op
  | OCreateIdentificationThunk : nat -> op
  | OCreateSelectionThunk : nat -> op
  | OCreateDigestionThunk : nat -> op
  | OCreateStrictEncode : nat -> op
  | OCreateShallowEncode : nat -> op
  | OGetType : nat -> op
  | ORunInternal : op
  | OReturn : nat -> op.
  Parameter get_prog : list S.raw -> list op.
  Parameter internal : list (list S.raw) -> list (list S.raw).
End PROGRAM.

Module ApplyTree (S : STORAGE) (P : PROGRAM S).
  Module H := Handles S.
  Import H P.
  Record state := State { hs : list handle; ds : list (list raw) }.
  Definition hpush s h := State (h :: hs s) (ds s).
  Definition dpush s d := State (hs s) (d :: ds s).
  Definition run_internal s := State (hs s) (internal (ds s)).
  Lemma run_internal_hs s : hs (run_internal s) = hs s.
  Proof. reflexivity. Qed.
  Lemma run_internal_ds_equiv s1 s2 : ds s1 = ds s2 -> ds (run_internal s1) = ds (run_internal s2).
  Proof. unfold run_internal; simpl; congruence. Qed.
  Inductive step_result := Continue : state -> step_result | Return : handle -> step_result.
  Definition handle_at s i := nth_error (hs s) i.
  Definition push_thunk s i (f : TreeName -> thunk) :=
    match handle_at s i with
    | Some (Data (Object (TreeObj t))) | Some (Data (Ref (TreeRef t))) =>
      Some (Continue (hpush s (Thunk (f t))))
    | _ => None
    end.
  Definition push_encode s i (f : thunk -> encode) :=
    match handle_at s i with
    | Some (Thunk th) => Some (Continue (hpush s (Encode (f th))))
    | _ => None
    end.
  Definition step instr s :=
    match instr with
    | OGetBlobData i =>
      match handle_at s i with
      | Some (Data (Object (BlobObj b))) => Some (Continue (dpush s (get_blob_data b)))
      | _ => None
      end
    | OGetTreeData i j =>
      match handle_at s i with
      | Some (Data (Object (TreeObj t))) =>
        if j <? get_tree_size t then Some (Continue (hpush s (get_tree_data t j))) else None
      | _ => None
      end
    | OCreateBlob i =>
      match nth_error (ds s) i with
      | Some d => Some (Continue (hpush s (HBlobObj (create_blob d))))
      | None => None
      end
    | OCreateTree xs =>
      if forallb (fun i => i <? length (hs s)) xs then
        Some (Continue (hpush s (HTreeObj (create_tree
          (map (fun i => nth i (hs s) (HBlobObj (create_blob []))) xs)))))
      else None
    | OCreateBlobRef i =>
      match handle_at s i with
      | Some (Data (Object (BlobObj b))) => Some (Continue (hpush s (HBlobRef b)))
      | _ => None
      end
    | OCreateTreeRef i =>
      match handle_at s i with
      | Some (Data (Object (TreeObj t))) => Some (Continue (hpush s (HTreeRef t)))
      | _ => None
      end
    | OCreateApplicationThunk i => push_thunk s i Application
    | OCreateIdentificationThunk i =>
      match handle_at s i with
      | Some (Data d) => Some (Continue (hpush s (Thunk (Identification d))))
      | _ => None
      end
    | OCreateSelectionThunk i => push_thunk s i Selection
    | OCreateDigestionThunk i => push_thunk s i Digestion
    | OCreateStrictEncode i => push_encode s i Strict
    | OCreateShallowEncode i => push_encode s i Shallow
    | OGetType i =>
      (** This opcode has its own historical numbering: blob-ref=1,
          tree-object=2. It differs from digest's get_type and rejects encodes. *)
      match handle_at s i with
      | Some (Data (Object (BlobObj _))) => Some (Continue (dpush s (from_nat 0)))
      | Some (Data (Ref (BlobRef _))) => Some (Continue (dpush s (from_nat 1)))
      | Some (Data (Object (TreeObj _))) => Some (Continue (dpush s (from_nat 2)))
      | Some (Data (Ref (TreeRef _))) => Some (Continue (dpush s (from_nat 3)))
      | Some (Thunk _) => Some (Continue (dpush s (from_nat 4)))
      | _ => None
      end
    | OReturn i => omap Return (handle_at s i)
    | ORunInternal => Some (Continue (run_internal s))
    end.
  Fixpoint exec prog s :=
    match prog with
    | [] => None
    | instr :: rest =>
      match step instr s with
      | None => None
      | Some (Return r) => Some r
      | Some (Continue s') => exec rest s'
      end
    end.
  Definition state_init t := State [HTreeObj t] [].
  Definition get_tree_prog t :=
    match get_tree_raw t with
    | Data (Object (BlobObj b)) :: _ => Some (get_prog (get_blob_data b))
    | _ => None
    end.
  Definition apply_tree t := obind (get_tree_prog t) (fun prog => exec prog (state_init t)).
  Definition rel_state (X : handle -> handle -> Prop) s1 s2 :=
    Forall2 X (hs s1) (hs s2) /\ ds s1 = ds s2.
  Definition rel_step X r1 r2 :=
    match r1, r2 with
    | Continue s1, Continue s2 => rel_state X s1 s2
    | Return h1, Return h2 => X h1 h2
    | _, _ => False
    end.

  Lemma exec_congruence (X : handle -> handle -> Prop)
      (step_X : forall instr s1 s2, rel_state X s1 s2 -> rel_opt (rel_step X) (step instr s1) (step instr s2))
      prog s1 s2 : rel_state X s1 s2 -> rel_opt X (exec prog s1) (exec prog s2).
  Proof.
    revert s1 s2; induction prog as [|instr prog IH]; intros s1 s2 R; simpl; auto.
    pose proof (step_X instr s1 s2 R) as T.
    destruct (step instr s1) as [[s1'|h1]|], (step instr s2) as [[s2'|h2]|];
      simpl in *; try contradiction; auto.
  Qed.

  Lemma Forall2_length (X : handle -> handle -> Prop) xs ys :
    Forall2 X xs ys -> length xs = length ys.
  Proof. induction 1; simpl; congruence. Qed.
  Lemma Forall2_nth_error (X : handle -> handle -> Prop) xs ys i :
    Forall2 X xs ys -> rel_opt X (nth_error xs i) (nth_error ys i).
  Proof. intro R; revert i; induction R; intros [|i]; simpl; auto. Qed.
  Lemma Forall2_nth (X : handle -> handle -> Prop) xs ys i d1 d2 :
    Forall2 X xs ys -> i < length xs -> X (nth i xs d1) (nth i ys d2).
  Proof.
    intro R; revert i; induction R; intros [|i] B; simpl in *; try lia; auto.
    apply IHR; lia.
  Qed.
  Lemma Forall2_mono (X Y : handle -> handle -> Prop) xs ys :
    (forall x y, X x y -> Y x y) -> Forall2 X xs ys -> Forall2 Y xs ys.
  Proof. intros XY R; induction R; constructor; auto. Qed.
  Lemma Forall2_and (X Y : handle -> handle -> Prop) xs ys :
    Forall2 X xs ys -> Forall2 Y xs ys -> Forall2 (fun x y => X x y /\ Y x y) xs ys.
  Proof. intro R; induction R; intro T; inversion T; subst; constructor; auto. Qed.
  Lemma rel_hpush X s1 s2 h1 h2 :
    rel_state X s1 s2 -> X h1 h2 -> rel_state X (hpush s1 h1) (hpush s2 h2).
  Proof. intros [R D] H; split; simpl; auto; constructor; auto. Qed.
  Lemma rel_dpush X s1 s2 d1 d2 :
    rel_state X s1 s2 -> d1 = d2 -> rel_state X (dpush s1 d1) (dpush s2 d2).
  Proof. intros [R D] ->; split; simpl; congruence. Qed.

  Section Congruence.
    Variable X : handle -> handle -> Prop.
    Hypothesis blob_cong : forall b1 b2, X (HBlobObj b1) (HBlobObj b2) -> get_blob_data b1 = get_blob_data b2.
    Hypothesis tree_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> Forall2 X (get_tree_raw t1) (get_tree_raw t2).
    Hypothesis blob_complete : forall d1 d2, d1 = d2 -> X (HBlobObj (create_blob d1)) (HBlobObj (create_blob d2)).
    Hypothesis tree_complete : forall xs ys, Forall2 X xs ys -> X (HTreeObj (create_tree xs)) (HTreeObj (create_tree ys)).
    Hypothesis blob_ref_cong : forall b1 b2, X (HBlobObj b1) (HBlobObj b2) -> X (HBlobRef b1) (HBlobRef b2).
    Hypothesis tree_ref_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> X (HTreeRef t1) (HTreeRef t2).
    Hypothesis blob_ref_complete : forall b1 b2, X (HBlobRef b1) (HBlobRef b2) -> X (HBlobObj b1) (HBlobObj b2).
    Hypothesis tree_ref_complete : forall t1 t2, X (HTreeRef t1) (HTreeRef t2) -> X (HTreeObj t1) (HTreeObj t2).
    Hypothesis strict_encode_cong : forall t1 t2, X (Thunk t1) (Thunk t2) -> X (Encode (Strict t1)) (Encode (Strict t2)).
    Hypothesis shallow_encode_cong : forall t1 t2, X (Thunk t1) (Thunk t2) -> X (Encode (Shallow t1)) (Encode (Shallow t2)).
    Hypothesis application_thunk_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> X (Thunk (Application t1)) (Thunk (Application t2)).
    Hypothesis selection_thunk_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> X (Thunk (Selection t1)) (Thunk (Selection t2)).
    Hypothesis digestion_thunk_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> X (Thunk (Digestion t1)) (Thunk (Digestion t2)).
    Hypothesis identification_thunk_cong : forall d1 d2, X (Data d1) (Data d2) -> X (Thunk (Identification d1)) (Thunk (Identification d2)).

    Definition typed_X h1 h2 := X h1 h2 /\ same_typed_handle h1 h2.
    Lemma rel_state_typed s1 s2 :
      rel_state X s1 s2 -> rel_state same_typed_handle s1 s2 -> rel_state typed_X s1 s2.
    Proof. intros [R D] [T _]; split; auto; apply Forall2_and; assumption. Qed.
    Lemma typed_tree_cong t1 t2 : typed_X (HTreeObj t1) (HTreeObj t2) -> Forall2 typed_X (get_tree_raw t1) (get_tree_raw t2).
    Proof.
      intros [R T]; apply Forall2_and; [apply tree_cong; exact R|].
      inversion T; subst.
      match goal with H : same_typed_tree _ _ |- _ => inversion H; subst; assumption end.
    Qed.
    Lemma typed_blob_complete d : typed_X (HBlobObj (create_blob d)) (HBlobObj (create_blob d)).
    Proof. split; [apply blob_complete; reflexivity|constructor]. Qed.
    Lemma typed_tree_complete xs ys : Forall2 typed_X xs ys -> typed_X (HTreeObj (create_tree xs)) (HTreeObj (create_tree ys)).
    Proof.
      intro R; split.
      - apply tree_complete; eapply (Forall2_mono typed_X X); [intros x y [H _]; exact H|exact R].
      - apply tree_obj, tree; rewrite !get_tree_raw_create_tree.
        eapply (Forall2_mono typed_X same_typed_handle); [intros x y [_ H]; exact H|exact R].
    Qed.
    Lemma typed_handle_at s1 s2 i : rel_state typed_X s1 s2 -> rel_opt typed_X (handle_at s1 i) (handle_at s2 i).
    Proof. intros [R _]; apply Forall2_nth_error; exact R. Qed.
    Lemma typed_tree_nth t1 t2 j : typed_X (HTreeObj t1) (HTreeObj t2) ->
      j < get_tree_size t1 -> typed_X (get_tree_data t1 j) (get_tree_data t2 j).
    Proof. intros R B; unfold get_tree_data; apply Forall2_nth; [apply typed_tree_cong; exact R|exact B]. Qed.
    Lemma typed_index_list s1 s2 xs : rel_state typed_X s1 s2 ->
      forallb (fun i => i <? length (hs s1)) xs = true ->
      Forall2 typed_X (map (fun i => nth i (hs s1) (HBlobObj (create_blob []))) xs)
                     (map (fun i => nth i (hs s2) (HBlobObj (create_blob []))) xs).
    Proof.
      intros [R _]; induction xs as [|i xs IH]; intro B; simpl in *; constructor.
      - apply andb_true_iff in B; apply Forall2_nth; [exact R|apply Nat.ltb_lt; tauto].
      - apply IH; apply andb_true_iff in B; tauto.
    Qed.
    #[local] Hint Resolve blob_ref_cong tree_ref_cong blob_ref_complete tree_ref_complete
      strict_encode_cong shallow_encode_cong application_thunk_cong
      selection_thunk_cong digestion_thunk_cong identification_thunk_cong
      blob_obj blob_ref tree_ref typed_thunk encode_strict encode_shallow : program_cong.

    Ltac lookup_cases s1 s2 i RS :=
      let At := fresh "At" in
      pose proof (typed_handle_at s1 s2 i RS) as At;
      destruct (handle_at s1 i) as [h1|] eqn:?;
      destruct (handle_at s2 i) as [h2|] eqn:?;
      cbn [rel_opt] in At; try contradiction; try exact I;
      destruct At as [Rel Ty]; inversion Ty; subst;
      cbn [rel_opt rel_step]; try exact I.
    Ltac finish_push RS :=
      apply rel_hpush; [exact RS|split; [eauto 4 with program_cong|constructor]].

    Lemma step_typed_X instr s1 s2 : rel_state typed_X s1 s2 ->
      rel_opt (rel_step typed_X) (step instr s1) (step instr s2).
    Proof.
      intro RS; pose proof RS as [Handles Datas].
      pose proof (Forall2_length typed_X _ _ Handles) as Lengths.
      destruct instr as [i|i j|i|xs|i|i|i|i|i|i|i|i|i| |i];
        cbv beta iota zeta delta [step push_thunk push_encode].
      - lookup_cases s1 s2 i RS.
        apply rel_dpush; [exact RS|apply blob_cong; exact Rel].
      - lookup_cases s1 s2 i RS.
        assert (typed_X (HTreeObj t1) (HTreeObj t2)) as Trees by (split; assumption).
        pose proof (Forall2_length typed_X _ _ (typed_tree_cong _ _ Trees)) as Size.
        change (get_tree_size t1 = get_tree_size t2) in Size.
        change (rel_opt (rel_step typed_X)
          (if j <? get_tree_size t1 then Some (Continue (hpush s1 (get_tree_data t1 j))) else None)
          (if j <? get_tree_size t2 then Some (Continue (hpush s2 (get_tree_data t2 j))) else None)).
        rewrite <- Size; destruct (j <? get_tree_size t1) eqn:B; [|exact I].
        apply rel_hpush; [exact RS|apply typed_tree_nth; [exact Trees|apply Nat.ltb_lt; exact B]].
      - rewrite <- Datas; destruct (nth_error (ds s1) i); [|exact I].
        apply rel_hpush; [exact RS|apply typed_blob_complete].
      - rewrite <- Lengths; destruct (forallb (fun i => i <? length (hs s1)) xs) eqn:B; [|exact I].
        apply rel_hpush; [exact RS|apply typed_tree_complete, typed_index_list; assumption].
      - lookup_cases s1 s2 i RS; finish_push RS.
      - lookup_cases s1 s2 i RS; finish_push RS.
      - lookup_cases s1 s2 i RS; finish_push RS.
      - lookup_cases s1 s2 i RS; finish_push RS.
      - lookup_cases s1 s2 i RS; finish_push RS.
      - lookup_cases s1 s2 i RS; finish_push RS.
      - lookup_cases s1 s2 i RS; finish_push RS.
      - lookup_cases s1 s2 i RS; finish_push RS.
      - lookup_cases s1 s2 i RS; (apply rel_dpush; [exact RS|reflexivity]).
      - split; simpl; [exact Handles|rewrite Datas; reflexivity].
      - pose proof (typed_handle_at s1 s2 i RS) as At.
        eapply rel_opt_map; [intros h1 h2 H; exact H|exact At].
    Qed.

    Lemma typed_rel_state_split s1 s2 : rel_state typed_X s1 s2 ->
      rel_state X s1 s2 /\ rel_state same_typed_handle s1 s2.
    Proof.
      intros [R D]; split; split; try exact D.
      - eapply (Forall2_mono typed_X X); [intros x y [H _]; exact H|exact R].
      - eapply (Forall2_mono typed_X same_typed_handle); [intros x y [_ H]; exact H|exact R].
    Qed.
    Lemma typed_rel_step_split r1 r2 : rel_step typed_X r1 r2 ->
      rel_step X r1 r2 /\ rel_step same_typed_handle r1 r2.
    Proof.
      destruct r1, r2; simpl; try contradiction; auto using typed_rel_state_split.
    Qed.
    Lemma typed_rel_opt_split x y : rel_opt typed_X x y ->
      rel_opt X x y /\ rel_opt same_typed_handle x y.
    Proof. destruct x, y; unfold rel_opt, typed_X; simpl; tauto. Qed.
    Lemma step_X instr s1 s2 :
      rel_state X s1 s2 /\ rel_state same_typed_handle s1 s2 ->
      rel_opt (rel_step X) (step instr s1) (step instr s2) /\
      rel_opt (rel_step same_typed_handle) (step instr s1) (step instr s2).
    Proof.
      intros [R T]; pose proof (step_typed_X instr s1 s2 (rel_state_typed s1 s2 R T)) as Step.
      destruct (step instr s1), (step instr s2); simpl in *; try contradiction; auto using typed_rel_step_split.
    Qed.
    Lemma exec_X prog s1 s2 :
      rel_state X s1 s2 /\ rel_state same_typed_handle s1 s2 ->
      rel_opt X (exec prog s1) (exec prog s2) /\ rel_opt same_typed_handle (exec prog s1) (exec prog s2).
    Proof.
      intros [R T]; apply typed_rel_opt_split.
      apply (exec_congruence typed_X step_typed_X prog s1 s2), rel_state_typed; assumption.
    Qed.
    Lemma state_init_typed t1 t2 : typed_X (HTreeObj t1) (HTreeObj t2) ->
      rel_state typed_X (state_init t1) (state_init t2).
    Proof. intro T; split; [constructor; [exact T|constructor]|reflexivity]. Qed.
    Lemma state_init_X t1 t2 :
      X (HTreeObj t1) (HTreeObj t2) /\ same_typed_handle (HTreeObj t1) (HTreeObj t2) ->
      rel_state X (state_init t1) (state_init t2) /\ rel_state same_typed_handle (state_init t1) (state_init t2).
    Proof. intro R; apply typed_rel_state_split, state_init_typed; exact R. Qed.
    Lemma get_prog_X t1 t2 :
      X (HTreeObj t1) (HTreeObj t2) /\ same_typed_handle (HTreeObj t1) (HTreeObj t2) ->
      get_tree_prog t1 = get_tree_prog t2.
    Proof.
      intro R; pose proof (typed_tree_cong t1 t2 R) as Raw.
      unfold get_tree_prog; destruct (get_tree_raw t1), (get_tree_raw t2);
        inversion Raw; subst; try reflexivity.
      match goal with Head : typed_X _ _ |- _ => destruct Head as [HX HT]; inversion HT; subst end;
        try reflexivity.
      cbn [HBlobObj]; rewrite (blob_cong _ _ HX); reflexivity.
    Qed.
    Lemma apply_tree_X t1 t2 :
      X (HTreeObj t1) (HTreeObj t2) /\ same_typed_handle (HTreeObj t1) (HTreeObj t2) ->
      rel_opt X (apply_tree t1) (apply_tree t2) /\ rel_opt same_typed_handle (apply_tree t1) (apply_tree t2).
    Proof.
      intro R; unfold apply_tree; rewrite (get_prog_X t1 t2 R).
      destruct (get_tree_prog t2); [apply exec_X, state_init_X; exact R|split; exact I].
    Qed.
  End Congruence.
End ApplyTree.
