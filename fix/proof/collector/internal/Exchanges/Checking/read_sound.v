  (** Reading the requested kind from a valid witness gives a claim justified by the coupon
      rules. *)
  Lemma read_sound kind witness lhs rhs :
    valid witness -> read kind witness = Some (lhs, rhs) -> relation_of kind lhs rhs.
  Proof.
    unfold read; destruct (Wire.decode witness) as [[[[decoded_kind authors] a] b]|] eqn:Decoded;
      [|discriminate].
    destruct (API.statement_kind_eq_dec decoded_kind kind); [subst decoded_kind|discriminate].
    intros Valid Checked; inversion Checked; subst; exact (Valid _ _ _ _ Decoded).
  Qed.

  Definition unary input (output : statement_kind) derive coupons lhs rhs := match coupons with
    | [coupon] => match read input coupon with
      | Some (a, b) => match derive a b with
        | Some (x, y) => if API.handle_eqb lhs x && API.handle_eqb rhs y then Some output else None
        | None => None end
      | None => None end
    | _ => None end.

  (** A sound unary inference, supplied with valid premises, yields a sound checked
      exchange. *)
  Lemma unary_sound input output derive
      (Rule : forall a b x y, relation_of input a b -> derive a b = Some (x, y) -> relation_of output x y)
      coupons lhs rhs kind :
    Forall valid coupons -> unary input output derive coupons lhs rhs = Some kind -> relation_of kind lhs rhs.
  Proof.
    (* The witness names both endpoints of the premise. Exact matching prevents a valid
        premise about some other endpoints from being used here; once these endpoints
        agree, the supplied inference rule determines the conclusion. *)
    intros Valid Checked; unfold unary in Checked.
    destruct coupons as [|premise [|extra rest]]; try discriminate.
    destruct (read input premise) as [[a b]|] eqn:Read; [|discriminate].
    destruct (derive a b) as [[x y]|] eqn:Derived; [|discriminate].
    destruct (API.handle_eqb lhs x && API.handle_eqb rhs y) eqn:Same; [|discriminate].
    apply andb_true_iff in Same as [Left Right].
    apply API.handle_eqb_spec in Left; apply API.handle_eqb_spec in Right; subst lhs rhs.
    inversion Checked; subst kind; eapply Rule; [|exact Derived].
    exact (read_sound _ _ _ _ (Forall_inv Valid) Read).
  Qed.

  Definition binary first second (output : statement_kind) derive coupons lhs rhs := match coupons with
    | [one; two] => match read first one, read second two with
      | Some (a, b), Some (c, d) => match derive a b c d with
        | Some (x, y) => if API.handle_eqb lhs x && API.handle_eqb rhs y then Some output else None
        | None => None end
      | _, _ => None end
    | _ => None end.

  (** A sound binary inference, supplied with valid premises, yields a sound checked
      exchange. *)
  Lemma binary_sound first second output derive
      (Rule : forall a b c d x y, relation_of first a b -> relation_of second c d ->
        derive a b c d = Some (x, y) -> relation_of output x y)
      coupons lhs rhs kind :
    Forall valid coupons -> binary first second output derive coupons lhs rhs = Some kind -> relation_of kind lhs rhs.
  Proof.
    (* Each witness first yields its own authenticated premise. The endpoint checks
        identify those premises with the inputs expected by the inference rule, so the
        rule composes exactly those two facts without searching for a middle term. *)
    intros Valid Checked; unfold binary in Checked.
    destruct coupons as [|one [|two [|extra rest]]]; try discriminate.
    destruct (read first one) as [[a b]|] eqn:One; [|discriminate].
    destruct (read second two) as [[c d]|] eqn:Two; [|discriminate].
    destruct (derive a b c d) as [[x y]|] eqn:Derived; [|discriminate].
    destruct (API.handle_eqb lhs x && API.handle_eqb rhs y) eqn:Same; [|discriminate].
    apply andb_true_iff in Same as [Left Right].
    apply API.handle_eqb_spec in Left; apply API.handle_eqb_spec in Right; subst lhs rhs.
    inversion Checked; subst kind; eapply Rule; [| |exact Derived].
    - exact (read_sound _ _ _ _ (Forall_inv Valid) One).
    - exact (read_sound _ _ _ _ (Forall_inv (Forall_inv_tail Valid)) Two).
  Qed.

  Fixpoint children kind xs ys coupons := match xs, ys, coupons with
    | [], [], [] => true
    | x :: xs, y :: ys, coupon :: coupons => match read kind coupon with
        | Some (a, b) => API.handle_eqb x a && API.handle_eqb y b && children kind xs ys coupons
        | None => false end
    | _, _, _ => false end.

  (** Successful checks of ordered child witnesses establish the requested relation
      pointwise. *)
  Lemma children_sound kind xs ys coupons :
    Forall valid coupons -> children kind xs ys coupons = true -> Forall2 (relation_of kind) xs ys.
  Proof.
    (* The list position assigns each witness to a specific child pair. Requiring the
        three lists to end together prevents missing or extra witnesses from silently
        changing the intended pointwise judgement. *)
    revert ys coupons; induction xs as [|x xs IH]; intros [|y ys] [|coupon coupons] Valid Checked;
      cbn [children] in Checked; try discriminate; [constructor|].
    destruct (read kind coupon) as [[a b]|] eqn:Read; [|discriminate].
    apply andb_true_iff in Checked as [Same Rest]; apply andb_true_iff in Same as [Left Right].
    apply API.handle_eqb_spec in Left; apply API.handle_eqb_spec in Right; subst a b; constructor.
    - exact (read_sound _ _ _ _ (Forall_inv Valid) Read).
    - eapply IH; [exact (Forall_inv_tail Valid)|exact Rest].
  Qed.

  Definition tree kind coupons lhs rhs := match lhs, rhs with
    | Data (Object (TreeObj a)), Data (Object (TreeObj b)) =>
      if Bool.eqb (tree_tag a) (tree_tag b) && children kind (get_tree_raw a) (get_tree_raw b) coupons
      then Some kind else None
    | _, _ => None end.

  (** A sound pointwise Tree inference yields a sound checked Tree exchange. *)
  Lemma tree_sound input
      (Rule : forall a b, tree_tag a = tree_tag b ->
        Forall2 (relation_of input) (get_tree_raw a) (get_tree_raw b) -> relation_of input (HTreeObj a) (HTreeObj b))
      coupons lhs rhs kind :
    Forall valid coupons -> tree input coupons lhs rhs = Some kind -> relation_of kind lhs rhs.
  Proof.
    destruct lhs as [[[bl|a]|reference]|th|e], rhs as [[[br|b]|reference']|th'|e'];
      cbn [tree]; intros Valid Checked; try discriminate.
    destruct (Bool.eqb (tree_tag a) (tree_tag b) && children input (get_tree_raw a) (get_tree_raw b) coupons)
      eqn:Test; [|discriminate].
    apply andb_true_iff in Test as [Tags Entries]; apply Bool.eqb_prop in Tags.
    inversion Checked; subst kind; apply Rule; [exact Tags|eapply children_sound; eassumption].
  Qed.

  Inductive constructor := Application | Selection | Identification | Strict | Shallow | BlobReference | TreeReference.

  Definition construct kind value := match kind, value with
    | Application, Data (Object (TreeObj t)) => Some (Thunk (H.Application t))
    | Selection, Data (Object (TreeObj t)) => Some (Thunk (H.Selection t))
    | Identification, Data (Ref d) => Some (Thunk (H.Identification d))
    | Strict, Thunk t => Some (Encode (H.Strict t))
    | Shallow, Thunk t => Some (Encode (H.Shallow t))
    | BlobReference, Data (Object (BlobObj b)) => Some (HBlobRef b)
    | TreeReference, Data (Object (TreeObj t)) => if is_value (HTreeObj t) then Some (HTreeRef t) else None
    | _, _ => None end.

  Definition mapped kind a b :=
    obind (construct kind a) (fun x => omap (pair x) (construct kind b)).

  (** Applying a supported constructor to both sides of an equivalence gives an equivalence
      justified by the coupon rules. *)
  Lemma mapped_good kind a b x y :
    coupon_equiv a b -> mapped kind a b = Some (x, y) -> coupon_equiv x y.
  Proof.
    destruct kind; destruct a as [[[ba|ta]|da]|ua|ea]; destruct b as [[[bb|tb]|db]|ub|eb];
      intros Related Checked; unfold mapped, construct in Checked; cbn in Checked;
      repeat match type of Checked with
      | context [if ?condition then _ else _] => destruct condition eqn:?; cbn [obind omap] in Checked
      end; try discriminate; inversion Checked; subst;
      eauto using CouponEqApplication, CouponEqSelection, CouponEqIdentification,
        CouponEqEncodeStrict, CouponEqEncodeShallow, CouponBlobRefEq.
    apply CouponTreeRefEq; [apply EC.value_flag; assumption|apply EC.value_flag; assumption|exact Related].
  Qed.

  Definition force_encode (strict : bool) a b := match a, b with
    | Thunk t, Data d => Some (Encode (if strict then H.Strict t else H.Shallow t),
        if strict then lift (Data d) else lower (Data d))
    | _, _ => None end.

  (** A checked Force-to-Encode exchange gives an equivalence justified by the Force
      premise. *)
  Lemma force_encode_good strict a b x y :
    coupon_force a b -> force_encode strict a b = Some (x, y) -> coupon_equiv x y.
  Proof.
    destruct a as [a|t|a], b as [d|u|e]; cbn [force_encode]; intros Forced Checked; try discriminate.
    destruct strict; cbn in Checked; inversion Checked; subst.
    - exact (CouponForceToEncodeStrict t (Data d) Forced).
    - exact (CouponForceToEncodeShallow t (Data d) Forced).
  Qed.

  Definition think_step (mode : nat) (a b : handle) := match b with
    | Thunk t => if mode =? 0 then Some (a, Thunk t) else None
    | Encode (H.Strict t) => if mode =? 1 then Some (a, Thunk t) else None
    | Encode (H.Shallow t) => if mode =? 2 then Some (a, Thunk t) else None
    | _ => None end.

  (** A checked Think-to-Thunks exchange gives equivalence of the corresponding bare
      Thunks. *)
  Lemma think_step_good mode a b x y :
    coupon_think a b -> think_step mode a b = Some (x, y) -> coupon_equiv x y.
  Proof.
    intros Thought Checked.
    pose proof (coupon_think_sound _ _ Thought) as Sound.
    destruct a as [data|t|encoded]; try contradiction.
    destruct b as [d|u|[u|u]]; cbn [think_step] in Checked; try discriminate;
      repeat match type of Checked with
      | context [if ?condition then _ else _] => destruct condition eqn:?; try discriminate
      end; inversion Checked; subst;
      eauto using CouponThunkEq, CouponThunkEncodeEq, CouponThunkEncodeShallowEq.
  Qed.
