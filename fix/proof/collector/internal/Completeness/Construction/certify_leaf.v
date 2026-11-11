    (** Leaf evaluation can be certified without computation premises. *)
    Lemma certify_leaf value : EC.leaf value -> certifies issuing_author Eval value value.
    Proof.
      intro Leaf; destruct value as [[[b|t]|r]|th|e];
        cbn [EC.leaf] in Leaf; try contradiction; exchange X.EvalLeaf (@nil handle).
    Qed.

    (** A certified equivalence can be reversed through the symmetry exchange. *)
    Lemma certify_sym a b : certifies issuing_author Equivalent a b -> certifies issuing_author Equivalent b a.
    Proof.
      intros [p [Issued Read]]; exchange X.Sym [p].
    Qed.

    (** A certified equivalence transports a certified evaluation to the equivalent input. *)
    Lemma certify_eval_eq a b result :
      certifies issuing_author Eval a result -> certifies issuing_author Equivalent a b -> certifies issuing_author Eval b result.
    Proof.
      intros [p [First One]] [q [Second Two]]; exchange X.EvalEq [p; q].
    Qed.

    (** A certified Think step returning Data certifies Force to that Data. *)
    Lemma certify_force_data th d :
      certifies issuing_author Think (Thunk th) (Data d) -> certifies issuing_author Force (Thunk th) (Data d).
    Proof.
      intros [p [Issued Read]]; exchange X.ThinkToForce [p].
    Qed.

    (** A Force certificate certifies the equivalence of an Encode to its appropriately
        lifted or lowered result. *)
    Lemma certify_force_encode (strict : bool) th d :
      certifies issuing_author Force (Thunk th) (Data d) ->
      certifies issuing_author Equivalent (Encode (if strict then H.Strict th else H.Shallow th))
        (if strict then lift (Data d) else lower (Data d)).
    Proof.
      intros [p [Issued Read]]; destruct strict;
        [exchange X.ForceToEncodeStrict [p]|exchange X.ForceToEncodeShallow [p]].
    Qed.

    (** A Force certificate certifies evaluation of the shallow Encode to the lowered
        result. *)
    Lemma certify_shallow th d : certifies issuing_author Force (Thunk th) (Data d) ->
      certifies issuing_author Eval (Encode (Shallow th)) (lower (Data d)).
    Proof.
      intro Forced; destruct (EC.lower_reference d) as [reference [Lower Lift]].
      change (lower (Data d) = Data (Ref reference)) in Lower.
      eapply certify_eval_eq with (a := lower (Data d)); [rewrite Lower; apply certify_leaf; exact I|].
      apply certify_sym; exact (certify_force_encode false th d Forced).
    Qed.

    (** A certified equivalence transports a Force certificate to the equivalent input. *)
    Lemma certify_force_substitute a b d : certifies issuing_author Equivalent a b ->
      certifies issuing_author Force a (Data d) -> certifies issuing_author Force b (Data d).
    Proof.
      intros [p [First One]] [q [Second Two]]; exchange X.ForceEq [p; q].
    Qed.

    (** A certified Think step returning a Thunk or Encode certifies equivalence to the
        next bare Thunk. *)
    Lemma certify_next th value next : EC.next_thunk value = Some next ->
      certifies issuing_author Think (Thunk th) value -> certifies issuing_author Equivalent (Thunk th) (Thunk next).
    Proof.
      intros Next [p [Issued Read]].
      destruct value as [d|t|[t|t]]; cbn [EC.next_thunk] in Next; try discriminate;
        inversion Next; subst t.
      - exchange X.ThunkEq [p].
      - exchange X.ThunkStrictEq [p].
      - exchange X.ThunkShallowEq [p].
    Qed.

    (** Identification of a Ref has a Think certificate without computation premises. *)
    Lemma certify_identification reference : certifies issuing_author Think
      (Thunk (Identification reference)) (Data (Ref reference)).
    Proof.
      exchange X.ThinkIdentification (@nil handle).
    Qed.

    (** A certified combination evaluation and a primitive application certificate certify
        the application Thunk's Think step. *)
    Lemma certify_application tree ready result :
      certifies issuing_author Eval (HTreeObj tree) (HTreeObj ready) ->
      value_handle (HTreeObj ready) -> A.apply_tree ready = Some result ->
      certifies issuing_author Think (Thunk (Application tree)) result.
    Proof.
      (* The evaluation certificate gets the original combination to a Value combination.
          Only that ready combination may receive a primitive Apply certificate. The
          public Think-application exchange connects the resulting one-step computation to
          the original application Thunk. *)
      intros [p [Issued Read]] Value Applied.
      set (q := Wire.witness (HTreeObj (create_tree [])) 2 (HTreeObj ready) result).
      assert (Second : read Apply q = Some (HTreeObj ready, result)).
      { apply (read_encoded _ (HTreeObj (create_tree [])) _ _ _); reflexivity. }
      assert (issued issuing_author q) by (apply IssuedApplication; assumption).
      exchange X.ThinkApplication [p; q].
    Qed.

    (** A certified operand-Tree evaluation and a primitive selection certificate certify
        the selection Thunk's Think step. *)
    Lemma certify_selection tree ready result :
      certifies issuing_author Eval (HTreeObj tree) (HTreeObj ready) -> Sl.select ready = Some result ->
      certifies issuing_author Think (Thunk (Selection tree)) result.
    Proof.
      (* The operand evaluation certificate determines the ready selection Tree. A
          primitive Select certificate speaks about selection from that Tree; the public
          exchange connects this storage fact back to the original selection Thunk. *)
      intros [p [Issued Read]] Selected.
      set (q := Wire.witness (HTreeObj (create_tree [])) 6 (HTreeObj ready) result).
      assert (Second : read Select q = Some (HTreeObj ready, result)).
      { apply (read_encoded _ (HTreeObj (create_tree [])) _ _ _); reflexivity. }
      assert (issued issuing_author q) by (apply IssuedSelection; assumption).
      exchange X.ThinkSelection [p; q].
    Qed.
