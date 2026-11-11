(** Specifies immutable Blob and Tree storage, comparability, full evaluation, and finite
    Object containment. *)

From Stdlib Require Import List ZArith Arith Wellfounded Lia.
From FixProof.Semantics Require Export Types.
Import ListNotations.

Module Type STORAGE.
  Include VALUES.

  Parameter get_blob_data : BlobName -> list raw.
  Parameter get_tree_raw : TreeName -> list handle.
  Parameter create_blob : list raw -> BlobName.
  Parameter create_tree_tagged : bool -> list handle -> TreeName.
  Parameter tree_tag : TreeName -> bool.
  Parameter tree_comparable : TreeName -> bool.
  Definition is_comparable h :=
    match h with
    | Data (Object (BlobObj _)) | Data (Ref (BlobRef _)) => true
    | Data (Object (TreeObj t)) | Data (Ref (TreeRef t)) => tree_comparable t
    | Thunk _ | Encode _ => false
    end.
  Parameter tree_value : TreeName -> bool.
  Definition is_value h :=
    match h with
    | Data (Object (TreeObj t)) => tree_value t
    | Data (Object (BlobObj _)) | Data (Ref _) | Thunk _ => true
    | Encode _ => false
    end.
  Definition create_tree := create_tree_tagged false.
  Definition to_nat := decode_u64.
  Definition from_nat := encode_u64.
  (** Reading a newly created Blob recovers exactly its input bytes. *)
  Axiom get_blob_data_create_blob : forall x, get_blob_data (create_blob x) = x.
  (** Reading a newly created Tree recovers exactly its input children. *)
  Axiom get_tree_raw_create_tree_tagged : forall tag xs,
    get_tree_raw (create_tree_tagged tag xs) = xs.
  (** Tree creation preserves the requested Tag marker. *)
  Axiom tree_tag_create_tree_tagged : forall tag xs,
    tree_tag (create_tree_tagged tag xs) = tag.
  (** A newly created Tree is Comparable exactly when every child is Comparable. *)
  Axiom tree_comparable_create_tree_tagged : forall tag xs,
    tree_comparable (create_tree_tagged tag xs) = forallb is_comparable xs.
  (** A newly created Tree is fully evaluated exactly when every child is fully evaluated. *)
  Axiom tree_value_create_tree_tagged : forall tag xs,
    tree_value (create_tree_tagged tag xs) = forallb is_value xs.
  (** Rebuilding a Tree from its marker and children recovers the original Tree name. *)
  (** This converse to reading a newly constructed Tree rules out extra identity hidden in
      a Tree name. Reconstructing unchanged fields cannot change the represented Tree; the
      proof uses this to transfer constructor laws to existing stored Trees. *)
  Axiom create_tree_get_tree_raw : forall t,
    create_tree_tagged (tree_tag t) (get_tree_raw t) = t.
Load "fix/proof/storage/internal/Storage/Contents".
  Definition tree_child t2 t1 := In (Data (Object (TreeObj t2))) (get_tree_raw t1).
  (** Following direct Object-Tree children is well founded; Object containment cannot
      descend forever. *)
  (** Only exposed Object containment must be well founded. Ref edges are deliberately
      excluded: they are evaluation boundaries, so the structural inductions here do not
      require the entire reachable reference graph to be unfolded. *)
  Axiom wfp_tree_child : well_founded tree_child.
Load "fix/proof/storage/internal/Storage/Relations".
End STORAGE.
