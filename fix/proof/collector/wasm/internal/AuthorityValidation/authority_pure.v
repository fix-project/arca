Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module A := WasmAuthentication.
  Import A A.V A.V.V A.V.V.Support.
  Module Authority := WasmAuthority.
  Import Authority.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Ltac authority_pure :=
    cbn [authority_locals]; unfold Authority.Support.word, Authority.Support.descriptor; checked_pure.
