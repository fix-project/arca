From Stdlib Require Import List Arith ClassicalDescription.
From FixProof Require Import Handle.

Module Type COUPON_STORAGE (S : STORAGE).
  Inductive coupon_type := Force | Storage | Apply | Slice | Think | Eval | Eq.
  Parameter is_coupon : S.handle -> bool.
  Parameter create_coupon : coupon_type -> S.handle -> S.handle -> S.handle.
  Parameter get_coupon_lhs get_coupon_rhs : S.handle -> option S.handle.
  Parameter get_coupon_type : S.handle -> option coupon_type.
  Axiom get_coupon_lhs_match : forall r x y, get_coupon_lhs (create_coupon r x y) = Some x.
  Axiom get_coupon_rhs_match : forall r x y, get_coupon_rhs (create_coupon r x y) = Some y.
  Axiom get_coupon_type_match : forall r x y, get_coupon_type (create_coupon r x y) = Some r.
  Axiom get_coupon_type_exists : forall c t, get_coupon_type c = Some t -> exists r x y, c = create_coupon r x y.
End COUPON_STORAGE.

Module CouponApi (S : STORAGE) (C : COUPON_STORAGE S).
  Module H := Handles S.
  Import H C.
  Definition get_tree_size_api h :=
    match h with Data (Object (TreeObj t)) => Some (get_tree_size t) | _ => None end.
  Definition get_tree_data_api h i :=
    match h with Data (Object (TreeObj t)) =>
      nth_error (get_tree_raw t) i
    | _ => None end.
  Definition create_application_thunk_api h :=
    match h with Data (Object (TreeObj t)) => Some (Thunk (Application t)) | _ => None end.
  Definition create_selection_thunk_api h :=
    match h with Data (Object (TreeObj t)) | Data (Ref (TreeRef t)) => Some (Thunk (Selection t)) | _ => None end.
  Definition create_identification_thunk_api h :=
    match h with Data d => Some (Thunk (Identification d)) | _ => None end.
  Definition create_strict_encode_api h := match h with Thunk th => Some (Encode (Strict th)) | _ => None end.
  Definition create_shallow_encode_api h := match h with Thunk th => Some (Encode (Shallow th)) | _ => None end.
  Definition is_equal (h1 h2 : handle) := if excluded_middle_informative (h1 = h2) then true else false.
  Definition coupon_type_eq_dec (t1 t2 : coupon_type) : {t1=t2}+{t1<>t2}.
  Proof. decide equality. Defined.
  Definition is_type t h :=
    match get_coupon_type h with
    | Some t' => if coupon_type_eq_dec t t' then true else false
    | None => false end.
  Definition is_force_coupon := is_type Force.
  Definition is_storage_coupon := is_type Storage.
  Definition is_apply_coupon := is_type Apply.
  Definition is_slice_coupon := is_type Slice.
  Definition is_think_coupon := is_type Think.
  Definition is_eq_coupon := is_type Eq.
  Definition is_eval_coupon := is_type Eval.
  Lemma is_equal_match h1 h2 : is_equal h1 h2 = true <-> h1 = h2.
  Proof. unfold is_equal; destruct (excluded_middle_informative _); intuition discriminate. Qed.
  Lemma is_type_match t h : is_type t h = true <-> get_coupon_type h = Some t.
  Proof.
    unfold is_type; destruct (get_coupon_type h) as [t'|] eqn:T;
      [destruct (coupon_type_eq_dec t t'); subst|]; intuition congruence.
  Qed.
End CouponApi.
