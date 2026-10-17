From Stdlib Require Import List Arith Wellfounded.
Import ListNotations.

(** The abstract storage assumptions from fix_handle.thy. They are a module
    interface, so every client theorem records its dependence on the backend. *)
Module Type STORAGE.
  Parameter BlobName TreeName raw : Set.
  Inductive object : Set := BlobObj : BlobName -> object | TreeObj : TreeName -> object.
  Inductive reference : Set := BlobRef : BlobName -> reference | TreeRef : TreeName -> reference.
  Inductive data : Set := Object : object -> data | Ref : reference -> data.
  Inductive thunk : Set :=
  | Application : TreeName -> thunk
  | Identification : data -> thunk
  | Selection : TreeName -> thunk
  | Digestion : TreeName -> thunk.
  Inductive encode : Set := Strict : thunk -> encode | Shallow : thunk -> encode.
  Inductive handle : Set := Data : data -> handle | Thunk : thunk -> handle | Encode : encode -> handle.

  Parameter get_blob_data : BlobName -> list raw.
  Parameter get_tree_raw : TreeName -> list handle.
  Parameter create_blob : list raw -> BlobName.
  Parameter create_tree : list handle -> TreeName.
  Parameter to_nat : list raw -> nat.
  Parameter from_nat : nat -> list raw.
  Axiom get_blob_data_create_blob : forall x, get_blob_data (create_blob x) = x.
  Axiom get_tree_raw_create_tree : forall xs, get_tree_raw (create_tree xs) = xs.
  Axiom create_tree_get_tree_raw : forall t, create_tree (get_tree_raw t) = t.
  Axiom from_nat_to_nat : forall r, from_nat (to_nat r) = r.
  Axiom to_nat_from_nat : forall n, to_nat (from_nat n) = n.
  Definition tree_child t2 t1 := In (Data (Object (TreeObj t2))) (get_tree_raw t1).
  Axiom wfp_tree_child : well_founded tree_child.
  Inductive same_typed_handle : handle -> handle -> Prop :=
  | blob_obj : forall b1 b2, same_typed_handle (Data (Object (BlobObj b1))) (Data (Object (BlobObj b2)))
  | blob_ref : forall b1 b2, same_typed_handle (Data (Ref (BlobRef b1))) (Data (Ref (BlobRef b2)))
  | tree_obj : forall t1 t2, same_typed_tree t1 t2 -> same_typed_handle (Data (Object (TreeObj t1))) (Data (Object (TreeObj t2)))
  | tree_ref : forall t1 t2, same_typed_handle (Data (Ref (TreeRef t1))) (Data (Ref (TreeRef t2)))
  | typed_thunk : forall th1 th2, same_typed_handle (Thunk th1) (Thunk th2)
  | encode_shallow : forall e1 e2, same_typed_handle (Encode (Shallow e1)) (Encode (Shallow e2))
  | encode_strict : forall e1 e2, same_typed_handle (Encode (Strict e1)) (Encode (Strict e2))
  with same_typed_tree : TreeName -> TreeName -> Prop :=
  | tree : forall t1 t2, Forall2 same_typed_handle (get_tree_raw t1) (get_tree_raw t2) -> same_typed_tree t1 t2.

End STORAGE.

Module Handles (S : STORAGE).
  Include S.
  Definition HBlobObj b := Data (Object (BlobObj b)).
  Definition HTreeObj t := Data (Object (TreeObj t)).
  Definition HBlobRef b := Data (Ref (BlobRef b)).
  Definition HTreeRef t := Data (Ref (TreeRef t)).
  (** Isabelle's out-of-bounds nth is unspecified. All meaningful uses of
      get_tree_data are guarded by a size check; this default is unobservable. *)
  Definition get_tree_data t i := nth i (get_tree_raw t) (HBlobObj (create_blob [])).
  Definition get_tree_size t := length (get_tree_raw t).
  Definition get_blob_size b := length (get_blob_data b).

  Definition rel_opt {A B} (X : A -> B -> Prop) (x : option A) (y : option B) :=
    match x, y with
    | None, None => True
    | Some a, Some b => X a b
    | _, _ => False
    end.
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
  Definition not_encode h := match h with Encode _ => False | _ => True end.

  Lemma rel_opt_map {A B C D} (X : A -> B -> Prop) (Y : C -> D -> Prop)
      (f : A -> C) (g : B -> D) x y :
    (forall a b, X a b -> Y (f a) (g b)) ->
    rel_opt X x y -> rel_opt Y (omap f x) (omap g y).
  Proof. destruct x, y; simpl; auto. Qed.
End Handles.
