(** Defines coupon inference rules and proves their semantic soundness,
    including that equivalent Blob Objects have identical contents. *)

From Stdlib Require Import List Arith.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Equivalence Require Import Closure Indistinguishability.

Module Coupon (S : STORAGE) (P : PROGRAM S).
  Module O := Indistinguishability S P.
  Module EC := EquivalenceClosure S P.
  Import EC EC.E EC.E.EP EC.E.EP.E EC.E.EP.E.H.

  Definition coupon_indistinguishable := O.indistinguishable.

  Reserved Notation "h =[ 'Force' ]=> r" (at level 70, no associativity).
  Reserved Notation "h =[ 'Apply' ]=> r" (at level 70, no associativity).
  Reserved Notation "h =[ 'Select' ]=> r" (at level 70, no associativity).
  Reserved Notation "h =[ 'Think' ]=> r" (at level 70, no associativity).
  Reserved Notation "h =[ 'Eval' ]=> r" (at level 70, no associativity).
  Reserved Notation "h =[ 'Equivalent' ]=> r" (at level 70, no associativity).

  Inductive coupon_force : handle -> handle -> Prop :=
  | CouponForce : forall th r,
      force th = Some r ->
      Thunk th =[ Force ]=> r
  | CouponThinkToForce : forall th d,
      Thunk th =[ Think ]=> Data d ->
      Thunk th =[ Force ]=> Data d
  | CouponForceEq : forall h1 h2 h3,
      h1 =[ Equivalent ]=> h2 ->
      h1 =[ Force ]=> h3 ->
      h2 =[ Force ]=> h3
  with coupon_apply : handle -> handle -> Prop :=
  | CouponApply : forall t result,
      value_handle (HTreeObj t) ->
      A.apply_tree t = Some result ->
      HTreeObj t =[ Apply ]=> result
  with coupon_select : handle -> handle -> Prop :=
  | CouponSelect : forall t r,
      Sl.select t = Some r ->
      HTreeObj t =[ Select ]=> r
  with coupon_think : handle -> handle -> Prop :=
  | CouponThinkApplication : forall t t' h,
      HTreeObj t =[ Eval ]=> HTreeObj t' ->
      HTreeObj t' =[ Apply ]=> h ->
      Thunk (Application t) =[ Think ]=> h
  | CouponThinkSelection : forall t t' h,
      HTreeObj t =[ Eval ]=> HTreeObj t' ->
      HTreeObj t' =[ Select ]=> h ->
      Thunk (Selection t) =[ Think ]=> h
  | CouponThinkIdentification : forall d,
      Thunk (Identification d) =[ Think ]=> Data (Ref d)
  with coupon_eval : handle -> handle -> Prop :=
  | CouponEvalBlobObj : forall b,
      HBlobObj b =[ Eval ]=> HBlobObj b
  | CouponEvalTreeObj : forall t1 t2,
      tree_tag t1 = tree_tag t2 ->
      Forall2 coupon_eval (get_tree_raw t1) (get_tree_raw t2) ->
      HTreeObj t1 =[ Eval ]=> HTreeObj t2
  | CouponEvalRef : forall r,
      Data (Ref r) =[ Eval ]=> Data (Ref r)
  | CouponEvalThunk : forall th,
      Thunk th =[ Eval ]=> Thunk th
  | CouponEvalEq : forall h1 h2 h3,
      h1 =[ Eval ]=> h2 ->
      h1 =[ Equivalent ]=> h3 ->
      h3 =[ Eval ]=> h2
  with coupon_equiv : handle -> handle -> Prop :=
  | CouponThunkEq : forall t1 t2,
      Thunk t1 =[ Think ]=> Thunk t2 ->
      Thunk t1 =[ Equivalent ]=> Thunk t2
  | CouponThunkEncodeEq : forall t1 t2,
      Thunk t1 =[ Think ]=> Encode (Strict t2) ->
      Thunk t1 =[ Equivalent ]=> Thunk t2
  | CouponThunkEncodeShallowEq : forall t1 t2,
      Thunk t1 =[ Think ]=> Encode (Shallow t2) ->
      Thunk t1 =[ Equivalent ]=> Thunk t2
  | CouponForceResultEq : forall h1 h1' h2 h2',
      h1 =[ Force ]=> h1' ->
      h2 =[ Force ]=> h2' ->
      h1' =[ Equivalent ]=> h2' ->
      h1 =[ Equivalent ]=> h2
  | CouponForceToEncodeStrict : forall th h,
      Thunk th =[ Force ]=> h ->
      Encode (Strict th) =[ Equivalent ]=> lift h
  | CouponForceToEncodeShallow : forall th h,
      Thunk th =[ Force ]=> h ->
      Encode (Shallow th) =[ Equivalent ]=> lower h
  | CouponEqApplication : forall t1 t2,
      HTreeObj t1 =[ Equivalent ]=> HTreeObj t2 ->
      Thunk (Application t1) =[ Equivalent ]=> Thunk (Application t2)
  | CouponEqSelection : forall t1 t2,
      HTreeObj t1 =[ Equivalent ]=> HTreeObj t2 ->
      Thunk (Selection t1) =[ Equivalent ]=> Thunk (Selection t2)
  | CouponEqIdentification : forall d1 d2,
      Data (Ref d1) =[ Equivalent ]=> Data (Ref d2) ->
      Thunk (Identification d1) =[ Equivalent ]=> Thunk (Identification d2)
  | CouponEqEncodeStrict : forall t1 t2,
      Thunk t1 =[ Equivalent ]=> Thunk t2 ->
      Encode (Strict t1) =[ Equivalent ]=> Encode (Strict t2)
  | CouponEqEncodeShallow : forall t1 t2,
      Thunk t1 =[ Equivalent ]=> Thunk t2 ->
      Encode (Shallow t1) =[ Equivalent ]=> Encode (Shallow t2)
  | CouponEvalResultEq : forall h1 h2,
      h1 =[ Eval ]=> h2 ->
      h1 =[ Equivalent ]=> h2
  | CouponTreeRefEq : forall t1 t2,
      HTreeObj t1 =[ Equivalent ]=> HTreeObj t2 ->
      HTreeRef t1 =[ Equivalent ]=> HTreeRef t2
  | CouponBlobRefEq : forall b1 b2,
      HBlobObj b1 =[ Equivalent ]=> HBlobObj b2 ->
      HBlobRef b1 =[ Equivalent ]=> HBlobRef b2
  | CouponTreeEq : forall t1 t2,
      tree_tag t1 = tree_tag t2 ->
      Forall2 coupon_equiv (get_tree_raw t1) (get_tree_raw t2) ->
      HTreeObj t1 =[ Equivalent ]=> HTreeObj t2
  | CouponBlobEq : forall b1 b2,
      get_blob_data b1 = get_blob_data b2 ->
      HBlobObj b1 =[ Equivalent ]=> HBlobObj b2
  | CouponSelf : forall h,
      h =[ Equivalent ]=> h
  | CouponSym : forall h1 h2,
      h1 =[ Equivalent ]=> h2 ->
      h2 =[ Equivalent ]=> h1
  | CouponTrans : forall h1 h2 h3,
      h1 =[ Equivalent ]=> h2 ->
      h2 =[ Equivalent ]=> h3 ->
      h1 =[ Equivalent ]=> h3
  where "h =[ 'Force' ]=> r" := (coupon_force h r)
  and "h =[ 'Apply' ]=> r" := (coupon_apply h r)
  and "h =[ 'Select' ]=> r" := (coupon_select h r)
  and "h =[ 'Think' ]=> r" := (coupon_think h r)
  and "h =[ 'Eval' ]=> r" := (coupon_eval h r)
  and "h =[ 'Equivalent' ]=> r" := (coupon_equiv h r).

