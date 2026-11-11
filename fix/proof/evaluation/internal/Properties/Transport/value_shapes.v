    Variable X : handle -> handle -> Prop.
    Variable Props : relation_properties X.

    (** Related handles that are not Encodes have the same shape. *)
    Lemma value_shapes x y : X x y -> ~(exists e, x = Encode e) -> ~(exists e, y = Encode e) -> same_shape x y.
    Proof.
      intros Related Nx Ny; unfold same_shape.
      eapply (Sl.X_preserves_type X Props); [exact Related| |].
      - destruct x; cbn [Sl.H.not_encode]; auto; exfalso; apply Nx; eexists; reflexivity.
      - destruct y; cbn [Sl.H.not_encode]; auto; exfalso; apply Ny; eexists; reflexivity.
    Qed.
    (** Related Values have matching handle types. *)
    Lemma values_same_type x y : X x y -> value_handle x -> value_handle y -> same_typed_handle x y.
    Proof. apply (value_handle_to_sametypedness X (tree_cong X Props) (tag_cong X Props) value_shapes). Qed.
    (** The strengthened relation holds for related Data. *)
    Lemma related_data_strengthen d1 d2 : X (Data d1) (Data d2) -> strengthen X (Data d1) (Data d2).
    Proof.
      apply (data_to_relaxed X (blob_ref_complete X Props) (tree_ref_complete X Props)
        (preserve_tree_ref X Props) (preserve_blob_ref X Props) (preserve_tree X Props) (preserve_blob X Props)).
    Qed.
    (** Related lifted forcing results give the relaxed relation between the forcing
        results. *)
    Lemma strict_force_to_relaxed t1 t2 :
      rel_opt X (omap lift (force t1)) (omap lift (force t2)) -> rel_opt (relaxed_X X) (force t1) (force t2).
    Proof.
      destruct (force t1) as [h1|] eqn:F1, (force t2) as [h2|] eqn:F2;
        cbn [rel_opt omap]; try contradiction; [|auto].
      intro R; destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->]; exact R.
    Qed.
    (** Related strict Encodes satisfy the strengthened relation. *)
    Lemma strict_strengthen t1 t2 : X (Encode (Strict t1)) (Encode (Strict t2)) -> strengthen X (Encode (Strict t1)) (Encode (Strict t2)).
    Proof.
      intro R; destruct (strict_encode_reasons X Props _ _ R) as [Thunks|Exec].
      - left; exact Thunks.
      - right; rewrite !execute_unfold in Exec; apply strict_force_to_relaxed; exact Exec.
    Qed.
    (** Related shallow Encodes satisfy the strengthened relation. *)
    Lemma shallow_strengthen t1 t2 : X (Encode (Shallow t1)) (Encode (Shallow t2)) -> strengthen X (Encode (Shallow t1)) (Encode (Shallow t2)).
    Proof.
      intro R; destruct (shallow_encode_reasons X Props _ _ R) as [Thunks|Exec].
      - left; exact Thunks.
      - right; rewrite !execute_unfold in Exec.
        eapply force_to_lift; [exact (blob_ref_complete X Props)|exact (tree_ref_complete X Props)|
          exact (preserve_tree_ref X Props)|exact (preserve_blob_ref X Props)|exact Exec].
    Qed.
    (** Related handles of matching types satisfy the strengthened relation. *)
    Lemma related_typed_strengthen h1 h2 : X h1 h2 -> same_typed_handle h1 h2 -> strengthen X h1 h2.
    Proof.
      intros R Typed; inversion Typed; subst.
      - apply related_data_strengthen; exact R.
      - apply related_data_strengthen; exact R.
      - apply related_data_strengthen; exact R.
      - apply related_data_strengthen; exact R.
      - exact R.
      - apply shallow_strengthen; exact R.
      - apply strict_strengthen; exact R.
    Qed.
    (** Applying related Value combinations preserves related results and matching types,
        or both applications fail. *)
    Lemma apply_tree_values t1 t2 : X (HTreeObj t1) (HTreeObj t2) ->
      value_handle (HTreeObj t1) -> value_handle (HTreeObj t2) ->
      rel_opt X (A.apply_tree t1) (A.apply_tree t2) /\ rel_opt same_typed_handle (A.apply_tree t1) (A.apply_tree t2).
    Proof.
      intros R V1 V2; eapply (A.apply_tree_X X Props).
      split; [exact R|apply values_same_type; assumption].
    Qed.
    (** Applying related Value combinations preserves the strengthened result relation and
        agreement on failure. *)
    Lemma apply_tree_strengthen t1 t2 : X (HTreeObj t1) (HTreeObj t2) ->
      value_handle (HTreeObj t1) -> value_handle (HTreeObj t2) ->
      rel_opt (strengthen X) (A.apply_tree t1) (A.apply_tree t2).
    Proof.
      intros R V1 V2; destruct (apply_tree_values _ _ R V1 V2) as [Related Typed].
      destruct (A.apply_tree t1), (A.apply_tree t2); simpl in *; try contradiction; [|exact I].
      apply related_typed_strengthen; assumption.
    Qed.

    (** Selection preserves related results when both operand Trees have no immediate
        Encodes. *)
    Lemma slice_values t1 t2 : X (HTreeObj t1) (HTreeObj t2) ->
      Forall not_encode (get_tree_raw t1) -> Forall not_encode (get_tree_raw t2) ->
      rel_opt X (Sl.select t1) (Sl.select t2).
    Proof.
      intros R N1 N2; eapply (Sl.select_X X Props).
      repeat split; assumption.
    Qed.
