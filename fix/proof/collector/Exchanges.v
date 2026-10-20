(** Defines the executable abstract exchange checker and coupon issuer.
    Proves that accepted exchanges preserve claims and propagate their
    conditional trust assumptions. *)

From Stdlib Require Import List Arith Bool Lia.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Collector Require Import Matching Logic Encoding.
From FixProof.Shell Require Import TagCreation.

Module Type TAG_AUTHORS (S : STORAGE).
  Parameter add_author : S.TreeName -> S.handle -> S.TreeName.
  (** Issuing a claim retains every supplied author in the updated author list. *)
  Parameter add_author_preserves_authors : forall authors issuing_author,
    incl (S.get_tree_raw authors) (S.get_tree_raw (add_author authors issuing_author)).
End TAG_AUTHORS.

Module CouponExchanges (S : STORAGE) (P : PROGRAM S)
  (T : TAG_AUTHORS S).
  Module API := CouponMatching S.
  Module D := Coupon S P.
  Module Wire := CouponClaims S.
  Import D D.EC.E.EP D.EC.E.EP.E D.EC.E.EP.E.H ListNotations.

  Definition relation_of (t : statement_kind) l r := match t with
    | Force => D.coupon_force l r
    | Apply => D.coupon_apply l r | Select => D.coupon_select l r
    | Think => D.coupon_think l r | Eval => D.coupon_eval l r
    | Equivalent => D.coupon_equiv l r
    | Indistinguishable => D.coupon_indistinguishable l r end.

  (** Witness authentication and logical validity are different obligations.
      Authentication determines which claim a stored Tag actually contains; validity says
      that this decoded claim follows from the abstract rules. The checker must first
      obtain the former before any rule may use the latter. *)
  Definition valid witness := forall kind authors lhs rhs,
    Wire.decode witness = Some (kind, authors, lhs, rhs) -> relation_of kind lhs rhs.

  Definition read kind witness := match Wire.decode witness with
    | Some (decoded_kind, _, lhs, rhs) =>
      if API.statement_kind_eq_dec decoded_kind kind then Some (lhs, rhs) else None
    | None => None end.

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
    | TreeReference, Data (Object (TreeObj t)) => Some (HTreeRef t)
    | _, _ => None end.

  Definition mapped kind a b :=
    obind (construct kind a) (fun x => omap (pair x) (construct kind b)).

  (** Applying a supported constructor to both sides of an equivalence gives an equivalence
      justified by the coupon rules. *)
  Lemma mapped_good kind a b x y :
    coupon_equiv a b -> mapped kind a b = Some (x, y) -> coupon_equiv x y.
  Proof.
    destruct kind; destruct a as [[[ba|ta]|da]|ua|ea]; destruct b as [[[bb|tb]|db]|ub|eb];
      cbn [mapped construct obind omap]; intros Related Checked; try discriminate;
      inversion Checked; subst; eauto using CouponEqApplication, CouponEqSelection, CouponEqIdentification,
        CouponEqEncodeStrict, CouponEqEncodeShallow, CouponBlobRefEq, CouponTreeRefEq.
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

  Ltac check_handle_eq left right :=
    let Same := fresh "Same" in
    destruct (API.handle_eqb left right) eqn:Same; [|discriminate];
    apply API.handle_eqb_spec in Same; subst right.

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
      | Some (a, x), Some (b, y), Some (c, d) =>
        if (API.handle_eqb lhs a && API.handle_eqb rhs b) && (API.handle_eqb c x && API.handle_eqb d y)
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
    destruct (read Force two) as [[b y]|] eqn:Second; [|discriminate].
    destruct (read Equivalent related) as [[c d]|] eqn:Related; [|discriminate].
    destruct ((API.handle_eqb lhs a && API.handle_eqb rhs b) && (API.handle_eqb c x && API.handle_eqb d y))
      eqn:Same; [|discriminate].
    apply andb_true_iff in Same as [Ends Results].
    apply andb_true_iff in Ends as [Left Right]; apply andb_true_iff in Results as [Source Target].
    apply API.handle_eqb_spec in Left, Right, Source, Target; subst lhs rhs c d.
    inversion Checked; subst kind; eapply CouponForceResultEq.
    - exact (read_sound _ _ _ _ (Forall_inv Valid) First).
    - exact (read_sound _ _ _ _ (Forall_inv (Forall_inv_tail Valid)) Second).
    - exact (read_sound _ _ _ _ (Forall_inv (Forall_inv_tail (Forall_inv_tail Valid))) Related).
  Qed.

  Inductive exchange :=
  | TreeEq | ForceResultEq | ThinkApplication | ThinkToForce | EqApplication | EvalLeaf | EvalTree
  | ForceToEncodeStrict | EvalEq | EqEncodeStrict | Sym | Trans | Self
  | ThunkEq | ThunkStrictEq | ThunkShallowEq | ForceToEncodeShallow
  | EqSelection | EqIdentification | EqEncodeShallow | EvalResultEq | TreeRefEq | BlobRefEq
  | ForceEq | ThinkSelection | ThinkIdentification.

  Definition decode_exchange code := match code with
    | 0 => Some TreeEq
    | 1 => Some ForceResultEq
    | 2 => Some ThinkApplication
    | 3 => Some ThinkToForce
    | 4 => Some EqApplication
    | 5 => Some EvalLeaf
    | 6 => Some EvalTree
    | 7 => Some ForceToEncodeStrict
    | 8 => Some EvalEq
    | 9 => Some EqEncodeStrict
    | 10 => Some Sym
    | 11 => Some Trans
    | 12 => Some Self
    | 13 => Some ThunkEq
    | 14 => Some ThunkStrictEq
    | 15 => Some ThunkShallowEq
    | 16 => Some ForceToEncodeShallow
    | 17 => Some EqSelection
    | 18 => Some EqIdentification
    | 19 => Some EqEncodeShallow
    | 20 => Some EvalResultEq
    | 21 => Some TreeRefEq
    | 22 => Some BlobRefEq
    | 24 => Some ForceEq
    | 25 => Some ThinkSelection
    | 26 => Some ThinkIdentification
    | _ => None
    end.

  Definition check rule coupons lhs rhs := match rule with
    | TreeEq => tree Equivalent coupons lhs rhs
    | ForceResultEq => force_results coupons lhs rhs
    | ThinkApplication => binary Eval Apply Think think_application coupons lhs rhs
    | ThinkToForce => unary Think Force think_data coupons lhs rhs
    | EqApplication => unary Equivalent Equivalent (mapped Application) coupons lhs rhs
    | EvalLeaf => leaf coupons lhs rhs
    | EvalTree => tree Eval coupons lhs rhs
    | ForceToEncodeStrict => unary Force Equivalent (force_encode true) coupons lhs rhs
    | EvalEq => binary Eval Equivalent Eval substitute_eval coupons lhs rhs
    | EqEncodeStrict => unary Equivalent Equivalent (mapped Strict) coupons lhs rhs
    | Sym => unary Equivalent Equivalent (fun a b => Some (b, a)) coupons lhs rhs
    | Trans => binary Equivalent Equivalent Equivalent transitive coupons lhs rhs
    | Self => self coupons lhs rhs
    | ThunkEq => unary Think Equivalent (think_step 0) coupons lhs rhs
    | ThunkStrictEq => unary Think Equivalent (think_step 1) coupons lhs rhs
    | ThunkShallowEq => unary Think Equivalent (think_step 2) coupons lhs rhs
    | ForceToEncodeShallow => unary Force Equivalent (force_encode false) coupons lhs rhs
    | EqSelection => unary Equivalent Equivalent (mapped Selection) coupons lhs rhs
    | EqIdentification => unary Equivalent Equivalent (mapped Identification) coupons lhs rhs
    | EqEncodeShallow => unary Equivalent Equivalent (mapped Shallow) coupons lhs rhs
    | EvalResultEq => unary Eval Equivalent (fun a b => Some (a, b)) coupons lhs rhs
    | TreeRefEq => unary Equivalent Equivalent (mapped TreeReference) coupons lhs rhs
    | BlobRefEq => unary Equivalent Equivalent (mapped BlobReference) coupons lhs rhs
    | ForceEq => binary Equivalent Force Force substitute_force coupons lhs rhs
    | ThinkSelection => binary Eval Select Think think_selection coupons lhs rhs
    | ThinkIdentification => identification coupons lhs rhs
    end.

  (** Every accepted exchange with valid ordered witnesses has a conclusion justified by the
      coupon rules.

      Each branch checks the premises and side conditions of one inference rule. Matching
      the supplied endpoints gives that rule's conclusion. *)
  Theorem check_sound rule coupons lhs rhs kind :
    Forall valid coupons -> check rule coupons lhs rhs = Some kind -> relation_of kind lhs rhs.
  Proof.
    (* The explicit request code chooses the inference rule, and each branch checks only
        that rule's ordered premises and endpoints. The earlier helper lemmas establish
        the rule-shaped implications, so an accepted branch contributes exactly one
        legitimate logical constructor. *)
    intros Valid Checked; destruct rule; cbn [check] in Checked;
      try solve [eapply (tree_sound Equivalent); [intros; apply CouponTreeEq; assumption|exact Valid|exact Checked]];
      try solve [eapply (tree_sound Eval); [intros; apply CouponEvalTreeObj; assumption|exact Valid|exact Checked]];
      try solve [eauto using force_results_sound, leaf_sound, self_sound, identification_sound];
      try solve [eapply unary_sound; [|exact Valid|exact Checked]; intros; cbn [relation_of] in *;
        eauto using mapped_good, force_encode_good, think_step_good, think_data_good];
      try solve [eapply binary_sound; [|exact Valid|exact Checked]; intros; cbn [relation_of] in *;
        eauto using think_application_good, substitute_eval_good, transitive_good,
          substitute_force_good, think_selection_good].
    - eapply unary_sound; [|exact Valid|exact Checked].
      intros a b x y Related Derived; inversion Derived; subst; apply CouponSym; exact Related.
    - eapply unary_sound; [|exact Valid|exact Checked].
      intros a b x y Evaluated Derived; inversion Derived; subst; apply CouponEvalResultEq; exact Evaluated.
  Qed.

  Definition authors_of witness := match Wire.decode witness with
    | Some (_, authors, _, _) => get_tree_raw authors | None => [] end.
  Definition dependencies := flat_map authors_of.
  Definition output_authors issuing_author coupons := T.add_author (create_tree (dependencies coupons)) issuing_author.
  Definition issue issuing_author kind lhs rhs coupons := Wire.encode kind (output_authors issuing_author coupons) lhs rhs.
  Definition stable_conditional trusted witness := forall kind authors lhs rhs,
    Wire.decode witness = Some (kind, authors, lhs, rhs) ->
    Forall (fun author => value_handle author -> trusted author) (get_tree_raw authors) -> relation_of kind lhs rhs.

  (** Every authority on which a premise depends remains among the conclusion's authors.
      This prevents an exchange from turning a conditional premise into a claim with fewer
      trust requirements. *)
  Lemma authors_cover issuing_author coupons witness kind authors lhs rhs :
    In witness coupons -> Wire.decode witness = Some (kind, authors, lhs, rhs) ->
    incl (get_tree_raw authors) (get_tree_raw (output_authors issuing_author coupons)).
  Proof.
    intros Member Decoded author Author; unfold output_authors; apply T.add_author_preserves_authors.
    rewrite get_tree_raw_create_tree; unfold dependencies; apply in_flat_map.
    exists witness; split; [exact Member|unfold authors_of; rewrite Decoded; exact Author].
  Qed.

  (** An accepted exchange preserves conditional authority: if the stable authors retained
      in its output are trusted, its conclusion follows. Unrestricted extra authors can be
      retained without becoming a source of unconditional conclusions. *)
  Theorem issue_conditional trusted issuing_author rule coupons lhs rhs kind output :
    Forall (stable_conditional trusted) coupons -> check rule coupons lhs rhs = Some kind ->
    issue issuing_author kind lhs rhs coupons = Some output -> stable_conditional trusted output.
  Proof.
    (* Argue under the output's trust assumption. Author retention makes that assumption
        strong enough to activate every conditional input claim. Only after obtaining
        these premise facts is ordinary exchange soundness applicable. *)
    intros Inputs Checked Issued operation authors left right Decoded Trusted.
    apply Wire.encode_decode in Issued; rewrite Issued in Decoded;
      inversion Decoded; subst operation authors left right.
    (* The exchange theorem requires actual premise facts. They become available only
        within the scope of the output trust assumption; this implication is what must be
        preserved in the issued result. *)
    assert (PremisesValid : Forall valid coupons).
    { rewrite Forall_forall in Inputs |- *; intros witness Member input_kind input_authors a b Read.
      (* A premise's required authorities are a subset of the output authorities. Thus no
          dependency can disappear when the premise is consumed. *)
      assert (AuthorsCovered : incl (get_tree_raw input_authors)
        (get_tree_raw (output_authors issuing_author coupons)))
        by (eauto using authors_cover).
      (* Restrict the output trust assumption to this premise's stable authors. Its
          conditional claim can now be invoked without trusting any additional party. *)
      assert (AuthorsTrusted : Forall (fun author => value_handle author -> trusted author)
        (get_tree_raw input_authors))
        by (rewrite Forall_forall in Trusted |- *; hauto unfold!: incl).
      exact (Inputs witness Member _ _ _ _ Read AuthorsTrusted). }
    hauto cases: - use: check_sound.
  Qed.

End CouponExchanges.
