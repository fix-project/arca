Import ListNotations.
Local Open Scope list_scope.
Local Transparent Archi.big_endian.
  Import C S.
  Module Support := WasmSupport.
  Import Support.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.

  Definition memory_capacity store := exists page,
    s_mems store = [page] /\ meminst_type page = Build_limits 1%N None /\ mem_length page = page_size.
