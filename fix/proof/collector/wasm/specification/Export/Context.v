Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Import WasmEntryExecution WasmCollect WasmExchange WasmExchange.Native WasmExchange.Native.Dispatcher
    WasmExchange.Native.Dispatcher.D WasmExchange.Native.Dispatcher.D.V
    WasmExchange.Native.Dispatcher.D.V.V WasmExchange.Native.Dispatcher.D.V.V.Support.
  Module TreeExecution := WasmTreeEntryExecution.
  Module Policy := WasmAuthorityPolicy.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.
