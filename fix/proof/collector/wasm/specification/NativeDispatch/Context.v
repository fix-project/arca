Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Dispatcher := WasmDispatch.
  Import Dispatcher Dispatcher.D Dispatcher.D.V Dispatcher.D.V.V Dispatcher.D.V.V.Support.
  Module Direct := WasmDirectValidation.
  Module Congruence := WasmCongruenceValidation.
  Module Thunks := WasmThunkValidation.
  Module Substitution := WasmSubstitutionValidation.
  Module Forces := WasmForceValidation.
  Module Thoughts := WasmThinkValidation.
  Module Results := WasmResultValidation.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
