Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.
  Local Notation blobh := G.E.H.H.HBlobObj.
  Local Notation local_get_run := (G.E.collector_local_get combination true true handle_bytes).
  Definition word n := VAL_num (VAL_int32 (wasm_nat n)).
  Definition descriptor index := VAL_num (VAL_int32 index).

  Definition retains before after := forall index value,
    G.E.H.resolve before index = Some value -> G.E.H.resolve after index = Some value.

  Definition scope before after extra := retains before after /\
    G.E.H.D.live_count (G.E.H.slots after) <= G.E.H.D.live_count (G.E.H.slots before) + extra.
