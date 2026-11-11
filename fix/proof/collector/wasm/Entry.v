From FixProof.Collector.wasm Require Import Backend.
From compcert Require Import Memdata.
From Wasm Require Import properties type_preservation.
From Wasm Require Import typing properties.
Import ListNotations.
Local Open Scope list_scope.

Module WasmEntry (C : COLLECTOR_CONTEXT).
Module WasmSupport.
Load "fix/proof/collector/wasm/specification/Support/word".
Load "fix/proof/collector/wasm/internal/Support/Descriptors".
Load "fix/proof/collector/wasm/machinery/Support/Instructions".
Load "fix/proof/collector/wasm/specification/Support/compared".
Load "fix/proof/collector/wasm/machinery/Support/Comparison".
End WasmSupport.
Module WasmChecked.
Load "fix/proof/collector/wasm/specification/Checked/checked".
Load "fix/proof/collector/wasm/machinery/Checked/Composition".
End WasmChecked.
Module WasmResources.
Load "fix/proof/collector/wasm/specification/Resources/memory_capacity".
Load "fix/proof/collector/wasm/machinery/Resources/Memory".
End WasmResources.
Load "fix/proof/collector/wasm/internal/Witnesses".
Load "fix/proof/collector/wasm/machinery/Acceptance".
Module WasmValidation.
Load "fix/proof/collector/wasm/internal/Validation/native_import_traps".
Load "fix/proof/collector/wasm/specification/Validation/Lengths".
Load "fix/proof/collector/wasm/internal/Validation/lengths_bounded_weaken".
End WasmValidation.
Load "fix/proof/collector/wasm/internal/TreeLoop".
Load "fix/proof/collector/wasm/internal/Rules".
Load "fix/proof/collector/wasm/machinery/DispatchSupport".
Load "fix/proof/collector/wasm/machinery/GuardValidation".
Load "fix/proof/collector/wasm/internal/Constructors".
Load "fix/proof/collector/wasm/machinery/Integer".
Load "fix/proof/collector/wasm/internal/Premise".
Load "fix/proof/collector/wasm/machinery/MappedValidation".
Load "fix/proof/collector/wasm/machinery/Authentication".
Module WasmAuthorityGuard.
Load "fix/proof/collector/wasm/specification/AuthorityGuard/Context".
Load "fix/proof/collector/wasm/machinery/AuthorityGuard/Traversal".
Load "fix/proof/collector/wasm/specification/AuthorityGuard/witness_limit".
Load "fix/proof/collector/wasm/machinery/AuthorityGuard/Limit".
End WasmAuthorityGuard.
Load "fix/proof/collector/wasm/internal/Authority".
Module WasmAuthorityValidation.
Load "fix/proof/collector/wasm/internal/AuthorityValidation/authority_pure".
Load "fix/proof/collector/wasm/specification/AuthorityValidation/Authorized".
Load "fix/proof/collector/wasm/internal/AuthorityValidation/checked_authority_iteration".
End WasmAuthorityValidation.
Load "fix/proof/collector/wasm/internal/TreeValidation".
Load "fix/proof/collector/wasm/machinery/BinaryRules".
Load "fix/proof/collector/wasm/machinery/ForceRules".
Load "fix/proof/collector/wasm/machinery/StrictValidation".
Load "fix/proof/collector/wasm/internal/Dispatch".
Load "fix/proof/collector/wasm/machinery/Prefixes".
Load "fix/proof/collector/wasm/machinery/BinaryPrefixValidation".
Load "fix/proof/collector/wasm/machinery/ForceValidation".
Load "fix/proof/collector/wasm/internal/Reconstruction".
Load "fix/proof/collector/wasm/machinery/ThinkValidation".
Load "fix/proof/collector/wasm/machinery/IssuePrefix".
Load "fix/proof/collector/wasm/machinery/DirectValidation".
Load "fix/proof/collector/wasm/machinery/CongruenceValidation".
Load "fix/proof/collector/wasm/machinery/ThunkValidation".
Load "fix/proof/collector/wasm/machinery/SubstitutionValidation".
Load "fix/proof/collector/wasm/machinery/ResultValidation".
Module WasmNativeDispatch.
Load "fix/proof/collector/wasm/specification/NativeDispatch/Context".
Load "fix/proof/collector/wasm/machinery/NativeDispatch/collector_dispatch_validation".
Load "fix/proof/collector/wasm/specification/NativeDispatch/collector_dispatch_validation".
End WasmNativeDispatch.
Load "fix/proof/collector/wasm/machinery/Resolution".
Load "fix/proof/collector/wasm/internal/Exchange".
Load "fix/proof/collector/wasm/machinery/CollectPrefix".
Load "fix/proof/collector/wasm/internal/Issuance".
Module WasmOutput.
Load "fix/proof/collector/wasm/specification/Output/Context".
Load "fix/proof/collector/wasm/machinery/Output/collector_output_executes".
Load "fix/proof/collector/wasm/specification/Output/collector_output_executes".
Load "fix/proof/collector/wasm/machinery/Output/collector_output_encodes".
Load "fix/proof/collector/wasm/specification/Output/collector_output_encodes".
Load "fix/proof/collector/wasm/machinery/Output/issued_claim_serialised".
Load "fix/proof/collector/wasm/specification/Output/issued_claim_serialised".
End WasmOutput.
Load "fix/proof/collector/wasm/internal/Collect".
Module WasmEntryFields.
Load "fix/proof/collector/wasm/internal/EntryFields/entry_locals".
Load "fix/proof/collector/wasm/specification/EntryFields/TreeRequest".
Load "fix/proof/collector/wasm/internal/EntryFields/entry_tail".
End WasmEntryFields.
Load "fix/proof/collector/wasm/machinery/EntryPrefix".
Load "fix/proof/collector/wasm/internal/EntryExecution".
Load "fix/proof/collector/wasm/internal/AuthorityPolicy".
Load "fix/proof/collector/wasm/machinery/ReconstructionValidation".
Load "fix/proof/collector/wasm/internal/TreeEntryExecution".
Load "fix/proof/collector/wasm/internal/NativeSuccess".
Load "fix/proof/collector/wasm/internal/BinarySuccess".
Load "fix/proof/collector/wasm/internal/ForceSuccess".
Module WasmExport.
Load "fix/proof/collector/wasm/specification/Export/Context".
Load "fix/proof/collector/wasm/machinery/Export/exported_exchange_refinement".
Load "fix/proof/collector/wasm/specification/Export/exported_exchange_refinement".
Load "fix/proof/collector/wasm/machinery/Export/exported_tree_exchange_sound".
Load "fix/proof/collector/wasm/specification/Export/exported_tree_exchange_sound".
Load "fix/proof/collector/wasm/machinery/Export/initial_descriptor_budget".
Load "fix/proof/collector/wasm/specification/Export/initial_descriptor_budget".
End WasmExport.
Load "fix/proof/collector/wasm/specification/Refinement".

Definition exported_exchange_refinement := WasmExport.exported_exchange_refinement.
Definition exported_tree_exchange_sound := WasmExport.exported_tree_exchange_sound.
Definition initial_descriptor_budget := WasmExport.initial_descriptor_budget.
End WasmEntry.
