From Wasm Require Import datatypes numerics memory operations typing host properties.
From Stdlib Require Import List ZArith.
From FixProof Require Import Handle CouponApi.
Import ListNotations.

(** WasmCert-Coq represents externrefs by addresses (N), rather than the
    uninterpreted Isabelle host type. Retain the same abstract embedding laws. *)
Module Type EXTERNREFS (S : STORAGE).
  Parameter to_externref : S.handle -> externaddr.
  Parameter to_handle : externaddr -> S.handle.
  Axiom to_handle_to_externref : forall y, to_handle (to_externref y) = y.
End EXTERNREFS.

(** The host operation alphabet is shared by all execution modules. Keeping
    this datatype outside the backend functor makes their Wasm states composable. *)
Inductive fixpoint_function :=
| fixpoint_is_equal | fixpoint_is_storage_coupon | fixpoint_is_force_coupon
| fixpoint_is_eq_coupon | fixpoint_is_eval_coupon | fixpoint_is_apply_coupon
| fixpoint_is_think_coupon | fixpoint_create_application_thunk
| fixpoint_create_strict_encode | fixpoint_create_shallow_encode
| fixpoint_get_coupon_lhs | fixpoint_get_coupon_rhs
| fixpoint_create_eq_coupon | fixpoint_create_eval_coupon
| fixpoint_create_think_coupon | fixpoint_create_force_coupon
| fixpoint_get_tree_size | fixpoint_get_tree_data
| fixpoint_is_blob_obj | fixpoint_is_data | fixpoint_is_object.
Definition fixpoint_function_eq_dec (f g : fixpoint_function) : {f=g}+{f<>g}.
Proof. decide equality. Defined.
#[export] Instance fixpoint_host_functions : host_function_class :=
  {| host_function := fixpoint_function; host_function_eq_dec := fixpoint_function_eq_dec |}.

