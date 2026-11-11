(** Defines fuel-bounded Think, Force, Execute, and Eval operations, together
    with the Values evaluation can produce. *)

From Stdlib Require Import List.
From FixProof.Semantics Require Import Handle Identify Select ApplyTree.
Import ListNotations.

Module Evaluation (S : STORAGE) (P : PROGRAM S).
  Module H := Handles S.
  Module A := ApplyTree S P.
  Module I := Identify S.
  Module Sl := Select S.
  Import H.
  Definition is_thunk h := match h with Thunk _ => true | _ => false end.
  Definition lift_data d :=
    match d with
    | Ref (TreeRef t) => Object (TreeObj t)
    | Ref (BlobRef b) => Object (BlobObj b)
    | Object o => Object o
    end.
  Definition lift h := match h with Data d => Data (lift_data d) | _ => h end.
  Definition lower_data d :=
    match d with
    | Object (TreeObj t) => Ref (TreeRef t)
    | Object (BlobObj b) => Ref (BlobRef b)
    | Ref r => Ref r
    end.
  Definition lower h := match h with Data d => Data (lower_data d) | _ => h end.
  (** Lowering Data returns the Ref corresponding to its underlying Object. *)
  Lemma lower_data_reference d : lower (Data d) = Data (Ref (data_reference d)).
  Proof. destruct d as [[b|t]|reference]; reflexivity. Qed.
  Definition get_thunk_inner th :=
    match th with
    | Application t | Selection t => HTreeObj t
    | Identification d => Data (Ref d)
    end.

Load "fix/proof/semantics/machinery/Evaluation/Fuel".
  Inductive value_tree : TreeName -> Prop :=
  | value_tree_intro : forall t, Forall value_handle (get_tree_raw t) -> value_tree t
  with value_handle : handle -> Prop :=
  | blob_obj_handle : forall b, value_handle (HBlobObj b)
  | tree_obj_handle : forall t, value_tree t -> value_handle (HTreeObj t)
  | ref_handle : forall r, value_handle (Data (Ref r))
  | thunk_handle : forall th, value_handle (Thunk th).

Load "fix/proof/semantics/internal/Evaluation/Values".
End Evaluation.
