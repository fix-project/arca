(** Specifies the embedding of Fix handles in Wasm externrefs and provides
    conversions between abstract handles and Wasm values. *)

From Wasm Require Import datatypes numerics memory operations typing host properties.
From Stdlib Require Import List ZArith.
From FixProof.Semantics Require Import Handle.
From FixProof.Shell Require Import Operations.
Import ListNotations.

Module Type EXTERNREFS (S : STORAGE).
  Parameter to_externref : S.handle -> externaddr.
  Parameter to_handle : externaddr -> S.handle.
  (** Embedding a Fix handle as an externref and decoding it recovers the original handle. *)
  Axiom to_handle_to_externref : forall y, to_handle (to_externref y) = y.
End EXTERNREFS.

Module Externrefs (S : STORAGE) (R : EXTERNREFS S).
  Module H := Handles S.
  Module Api := Operations S.
  Import H Api R.
  Definition extern_value h := VAL_ref (VAL_ref_extern (to_externref h)).
  Definition i32_of_nat n := Wasm_int.int_of_Z i32m (Z.of_nat n).
  Definition nat_of_i32 n := Wasm_int.nat_of_uint i32m n.
  Definition wasm_bool (b : bool) := i32_of_nat (if b then 1 else 0).
  Definition bool_result b := Some [VAL_num (VAL_int32 (wasm_bool b))].
  Definition handle_result x := H.omap (fun h => [extern_value h]) x.
End Externrefs.
