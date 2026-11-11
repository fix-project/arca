(** Checks the parsed collector module against its import signatures and
    records the instruction bodies used by the execution proofs. *)

From Stdlib Require Import List String NArith ZArith Lia Relation_Operators.
From FixProof.Support Require Import WasmNatural.
From Wasm Require Import datatypes instantiation_func interp_instantiate_sound opsem.
From FixProof.Collector.wasm Require Import Binary.
From FixProof.Semantics Require Import Handle.
From FixProof.Shell Require Import ABI.
Import ListNotations.
Local Open Scope string_scope.
Local Open Scope list_scope.

Definition native_size_bound length :=
  (Z.of_nat length < Z.of_N native_table_limit + 1)%Z.

Lemma native_size_bound_word length : native_size_bound length ->
  (Z.of_nat length < 2 ^ 32)%Z.
Proof. unfold native_size_bound, native_table_limit; cbn; lia. Qed.

Lemma native_size_bound_N length : native_size_bound length <->
  (N.of_nat length <= native_table_limit)%N.
Proof. unfold native_size_bound, native_table_limit; cbn; lia. Qed.

Lemma native_size_comparison length : (Z.of_nat length < 2 ^ 64)%Z ->
  app_relop (Relop_i (ROI_lt SX_U)) (VAL_int64 (wasm_nat64 length))
    (VAL_int64 (Wasm_int.int_of_Z i64m 134217729%Z)) =
  Z.ltb (Z.of_nat length) (Z.of_N native_table_limit + 1)%Z.
Proof.
  intro Wide.
  change (Wasm_int.Int64.ltu (wasm_nat64 length) (Wasm_int.Int64.repr 134217729) =
    Z.ltb (Z.of_nat length) (Z.of_N native_table_limit + 1)%Z).
  unfold Wasm_int.Int64.ltu.
  rewrite (wasm_nat64_unsigned length Wide).
  rewrite Wasm_int.Int64.unsigned_repr by (vm_compute; intuition congruence).
  unfold native_table_limit; cbn.
  destruct (Coqlib.zlt (Z.of_nat length) 134217729);
    destruct (Z.ltb (Z.of_nat length) 134217729) eqn:Compared;
    try reflexivity; first [apply Z.ltb_lt in Compared|apply Z.ltb_ge in Compared]; lia.
Qed.