Module Host (S : STORAGE) (C : COUPON_STORAGE S) (R : EXTERNREFS S).
  Module H := Handles S.
  Module Api := CouponApi S C.
  Import H Api C R.
  Definition type_rr_i32 := Tf [T_ref T_externref; T_ref T_externref] [T_num T_i32].
  Definition type_r_i32 := Tf [T_ref T_externref] [T_num T_i32].
  Definition type_r_r := Tf [T_ref T_externref] [T_ref T_externref].
  Definition type_rr_r := Tf [T_ref T_externref; T_ref T_externref] [T_ref T_externref].
  Definition type_ri32_r := Tf [T_ref T_externref; T_num T_i32] [T_ref T_externref].
  Definition function_signature f :=
    match f with
    | fixpoint_is_equal => type_rr_i32
    | fixpoint_is_storage_coupon | fixpoint_is_force_coupon | fixpoint_is_eq_coupon
    | fixpoint_is_eval_coupon | fixpoint_is_apply_coupon | fixpoint_is_think_coupon
    | fixpoint_get_tree_size | fixpoint_is_blob_obj | fixpoint_is_data | fixpoint_is_object => type_r_i32
    | fixpoint_create_application_thunk | fixpoint_create_strict_encode | fixpoint_create_shallow_encode
    | fixpoint_get_coupon_lhs | fixpoint_get_coupon_rhs => type_r_r
    | fixpoint_create_eq_coupon | fixpoint_create_eval_coupon | fixpoint_create_think_coupon
    | fixpoint_create_force_coupon => type_rr_r
    | fixpoint_get_tree_data => type_ri32_r
    end.
  Definition extern_value h := VAL_ref (VAL_ref_extern (to_externref h)).
  Definition i32_of_nat n := Wasm_int.int_of_Z i32m (Z.of_nat n).
  Definition nat_of_i32 n := Wasm_int.nat_of_uint i32m n.
  Definition wasm_bool (b : bool) := i32_of_nat (if b then 1 else 0).
  Definition bool_result b := Some [VAL_num (VAL_int32 (wasm_bool b))].
  Definition handle_result x := H.omap (fun h => [extern_value h]) x.
  (** The value-level implementation of Host.thy's 21 host equations. Its
      store-preserving, typed WasmCert-Coq host instance appears below. *)
  Definition host_values f vs : option (list value) :=
    match f, vs with
    | fixpoint_is_equal, [VAL_ref (VAL_ref_extern r1); VAL_ref (VAL_ref_extern r2)] =>
      bool_result (is_equal (to_handle r1) (to_handle r2))
    | fixpoint_is_storage_coupon, [VAL_ref (VAL_ref_extern r)] => bool_result (is_storage_coupon (to_handle r))
    | fixpoint_is_force_coupon, [VAL_ref (VAL_ref_extern r)] => bool_result (is_force_coupon (to_handle r))
    | fixpoint_is_eq_coupon, [VAL_ref (VAL_ref_extern r)] => bool_result (is_eq_coupon (to_handle r))
    | fixpoint_is_eval_coupon, [VAL_ref (VAL_ref_extern r)] => bool_result (is_eval_coupon (to_handle r))
    | fixpoint_is_apply_coupon, [VAL_ref (VAL_ref_extern r)] => bool_result (is_apply_coupon (to_handle r))
    | fixpoint_is_think_coupon, [VAL_ref (VAL_ref_extern r)] => bool_result (is_think_coupon (to_handle r))
    | fixpoint_create_application_thunk, [VAL_ref (VAL_ref_extern r)] => handle_result (create_application_thunk_api (to_handle r))
    | fixpoint_create_strict_encode, [VAL_ref (VAL_ref_extern r)] => handle_result (create_strict_encode_api (to_handle r))
    | fixpoint_create_shallow_encode, [VAL_ref (VAL_ref_extern r)] => handle_result (create_shallow_encode_api (to_handle r))
    | fixpoint_get_coupon_lhs, [VAL_ref (VAL_ref_extern r)] => handle_result (get_coupon_lhs (to_handle r))
    | fixpoint_get_coupon_rhs, [VAL_ref (VAL_ref_extern r)] => handle_result (get_coupon_rhs (to_handle r))
    | fixpoint_create_eq_coupon, [VAL_ref (VAL_ref_extern r1); VAL_ref (VAL_ref_extern r2)] =>
      Some [extern_value (create_coupon Eq (to_handle r1) (to_handle r2))]
    | fixpoint_create_eval_coupon, [VAL_ref (VAL_ref_extern r1); VAL_ref (VAL_ref_extern r2)] =>
      Some [extern_value (create_coupon Eval (to_handle r1) (to_handle r2))]
    | fixpoint_create_think_coupon, [VAL_ref (VAL_ref_extern r1); VAL_ref (VAL_ref_extern r2)] =>
      Some [extern_value (create_coupon Think (to_handle r1) (to_handle r2))]
    | fixpoint_create_force_coupon, [VAL_ref (VAL_ref_extern r1); VAL_ref (VAL_ref_extern r2)] =>
      Some [extern_value (create_coupon Force (to_handle r1) (to_handle r2))]
    | fixpoint_get_tree_size, [VAL_ref (VAL_ref_extern r)] =>
      H.omap (fun n => [VAL_num (VAL_int32 (i32_of_nat n))]) (get_tree_size_api (to_handle r))
    | fixpoint_get_tree_data, [VAL_ref (VAL_ref_extern r); VAL_num (VAL_int32 n)] =>
      handle_result (get_tree_data_api (to_handle r) (nat_of_i32 n))
    | fixpoint_is_blob_obj, [VAL_ref (VAL_ref_extern r)] => bool_result (get_type (to_handle r) =? 0)
    | fixpoint_is_object, [VAL_ref (VAL_ref_extern r)] => bool_result
      ((get_type (to_handle r) =? 0) || (get_type (to_handle r) =? 1))%bool
    | fixpoint_is_data, [VAL_ref (VAL_ref_extern r)] => bool_result
      ((get_type (to_handle r) =? 0) || (get_type (to_handle r) =? 1) ||
       (get_type (to_handle r) =? 2) || (get_type (to_handle r) =? 3))%bool
    | _, _ => None
    end.
  Lemma fixpoint_is_equal_impl r1 r2 :
    host_values fixpoint_is_equal [VAL_ref (VAL_ref_extern r1); VAL_ref (VAL_ref_extern r2)] =
    Some [VAL_num (VAL_int32 (wasm_bool (is_equal (to_handle r1) (to_handle r2))))].
  Proof. reflexivity. Qed.

  Section WasmHost.
    Context `{mem : BlockUpdateMemory}.

    Lemma host_values_typing (s : store_record) f vs out :
      host_values f vs = Some out ->
      (match function_signature f with Tf _ rets => values_typing s out rets end) = true.
    Proof.
      intro E; destruct f;
        cbv beta iota zeta delta [host_values bool_result handle_result H.omap Api.H.omap option_map] in E;
        repeat (cbv beta iota zeta delta [host_values bool_result handle_result H.omap Api.H.omap option_map] in E;
          match type of E with
          | context [match ?x with _ => _ end] => destruct x eqn:?
          end);
        try discriminate;
        inversion E; subst; reflexivity.
    Qed.

    Definition host_application_impl (_ : unit) (s : store_record) t f vs (_ : unit) out :=
      (t = function_signature f /\
       out = H.omap (fun values => (s, result_values values)) (host_values f vs)) \/
      (t <> function_signature f /\ out = None).

    (** The interpreter asks for an implementation at every signature,
        including ill-typed calls. Reject those calls explicitly. *)
    Definition host_execute (_ : unit) (s : store_record) t f vs :=
      (tt, if function_type_eq_dec t (function_signature f)
           then H.omap (fun values => (s, result_values values)) (host_values f vs)
           else None).

    Lemma host_execute_correct hs s t f vs hs' out :
      host_execute hs s t f vs = (hs', out) -> host_application_impl hs s t f vs hs' out.
    Proof.
      unfold host_execute, host_application_impl.
      destruct (function_type_eq_dec t (function_signature f)) as [Sig|Sig];
        intro Result; inversion Result; subst; [left|right]; auto.
    Qed.

    #[export] Instance fixpoint_host : host.
    Proof.
      refine {| host_state := unit; host_application := host_application_impl |}.
      - intros hs0 s t f vs hs1 s' r [[_ E]|[_ E]]; [|discriminate].
        destruct (host_values f vs) eqn:V; simpl in E; inversion E; subst.
        apply store_extension_same.
      - intros hs0 s t f vs hs1 s' r [[_ E]|[_ E]] Typed; [|discriminate].
        destruct (host_values f vs) eqn:V; simpl in E; inversion E; subst; exact Typed.
      - intros hs0 args rets s f vs hs1 s' r Typed [[Sig E]|[_ E]]; [|discriminate].
        destruct (host_values f vs) eqn:V; simpl in E; inversion E; subst.
        pose proof (host_values_typing s f vs l V) as Result.
        rewrite <- Sig in Result; exact Result.
    Defined.
  End WasmHost.
End Host.
