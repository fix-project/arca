(** Groups the storage, program, externref, memory, and privileged comparison
    interfaces shared by the native collector proofs. *)

From Wasm Require Export datatypes operations opsem.
From FixProof.Semantics Require Export Handle ApplyTree.
From FixProof.Collector Require Export Matching Encoding Exchanges.
From FixProof.Shell Require Export ABI Externref Host Memory Tables Privileged.
From FixProof.Collector.wasm Require Export Binary Guards Execution Layout.
From FixProof.Support Require Export BlobInteger WasmNatural.
From Stdlib Require Export List Bool NArith ZArith Lia Relation_Operators.
Import ListNotations.
Local Open Scope list_scope.

Module Type COLLECTOR_CONTEXT.
  Declare Module S : STORAGE.
  Declare Module P : PROGRAM S.
  Declare Module R : EXTERNREFS S.
  Declare Instance mem : BlockUpdateMemory.
  Module G := WasmGuards S P R.
  Module X := CouponExchanges S P G.E.H.Tables.T.
  Module Claims := CouponClaims S.
  Import S R.
  Parameter combination : TreeName.
  Include PRIVILEGED_HANDLES S.
  #[export] Instance collector_host : host :=
    G.E.H.shell_host combination G.E.collector_instance true handle_bytes.
End COLLECTOR_CONTEXT.
