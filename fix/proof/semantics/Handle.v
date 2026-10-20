(** Defines handle observations and shared structural lemmas. *)

From Stdlib Require Import List ZArith Arith Wellfounded Lia.
From FixProof.Storage Require Export Interface.
Import ListNotations.


Module Handles (S : STORAGE).
  Include S.
  Definition HBlobObj b := Data (Object (BlobObj b)).
  Definition HTreeObj t := Data (Object (TreeObj t)).
  Definition HBlobRef b := Data (Ref (BlobRef b)).
  Definition HTreeRef t := Data (Ref (TreeRef t)).
  Definition data_reference d := match d with
    | Object (BlobObj b) => BlobRef b
    | Object (TreeObj t) => TreeRef t
    | Ref reference => reference
    end.
  (** Isabelle's out-of-bounds nth is unspecified. All meaningful uses of
      get_tree_data are guarded by a size check; this default is unobservable. *)
  Definition get_tree_data t i := nth i (get_tree_raw t) (HBlobObj (create_blob [])).
  Definition get_tree_size t := length (get_tree_raw t).
  Definition get_blob_size b := length (get_blob_data b).

  (** Identical byte sequences produce Blobs related by any reflexive structural relation. *)
  Lemma blob_complete X (Laws : congruence X) d1 d2 :
    d1 = d2 -> X (HBlobObj (create_blob d1)) (HBlobObj (create_blob d2)).
  Proof. intros ->; apply (X_self X Laws). Qed.

  Definition rel_opt {A B} (X : A -> B -> Prop) (x : option A) (y : option B) :=
    match x, y with
    | None, None => True
    | Some a, Some b => X a b
    | _, _ => False
    end.
  (** Matching optional list lookups preserve a pointwise relation. *)
  Lemma Forall2_nth_error (X : handle -> handle -> Prop) xs ys i :
    Forall2 X xs ys -> rel_opt X (nth_error xs i) (nth_error ys i).
  Proof. intro R; revert i; induction R; intros [|i]; simpl; auto. Qed.
  (** Matching in-bounds list lookups preserve a pointwise relation, regardless of
      defaults. *)
  Lemma Forall2_nth (X : handle -> handle -> Prop) xs ys i d1 d2 :
    Forall2 X xs ys -> i < length xs -> X (nth i xs d1) (nth i ys d2).
  Proof.
    intro R; revert i; induction R; intros [|i] B; simpl in *; try lia; auto.
    apply IHR; lia.
  Qed.
  Definition obind {A B} (x : option A) (f : A -> option B) :=
    match x with Some a => f a | None => None end.
  Definition omap {A B} (f : A -> B) (x : option A) :=
    match x with Some a => Some (f a) | None => None end.
  Definition get_type h :=
    match h with
    | Data (Object (BlobObj _)) => 0
    | Data (Object (TreeObj _)) => 1
    | Data (Ref (BlobRef _)) => 2
    | Data (Ref (TreeRef _)) => 3
    | Thunk _ => 4
    | Encode _ => 5
    end.
  Definition value_type h :=
    match h with
    | Data (Object _) => 0
    | Data (Ref _) => 1
    | Thunk _ => 2
    | Encode _ => 3
    end.
  Definition not_encode h := match h with Encode _ => False | _ => True end.

  (** Mapping relation-preserving functions over optional values preserves their relation. *)
  Lemma rel_opt_map {A B C D} (X : A -> B -> Prop) (Y : C -> D -> Prop)
      (f : A -> C) (g : B -> D) x y :
    (forall a b, X a b -> Y (f a) (g b)) ->
    rel_opt X x y -> rel_opt Y (omap f x) (omap g y).
  Proof. destruct x, y; simpl; auto. Qed.
End Handles.
