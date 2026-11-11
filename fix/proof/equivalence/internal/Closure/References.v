  (** Equivalent Blob Objects have equivalent Blob Refs. *)
  Lemma equiv_blob_to_ref b1 b2 : computational_equiv (HBlobObj b1) (HBlobObj b2) -> computational_equiv (HBlobRef b1) (HBlobRef b2).
  Proof. apply (equiv_to_lower (HBlobObj b1) (HBlobObj b2)). Qed.
  (** Equivalent Tree Objects have equivalent Tree Refs. *)
  Lemma equiv_tree_to_ref t1 t2 : computational_equiv (HTreeObj t1) (HTreeObj t2) -> computational_equiv (HTreeRef t1) (HTreeRef t2).
  Proof. apply (equiv_to_lower (HTreeObj t1) (HTreeObj t2)). Qed.
  (** Equivalent strict Encodes have equivalent shallow counterparts. *)
  Lemma equiv_strict_to_shallow t1 t2 : computational_equiv (Encode (Strict t1)) (Encode (Strict t2)) -> computational_equiv (Encode (Shallow t1)) (Encode (Shallow t2)).
  Proof. apply (equiv_to_lower (Encode (Strict t1)) (Encode (Strict t2))). Qed.
  (** Equivalent Blob Refs have equivalent underlying Blob Objects. *)
  Lemma equiv_ref_to_blob b1 b2 : computational_equiv (HBlobRef b1) (HBlobRef b2) -> computational_equiv (HBlobObj b1) (HBlobObj b2).
  Proof. apply (equiv_to_lift (HBlobRef b1) (HBlobRef b2)). Qed.
  (** Equivalent Tree Refs have equivalent underlying Tree Objects. *)
  Lemma equiv_ref_to_tree t1 t2 : computational_equiv (HTreeRef t1) (HTreeRef t2) -> computational_equiv (HTreeObj t1) (HTreeObj t2).
  Proof. apply (equiv_to_lift (HTreeRef t1) (HTreeRef t2)). Qed.
  (** Equivalent shallow Encodes have equivalent strict counterparts. *)
  Lemma equiv_shallow_to_strict t1 t2 : computational_equiv (Encode (Shallow t1)) (Encode (Shallow t2)) -> computational_equiv (Encode (Strict t1)) (Encode (Strict t2)).
  Proof. apply (equiv_to_lift (Encode (Shallow t1)) (Encode (Shallow t2))). Qed.
  (** Thunks forcing to Data with equivalent lifted Objects are computationally equivalent. *)
  Theorem force_some_to_equiv th1 th2 r1 r2 : force th1 = Some r1 -> force th2 = Some r2 ->
    relaxed_X computational_equiv r1 r2 -> computational_equiv (Thunk th1) (Thunk th2).
  Proof.
    (* Locate the last Think step in each successful force. The lifted Data results
        justify equivalence of those last Thunks; the preceding computation paths relate
        the original Thunks to them. Joining these paths proves the claim without
        comparing how many steps each force took. *)
    intros First Second Related.
    destruct (force_data _ _ First) as [d1 ->], (force_data _ _ Second) as [d2 ->].
    assert (LastFirst : exists last, think last = Some (Data d1) /\
      computational_equiv (Thunk th1) (Thunk last))
      by (hauto cases: - use: force_to_the_last_thunk).
    assert (LastSecond : exists last, think last = Some (Data d2) /\
      computational_equiv (Thunk th2) (Thunk last))
      by (hauto cases: - use: force_to_the_last_thunk).
    hauto cases: - use: equiv_lifted_data_to_think, equiv_sym, equiv_trans.
  Qed.
