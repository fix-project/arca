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
