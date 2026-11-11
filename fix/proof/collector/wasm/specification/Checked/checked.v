Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Support := WasmSupport.
  Import Support.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
      Local Notation configuration := (G.E.H.state * store_record * frame * list administrative_instruction)%type.

      Definition checked (initial : configuration) (post : configuration -> Prop) :=
        exists final, reduce_trans initial final /\
          ((let '(_, _, _, instructions) := final in instructions = [AI_trap]) \/ post final).
