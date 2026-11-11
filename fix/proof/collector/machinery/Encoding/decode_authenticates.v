  Theorem decode_authenticates_proof supplied kind author lhs rhs :
    decode supplied = Some (kind, author, lhs, rhs) ->
    exists bundle tag claim code n,
      supplied = HTreeObj bundle /\
      get_tree_raw bundle = [HTreeObj tag; HTreeObj claim] /\
      get_tree_raw tag = [author; HTreeRef claim] /\
      tree_tag tag = true /\
      get_tree_raw claim = [HBlobObj code; lhs; rhs] /\
      to_nat (get_blob_data code) = Some n /\
      Code.statement_kind n = Some kind.
  Proof.
    (* The witness supplies both a Tag and the assertion Object, so the collector need
        not load a Ref. The decisive check equates the Tag's stored Ref with the Ref of
        the supplied Object; only then may its code and endpoints be interpreted as the
        Tag's assertion. *)
    destruct supplied as [[[b|bundle]|ref]|th|e]; cbn [decode]; try discriminate.
    destruct (get_tree_raw bundle) as [|[[[b|tag]|ref]|th|e] [|[[[b'|claim]|ref']|th'|e'] rest]] eqn:Bundle;
      try discriminate; destruct rest; try discriminate.
    destruct (tree_tag tag) eqn:Tagged; [|discriminate].
    destruct (get_tree_raw tag) as [|a [|reference rest]] eqn:Fields;
      try discriminate; destruct rest; try discriminate.
    destruct (API.handle_eqb reference (HTreeRef claim)) eqn:Same; [|discriminate].
    apply API.handle_eqb_spec in Same; subst reference.
    destruct (get_tree_raw claim) as
      [|[[[code|t]|ref]|th|e] [|l [|r fields]]] eqn:Claim;
      try discriminate; destruct fields; try discriminate.
    unfold obind, omap.
    destruct (to_nat (get_blob_data code)) as [n|] eqn:Number; [|discriminate].
    destruct (Code.statement_kind n) as [k|] eqn:Kind; [|discriminate].
    intro Decoded; inversion Decoded; subst k a l r.
    exists bundle, tag, claim, code, n; repeat split; assumption || reflexivity.
  Qed.
