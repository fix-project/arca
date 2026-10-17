From Stdlib Require Import List NArith ZArith String.
From Wasm Require Import datatypes operations.
From FixProof Require Import Init.
Import ListNotations.
Open Scope N_scope.
Open Scope list_scope.

(** Function indices are checked against the compiled module, rather than
    inferred from names in the text format. Imports occupy indices 0--20. *)
Definition func_make_eq_tree_coupon_idx := 21%N.
Definition func_make_eval_tree_coupon_idx := 22%N.
Definition func_make_force_result_eq_coupon_idx := 23%N.
Definition func_make_eval_eq_coupon_idx := 24%N.
Definition func_make_think_application_coupon_idx := 25%N.
Definition func_make_think_to_force_coupon_idx := 26%N.
Definition func_make_force_to_encode_strict_coupon_idx := 27%N.
Definition func_make_eval_blobobj_coupon_idx := 28%N.
Definition func_make_eq_application_coupon_idx := 29%N.
Definition func_make_eq_encode_strict_coupon_idx := 30%N.
Definition func_make_sym_coupon_idx := 31%N.
Definition func_make_trans_coupon_idx := 32%N.
Definition func_make_self_coupon_idx := 33%N.
Definition func_make_coupon_idx := 34%N.

Definition self_success_body := [BI_local_get 0; BI_local_get 1; BI_call 12].
Definition self_failure_body := [BI_unreachable].
Definition self_body := [BI_local_get 0; BI_local_get 1; BI_call 0;
  BI_if (BT_valtype (Some (T_ref T_externref))) self_success_body self_failure_body].
Definition self_code := Build_module_func 0 [] self_body.

Lemma coupon_module_self_code : lookup_N (mod_funcs coupon_module) 12 = Some self_code.
Proof. reflexivity. Qed.

Lemma coupon_module_self_signature : lookup_N (mod_types coupon_module) (modfunc_type self_code) =
  Some (Tf [T_ref T_externref; T_ref T_externref] [T_ref T_externref]).
Proof. reflexivity. Qed.

Lemma coupon_module_dispatch_functions :
  map (fun es => match es with [BI_ref_func i] => Some i | _ => None end)
    (modelem_init (hd (Build_module_element T_funcref [] ME_declarative) (mod_elems coupon_module))) =
  map (@Some N) [func_make_eq_tree_coupon_idx; func_make_eq_application_coupon_idx;
    func_make_force_result_eq_coupon_idx; func_make_eq_encode_strict_coupon_idx;
    func_make_think_application_coupon_idx; func_make_think_to_force_coupon_idx;
    func_make_force_to_encode_strict_coupon_idx; func_make_eval_eq_coupon_idx;
    func_make_eval_blobobj_coupon_idx; func_make_eval_tree_coupon_idx;
    func_make_sym_coupon_idx; func_make_trans_coupon_idx; func_make_self_coupon_idx].
Proof. reflexivity. Qed.

Lemma coupon_module_exports : mod_exports coupon_module =
  map (fun p => Build_module_export (list_byte_of_string (fst p)) (MED_func (snd p)))
    [("make_eq_tree_coupon"%string, func_make_eq_tree_coupon_idx);
     ("make_eval_tree_coupon"%string, func_make_eval_tree_coupon_idx);
     ("make_force_result_eq_coupon"%string, func_make_force_result_eq_coupon_idx);
     ("make_eval_eq_coupon"%string, func_make_eval_eq_coupon_idx);
     ("make_think_application_coupon"%string, func_make_think_application_coupon_idx);
     ("make_think_to_force_coupon"%string, func_make_think_to_force_coupon_idx);
     ("make_force_to_encode_strict_coupon"%string, func_make_force_to_encode_strict_coupon_idx);
     ("make_eval_blobobj_coupon"%string, func_make_eval_blobobj_coupon_idx);
     ("make_eq_application_coupon"%string, func_make_eq_application_coupon_idx);
     ("make_eq_encode_strict_coupon"%string, func_make_eq_encode_strict_coupon_idx);
     ("make_sym_coupon"%string, func_make_sym_coupon_idx);
     ("make_trans_coupon"%string, func_make_trans_coupon_idx);
     ("make_self_coupon"%string, func_make_self_coupon_idx);
     ("make_coupon"%string, func_make_coupon_idx)] ++
  [Build_module_export (list_byte_of_string "coupons"%string) (MED_table 0)].
Proof. reflexivity. Qed.

Definition eval_blob_create_body := [BI_local_get 0; BI_local_get 1; BI_call 13].
Definition eval_blob_equal_body := [BI_local_get 0; BI_local_get 1; BI_call 0;
  BI_if (BT_valtype (Some (T_ref T_externref))) eval_blob_create_body [BI_unreachable]].
