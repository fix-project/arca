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

Load "fix/proof/collector/internal/Encoding/Codes".
End CouponEncoding.

Module CouponClaims (S : STORAGE).
  Module Code := CouponEncoding S.
  Module API := CouponMatching S.
  Module H := Handles S.
  Import H.

  Definition witness author code lhs rhs :=
    let claim := create_tree [HBlobObj (create_blob (from_nat code)); lhs; rhs] in
    let tag := create_tree_tagged true [author; HTreeRef claim] in
    HTreeObj (create_tree [HTreeObj tag; HTreeObj claim]).

  Definition encode kind author lhs rhs :=
    omap (fun code => witness author code lhs rhs) (Code.statement_code kind).

  (** Only Objects supplied in the witness are inspected, never a Ref's backing data. *)
  Definition decode supplied := match supplied with
    | Data (Object (TreeObj bundle)) => match get_tree_raw bundle with
      | [Data (Object (TreeObj tag)); Data (Object (TreeObj claim))] =>
        if tree_tag tag then match get_tree_raw tag with
          | [author; reference] =>
            if API.handle_eqb reference (HTreeRef claim) then
              match get_tree_raw claim with
              | [Data (Object (BlobObj code)); lhs; rhs] =>
                obind (to_nat (get_blob_data code)) (fun n =>
                  omap (fun kind => (kind, author, lhs, rhs)) (Code.statement_kind n))
              | _ => None end
            else None
          | _ => None end
        else None
      | _ => None end
    | _ => None end.

  Definition author supplied := match supplied with
    | Data (Object (TreeObj bundle)) => match get_tree_raw bundle with
      | [Data (Object (TreeObj tag)); Data (Object (TreeObj claim))] =>
        if tree_tag tag then match get_tree_raw tag, get_tree_raw claim with
          | [issuing_author; reference], [_; _; _] =>
            if API.handle_eqb reference (HTreeRef claim) then Some issuing_author else None
          | _, _ => None end
        else None
      | _ => None end
    | _ => None end.

  #[export] Hint Rewrite get_tree_raw_create_tree get_tree_raw_create_tree_tagged
    tree_tag_create_tree_tagged get_blob_data_create_blob API.handle_eqb_refl : coupon_claims.

  (** Encoding and then decoding a certificate witness recovers its kind, author, and
      endpoints. *)
Load "fix/proof/collector/machinery/Encoding/encode_decode".
  Theorem encode_decode kind author lhs rhs encoded :
    encode kind author lhs rhs = Some encoded ->
    decode encoded = Some (kind, author, lhs, rhs).
  Proof. apply encode_decode_proof. Qed.

  (** A decoded claim is tied to the assertion Object named by a real Tag. Supplying a
      correctly shaped but different assertion cannot change what the certificate says. *)
Load "fix/proof/collector/machinery/Encoding/decode_authenticates".
  Theorem decode_authenticates supplied kind author lhs rhs :
    decode supplied = Some (kind, author, lhs, rhs) ->
    exists bundle tag claim code n,
      supplied = HTreeObj bundle /\
      get_tree_raw bundle = [HTreeObj tag; HTreeObj claim] /\
      get_tree_raw tag = [author; HTreeRef claim] /\
      tree_tag tag = true /\
      get_tree_raw claim = [HBlobObj code; lhs; rhs] /\
      to_nat (get_blob_data code) = Some n /\
      Code.statement_kind n = Some kind.
  Proof. apply decode_authenticates_proof. Qed.
Load "fix/proof/collector/internal/Encoding/Authors".
End CouponClaims.
