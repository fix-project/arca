(** Specifies deterministic program execution through Fix Shell operations. *)

From Stdlib Require Import List.
From FixProof.Storage Require Export Interface.

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
  | OCreateStrictEncode : nat -> op
  | OCreateShallowEncode : nat -> op
  | OGetType : nat -> op
  | ORunInternal : op
  | OReturn : nat -> op.
  Parameter get_prog : list S.raw -> list op.
  Parameter internal : list (list S.raw) -> list (list S.raw).
End PROGRAM.
