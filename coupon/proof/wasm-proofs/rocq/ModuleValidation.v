From Stdlib Require Import List String.
From Wasm Require Import datatypes instantiation_func interp_instantiate_sound.
From FixProof Require Import Init.

(** These are facts about the generated coupon.wat AST, rather than an
    assumed module shape. Execution proofs can depend on them directly. *)
Lemma coupon_module_type_checked : exists imports exports,
  module_type_checker coupon_module = (Some (imports, exports), "ok"%string).
Proof. vm_compute; eexists; eexists; reflexivity. Qed.

Lemma coupon_module_well_typed : exists imports exports,
  instantiation_spec.module_typing coupon_module imports exports.
Proof.
  destruct coupon_module_type_checked as [imports [exports Checked]].
  exists imports, exports; eapply module_type_checker_sound; exact Checked.
Qed.

Lemma coupon_module_import_count : List.length (mod_imports coupon_module) = 21.
Proof. reflexivity. Qed.

Lemma coupon_module_table_count : List.length (mod_tables coupon_module) = 2.
Proof. reflexivity. Qed.

Lemma coupon_module_function_count : List.length (mod_funcs coupon_module) = 14.
Proof. reflexivity. Qed.
