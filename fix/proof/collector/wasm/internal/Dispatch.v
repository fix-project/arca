Module WasmDispatch.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module D := WasmDispatchSupport.
  Import D D.V D.V.V D.V.V.Support.
  Module Guards := WasmGuardValidation.
  Module Trees := WasmTreeValidation.
  Module Authentication := WasmAuthentication.
  Module Constructors := WasmConstructors.
  Module Premises := WasmPremise.
  Module Mapped := WasmMappedValidation.
  Module Strict := WasmStrictValidation.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Ltac keep := first [assumption | match goal with
    | Scoped : scope ?before ?after ?extra |- G.E.H.resolve ?after ?index = Some ?value =>
      apply (proj1 Scoped); keep
    end].

  Ltac find_escape instructions :=
    lazymatch instructions with
    | AI_label ?arity ?continuation ?body :: ?suffix =>
      let found := find_escape body in
      lazymatch found with (?result, ?depth, ?context) =>
        constr:((result, Datatypes.S depth, LH_rec [] arity continuation context suffix))
      end
    | ?head :: AI_basic BI_return :: ?suffix =>
      let parsed := eval cbn [e_to_v_opt option_map v_to_e] in (e_to_v_opt head) in
      lazymatch parsed with Some (VAL_num (VAL_int64 ?bits)) =>
        let result := eval vm_compute in (Wasm_int.nat_of_uint i64m bits) in
        constr:((result, O, LH_base [] suffix))
      end
    end.

  Ltac finish_logic :=
    cbn [X.check];
    repeat match goal with
    | Fields : get_tree_raw ?node = ?values |- context [get_tree_raw ?node] => rewrite Fields
    end;
    cbn [X.self X.leaf X.identification X.unary X.binary];
    try unfold X.force_results;
    repeat match goal with
    | Read : X.read ?kind ?witness = Some (?left, ?right) |- context [X.read ?kind ?witness] => rewrite Read
    | Leaf : X.reflexive_eval ?input = true |- context [X.reflexive_eval ?input] => rewrite Leaf
    end;
    try unfold X.mapped;
    try unfold X.transitive; try unfold X.substitute_eval; try unfold X.substitute_force;
    try unfold X.think_application; try unfold X.think_selection; try unfold X.force_encode;
    cbn [X.think_step X.think_data X.mapped X.D.Context.EC.E.EP.E.H.obind X.D.Context.EC.E.EP.E.H.omap];
    repeat match goal with
    | Construct : X.construct ?kind ?input = Some ?output |- context [X.construct ?kind ?input] =>
      rewrite Construct; cbn [X.D.Context.EC.E.EP.E.H.obind X.D.Context.EC.E.EP.E.H.omap]
    end;
    cbn [Nat.eqb]; try assumption; rewrite ?X.API.handle_eqb_refl; reflexivity.

  Ltac finish_branch :=
    lazymatch goal with
    | |- checked (?current, ?store, Build_frame ?locals G.E.collector_instance, ?instructions)
        (branch_validated ?before ?code ?lhs_index ?rhs_index ?witnesses_index ?rule ?witnesses ?lhs ?rhs ?offset ?budget) =>
      let instructions := eval cbn [to_e_list v_to_e_list List.map seq.map List.app seq.cat Base.result_word] in instructions in
      let found := find_escape instructions in
      lazymatch found with (?result, ?depth, ?context) =>
      let kind := eval hnf in (Claims.Code.statement_kind result) in
      lazymatch kind with Some ?kind =>
      eapply checked_result; [apply rt_refl|];
      exists current, store, locals, result, kind, depth, context;
      split; [reflexivity|]; split;
      [ first [assumption|apply scope_refl|
          eapply scope_weaken; [solve [eauto 8 using scope_trans, scope_refl, compared_scope]|lia]]
      | split; [assumption|]; split; [assumption|]; split; [assumption|]; split; [assumption|]; split; [reflexivity|finish_logic] ]
      end end
    end.

  Ltac dispatch_pure :=
    cbn [Base.endpoint_locals Base.scratch_locals Base.single_locals Base.optional_descriptor firstn];
    unfold Base.Support.word, Base.Support.descriptor, Base.result_word;
    checked_pure.

  Lemma self_validation state store locals lhs_index rhs_index witnesses_index lhs rhs witnesses :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.within_capacity state -> G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state witnesses_index = Some (treeh witnesses) -> lengths_bounded 1 (treeh witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    checked
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list (Base.rule_arguments 12 lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
      (fun final => exists next updated result kind,
        final = (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [Base.result_word result]) /\
        scope state next 0 /\ s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Claims.Code.statement_kind result = Some kind /\ X.check X.Self (get_tree_raw witnesses) lhs rhs = Some kind).
  Proof.
    intros Functions Typed Capacity Within LhsLive RhsLive WitnessesLive Bounds Bytes.
    apply checked_rule_call; [exact Functions|].

    set (inside := Base.initial_locals 12 lhs_index rhs_index witnesses_index).
    unfold Base.rule_body at 1; cbn [native_exchange modfunc_body mod_funcs evaluation_module nth].
    do 9 dispatch_pure.
    apply branch_label.
    dispatch_pure.
    checked_use (checked_arity state store inside witnesses_index (treeh witnesses) 0 Functions WitnessesLive Bounds
      ltac:(vm_compute; congruence)).
    intros final [Final [actual [Tree Length]]]; inversion Tree; subst actual final; normalize_checked.
    apply length_zero_iff_nil in Length.
    do 2 dispatch_pure.
    checked_use (checked_same state store inside lhs_index rhs_index lhs rhs Functions LhsLive RhsLive Bytes).
    intros final [Final Same]; subst final rhs; normalize_checked.

    assert (WithinNext : G.E.H.within_capacity (compared state)) by exact Within.
    finish_branch.
  Qed.

  Lemma identification_validation state store locals lhs_index rhs_index witnesses_index lhs rhs witnesses :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.within_capacity state -> G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state witnesses_index = Some (treeh witnesses) -> lengths_bounded 1 (treeh witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    checked
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list (Base.rule_arguments 26 lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
      (fun final => exists next updated result kind,
        final = (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [Base.result_word result]) /\
        scope state next 32 /\ s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Claims.Code.statement_kind result = Some kind /\ X.check X.ThinkIdentification (get_tree_raw witnesses) lhs rhs = Some kind).
  Proof.
    intros Functions Typed Capacity Within LhsLive RhsLive WitnessesLive Bounds Bytes Room.
    apply checked_rule_call; [exact Functions|].

    set (inside := Base.initial_locals 26 lhs_index rhs_index witnesses_index).
    unfold Base.rule_body at 1; cbn [native_exchange modfunc_body mod_funcs evaluation_module nth].
    do 15 dispatch_pure.
    apply branch_label.
    dispatch_pure.
    checked_use (checked_arity state store inside witnesses_index (treeh witnesses) 0 Functions WitnessesLive Bounds
      ltac:(vm_compute; congruence)).
    intros final [Final [actual [Tree Length]]]; inversion Tree; subst actual final; normalize_checked.
    apply length_zero_iff_nil in Length.
    dispatch_pure.
    pose proof (Guards.checked_type_guard state store inside rhs_index rhs 1 Functions RhsLive ltac:(lia)) as Guard.
    lazymatch type of Guard with Guards.V.V.checked ?initial ?post => change (checked initial post) in Guard end.
    unfold Guards.V.V.Support.word, Guards.V.V.Support.descriptor in Guard.
    checked_use Guard.
    intros final [Final Classification]; subst final; normalize_checked.
    destruct rhs as [[[blob|node]|reference]|thunk|encoded]; cbn [G.E.H.H.value_type] in Classification; try discriminate.
    set (value := Thunk (G.E.H.H.Identification reference)).
    assert (Values : G.E.H.host_values combination G.E.collector_instance true true handle_bytes state store
      shell_identification [descriptor rhs_index] = G.E.H.allocate state value).
    { unfold G.E.H.host_values, G.E.H.transform, descriptor; rewrite RhsLive; reflexivity. }
    destruct (new_handle_run state store inside 15%N shell_identification [descriptor rhs_index] value
      Functions eq_refl eq_refl eq_refl ltac:(discriminate) Values Room)
      as [allocated [copy [Created [Present [Fresh [Scoped Observations]]]]]].
    do 2 dispatch_pure; checked_advance_under [descriptor lhs_index] Created.
    set (slots := [None;None;None;None;None;Some (wasm_nat copy);None]).
    set (following := Base.scratch_locals 26 lhs_index rhs_index witnesses_index slots 0).
    assert (Left1 : G.E.H.resolve allocated lhs_index = Some lhs) by (apply (proj1 Scoped); exact LhsLive).
    checked_use (checked_same allocated store following lhs_index (wasm_nat copy) lhs value Functions Left1 Present Bytes).
    intros final [Final Same]; subst final lhs; normalize_checked.
    assert (WithinNext : G.E.H.within_capacity (compared allocated)).
    { change (G.E.H.within_capacity allocated); eapply Resources.execution_descriptor_capacity; eassumption. }


    unfold value.
    finish_branch.
  Qed.

  Lemma eval_leaf_validation state store locals lhs_index rhs_index witnesses_index lhs rhs witnesses :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.within_capacity state -> G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state witnesses_index = Some (treeh witnesses) -> lengths_bounded 1 (treeh witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    checked
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list (Base.rule_arguments 5 lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
      (fun final => exists next updated result kind,
        final = (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [Base.result_word result]) /\
        scope state next 32 /\ s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Claims.Code.statement_kind result = Some kind /\ X.check X.EvalLeaf (get_tree_raw witnesses) lhs rhs = Some kind).
  Proof.
    intros Functions Typed Capacity Within LhsLive RhsLive WitnessesLive Bounds Bytes.
    apply checked_rule_call; [exact Functions|].

    set (inside := Base.initial_locals 5 lhs_index rhs_index witnesses_index).
    unfold Base.rule_body at 1; cbn [native_exchange modfunc_body mod_funcs evaluation_module nth].
    do 12 dispatch_pure.
    apply branch_label.
    dispatch_pure.
    checked_use (checked_arity state store inside witnesses_index (treeh witnesses) 0 Functions WitnessesLive Bounds
      ltac:(vm_compute; congruence)).
    intros final [Final [actual [Tree Length]]]; inversion Tree; subst actual final; normalize_checked.
    apply length_zero_iff_nil in Length.
    pose proof (Guards.checked_leaf_guard state store inside lhs_index lhs Functions eq_refl LhsLive) as LeafGuard.
    lazymatch type of LeafGuard with Guards.V.V.checked ?initial ?post => change (checked initial post) in LeafGuard end.
    unfold Guards.V.V.Support.word, Guards.V.V.Support.descriptor in LeafGuard.
    checked_use LeafGuard.
    intros final [Final Leaf]; subst final; normalize_checked.
    do 2 dispatch_pure.
    checked_use (checked_same state store inside lhs_index rhs_index lhs rhs Functions LhsLive RhsLive Bytes).
    intros final [Final Same]; subst final rhs; normalize_checked.

    assert (WithinNext : G.E.H.within_capacity (compared state)) by exact Within.
    finish_branch.
  Qed.

  Lemma tree_validation state store locals lhs_index rhs_index witnesses_index lhs rhs witnesses (evaluate : bool) :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.within_capacity state -> G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state witnesses_index = Some (treeh witnesses) ->
    lengths_bounded 1 lhs -> lengths_bounded 1 rhs -> lengths_bounded 1 (treeh witnesses) ->
    Forall (lengths_bounded 3) (get_tree_raw witnesses) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 10 * List.length (get_tree_raw witnesses) <= G.E.H.capacity ->
    checked
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list (Base.rule_arguments (if evaluate then 6 else 0) lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
      (fun final => exists next updated result kind,
        final = (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [Base.result_word result]) /\
        scope state next (10 * List.length (get_tree_raw witnesses)) /\ s_funcs updated = s_funcs G.E.allocated_store /\ store_typing updated /\ Resources.memory_capacity updated /\
        Claims.Code.statement_kind result = Some kind /\
        X.check (if evaluate then X.EvalTree else X.TreeEq) (get_tree_raw witnesses) lhs rhs = Some kind).
  Proof.
    intros Functions Typed Capacity Within LhsLive RhsLive WitnessesLive LeftBound RightBound WitnessBound Bounds Bytes Room.
    apply checked_rule_call; [exact Functions|].

    set (inside := Base.initial_locals (if evaluate then 6 else 0) lhs_index rhs_index witnesses_index).
    set (kind := if evaluate then 1 else 0).
    set (claim_kind := if evaluate then Eval else Equivalent).
    assert (Kind : Claims.Code.statement_kind kind = Some claim_kind) by (destruct evaluate; reflexivity).
    unfold Base.rule_body at 1; rewrite native_exchange_body; cbn [nth].
    let finish := ltac:(
      apply branch_label; do 3 dispatch_pure;
      let Tree := fresh "Tree" in
      pose proof (Trees.checked_tree_rule state store inside kind claim_kind lhs_index rhs_index witnesses_index lhs rhs witnesses
        Functions Typed Capacity LhsLive RhsLive WitnessesLive LeftBound RightBound WitnessBound Bounds Kind Bytes Room) as Tree;
      lazymatch type of Tree with Trees.A.V.V.checked ?initial ?post => change (checked initial post) in Tree end;
      unfold Trees.A.V.V.Support.word, Trees.A.V.V.Support.descriptor in Tree;
      let Safe := fresh "Safe" in
      lazymatch type of Tree with checked ?initial ?post =>
        pose proof (checked_capacity initial post Within Tree) as Safe end;
      checked_use Safe;
      let final := fresh "final" in let next := fresh "next" in let updated := fresh "updated" in
      let Final := fresh "Final" in let Scoped := fresh "Scoped" in let SameFunctions := fresh "SameFunctions" in
      let NextTyped := fresh "NextTyped" in let NextCapacity := fresh "NextCapacity" in let Correct := fresh "Correct" in
      let WithinNext := fresh "WithinNext" in
      intros final [[next [updated [Final [Scoped [SameFunctions [NextTyped [NextCapacity Correct]]]]]]] WithinNext];
      subst final; normalize_checked;
      let NextFunctions := fresh "NextFunctions" in
      assert (NextFunctions : s_funcs updated = s_funcs G.E.allocated_store) by (rewrite SameFunctions; exact Functions);
      finish_branch) in
    destruct evaluate; [do 6 dispatch_pure; finish|do 3 dispatch_pure; finish].
  Qed.
End WasmDispatch.
