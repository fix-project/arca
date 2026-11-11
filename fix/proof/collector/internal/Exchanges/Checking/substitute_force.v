  Definition substitute_force (a b c d : handle) :=
    if API.handle_eqb a c then Some (b, d) else None.

  (** A checked substitution of equivalent inputs preserves the Force claim. *)
  Lemma substitute_force_good a b c d x y :
    coupon_equiv a b -> coupon_force c d -> substitute_force a b c d = Some (x, y) -> coupon_force x y.
  Proof.
    intros Related Forced Checked; unfold substitute_force in Checked.
    check_handle_eq a c; inversion Checked; subst.
    eapply CouponForceEq; eassumption.
  Qed.

  Definition think_selection (a b c d : handle) :=
    if API.handle_eqb b c then match a with
      | Data (Object (TreeObj t)) => Some (Thunk (H.Selection t), d)
      | _ => None end else None.

  (** A checked combination of Eval and Select premises gives the corresponding
      Think-selection claim. *)
  Lemma think_selection_good a b c d x y :
    coupon_eval a b -> coupon_select c d -> think_selection a b c d = Some (x, y) -> coupon_think x y.
  Proof.
    intros Evaluated Selected Checked; unfold think_selection in Checked.
    check_handle_eq b c.
    destruct a as [[[blob|t]|reference]|th|e]; try discriminate.
    destruct (coupon_select_sound _ _ Selected) as [ready [-> Select]].
    inversion Checked; subst; eapply CouponThinkSelection; eassumption.
  Qed.

  Definition reflexive_eval value := match value with
    | Data (Ref _) | Thunk _ | Data (Object (BlobObj _)) => true | _ => false end.

  Definition leaf (coupons : list handle) lhs rhs := match coupons with
    | [] => if reflexive_eval lhs && API.handle_eqb lhs rhs then Some Eval else None
    | _ => None end.

  (** A successful leaf-evaluation check yields a valid Eval claim. *)
  Lemma leaf_sound coupons lhs rhs kind :
    leaf coupons lhs rhs = Some kind -> relation_of kind lhs rhs.
  Proof.
    unfold leaf; destruct coupons; [|discriminate].
    destruct (reflexive_eval lhs && API.handle_eqb lhs rhs) eqn:Checked; [|discriminate].
    apply andb_true_iff in Checked as [Leaf Same]; apply API.handle_eqb_spec in Same; subst rhs.
    intros Created; inversion Created; subst; change (coupon_eval lhs lhs).
    destruct lhs as [[[blob|t]|reference]|th|e]; cbn [reflexive_eval] in Leaf; try discriminate;
      eauto using CouponEvalBlobObj, CouponEvalRef, CouponEvalThunk.
  Qed.

  Definition identification (coupons : list handle) lhs rhs := match coupons, rhs with
    | [], Data (Ref reference) =>
      if API.handle_eqb lhs (Thunk (H.Identification reference)) then Some Think else None
    | _, _ => None end.

  (** A successful identification check yields the Ref's valid Think-identification claim. *)
  Lemma identification_sound coupons lhs rhs kind :
    identification coupons lhs rhs = Some kind -> relation_of kind lhs rhs.
  Proof.
    unfold identification; destruct coupons; [|discriminate].
    destruct rhs as [[o|reference]|th|e]; try discriminate.
    destruct (API.handle_eqb lhs (Thunk (H.Identification reference))) eqn:Same; [|discriminate].
    apply API.handle_eqb_spec in Same; subst lhs.
    intros Created; inversion Created; subst; apply CouponThinkIdentification.
  Qed.

  Definition self (coupons : list handle) lhs rhs := match coupons with
    | [] => if API.handle_eqb lhs rhs then Some Equivalent else None | _ => None end.

  (** A successful reflexivity check yields a valid equivalence claim. *)
  Lemma self_sound coupons lhs rhs kind : self coupons lhs rhs = Some kind -> relation_of kind lhs rhs.
  Proof.
    unfold self; destruct coupons; [|discriminate].
    destruct (API.handle_eqb lhs rhs) eqn:Same; [|discriminate].
    apply API.handle_eqb_spec in Same; subst rhs; intros Checked; inversion Checked; subst; apply CouponSelf.
  Qed.

  Definition think_application (a b c d : handle) :=
    if API.handle_eqb b c then match a with
      | Data (Object (TreeObj t)) => Some (Thunk (H.Application t), d)
      | _ => None end else None.

  (** A checked combination of Eval and Apply premises gives the corresponding
      Think-application claim. *)
  Lemma think_application_good a b c d x y :
    coupon_eval a b -> coupon_apply c d -> think_application a b c d = Some (x, y) -> coupon_think x y.
  Proof.
    intros Evaluated Applied Checked; unfold think_application in Checked.
    check_handle_eq b c.
    destruct a as [[[blob|t]|reference]|th|e]; try discriminate.
    destruct (coupon_apply_sound _ _ Applied) as [ready [Source Rest]].
    rewrite Source in Evaluated, Applied; inversion Checked; subst; eapply CouponThinkApplication; eassumption.
  Qed.

  Definition think_data (a b : handle) := match b with Data _ => Some (a, b) | _ => None end.

  (** A checked Think step returning Data yields a valid Force claim. *)
  Lemma think_data_good a b x y :
    coupon_think a b -> think_data a b = Some (x, y) -> coupon_force x y.
  Proof.
    intros Thought Checked.
    pose proof (coupon_think_sound _ _ Thought) as Sound.
    destruct a as [data|th|encoded]; try contradiction.
    destruct b as [d|th''|e]; cbn [think_data] in Checked; try discriminate.
    inversion Checked; subst; apply CouponThinkToForce; exact Thought.
  Qed.

  Definition substitute_eval (a b c d : handle) := if API.handle_eqb a c then Some (d, b) else None.

  (** A checked substitution of equivalent inputs preserves the Eval claim. *)
  Lemma substitute_eval_good a b c d x y :
    coupon_eval a b -> coupon_equiv c d -> substitute_eval a b c d = Some (x, y) -> coupon_eval x y.
  Proof.
    intros Evaluated Related Checked; unfold substitute_eval in Checked.
    check_handle_eq a c; inversion Checked; subst; eapply CouponEvalEq; eassumption.
  Qed.

  Definition transitive (a b c d : handle) := if API.handle_eqb b c then Some (a, d) else None.

  (** A checked pair of equivalences with a matching middle endpoint yields their
      transitive equivalence. *)
  Lemma transitive_good a b c d x y :
    coupon_equiv a b -> coupon_equiv c d -> transitive a b c d = Some (x, y) -> coupon_equiv x y.
  Proof.
    intros First Second Checked; unfold transitive in Checked.
    check_handle_eq b c; inversion Checked; subst; eapply CouponTrans; eassumption.
  Qed.

  Definition force_results coupons lhs rhs := match coupons with
    | [one; two; related] => match read Force one, read Force two, read Equivalent related with
      | Some (a, Data x), Some (b, Data y), Some (c, d) =>
        if (API.handle_eqb lhs a && API.handle_eqb rhs b) && (API.handle_eqb c (lower (Data x)) && API.handle_eqb d (lower (Data y)))
        then Some Equivalent else None
      | _, _, _ => None end
    | _ => None end.

  (** Valid Force premises with equivalent results justify equivalence of their source
      Thunks. *)
  Lemma force_results_sound coupons lhs rhs kind :
    Forall valid coupons -> force_results coupons lhs rhs = Some kind -> relation_of kind lhs rhs.
  Proof.
    intros Valid Checked; unfold force_results in Checked.
    destruct coupons as [|one [|two [|related [|extra rest]]]]; try discriminate.
    destruct (read Force one) as [[a x]|] eqn:First; [|discriminate].
    destruct x as [x|tx|ex]; try discriminate.
    destruct (read Force two) as [[b y]|] eqn:Second; [|discriminate].
    destruct y as [y|ty|ey]; try discriminate.
    destruct (read Equivalent related) as [[c d]|] eqn:Related; [|discriminate].
    destruct ((API.handle_eqb lhs a && API.handle_eqb rhs b) && (API.handle_eqb c (lower (Data x)) && API.handle_eqb d (lower (Data y))))
      eqn:Same; [|discriminate].
    apply andb_true_iff in Same as [Ends Results].
    apply andb_true_iff in Ends as [Left Right]; apply andb_true_iff in Results as [Source Target].
    apply API.handle_eqb_spec in Left, Right, Source, Target; subst lhs rhs c d.
    inversion Checked; subst kind; eapply CouponForceResultEq.
    - exact (read_sound _ _ _ _ (Forall_inv Valid) First).
    - exact (read_sound _ _ _ _ (Forall_inv (Forall_inv_tail Valid)) Second).
    - exact (read_sound _ _ _ _ (Forall_inv (Forall_inv_tail (Forall_inv_tail Valid))) Related).
  Qed.
