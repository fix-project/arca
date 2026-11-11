From FixProof.Collector.wasm Require Import Backend.
From compcert Require Import Memdata.
From Wasm Require Import properties type_preservation.
From Wasm Require Import typing properties.
Import ListNotations.
Local Open Scope list_scope.

Module WasmEntry (C : COLLECTOR_CONTEXT).
Load "Support".
Load "Checked".
Load "Resources".
Load "Witnesses".
Load "Acceptance".
Load "Validation".
Load "TreeLoop".
Load "Rules".
Load "DispatchSupport".
Load "GuardValidation".
Load "Constructors".
Load "Integer".
Load "Premise".
Load "MappedValidation".
Load "Authentication".
Load "AuthorityGuard".
Load "Authority".
Load "AuthorityValidation".
Load "TreeValidation".
Load "BinaryRules".
Load "ForceRules".
Load "StrictValidation".
Load "Dispatch".
Load "Prefixes".
Load "BinaryPrefixValidation".
Load "ForceValidation".
Load "Reconstruction".
Load "ThinkValidation".
Load "IssuePrefix".
Load "DirectValidation".
Load "CongruenceValidation".
Load "ThunkValidation".
Load "SubstitutionValidation".
Load "ResultValidation".
Load "NativeDispatch".
Load "Resolution".
Load "Exchange".
Load "CollectPrefix".
Load "Issuance".
Load "Output".
Load "Collect".
Load "EntryFields".
Load "EntryPrefix".
Load "EntryExecution".
Load "AuthorityPolicy".
Load "ReconstructionValidation".
Load "TreeEntryExecution".
Load "NativeSuccess".
Load "BinarySuccess".
Load "ForceSuccess".
Load "Export".
Load "Refinement".

Definition exported_exchange_refinement := WasmExport.exported_exchange_refinement.
Definition exported_tree_exchange_sound := WasmExport.exported_tree_exchange_sound.
Definition initial_descriptor_budget := WasmExport.initial_descriptor_budget.
End WasmEntry.
