(** Models issuance from primitive computation certificates and collector
    exchanges. Proves issued claims sound and every terminating evaluation
    certifiable through the public exchanges. *)

From Stdlib Require Import List Bool.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Collector Require Import Matching Encoding Exchanges.
From FixProof.Evaluation Require Import Adequacy.
From FixProof.Shell Require Import TagCreation.
Import ListNotations.

Module CertificationCompleteness (S : STORAGE) (P : PROGRAM S).
  Module Authors := TagCreation S P.
  Module X := CouponExchanges S P Authors.
  Module EC := EvaluationAdequacy S P X.D.EC.E.EP.
  Import X X.D X.D.EC.E.EP X.D.EC.E.EP.E X.D.EC.E.EP.E.H.

  (** This records the certificates a scheduler could obtain: exact application and
      selection facts come from their trusted issuers, while other facts arise by accepted
      exchanges on certificates already obtained. An issuance derivation is finite
      evidence of availability; it does not require the collector to perform the
      computations being certified. *)
  Inductive issued issuing_author : handle -> Prop :=
  | IssuedApplication tree result :
      value_handle (HTreeObj tree) -> A.apply_tree tree = Some result ->
      issued issuing_author (Wire.witness (create_tree []) 2 (HTreeObj tree) result)
  | IssuedSelection tree result :
      Sl.select tree = Some result ->
      issued issuing_author (Wire.witness (create_tree []) 6 (HTreeObj tree) result)
  | IssuedExchange rule coupons lhs rhs kind output :
      Forall (issued issuing_author) coupons -> check rule coupons lhs rhs = Some kind ->
      issue issuing_author kind lhs rhs coupons = Some output -> issued issuing_author output.

  Definition certifies issuing_author kind lhs rhs := exists witness,
    issued issuing_author witness /\ read kind witness = Some (lhs, rhs).

  (** Reading a successfully encoded certificate recovers its claim endpoints. *)
  Lemma read_encoded kind authors lhs rhs output :
    Wire.encode kind authors lhs rhs = Some output -> read kind output = Some (lhs, rhs).
  Proof.
    intro Encoded; apply Wire.encode_decode in Encoded; unfold read; rewrite Encoded.
    destruct (API.statement_kind_eq_dec kind kind); [reflexivity|contradiction].
  Qed.

  (** A successful public exchange with previously issued premises can issue a certificate
      for its conclusion. *)
  Lemma issue_checked issuing_author rule coupons lhs rhs kind code :
    Forall (issued issuing_author) coupons -> check rule coupons lhs rhs = Some kind ->
    Wire.Code.statement_code kind = Some code -> certifies issuing_author kind lhs rhs.
  Proof.
    (* An abstract inference is useful to the scheduler only if it can be represented by
        an actual issued witness. The supported conclusion code supplies that
        representation, while the previous witnesses remain the ordered premises recorded
        in its issuance derivation. *)
    intros Inputs Checked Code.
    assert (OutputExists : exists output, issue issuing_author kind lhs rhs coupons = Some output).
    { unfold issue, Wire.encode; rewrite Code; eexists; reflexivity. }
    hauto cases: - use: IssuedExchange, read_encoded unfold!: certifies.
  Qed.

  (** Encoding a true claim produces a valid certificate witness. *)
  Lemma encoded_valid kind authors lhs rhs output :
    relation_of kind lhs rhs -> Wire.encode kind authors lhs rhs = Some output -> valid output.
  Proof.
    hauto cases: - use: Wire.encode_decode unfold!: valid.
  Qed.

  (** Every issued witness has a claim justified by the abstract coupon rules. *)
  Fixpoint issued_sound issuing_author witness (Derivation : issued issuing_author witness) {struct Derivation} : valid witness.
  Proof.
    (* Every possible source of a certificate must be justified. The two primitive
        sources require the actual computation facts in their constructors. For an
        exchange, the earlier issuance derivations justify its premises, and checker
        soundness justifies the new fact. *)
    destruct Derivation as [tree result Value Applied|tree result Selected|
      rule coupons lhs rhs kind output Inputs Checked Encoded].
    - eapply (encoded_valid Apply (create_tree []) (HTreeObj tree) result);
        [|reflexivity].
      apply CouponApply; assumption.
    - eapply (encoded_valid Select (create_tree []) (HTreeObj tree) result);
        [|reflexivity].
      apply CouponSelect; exact Selected.
    - eapply encoded_valid; [|exact Encoded].
      eapply check_sound; [|exact Checked].
      refine ((fix premises xs (Proof : Forall (issued issuing_author) xs) {struct Proof} :
        Forall valid xs := _) _ Inputs).
      destruct Proof as [|head tail Head Tail].
      + constructor.
      + constructor; [exact (issued_sound issuing_author head Head)|exact (premises tail Tail)].
  Qed.

  (** A scheduler-built issuance derivation establishes a genuine logical claim. The
      scheduler's choices of witnesses and exchanges add no inference power beyond the
      primitive certificates and checked collector rules. *)
  Corollary certification_sound issuing_author kind lhs rhs :
    certifies issuing_author kind lhs rhs -> relation_of kind lhs rhs.
  Proof. intros [witness [Issued Read]]; eauto using read_sound, issued_sound. Qed.

  #[local] Hint Rewrite API.handle_eqb_refl : coupon.
  Ltac simplify_check := repeat progress (cbn -[API.handle_eqb]; autorewrite with coupon).
  Ltac exchange exchange_rule premises :=
    eapply issue_checked with (rule := exchange_rule) (coupons := premises);
      [solve [auto using Forall_cons, Forall_nil]| |reflexivity];
    cbn [check unary binary];
    repeat match goal with
    | Read : read ?kind ?witness = _ |- context [read ?kind ?witness] => rewrite Read
    end;
    cbv beta iota zeta delta [substitute_eval substitute_force think_data force_encode think_step
      X.think_application X.think_selection];
    simplify_check; reflexivity.

  Section Certification.
    Variable issuing_author : handle.

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
      set (q := Wire.witness (create_tree []) 2 (HTreeObj ready) result).
      assert (Second : read Apply q = Some (HTreeObj ready, result)).
      { apply (read_encoded _ (create_tree []) _ _ _); reflexivity. }
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
      set (q := Wire.witness (create_tree []) 6 (HTreeObj ready) result).
      assert (Second : read Select q = Some (HTreeObj ready, result)).
      { apply (read_encoded _ (create_tree []) _ _ _); reflexivity. }
      assert (issued issuing_author q) by (apply IssuedSelection; assumption).
      exchange X.ThinkSelection [p; q].
    Qed.

    (** Each evaluation derivation yields an Eval certificate. For a shallow Encode it
        also yields the Force certificate and Data witness behind the resulting Ref,
        preserving information needed by later exchanges. *)
    Fixpoint certify_eval value result (Derivation : EC.eval_derivation value result) {struct Derivation} :
      certifies issuing_author Eval value result /\
      match value with Encode (Shallow th) => exists d,
        certifies issuing_author Force (Thunk th) (Data d) /\ lower (Data d) = result
      | _ => True end
    (** A following derivation turns a certificate for the preceding Think step into a
        Force certificate for Data with the required lowered result. *)
    with certify_follow value result (Derivation : EC.follow_derivation value result) {struct Derivation} :
      forall outer, certifies issuing_author Think (Thunk outer) value -> exists d,
        certifies issuing_author Force (Thunk outer) (Data d) /\ lower (Data d) = result.
    Proof.
      (* Merely certifying the lowered Ref would lose the Data witness needed to justify
          a strict consumer or a subsequent Force. Carry that extra witness through the
          induction so every evaluation rule can be reproduced by the public exchanges. *)
      - destruct Derivation as [h Leaf|input children Entries|th reference result ShallowCertified Loaded|
          reference|input ready intermediate result Evaluated Applied Follow|
          input ready intermediate result Evaluated Selected Follow].
        + split; [apply certify_leaf; exact Leaf|].
          destruct h as [d|t|[t|t]]; cbn [EC.leaf] in Leaf; try contradiction; exact I.
        + split; [|exact I].
          (* For a Tree, the recursive certificates must be assembled in the same order
              as the child derivations. This gives exactly the witness list accepted by
              the pointwise exchange. *)
          assert (exists coupons, X.children Eval (get_tree_raw input) children coupons = true /\
            Forall (issued issuing_author) coupons) as Children.
          { refine ((fix entries xs ys (Proof : Forall2 EC.eval_derivation xs ys) {struct Proof} :
              exists coupons, X.children Eval xs ys coupons = true /\ Forall (issued issuing_author) coupons := _)
                _ _ Entries).
            destruct Proof as [|x y xs ys Head Tail].
            - exists []; split; constructor.
            - destruct (certify_eval _ _ Head) as [[p [Issued Read]] _].
              destruct (entries _ _ Tail) as [coupons [Checked Rest]].
              exists (p :: coupons); split; [|constructor; assumption].
              cbn [X.children]; rewrite Read; simplify_check.
              cbn; exact Checked. }
          destruct Children as [coupons [Checked Issued]].
          eapply issue_checked with (rule := X.EvalTree) (coupons := coupons);
            [exact Issued| |reflexivity].
          change ((if Bool.eqb (tree_tag input) (tree_tag (create_tree_tagged (tree_tag input) children)) &&
            X.children Eval (get_tree_raw input)
              (get_tree_raw (create_tree_tagged (tree_tag input) children)) coupons
            then Some Eval else None) = Some Eval).
          rewrite tree_tag_create_tree_tagged, Bool.eqb_reflx,
            get_tree_raw_create_tree_tagged, Checked; reflexivity.
        + destruct (certify_eval _ _ ShallowCertified) as [_ [d [Forced Lower]]].
          destruct (certify_eval _ _ Loaded) as [LoadedCertified _].
          (* The shallow subderivation retains the forced Data witness. It therefore
              justifies replacing the strict request by the supplied lifted Object,
              allowing the Object's Eval certificate to be reused. *)
          assert (StrictEquivalent : certifies issuing_author Equivalent
              (Encode (Strict th)) (lift (Data (Ref reference)))).
          { destruct d as [[b|t]|[b|t]]; inversion Lower;
              exact (certify_force_encode true th _ Forced). }
          split; [|exact I].
          eapply certify_eval_eq; [exact LoadedCertified|].
          apply certify_sym; exact StrictEquivalent.
        + assert (Forced := certify_force_data _ _ (certify_identification reference)).
          split; [exact (certify_shallow _ _ Forced)|eexists; split; [exact Forced|reflexivity]].
        (* Before using an application or selection fact, certify how its original
            operands reached the ready Tree. Otherwise the primitive fact would concern
            the wrong computation input. *)
        + assert (CombinationCertified : certifies issuing_author Eval (HTreeObj input) (HTreeObj ready))
            by exact (proj1 (certify_eval _ _ Evaluated)).
          assert (Value : value_handle (HTreeObj ready))
            by (hauto cases: - use: (EC.eval_derivation_sound _ _ Evaluated), eval_to_value_handle).
          (* This certificate covers only the first Think step. The following derivation
              may still contain arbitrarily many deferred computations, so it must supply
              the final Force witness separately. *)
          assert (ApplicationCertified : certifies issuing_author Think
            (Thunk (Application input)) intermediate)
            by (hauto cases: - use: certify_application).
          assert (Forced : exists d, certifies issuing_author Force (Thunk (Application input)) (Data d) /\
            lower (Data d) = result)
            by exact (certify_follow _ _ Follow (Application input) ApplicationCertified).
          destruct Forced as (d & Forced & <-).
          split; eauto using certify_shallow.
        + assert (CombinationCertified : certifies issuing_author Eval (HTreeObj input) (HTreeObj ready))
            by exact (proj1 (certify_eval _ _ Evaluated)).
          assert (SelectionCertified : certifies issuing_author Think
            (Thunk (Selection input)) intermediate)
            by (hauto cases: - use: certify_selection).
          assert (Forced : exists d, certifies issuing_author Force (Thunk (Selection input)) (Data d) /\
            lower (Data d) = result)
            by exact (certify_follow _ _ Follow (Selection input) SelectionCertified).
          destruct Forced as (d & Forced & <-).
          split; eauto using certify_shallow.
      - destruct Derivation as [d|h th result Next Evaluated]; intros outer Thought.
        + exists d; split; [apply certify_force_data; exact Thought|reflexivity].
        + destruct (certify_eval _ _ Evaluated) as [_ [d [NextForced Lower]]].
          (* Following a returned Thunk changes which Thunk the next certificate
              concerns. This equivalence is the bridge back to the original request. *)
          assert (OuterEquivalent : certifies issuing_author Equivalent (Thunk th) (Thunk outer))
            by (hauto cases: - use: certify_next, certify_sym).
          (* Transport the next Thunk's Force certificate across that bridge, retaining
              the original request as the conclusion's source. *)
          assert (OuterForced : certifies issuing_author Force (Thunk outer) (Data d))
            by (hauto cases: - use: certify_force_substitute).
          exists d; auto.
    Qed.

    (** Every successful Fix evaluation admits a finite schedule of primitive certificate
        requests and public collector exchanges. This is completeness for the exposed
        logic, rather than a theorem that every Fix program terminates. *)
    Theorem terminating_evaluation_is_certifiable value result :
      eval value = Some result -> certifies issuing_author Eval value result.
    Proof.
      (* A successful run has a finite evaluation derivation. The certification
          construction follows that derivation and has kept all intermediate Data
          witnesses needed by the available exchanges, so no additional oracle or
          evaluation operation in the collector is required. *)
      intro Evaluated; apply (proj1 (certify_eval _ _ (EC.eval_derivation_complete _ _ Evaluated))).
    Qed.
  End Certification.
End CertificationCompleteness.