Definition eval_blob_body := [BI_local_get 0; BI_call 18;
  BI_if (BT_valtype (Some (T_ref T_externref))) eval_blob_equal_body [BI_unreachable]].
Definition eval_blob_code := Build_module_func 0 [] eval_blob_body.

Lemma coupon_module_eval_blob_code : lookup_N (mod_funcs coupon_module) 7 = Some eval_blob_code.
Proof. reflexivity. Qed.

Definition sym_lhs_body := [BI_local_get 2; BI_call 10; BI_local_get 1; BI_call 0;
  BI_if (BT_valtype (Some (T_ref T_externref))) self_success_body self_failure_body].
Definition sym_rhs_body := [BI_local_get 2; BI_call 11; BI_local_get 0; BI_call 0;
  BI_if (BT_valtype (Some (T_ref T_externref))) sym_lhs_body self_failure_body].
Definition sym_test_body := [BI_local_get 2; BI_call 3;
  BI_if (BT_valtype (Some (T_ref T_externref))) sym_rhs_body self_failure_body].
Definition sym_body := [BI_const_num (VAL_int32 (Wasm_int.int_zero i32m));
  BI_table_get 0; BI_local_set 2] ++ sym_test_body.
Definition sym_code := Build_module_func 0 [T_ref T_externref] sym_body.
Lemma coupon_module_sym_code : lookup_N (mod_funcs coupon_module) 10 = Some sym_code.
Proof. reflexivity. Qed.

Fixpoint guard_chain (guards : list (list basic_instruction)) (result : list basic_instruction) :=
  match guards with
  | [] => result
  | guard :: rest => guard ++
    [BI_if (BT_valtype (Some (T_ref T_externref))) (guard_chain rest result) self_failure_body]
  end.

Definition trans_guards :=
  [[BI_local_get 2; BI_call 3]; [BI_local_get 3; BI_call 3];
   [BI_local_get 2; BI_call 11; BI_local_get 3; BI_call 10; BI_call 0];
   [BI_local_get 0; BI_local_get 2; BI_call 10; BI_call 0];
   [BI_local_get 1; BI_local_get 3; BI_call 11; BI_call 0]].
Definition trans_test_body := guard_chain trans_guards self_success_body.
Definition trans_second_prefix := [BI_const_num (VAL_int32 (Wasm_int.int_of_Z i32m 1%Z));
  BI_table_get 0; BI_local_set 3].
Definition trans_body := [BI_const_num (VAL_int32 (Wasm_int.int_zero i32m));
  BI_table_get 0; BI_local_set 2] ++ trans_second_prefix ++ trans_test_body.
Definition trans_code := Build_module_func 0 [T_ref T_externref; T_ref T_externref] trans_body.
Lemma coupon_module_trans_code : lookup_N (mod_funcs coupon_module) 11 = Some trans_code.
Proof. reflexivity. Qed.

Definition one_coupon_body tests := [BI_const_num (VAL_int32 (Wasm_int.int_zero i32m));
  BI_table_get 0; BI_local_set 2] ++ tests.
Definition mapped_comparison (reverse : bool) getter mapper target :=
  (if reverse then [BI_local_get 2; BI_call getter; BI_call mapper; BI_local_get target]
   else [BI_local_get target; BI_local_get 2; BI_call getter; BI_call mapper]) ++ [BI_call 0].
Definition mapped_guards mapper reverse := [[BI_local_get 2; BI_call 3];
  mapped_comparison reverse 10 mapper 0; mapped_comparison reverse 11 mapper 1].
Definition mapped_body mapper reverse :=
  one_coupon_body (guard_chain (mapped_guards mapper reverse) self_success_body).
Definition eq_application_code := Build_module_func 0 [T_ref T_externref] (mapped_body 7 false).
Definition eq_encode_strict_code := Build_module_func 0 [T_ref T_externref] (mapped_body 8 true).
Lemma coupon_module_eq_application_code : lookup_N (mod_funcs coupon_module) 8 = Some eq_application_code.
Proof. reflexivity. Qed.
Lemma coupon_module_eq_encode_strict_code : lookup_N (mod_funcs coupon_module) 9 = Some eq_encode_strict_code.
Proof. reflexivity. Qed.

Definition think_to_force_guards := [[BI_local_get 2; BI_call 6];
  [BI_local_get 2; BI_call 11; BI_call 19];
  [BI_local_get 2; BI_call 10; BI_local_get 0; BI_call 0];
  [BI_local_get 2; BI_call 11; BI_local_get 1; BI_call 0]].
