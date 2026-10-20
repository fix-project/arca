(** Defines Ref-backed coupon witnesses and their encoding. Proves decoding
    recovers encoded claims and authenticates the supplied assertion Object. *)

From Stdlib Require Import List ZArith Lia.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle.
From FixProof.Collector Require Import Matching.
Import ListNotations.

Module CouponEncoding (S : STORAGE).
  Module H := Handles S.
  Import H.

  Definition statement_code kind := match kind with
    | Equivalent => Some 0 | Eval => Some 1 | Apply => Some 2
    | Force => Some 3 | Think => Some 4 | Select => Some 6
    | Indistinguishable => None
    end.

  Definition statement_kind code := match code with
    | 0 => Some Equivalent | 1 => Some Eval | 2 => Some Apply
    | 3 => Some Force | 4 => Some Think
    | 6 => Some Select
    | _ => None
    end.

  (** A supported statement kind survives encoding to its numeric code and decoding back. *)
  Lemma statement_code_roundtrip kind code :
    statement_code kind = Some code -> statement_kind code = Some kind.
  Proof. destruct kind; cbn; intro Code; inversion Code; reflexivity. Qed.

  (** A recognized statement code survives decoding and re-encoding unchanged. *)
  Lemma statement_kind_roundtrip code kind :
    statement_kind code = Some kind -> statement_code kind = Some code.
  Proof.
    do 7 (destruct code as [|code]; [cbn; intro Kind; inversion Kind; reflexivity|]); discriminate.
  Qed.

  (** Every supported statement code is less than seven. *)
  Lemma statement_code_bound kind code :
    statement_code kind = Some code -> code < 7.
  Proof. destruct kind; cbn; intro Code; inversion Code; auto with arith. Qed.

End CouponEncoding.

Module CouponClaims (S : STORAGE).
  Module Code := CouponEncoding S.
  Module API := CouponMatching S.
  Module H := Handles S.
  Import H.

  Definition witness authors code lhs rhs :=
    let claim := create_tree [HBlobObj (create_blob (from_nat code)); lhs; rhs] in
    let tag := create_tree_tagged true [HTreeObj authors; HTreeRef claim] in
    HTreeObj (create_tree [HTreeObj tag; HTreeObj claim]).

  Definition encode kind authors lhs rhs :=
    omap (fun code => witness authors code lhs rhs) (Code.statement_code kind).

  (** Only Objects supplied in the witness are inspected, never a Ref's backing data. *)
  Definition decode supplied := match supplied with
    | Data (Object (TreeObj bundle)) => match get_tree_raw bundle with
      | [Data (Object (TreeObj tag)); Data (Object (TreeObj claim))] =>
        if tree_tag tag then match get_tree_raw tag with
          | [Data (Object (TreeObj authors)); reference] =>
            if API.handle_eqb reference (HTreeRef claim) then
              match get_tree_raw claim with
              | [Data (Object (BlobObj code)); lhs; rhs] =>
                obind (to_nat (get_blob_data code)) (fun n =>
                  omap (fun kind => (kind, authors, lhs, rhs)) (Code.statement_kind n))
              | _ => None end
            else None
          | _ => None end
        else None
      | _ => None end
    | _ => None end.

  #[export] Hint Rewrite get_tree_raw_create_tree get_tree_raw_create_tree_tagged
    tree_tag_create_tree_tagged get_blob_data_create_blob API.handle_eqb_refl : coupon_claims.

  (** Encoding and then decoding a certificate witness recovers its kind, authors, and
      endpoints. *)
  Theorem encode_decode kind authors lhs rhs encoded :
    encode kind authors lhs rhs = Some encoded ->
    decode encoded = Some (kind, authors, lhs, rhs).
  Proof.
    unfold encode; destruct (Code.statement_code kind) as [code|] eqn:Code;
      cbn [omap]; intro Encoded; [inversion Encoded; subst encoded|discriminate].
    unfold witness, decode; repeat progress
      (cbn -[API.handle_eqb to_nat from_nat Code.statement_kind]; autorewrite with coupon_claims).
    rewrite to_nat_from_nat.
    - cbn [obind]; rewrite (Code.statement_code_roundtrip _ _ Code); reflexivity.
    - apply (Z.lt_trans _ 7);
        [pose proof (Code.statement_code_bound _ _ Code); lia|vm_compute; reflexivity].
  Qed.

  (** A decoded claim is tied to the assertion Object named by a real Tag. Supplying a
      correctly shaped but different assertion cannot change what the certificate says. *)
  Theorem decode_authenticates supplied kind authors lhs rhs :
    decode supplied = Some (kind, authors, lhs, rhs) ->
    exists bundle tag claim code n,
      supplied = HTreeObj bundle /\
      get_tree_raw bundle = [HTreeObj tag; HTreeObj claim] /\
      get_tree_raw tag = [HTreeObj authors; HTreeRef claim] /\
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
    destruct (get_tree_raw tag) as [|[[[b|a]|ref]|th|e] [|reference rest]] eqn:Fields;
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
  (** Successful decoding determines the Tag as the authors Tree followed by the assertion
      Ref. *)
  Lemma decode_tag_shape tag claim kind authors lhs rhs :
    decode (HTreeObj (create_tree [HTreeObj tag; HTreeObj claim])) =
      Some (kind, authors, lhs, rhs) ->
    tag = create_tree_tagged true [HTreeObj authors; HTreeRef claim].
  Proof.
    intro Decoded.
    hauto cases: - db: coupon_claims
      use: (decode_authenticates _ _ _ _ _ Decoded), (create_tree_get_tree_raw tag)
      unfold!: HTreeObj.
  Qed.

  (** Replacing a decoded Tag's authors preserves its authenticated claim and endpoints. *)
  Lemma decode_replace_authors tag claim kind authors new_authors lhs rhs :
    decode (HTreeObj (create_tree [HTreeObj tag; HTreeObj claim])) =
      Some (kind, authors, lhs, rhs) ->
    decode (HTreeObj (create_tree
      [HTreeObj (create_tree_tagged true [HTreeObj new_authors; HTreeRef claim]); HTreeObj claim])) =
      Some (kind, new_authors, lhs, rhs).
  Proof.
    (* Claim authentication depends on the Ref of the assertion Object, not on the author
        list's representation. Changing that list can alter trust requirements while
        leaving the asserted operation and endpoints fixed, which is the separation used
        by authority preservation. *)
    intro Decoded.
    pose proof (decode_authenticates _ _ _ _ _ Decoded) as Authenticated.
    hauto cases: - db: coupon_claims unfold!: decode, HTreeObj, obind, omap.
  Qed.

End CouponClaims.
