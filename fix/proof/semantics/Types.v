(** Defines the Fix value constructors independently of storage and execution. *)

From FixProof.Support Require Export BlobInteger.

Module Type VALUES.
  Parameter BlobName TreeName : Set.
  Definition raw := Integers.byte.
  Inductive object : Set := BlobObj : BlobName -> object | TreeObj : TreeName -> object.
  Inductive reference : Set := BlobRef : BlobName -> reference | TreeRef : TreeName -> reference.
  Inductive data : Set := Object : object -> data | Ref : reference -> data.
  Inductive thunk : Set :=
  | Application : TreeName -> thunk
  | Identification : reference -> thunk
  | Selection : TreeName -> thunk.
  Inductive encode : Set := Strict : thunk -> encode | Shallow : thunk -> encode.
  Inductive handle : Set := Data : data -> handle | Thunk : thunk -> handle | Encode : encode -> handle.

End VALUES.
