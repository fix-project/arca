  Section ReferenceCongruence.
    Variable X : handle -> handle -> Prop.
    (** Related Blob Objects have related Refs. *)
    Hypothesis blob_ref_cong : forall b1 b2, X (HBlobObj b1) (HBlobObj b2) -> X (HBlobRef b1) (HBlobRef b2).
    (** Related Tree Objects have related Refs. *)
    Hypothesis tree_ref_cong : forall t1 t2, X (HTreeObj t1) (HTreeObj t2) -> X (HTreeRef t1) (HTreeRef t2).
    (** Related Blob Refs have related underlying Objects. *)
    Hypothesis blob_ref_complete : forall b1 b2, X (HBlobRef b1) (HBlobRef b2) -> X (HBlobObj b1) (HBlobObj b2).
    (** Related Tree Refs have related underlying Objects. *)
    Hypothesis tree_ref_complete : forall t1 t2, X (HTreeRef t1) (HTreeRef t2) -> X (HTreeObj t1) (HTreeObj t2).
    (** A handle related from a Tree Ref is also a Tree Ref. *)
    Hypothesis X_preserve_tree_ref : forall t h, X (HTreeRef t) h -> exists t', h = HTreeRef t'.
    (** A handle related from a Blob Ref is also a Blob Ref. *)
    Hypothesis X_preserve_blob_ref : forall b h, X (HBlobRef b) h -> exists b', h = HBlobRef b'.
    (** A handle related from a Tree Object is also a Tree Object. *)
    Hypothesis X_preserve_tree : forall t h, X (HTreeObj t) h -> exists t', h = HTreeObj t'.
    (** A handle related from a Blob Object is also a Blob Object. *)
    Hypothesis X_preserve_blob : forall b h, X (HBlobObj b) h -> exists b', h = HBlobObj b'.

    Ltac reject_shape R :=
      try solve [destruct (X_preserve_blob_ref _ _ R) as [? E]; discriminate E];
      try solve [destruct (X_preserve_tree_ref _ _ R) as [? E]; discriminate E];
      try solve [destruct (X_preserve_blob _ _ R) as [? E]; discriminate E];
      try solve [destruct (X_preserve_tree _ _ R) as [? E]; discriminate E].
    (** Related Data remains related when compared through its lifted Objects. *)
    Lemma data_to_relaxed d1 d2 : X (Data d1) (Data d2) -> relaxed_X X (Data d1) (Data d2).
    Proof.
      intro R; destruct d1 as [[b1|t1]|[b1|t1]], d2 as [[b2|t2]|[b2|t2]];
        cbv beta iota delta [relaxed_X lift lift_data];
        try exact R;
        try solve [eauto using blob_ref_complete, tree_ref_complete]; reject_shape R.
    Qed.
    (** Related lowered Data has related lifted Objects. *)
    Lemma lower_to_lift d1 d2 : X (lower (Data d1)) (lower (Data d2)) -> X (lift (Data d1)) (lift (Data d2)).
    Proof.
      intro R; destruct d1 as [[b1|t1]|[b1|t1]], d2 as [[b2|t2]|[b2|t2]];
        cbv beta iota delta [lift lower lift_data lower_data] in *;
        try solve [eauto using blob_ref_complete, tree_ref_complete]; reject_shape R.
    Qed.
    (** Related lifted Objects have related lowered Refs. *)
    Lemma lift_to_lower d1 d2 : X (lift (Data d1)) (lift (Data d2)) -> X (lower (Data d1)) (lower (Data d2)).
    Proof.
      intro R; destruct d1 as [[b1|t1]|[b1|t1]], d2 as [[b2|t2]|[b2|t2]];
        cbv beta iota delta [lift lower lift_data lower_data] in *;
        try solve [apply blob_ref_cong; exact R];
        try solve [apply tree_ref_cong; exact R]; reject_shape R.
    Qed.
    (** Related lowered forcing results give the relaxed relation between the original
        forcing results. *)
    Lemma force_to_lift e1 e2 :
      rel_opt X (omap lower (force e1)) (omap lower (force e2)) ->
      rel_opt (relaxed_X X) (force e1) (force e2).
    Proof.
      destruct (force e1) as [h1|] eqn:F1, (force e2) as [h2|] eqn:F2;
        cbn [rel_opt omap]; try contradiction; [|auto].
      intro R; destruct (force_data _ _ F1) as [d1 ->], (force_data _ _ F2) as [d2 ->].
      change (X (lift (Data d1)) (lift (Data d2))); apply lower_to_lift; exact R.
    Qed.
  End ReferenceCongruence.

  (** The original eq_forces_to_induct hypotheses, grouped for reuse by R',
      R and their equivalence closure. These are premises, not backend axioms. *)
  Definition thunk_reasons (X : handle -> handle -> Prop) t1 t2 :=
    (exists tree1 tree2, X (HTreeObj tree1) (HTreeObj tree2) /\ t1 = Application tree1 /\ t2 = Application tree2) \/
    (exists tree1 tree2, X (HTreeObj tree1) (HTreeObj tree2) /\ t1 = Selection tree1 /\ t2 = Selection tree2) \/
    (exists d1 d2, X (Data (Ref d1)) (Data (Ref d2)) /\ t1 = Identification d1 /\ t2 = Identification d2) \/
    (think t1 = None /\ think t2 = None) \/
    (exists r1 r2, think t1 = Some r1 /\ think t2 = Some r2 /\ strengthen X r1 r2) \/
    think t1 = Some (Thunk t2) \/ think t1 = Some (Encode (Strict t2)) \/ think t1 = Some (Encode (Shallow t2)).

  Record relation_properties (X : handle -> handle -> Prop) : Prop := {
    (** The relation satisfies the structural congruence laws. *)
    structural : congruence X;
    (** A handle related to a Tree Ref is a Tree Ref or a shallow Encode. *)
    preserve_tree_ref_rev : forall t h, X h (HTreeRef t) -> (exists t', h = HTreeRef t') \/ (exists th, h = Encode (Shallow th));
    (** A handle related to a Blob Ref is a Blob Ref or a shallow Encode. *)
    preserve_blob_ref_rev : forall b h, X h (HBlobRef b) -> (exists b', h = HBlobRef b') \/ (exists th, h = Encode (Shallow th));
    (** A handle related to a Tree Object is a Tree Object or a strict Encode. *)
    preserve_tree_rev : forall t h, X h (HTreeObj t) -> (exists t', h = HTreeObj t') \/ (exists th, h = Encode (Strict th));
    (** A handle related to a Blob Object is a Blob Object or a strict Encode. *)
    preserve_blob_rev : forall b h, X h (HBlobObj b) -> (exists b', h = HBlobObj b') \/ (exists th, h = Encode (Strict th));
    (** An Encode related to a non-Encode executes to that handle. *)
    encode_eval : forall e h, X (Encode e) h -> ~(exists e', h = Encode e') -> executes_to e h;
    (** Successful Encode execution relates the Encode to its result. *)
    execute_result : forall e h, execute e = Some h -> X (Encode e) h;
    (** Related strict Encodes have related Thunks or related execution results, including
        shared failure. *)
    strict_encode_reasons : forall t1 t2, X (Encode (Strict t1)) (Encode (Strict t2)) ->
      X (Thunk t1) (Thunk t2) \/ rel_opt X (execute (Strict t1)) (execute (Strict t2));
    (** Related shallow Encodes have related Thunks or related execution results, including
        shared failure. *)
    shallow_encode_reasons : forall t1 t2, X (Encode (Shallow t1)) (Encode (Shallow t2)) ->
      X (Thunk t1) (Thunk t2) \/ rel_opt X (execute (Shallow t1)) (execute (Shallow t2));
    (** A shallow Encode is not directly related to a strict Encode. *)
    not_shallow_strict : forall t1 t2, ~X (Encode (Shallow t1)) (Encode (Strict t2));
    (** A strict Encode is not directly related to a shallow Encode. *)
    not_strict_shallow : forall t1 t2, ~X (Encode (Strict t1)) (Encode (Shallow t2));
    (** A non-Encode is not directly related to an Encode in the forward direction. *)
    encode_reverse_absent : forall e h, X h (Encode e) -> ~(exists e', h = Encode e') -> False;
    (** Related bare Thunks satisfy the cases required by Think transport. *)
    related_thunk_reasons : forall t1 t2, X (Thunk t1) (Thunk t2) -> thunk_reasons X t1 t2;
  }.
  Coercion structural : relation_properties >-> congruence.
