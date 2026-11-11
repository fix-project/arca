  Theorem strict_evaluation_proof th result :
    eval (Encode (Strict th)) = Some result <->
    exists reference,
      eval (Encode (Shallow th)) = Some (Data (Ref reference)) /\
      eval (lift (Data (Ref reference))) = Some result.
  Proof.
    (* The shallow result names the Object reached by forcing; it does not evaluate the
        contents of that Object. Using that same Ref as the intermediate witness isolates
        the extra work requested by strictness, without adding a second execution of the
        Thunk. *)
    rewrite eval_encode, shallow_evaluation, !execute_unfold; cbn [encode_to_thunk].
    destruct (force th) as [h|] eqn:Forced; cbn [omap obind].
    - destruct (force_data _ _ Forced) as [d ->].
      destruct (lower_reference d) as [reference [Lower Lift]].
      rewrite Lower; split.
      + intro Evaluated; exists reference; split; [reflexivity|rewrite Lift; exact Evaluated].
      + intros [other [Same Evaluated]]; inversion Same; subst other.
        rewrite Lift in Evaluated; exact Evaluated.
    - split; [discriminate|intros [reference [Impossible _]]; discriminate].
  Qed.
