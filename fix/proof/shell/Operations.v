(** Defines checked handle operations for Tree access, identification,
    Encode construction, and Tag inspection. *)

From Stdlib Require Import List.
From FixProof.Semantics Require Import Handle.

Module Operations (S : STORAGE).
  Module H := Handles S.
  Import H.
  Definition get_tree_data_api h i :=
    match h with Data (Object (TreeObj t)) =>
      nth_error (get_tree_raw t) i
    | _ => None end.
  Definition create_identification_thunk_api h :=
    match h with Data d => Some (Thunk (Identification (data_reference d))) | _ => None end.
  (** Identification construction treats an Object and its corresponding Ref identically. *)
  Lemma identification_object_reference d :
    create_identification_thunk_api (Data d) =
    create_identification_thunk_api (Data (Ref (data_reference d))).
  Proof. destruct d as [[b|t]|reference]; reflexivity. Qed.
  Definition create_strict_encode_api h := match h with Thunk th => Some (Encode (Strict th)) | _ => None end.
  Definition create_shallow_encode_api h := match h with Thunk th => Some (Encode (Shallow th)) | _ => None end.
  Definition is_tag h := match h with
    | Data (Object (TreeObj t)) => tree_tag t
    | _ => false end.
End Operations.
