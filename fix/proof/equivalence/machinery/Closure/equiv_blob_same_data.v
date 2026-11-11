  Theorem equiv_blob_same_data_proof h1 h2 : computational_equiv h1 h2 -> forall b1, h1 = HBlobObj b1 ->
    (exists b2, h2 = HBlobObj b2 /\ get_blob_data b1 = get_blob_data b2) \/
    (exists th b2, h2 = Encode (Strict th) /\ eval h2 = Some (HBlobObj b2) /\ get_blob_data b1 = get_blob_data b2).
  Proof.
    (* Remove exposed Encodes from the equivalence path. Every remaining edge between
        Blob Objects preserves bytes; equality of bytes composes along the path. If the
        other endpoint was an Encode, its execution and subsequent Blob evaluation recover
        that same content. *)
    intros Related b1 ->; pose proof (equiv_unencode _ _ Related) as Results; cbn [unencode] in Results.
    destruct (unencode h2) as [v2|] eqn:U2; [|contradiction].
    destruct (unencoded_equiv_blob_cong _ _ Results b1 Logic.eq_refl) as [b2 [-> Data]].
    destruct h2 as [d|th|e]; cbn [unencode] in U2.
    - left; exists b2; split; [congruence|exact Data].
    - discriminate.
    - destruct (execute_to_obj_strict _ _ U2) as [th ->].
      right; exists th, b2; split; [reflexivity|split; [rewrite eval_encode, U2; exact (eval_blob b2)|exact Data]].
  Qed.