Definition force_create_body := [BI_local_get 0; BI_local_get 1; BI_call 15].
Definition think_to_force_tests := guard_chain think_to_force_guards force_create_body.
Definition think_to_force_body := one_coupon_body think_to_force_tests.
Definition think_to_force_code := Build_module_func 0 [T_ref T_externref] think_to_force_body.
Lemma coupon_module_think_to_force_code : lookup_N (mod_funcs coupon_module) 5 = Some think_to_force_code.
Proof. reflexivity. Qed.

Definition force_to_encode_prefix_guards := [[BI_local_get 2; BI_call 2];
  [BI_local_get 2; BI_call 11; BI_call 20];
  [BI_local_get 2; BI_call 11; BI_local_get 1; BI_call 0]].
Definition force_to_encode_last := mapped_comparison true 10 8 0 ++
  [BI_if (BT_valtype (Some (T_ref T_externref))) self_success_body self_failure_body].
Definition force_to_encode_tests := guard_chain force_to_encode_prefix_guards force_to_encode_last.
Definition force_to_encode_body := one_coupon_body force_to_encode_tests.
Definition force_to_encode_code := Build_module_func 0 [T_ref T_externref] force_to_encode_body.
Lemma coupon_module_force_to_encode_code : lookup_N (mod_funcs coupon_module) 6 = Some force_to_encode_code.
Proof. reflexivity. Qed.

Definition two_coupon_body tests := one_coupon_body (trans_second_prefix ++ tests).
Definition eval_eq_guards := [[BI_local_get 2; BI_call 4]; [BI_local_get 3; BI_call 3];
  [BI_local_get 2; BI_call 10; BI_local_get 3; BI_call 10; BI_call 0];
  [BI_local_get 3; BI_call 11; BI_local_get 0; BI_call 0];
  [BI_local_get 2; BI_call 11; BI_local_get 1; BI_call 0]].
Definition eval_create_body := [BI_local_get 0; BI_local_get 1; BI_call 13].
Definition eval_eq_tests := guard_chain eval_eq_guards eval_create_body.
Definition eval_eq_body := two_coupon_body eval_eq_tests.
Definition eval_eq_code := Build_module_func 0 [T_ref T_externref; T_ref T_externref] eval_eq_body.
Lemma coupon_module_eval_eq_code : lookup_N (mod_funcs coupon_module) 3 = Some eval_eq_code.
Proof. reflexivity. Qed.

Definition think_application_prefix_guards := [[BI_local_get 2; BI_call 4]; [BI_local_get 3; BI_call 5];
  [BI_local_get 2; BI_call 11; BI_local_get 3; BI_call 10; BI_call 0]].
Definition think_application_last_guards := [mapped_comparison true 10 7 0;
  [BI_local_get 3; BI_call 11; BI_local_get 1; BI_call 0]].
Definition think_create_body := [BI_local_get 0; BI_local_get 1; BI_call 14].
Definition think_application_last := guard_chain think_application_last_guards think_create_body.
Definition think_application_tests := guard_chain think_application_prefix_guards think_application_last.
Definition think_application_body := two_coupon_body think_application_tests.
Definition think_application_code := Build_module_func 0 [T_ref T_externref; T_ref T_externref] think_application_body.
Lemma coupon_module_think_application_code : lookup_N (mod_funcs coupon_module) 4 = Some think_application_code.
Proof. reflexivity. Qed.

Definition third_coupon_prefix := [BI_const_num (VAL_int32 (Wasm_int.int_of_Z i32m 2%Z));
  BI_table_get 0; BI_local_set 4].
Definition three_coupon_body tests := two_coupon_body (third_coupon_prefix ++ tests).
Definition force_result_eq_tag_guards := [[BI_local_get 2; BI_call 2]; [BI_local_get 3; BI_call 2]; [BI_local_get 4; BI_call 3]].
Definition force_result_eq_endpoint_guards := [
  [BI_local_get 2; BI_call 11; BI_local_get 4; BI_call 10; BI_call 0];
  [BI_local_get 3; BI_call 11; BI_local_get 4; BI_call 11; BI_call 0];
  [BI_local_get 2; BI_call 10; BI_local_get 0; BI_call 0];
  [BI_local_get 3; BI_call 10; BI_local_get 1; BI_call 0]].
Definition force_result_eq_guards := force_result_eq_tag_guards ++ force_result_eq_endpoint_guards.
Definition force_result_eq_tests := guard_chain force_result_eq_tag_guards
  (guard_chain force_result_eq_endpoint_guards self_success_body).
Definition force_result_eq_body := three_coupon_body force_result_eq_tests.
Definition force_result_eq_code := Build_module_func 0 [T_ref T_externref; T_ref T_externref; T_ref T_externref] force_result_eq_body.
Lemma coupon_module_force_result_eq_code : lookup_N (mod_funcs coupon_module) 2 = Some force_result_eq_code.
Proof. reflexivity. Qed.
