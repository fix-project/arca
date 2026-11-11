Module WasmOutput.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Support := WasmSupport.
  Import Support.
  Module Resources := WasmResources.
  Module Issuance := WasmIssuance.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Lemma collector_output_executes state store locals combination_index lhs_index rhs_index number lhs rhs author :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state combination_index = Some (treeh combination) ->
    G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    nth_error (get_tree_raw combination) 1 = Some author ->
    G.E.H.T.authorizes (treeh combination) author ->
    G.E.H.D.live_count (G.E.H.slots state) + 6 <= G.E.H.capacity ->
    let code := G.E.H.H.HBlobObj (create_blob (serialise_i64 number)) in
    let claim := create_tree [code; lhs; rhs] in
    let output := treeh (create_tree_tagged true [author; G.E.H.H.HTreeRef claim]) in
    exists next updated index,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [descriptor combination_index; descriptor lhs_index; descriptor rhs_index; VAL_num (VAL_int64 number)] ++
            [AI_basic (BI_call 42%N)])
        (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [word index]) /\
      G.E.H.resolve next (wasm_nat index) = Some output /\
      G.E.H.resolve state (wasm_nat index) = None /\ scope state next 6 /\
      G.E.H.observations next = G.E.H.observations state /\
      s_funcs updated = s_funcs store /\ store_typing updated /\ Resources.memory_capacity updated.
  Proof.
    exact (Issuance.collector_issue_executes state store locals combination_index lhs_index rhs_index number lhs rhs author).
  Qed.
  Definition issued_claim lhs rhs number :=
    create_tree [G.E.H.H.HBlobObj (create_blob (from_nat number));lhs;rhs].

  Definition issued_tag author lhs rhs number :=
    create_tree_tagged true [author;G.E.H.H.HTreeRef (issued_claim lhs rhs number)].

  Lemma collector_output_encodes lhs rhs author number kind :
    Claims.Code.statement_kind number = Some kind ->
    X.Wire.encode kind author lhs rhs = Some (treeh (create_tree
      [treeh (issued_tag author lhs rhs number);treeh (issued_claim lhs rhs number)])).
  Proof.
    intro Kind; unfold X.Wire.encode.
    change (X.Wire.Code.statement_kind number = Some kind) in Kind.
    rewrite (X.Wire.Code.statement_kind_roundtrip _ _ Kind).
    cbn [X.Wire.H.omap].
    cbv beta iota zeta delta [X.Wire.witness issued_tag issued_claim]; reflexivity.
  Qed.

  Lemma issued_claim_serialised lhs rhs number :
    (Z.of_nat number < 2 ^ 64)%Z ->
    create_tree [G.E.H.H.HBlobObj (create_blob (serialise_i64 (wasm_nat64 number)));lhs;rhs] =
      issued_claim lhs rhs number.
  Proof.
    intro Bound; rewrite serialise_i64_encode_u64, wasm_nat64_unsigned by exact Bound.
    rewrite Nat2Z.id; reflexivity.
  Qed.
End WasmOutput.