Load "fix/proof/collector/internal/Logic/Primitives".
  (** A derived computation claim may use an equivalent representative of a result or
      source. The semantic interpretation must accommodate that substitution, or soundness
      would fail at the equivalence exchanges. Force compares the lifted Objects behind
      its Data results; Eval additionally requires the claimed representative to be a
      Value. Primitive Apply and Select claims retain their exact results. *)
  Definition force_result h r := match h, r with
    | Thunk th, Data d => exists actual,
      force th = Some (Data actual) /\
      computational_equiv (lift (Data d)) (lift (Data actual))
    | _, _ => False end.
  Definition think_result h r := match h with
    | Thunk th => exists actual,
      think actual = Some r /\ computational_equiv (Thunk th) (Thunk actual)
    | _ => False end.
  Definition eval_result h r := exists v, eval h = Some v /\ computational_equiv r v /\ value_handle r.

Load "fix/proof/collector/internal/Logic/Rules".
Load "fix/proof/collector/machinery/Logic/MutualInduction".
  (** Every abstract coupon derivation describes a valid computation claim or a valid
      computational equivalence. This is the logical soundness boundary on which the
      exchange checker relies. *)
Load "fix/proof/collector/machinery/Logic/coupon_sound".
  Theorem coupon_sound :
    (forall h r, coupon_force h r -> force_result h r) /\
    (forall h r, coupon_apply h r -> exists t,
      h = HTreeObj t /\ value_handle h /\ A.apply_tree t = Some r) /\
    (forall h r, coupon_select h r -> exists t, h = HTreeObj t /\ Sl.select t = Some r) /\
    (forall h r, coupon_think h r -> think_result h r) /\
    (forall h r, coupon_eval h r -> eval_result h r) /\
    (forall h1 h2, coupon_equiv h1 h2 -> computational_equiv h1 h2).
  Proof. apply coupon_sound_proof. Qed.
  (** The collector cannot derive an equivalence between two Blob Objects of different
      contents. Soundness turns the coupon derivation into computational equivalence,
      whose Blob-content invariant then excludes such a claim. *)
Load "fix/proof/collector/machinery/Logic/coupon_blob_same_data".
  Corollary coupon_blob_same_data b1 b2 : coupon_equiv (HBlobObj b1) (HBlobObj b2) -> get_blob_data b1 = get_blob_data b2.
  Proof. apply coupon_blob_same_data_proof. Qed.
End Coupon.
