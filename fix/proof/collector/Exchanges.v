(** Defines the executable abstract exchange checker and coupon issuer.
    Proves that accepted exchanges preserve claims and propagate their
    conditional trust assumptions. *)

From Stdlib Require Import List Arith Bool Lia.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Collector Require Import Matching Logic Encoding.

Module CouponExchanges (S : STORAGE) (P : PROGRAM S).
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

Load "fix/proof/collector/internal/Exchanges/Checking/read_sound".
Load "fix/proof/collector/machinery/Exchanges/Equality".
Load "fix/proof/collector/internal/Exchanges/Checking/substitute_force".
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

  Definition select_exchange code lhs :=
    match code with
    | 0 => match lhs with
      | Data (Object (TreeObj _)) => Some 0
      | Data (Ref (TreeRef _)) => Some 21
      | Data (Ref (BlobRef _)) => Some 22
      | Thunk (H.Application _) => Some 4
      | Thunk (H.Selection _) => Some 17
      | Thunk (H.Identification _) => Some 18
      | Encode (H.Strict _) => Some 9
      | Encode (H.Shallow _) => Some 19
      | _ => None end
    | 2 => match lhs with
      | Thunk (H.Application _) => Some 2
      | Thunk (H.Selection _) => Some 25
      | Thunk (H.Identification _) => Some 26
      | _ => None end
    | 5 => match lhs with Data (Object (TreeObj _)) => Some 6 | _ => Some 5 end
    | 7 => match lhs with Encode (H.Strict _) => Some 7 | Encode (H.Shallow _) => Some 16 | _ => None end
    | 1 | 3 | 8 | 10 | 11 | 12 | 13 | 14 | 15 | 20 | 24 => Some code
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
Load "fix/proof/collector/machinery/Exchanges/check_sound".
  Theorem check_sound rule coupons lhs rhs kind :
    Forall valid coupons -> check rule coupons lhs rhs = Some kind -> relation_of kind lhs rhs.
  Proof. apply check_sound_proof. Qed.

  Definition kernel_author := HTreeObj (create_tree []).

  Definition authenticated_by collector witness := match Wire.author witness with
    | Some author => API.handle_eqb author collector || API.handle_eqb author kernel_author
    | None => false end.

  Definition reconstruction_authority collector coupons :=
    forallb (authenticated_by collector) coupons.

  Definition issue issuing_author kind lhs rhs coupons output :=
    reconstruction_authority issuing_author coupons = true /\
    Wire.encode kind issuing_author lhs rhs = Some output.

  Definition stable_requirement trusted author :=
    value_handle author -> author = kernel_author \/ trusted author.

  Definition stable_conditional trusted witness := forall kind author lhs rhs,
    Wire.decode witness = Some (kind, author, lhs, rhs) ->
    stable_requirement trusted author -> relation_of kind lhs rhs.

  Definition reconstruct_tag collector coupons original fields :=
    match get_tree_raw fields with
    | _ :: _ =>
      if tree_tag original && reconstruction_authority collector coupons &&
        children Eval (get_tree_raw original) (get_tree_raw fields) coupons
      then Some (create_tree_tagged true (get_tree_raw fields)) else None
    | _ => None end.

Load "fix/proof/collector/internal/Exchanges/Authority".
Load "fix/proof/collector/machinery/Exchanges/reconstruction_validated_sound".
  Theorem reconstruction_validated_sound trusted collector coupons lhs rhs :
    trusted collector -> Forall (stable_conditional trusted) coupons ->
    reconstruction_authority collector coupons = true ->
    tree Eval coupons lhs rhs = Some Eval -> D.coupon_eval lhs rhs.
  Proof. apply reconstruction_validated_sound_proof. Qed.

Load "fix/proof/collector/machinery/Exchanges/reconstruct_tag_sound".
  Theorem reconstruct_tag_sound trusted collector coupons original fields result :
    trusted collector -> Forall (stable_conditional trusted) coupons ->
    reconstruct_tag collector coupons original fields = Some result ->
    D.coupon_eval (HTreeObj original) (HTreeObj result).
  Proof. apply reconstruct_tag_sound_proof. Qed.

Load "fix/proof/collector/machinery/Exchanges/issue_conditional".
  Theorem issue_conditional trusted issuing_author rule coupons lhs rhs kind output :
    Forall (stable_conditional trusted) coupons -> check rule coupons lhs rhs = Some kind ->
    issue issuing_author kind lhs rhs coupons output -> stable_conditional trusted output.
  Proof. apply issue_conditional_proof. Qed.

End CouponExchanges.
