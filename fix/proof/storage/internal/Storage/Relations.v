  Inductive same_typed_handle : handle -> handle -> Prop :=
  | blob_obj : forall b1 b2, same_typed_handle (Data (Object (BlobObj b1))) (Data (Object (BlobObj b2)))
  | blob_ref : forall b1 b2, same_typed_handle (Data (Ref (BlobRef b1))) (Data (Ref (BlobRef b2)))
  | tree_obj : forall t1 t2, same_typed_tree t1 t2 -> same_typed_handle (Data (Object (TreeObj t1))) (Data (Object (TreeObj t2)))
  | tree_ref : forall t1 t2, same_typed_handle (Data (Ref (TreeRef t1))) (Data (Ref (TreeRef t2)))
  | typed_thunk : forall th1 th2, same_typed_handle (Thunk th1) (Thunk th2)
  | encode_shallow : forall e1 e2, same_typed_handle (Encode (Shallow e1)) (Encode (Shallow e2))
  | encode_strict : forall e1 e2, same_typed_handle (Encode (Strict e1)) (Encode (Strict e2))
  with same_typed_tree : TreeName -> TreeName -> Prop :=
  | tree : forall t1 t2, tree_tag t1 = tree_tag t2 ->
      Forall2 same_typed_handle (get_tree_raw t1) (get_tree_raw t2) -> same_typed_tree t1 t2.

  (** These are obligations on a candidate relation X, not additional storage axioms. They
      package the structural laws needed to transport an operation across X; later proofs
      establish them separately for computational and observational relations. *)
  Record congruence (X : handle -> handle -> Prop) : Prop := {
    (** Related Blob Objects have identical bytes. *)
    blob_cong : forall b1 b2, X (Data (Object (BlobObj b1))) (Data (Object (BlobObj b2))) -> get_blob_data b1 = get_blob_data b2;
    (** Related Tree Objects have pointwise related children. *)
    tree_cong : forall t1 t2, X (Data (Object (TreeObj t1))) (Data (Object (TreeObj t2))) -> Forall2 X (get_tree_raw t1) (get_tree_raw t2);
    (** Related Tree Objects have matching Tag markers. *)
    tag_cong : forall t1 t2, X (Data (Object (TreeObj t1))) (Data (Object (TreeObj t2))) -> tree_tag t1 = tree_tag t2;
    (** Constructing Trees with matching markers preserves the pointwise relation. *)
    tree_complete : forall tag xs ys, Forall2 X xs ys -> X (Data (Object (TreeObj (create_tree_tagged tag xs)))) (Data (Object (TreeObj (create_tree_tagged tag ys))));
    (** Related Blob Objects have related Refs. *)
    blob_ref_cong : forall b1 b2, X (Data (Object (BlobObj b1))) (Data (Object (BlobObj b2))) -> X (Data (Ref (BlobRef b1))) (Data (Ref (BlobRef b2)));
    (** Related Tree Objects have related Refs. *)
    tree_ref_cong : forall t1 t2, X (Data (Object (TreeObj t1))) (Data (Object (TreeObj t2))) -> X (Data (Ref (TreeRef t1))) (Data (Ref (TreeRef t2)));
    (** Related Blob Refs have related underlying Objects. *)
    blob_ref_complete : forall b1 b2, X (Data (Ref (BlobRef b1))) (Data (Ref (BlobRef b2))) -> X (Data (Object (BlobObj b1))) (Data (Object (BlobObj b2)));
    (** Related Tree Refs have related underlying Objects. *)
    tree_ref_complete : forall t1 t2, X (Data (Ref (TreeRef t1))) (Data (Ref (TreeRef t2))) -> X (Data (Object (TreeObj t1))) (Data (Object (TreeObj t2)));
    (** Related bare Thunks give related strict Encodes. *)
    strict_encode_cong : forall t1 t2, X (Thunk t1) (Thunk t2) -> X (Encode (Strict t1)) (Encode (Strict t2));
    (** Related bare Thunks give related shallow Encodes. *)
    shallow_encode_cong : forall t1 t2, X (Thunk t1) (Thunk t2) -> X (Encode (Shallow t1)) (Encode (Shallow t2));
    (** Related combinations give related application Thunks. *)
    application_thunk_cong : forall t1 t2, X (Data (Object (TreeObj t1))) (Data (Object (TreeObj t2))) -> X (Thunk (Application t1)) (Thunk (Application t2));
    (** Related selection operand Trees give related selection Thunks. *)
    selection_thunk_cong : forall t1 t2, X (Data (Object (TreeObj t1))) (Data (Object (TreeObj t2))) -> X (Thunk (Selection t1)) (Thunk (Selection t2));
    (** Related Refs give related identification Thunks. *)
    identification_thunk_cong : forall d1 d2, X (Data (Ref d1)) (Data (Ref d2)) -> X (Thunk (Identification d1)) (Thunk (Identification d2));
    (** A handle related from a Tree Ref is also a Tree Ref. *)
    preserve_tree_ref : forall t h, X (Data (Ref (TreeRef t))) h -> exists t', h = (Data (Ref (TreeRef t')));
    (** A handle related from a Blob Ref is also a Blob Ref. *)
    preserve_blob_ref : forall b h, X (Data (Ref (BlobRef b))) h -> exists b', h = (Data (Ref (BlobRef b')));
    (** A handle related from a Tree Object is also a Tree Object. *)
    preserve_tree : forall t h, X (Data (Object (TreeObj t))) h -> exists t', h = (Data (Object (TreeObj t')));
    (** A handle related from a Blob Object is also a Blob Object. *)
    preserve_blob : forall b h, X (Data (Object (BlobObj b))) h -> exists b', h = (Data (Object (BlobObj b')));
    (** The relation preserves whether a handle is a bare Thunk. *)
    preserve_thunk : forall h1 h2, X h1 h2 -> ((exists t1, h1 = Thunk t1) <-> (exists t2, h2 = Thunk t2));
    (** Every handle is related to itself. *)
    X_self : forall h, X h h
  }.