(** WasmCert's executable type checker accepts the collector module. *)
Lemma evaluation_module_type_checked : exists imports exports,
  module_type_checker evaluation_module = (Some (imports, exports), "ok").
Proof. vm_compute; eexists; eexists; reflexivity. Qed.

(** The collector module satisfies WasmCert's declarative module typing rules. *)
Lemma evaluation_module_well_typed : exists imports exports,
  instantiation_spec.module_typing evaluation_module imports exports.
Proof.
  destruct evaluation_module_type_checked as [imports [exports Checked]].
  exists imports, exports; eapply module_type_checker_sound; exact Checked.
Qed.

(** The supplied Shell host functions match the collector's declared imports. *)
Lemma shell_host_imports_match :
  map (fun entry => (imp_module entry, imp_name entry,
    match imp_desc entry with
    | MID_func index => lookup_N (mod_types evaluation_module) index
    | _ => None end)) (mod_imports evaluation_module) =
  map (fun entry => (String.list_byte_of_string "fix", String.list_byte_of_string (fst entry),
    Some (shell_signature (snd entry)))) shell_function_imports.
Proof. reflexivity. Qed.

Definition evaluation_require :=
  nth 0 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

(** The parsed require helper has the specified guard-and-trap instruction body. *)
Lemma evaluation_require_body : modfunc_body evaluation_require =
  [BI_local_get 0%N; BI_testop T_i32 TO_eqz;
   BI_if (BT_valtype None) [BI_unreachable] []].
Proof. reflexivity. Qed.

Section Execution.
  Context `{hfc : host_function_class} `{mem : BlockUpdateMemory}.
  Context {ho : @host hfc mem}.
  Local Existing Instance ho.

  (** The parsed exchange dispatcher has the expected instruction suffix. *)
  Lemma evaluation_step_suffix hs s f es hs' s' f' es' suffix :
    reduce hs s f es hs' s' f' es' ->
    reduce hs s f (es ++ suffix) hs' s' f' (es' ++ suffix).
  Proof.
    intro Step; eapply r_label with (lh := LH_base [] suffix);
      [exact Step|reflexivity|reflexivity].
  Qed.

  (** The require helper accepts true and traps on false as specified by its instruction
      body. *)
  Lemma evaluation_require_executes hs s f (condition : bool) :
    lookup_N (f_locs f) 0%N = Some (VAL_num (VAL_int32 (wasm_bool condition))) ->
    reduce_trans (hs, s, f, to_e_list (modfunc_body evaluation_require))
      (hs, s, f, if condition then [] else [AI_trap]).
  Proof.
    intro Local; rewrite evaluation_require_body.
    eapply rt_trans with (y := (hs, s, f,
      [v_to_e (VAL_num (VAL_int32 (wasm_bool condition))); AI_basic (BI_testop T_i32 TO_eqz);
       AI_basic (BI_if (BT_valtype None) [BI_unreachable] [])])).
    - apply rt_step; change (reduce hs s f
        ([AI_basic (BI_local_get 0%N)] ++
          [AI_basic (BI_testop T_i32 TO_eqz); AI_basic (BI_if (BT_valtype None) [BI_unreachable] [])])
        hs s f ([v_to_e (VAL_num (VAL_int32 (wasm_bool condition)))] ++
          [AI_basic (BI_testop T_i32 TO_eqz); AI_basic (BI_if (BT_valtype None) [BI_unreachable] [])])).
      apply evaluation_step_suffix, r_local_get; exact Local.
    - eapply rt_trans with (y := (hs, s, f,
        [v_to_e (VAL_num (VAL_int32 (wasm_bool (negb condition))));
         AI_basic (BI_if (BT_valtype None) [BI_unreachable] [])])).
      + apply rt_step; apply evaluation_step_suffix with
          (es := [v_to_e (VAL_num (VAL_int32 (wasm_bool condition))); AI_basic (BI_testop T_i32 TO_eqz)])
          (es' := [v_to_e (VAL_num (VAL_int32 (wasm_bool (negb condition))))])
          (suffix := [AI_basic (BI_if (BT_valtype None) [BI_unreachable] [])]), r_simple;
          destruct condition; apply rs_testop_i32.
      + eapply rt_trans with (y := (hs, s, f,
          [AI_basic (BI_block (BT_valtype None) (if condition then [] else [BI_unreachable]))])).
        * apply rt_step, r_simple; destruct condition.
          -- apply rs_if_false; reflexivity.
          -- apply rs_if_true; vm_compute; discriminate.
        * eapply rt_trans with (y := (hs, s, f,
            [AI_label 0 [] (to_e_list (if condition then [] else [BI_unreachable]))])).
          -- apply rt_step; change (reduce hs s f
              ([] ++ [AI_basic (BI_block (BT_valtype None) (if condition then [] else [BI_unreachable]))])
              hs s f [AI_label 0 [] ([] ++ to_e_list (if condition then [] else [BI_unreachable]))]).
            eapply r_block with (vs := []) (n := 0) (t1s := []) (t2s := []); reflexivity.
          -- destruct condition.
             ++ apply rt_step, r_simple, rs_label_const; reflexivity.
             ++ eapply rt_trans with (y := (hs, s, f, [AI_label 0 [] [AI_trap]])).
                ** apply rt_step; eapply r_label with (lh := LH_rec [] 0 [] (LH_base [] []) []);
                     [apply r_simple, rs_unreachable|reflexivity|reflexivity].
                ** apply rt_step, r_simple, rs_label_trap.
  Qed.
End Execution.


Definition evaluation_claim := nth 8 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

Lemma evaluation_claim_body : modfunc_body evaluation_claim =
  ltac:(let code := eval cbv [modfunc_body evaluation_claim mod_funcs evaluation_module nth] in
    (modfunc_body evaluation_claim) in exact code).
Proof. reflexivity. Qed.

Definition evaluation_integer := nth 5 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

Lemma evaluation_integer_body : modfunc_body evaluation_integer =
  ltac:(let code := eval cbv [modfunc_body evaluation_integer mod_funcs evaluation_module nth] in
    (modfunc_body evaluation_integer) in exact code).
Proof. reflexivity. Qed.

Definition evaluation_premise := nth 9 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

Lemma evaluation_premise_header : modfunc_body evaluation_premise =
  ltac:(let code := eval cbv [modfunc_body evaluation_premise mod_funcs evaluation_module nth] in
    (modfunc_body evaluation_premise) in exact code).
Proof. reflexivity. Qed.

Definition native_lower := nth 6 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

Lemma native_lower_body : modfunc_body native_lower =
  ltac:(let code := eval cbv [modfunc_body native_lower mod_funcs evaluation_module nth] in
    (modfunc_body native_lower) in exact code).
Proof. reflexivity. Qed.

Definition native_data := nth 12 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

Lemma native_data_body : modfunc_body native_data =
  ltac:(let code := eval cbv [modfunc_body native_data mod_funcs evaluation_module nth] in
    (modfunc_body native_data) in exact code).
Proof. reflexivity. Qed.

Definition native_map := nth 13 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

Lemma native_map_body : modfunc_body native_map =
  ltac:(let code := eval cbv [modfunc_body native_map mod_funcs evaluation_module nth] in
    (modfunc_body native_map) in exact code).
Proof. reflexivity. Qed.

Definition native_mapped := nth 14 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

Lemma native_mapped_body : modfunc_body native_mapped =
  ltac:(let code := eval cbv [modfunc_body native_mapped mod_funcs evaluation_module nth] in
    (modfunc_body native_mapped) in exact code).
Proof. reflexivity. Qed.

Definition native_tree_rule := nth 15 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

Lemma native_tree_rule_body : modfunc_body native_tree_rule =
  ltac:(let code := eval cbv [modfunc_body native_tree_rule mod_funcs evaluation_module nth] in
    (modfunc_body native_tree_rule) in exact code).
Proof. reflexivity. Qed.

Definition native_exchange := nth 16 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

Lemma native_exchange_body : modfunc_body native_exchange =
  ltac:(let code := eval cbv [modfunc_body native_exchange mod_funcs evaluation_module nth] in
    (modfunc_body native_exchange) in exact code).
Proof. reflexivity. Qed.

Definition evaluation_apply := nth 17 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

Lemma evaluation_apply_body : modfunc_body evaluation_apply =
  ltac:(let code := eval cbv [modfunc_body evaluation_apply mod_funcs evaluation_module nth] in
    (modfunc_body evaluation_apply) in exact code).
Proof. reflexivity. Qed.

Definition native_issue := nth 20 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

Lemma evaluation_output_body : modfunc_body native_issue =
  ltac:(let code := eval cbv [modfunc_body native_issue mod_funcs evaluation_module nth] in
    (modfunc_body native_issue) in exact code).
Proof. reflexivity. Qed.

Definition native_authority := nth 18 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

Lemma native_authority_body : modfunc_body native_authority =
  ltac:(let code := eval cbv [modfunc_body native_authority mod_funcs evaluation_module nth] in
    (modfunc_body native_authority) in exact code).
Proof. reflexivity. Qed.

Definition native_reconstruct := nth 19 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

Lemma native_reconstruct_body : modfunc_body native_reconstruct =
  ltac:(let code := eval cbv [modfunc_body native_reconstruct mod_funcs evaluation_module nth] in
    (modfunc_body native_reconstruct) in exact code).
Proof. reflexivity. Qed.

Definition evaluation_entry := nth 21 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

Lemma evaluation_entry_body : modfunc_body evaluation_entry =
  ltac:(let code := eval cbv [modfunc_body evaluation_entry mod_funcs evaluation_module nth] in
    (modfunc_body evaluation_entry) in exact code).
Proof. reflexivity. Qed.

Definition native_resolve := nth 22 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

Lemma native_resolve_body : modfunc_body native_resolve =
  ltac:(let code := eval cbv [modfunc_body native_resolve mod_funcs evaluation_module nth] in
    (modfunc_body native_resolve) in exact code).
Proof. reflexivity. Qed.

Definition native_family := nth 23 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

Lemma native_family_body : modfunc_body native_family =
  ltac:(let code := eval cbv [modfunc_body native_family mod_funcs evaluation_module nth] in
    (modfunc_body native_family) in exact code).
Proof. reflexivity. Qed.


Definition native_witness_claim := nth 24 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).

Lemma native_witness_claim_body : modfunc_body native_witness_claim =
  ltac:(let code := eval cbv [modfunc_body native_witness_claim mod_funcs evaluation_module nth] in
    (modfunc_body native_witness_claim) in exact code).
Proof. reflexivity. Qed.

Definition exchange_endpoint (left : bool) :=
  nth (if left then 10 else 11) (mod_funcs evaluation_module) (Build_module_func 0%N [] []).
Lemma exchange_endpoint_body left : modfunc_body (exchange_endpoint left) =
  [BI_local_get 0%N;
   BI_const_num (VAL_int32 (wasm_nat (if left then 1 else 2))); BI_call 7%N].
Proof. destruct left; reflexivity. Qed.

Lemma collector_export_entry : mod_exports evaluation_module =
  [Build_module_export (String.list_byte_of_string "_fix_apply") (MED_func 43%N)].
Proof. reflexivity. Qed.

Lemma collector_memory_layout :
  mod_tables evaluation_module = [] /\ mod_globals evaluation_module = [] /\
  mod_mems evaluation_module = [Build_module_mem (Build_limits 1%N None)].
Proof. repeat split; reflexivity. Qed.
