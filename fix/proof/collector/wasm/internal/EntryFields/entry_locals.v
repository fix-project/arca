Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module V := WasmChecked.
  Import V V.Support.
  Module Base := WasmRules.
  Module Resources := WasmResources.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.

  Definition entry_locals root lhs rhs operation :=
    [descriptor root;word lhs;word rhs;word operation].
