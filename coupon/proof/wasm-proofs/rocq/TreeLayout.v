From Stdlib Require Import List NArith ZArith.
From Wasm Require Import datatypes operations.
From FixProof Require Import Init ModuleLayout.
Import ListNotations.
Open Scope N_scope.
Open Scope list_scope.

Definition tree_loop_check := [BI_local_get 4; BI_local_get 3; BI_relop T_i32 (Relop_i (ROI_ge SX_S)); BI_br_if 1].
Definition tree_increment := [BI_local_get 4; BI_const_num (VAL_int32 (Wasm_int.int_of_Z i32m 1%Z));
  BI_binop T_i32 (Binop_i BOI_add); BI_local_set 4; BI_br 0].
Definition tree_loop_code body := tree_loop_check ++ body ++ tree_increment.
Definition tree_loop body := [BI_block (BT_valtype None) [BI_loop (BT_valtype None) (tree_loop_code body)]].
Definition tree_tag_body tag := [BI_local_get 4; BI_table_get 0; BI_call tag;
  BI_if (BT_valtype None) [BI_nop] self_failure_body].
Definition tree_rhs_entry := [BI_local_get 1; BI_local_get 4; BI_call 17; BI_local_get 2; BI_call 11; BI_call 0;
  BI_if (BT_valtype None) [BI_nop] self_failure_body].
Definition tree_entry_body := [BI_local_get 4; BI_table_get 0; BI_local_set 2;
  BI_local_get 0; BI_local_get 4; BI_call 17; BI_local_get 2; BI_call 10; BI_call 0;
  BI_if (BT_valtype None) tree_rhs_entry self_failure_body].
Definition tree_reset := [BI_const_num (VAL_int32 (Wasm_int.int_zero i32m)); BI_local_set 4].
Definition tree_create create := [BI_local_get 0; BI_local_get 1; BI_call create].
Definition tree_size_right create := [BI_local_get 1; BI_call 16; BI_local_get 3; BI_relop T_i32 (Relop_i ROI_eq);
  BI_if (BT_valtype (Some (T_ref T_externref)))
    (tree_reset ++ tree_loop tree_entry_body ++ tree_create create) self_failure_body].
Definition tree_size_tests create := [BI_local_get 0; BI_call 16; BI_local_get 3; BI_relop T_i32 (Relop_i ROI_eq);
  BI_if (BT_valtype (Some (T_ref T_externref))) (tree_size_right create) self_failure_body].
Definition tree_body tag create := [BI_table_size 0; BI_local_set 3] ++ tree_reset ++
  tree_loop (tree_tag_body tag) ++ tree_size_tests create.
Definition eq_tree_code := Build_module_func 0 [T_ref T_externref; T_num T_i32; T_num T_i32] (tree_body 3 12).
Definition eval_tree_code := Build_module_func 0 [T_ref T_externref; T_num T_i32; T_num T_i32] (tree_body 4 13).
Lemma coupon_module_eq_tree_code : lookup_N (mod_funcs coupon_module) 0 = Some eq_tree_code.
Proof. reflexivity. Qed.
Lemma coupon_module_eval_tree_code : lookup_N (mod_funcs coupon_module) 1 = Some eval_tree_code.
Proof. reflexivity. Qed.
