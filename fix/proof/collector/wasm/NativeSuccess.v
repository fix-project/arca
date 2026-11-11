Module WasmRuleSuccess.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Base := WasmRules.
  Import Base Base.Support.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.

  Lemma rule_call_executes state store locals code lhs rhs witnesses next updated following kind depth (context : lholed depth) :
    s_funcs store = s_funcs G.E.allocated_store ->
    reduce_trans
      (state, store, Build_frame (initial_locals code lhs rhs witnesses) G.E.collector_instance, to_e_list rule_body)
      (next, updated, following, lfill context
        (v_to_e_list [result_word kind] ++ [AI_basic BI_return])) ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list (rule_arguments code lhs rhs witnesses) ++ [AI_basic (BI_call 38%N)])
      (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [result_word kind]).
  Proof.
    intros Functions Body.
    eapply G.E.collector_call_return with (code := native_exchange)
      (parameters := [T_num T_i64;T_num T_i32;T_num T_i32;T_num T_i32]) (results := [T_num T_i64])
      (defaults := repeat (word 0) 4) (address := 38%N)
      (arguments := rule_arguments code lhs rhs witnesses) (values := [result_word kind])
      (accepted := true) (final_frame := following) (context := context); try reflexivity.
    - rewrite Functions; reflexivity.
    - exact Body.
  Qed.

  Lemma native_reflexivity state store locals lhs rhs witnesses value coupons :
    s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state lhs = Some value -> G.E.H.resolve state rhs = Some value ->
    G.E.H.resolve state witnesses = Some (G.E.H.H.HTreeObj coupons) -> get_tree_raw coupons = [] ->
    observed_same state value value ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list (rule_arguments 12 lhs rhs witnesses) ++ [AI_basic (BI_call 38%N)])
      (compared state, store, Build_frame locals G.E.collector_instance, v_to_e_list [result_word 0]).
  Proof.
    intros Functions Left Right Witnesses Empty Compared.
    set (inside := initial_locals 12 lhs rhs witnesses).
    eapply rule_call_executes with (next := compared state) (updated := store)
      (following := Build_frame inside G.E.collector_instance)
      (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn 12 rule_body))); try exact Functions.
    unfold rule_body at 1; rewrite native_exchange_body; cbn [nth].
      unfold inside, initial_locals, rule_arguments, result_word.
      do 9 native_pure; cbn [lfill].
      eapply G.E.collector_runs_context with (prefix := [])
        (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]); apply G.E.collector_runs_label.
      native_pure.
      execute (arity_run state store inside witnesses coupons 0 Functions Witnesses
        ltac:(rewrite Empty; reflexivity) ltac:(vm_compute; congruence)).
      do 2 native_pure; execute (same_run state store inside lhs rhs value value true Functions Left Right Compared).
      apply rt_refl.
  Qed.

  Lemma native_tree_exchange state store locals lhs rhs witnesses left right coupons (evaluate : bool) :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Trees.Resources.memory_capacity store ->
    G.E.H.resolve state lhs = Some (G.E.H.H.HTreeObj left) ->
    G.E.H.resolve state rhs = Some (G.E.H.H.HTreeObj right) ->
    G.E.H.resolve state witnesses = Some (G.E.H.H.HTreeObj coupons) ->
    List.length (get_tree_raw right) = List.length (get_tree_raw left) ->
    List.length (get_tree_raw coupons) = List.length (get_tree_raw left) -> tree_tag left = tree_tag right ->
    native_size_bound (List.length (get_tree_raw left)) ->
    (forall current index, retains state current -> index < List.length (get_tree_raw left) ->
      Trees.iteration_ready current left right coupons (if evaluate then 1 else 0) index) ->
    G.E.H.D.live_count (G.E.H.slots state) + 10 * List.length (get_tree_raw left) <= G.E.H.capacity ->
    exists next updated,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list (rule_arguments (if evaluate then 6 else 0) lhs rhs witnesses) ++ [AI_basic (BI_call 38%N)])
        (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [result_word (if evaluate then 1 else 0)]) /\
      scope state next (10 * List.length (get_tree_raw left)) /\ s_funcs updated = s_funcs store /\ store_typing updated /\ Trees.Resources.memory_capacity updated.
  Proof.
    intros Functions Typed Capacity Left Right Witnesses RightCount WitnessCount Tags Bound Ready Room.
    set (inside := initial_locals (if evaluate then 6 else 0) lhs rhs witnesses).
    destruct (Trees.native_tree_rule_executes state store inside lhs rhs witnesses left right coupons (if evaluate then 1 else 0)
      Functions Typed Capacity Left Right Witnesses RightCount WitnessCount Tags Bound Ready Room)
      as [next [updated [TreeRun [Scoped [Obs [FunctionsNext [TypedNext CapacityNext]]]]]]].
    unfold Trees.Support.word, Trees.Support.descriptor in TreeRun.
    exists next, updated; split; [|auto].
    eapply rule_call_executes with (next := next) (updated := updated)
      (following := Build_frame inside G.E.collector_instance)
      (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn (if evaluate then 8 else 4) rule_body)));
      try exact Functions.
    - { unfold rule_body at 1; rewrite native_exchange_body; cbn [nth].
      unfold inside, initial_locals, rule_arguments, result_word.
      destruct evaluate; [do 6 native_pure|do 3 native_pure].
      all: cbn [lfill].
      all: eapply G.E.collector_runs_context with (prefix := [])
        (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]); apply G.E.collector_runs_label.
      all: do 3 native_pure; execute TreeRun; apply rt_refl. }
  Qed.

  Lemma native_mapped_exchange state store locals kind lhs_index rhs_index witnesses_index lhs rhs witnesses bundle tag claim author code a b :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Trees.Resources.memory_capacity store ->
    G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state witnesses_index = Some (G.E.H.H.HTreeObj witnesses) ->
    get_tree_raw witnesses = [G.E.H.H.HTreeObj bundle] ->
    get_tree_raw bundle = [G.E.H.H.HTreeObj tag; G.E.H.H.HTreeObj claim] ->
    get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] -> tree_tag tag = true ->
    get_tree_raw claim = [G.E.H.H.HBlobObj code; a; b] -> decode_u64 (get_blob_data code) = Some 0 ->
    X.construct kind a = Some lhs -> X.construct kind b = Some rhs ->
    observed_same state (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim) ->
    observed_same (compared state) lhs lhs -> observed_same (compared (compared state)) rhs rhs ->
    observed_same (compared (compared (compared state))) a a ->
    observed_same (compared (compared (compared (compared state)))) b b ->
    G.E.H.D.live_count (G.E.H.slots state) + 12 <= G.E.H.capacity ->
    exists next updated,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list (rule_arguments (mapped_rule kind) lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
        (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [result_word 0]) /\
      scope state next 12 /\ s_funcs updated = s_funcs store /\ store_typing updated /\ Trees.Resources.memory_capacity updated.
  Proof.
    intros Functions Typed Capacity LhsLive RhsLive WitnessesLive Witnesses Bundle Tag Tagged Claim Decoded
      ConstructA ConstructB SameRef SameLhs SameRhs SameA SameB Room.
    set (initial := initial_locals (mapped_rule kind) lhs_index rhs_index witnesses_index).
    destruct (Acceptance.Witnesses.witness_claim_executes state store initial witnesses_index witnesses 0 bundle tag claim author
      Functions WitnessesLive ltac:(rewrite Witnesses; reflexivity) ltac:(lia) Bundle Tag Tagged
      ltac:(rewrite Claim; reflexivity) SameRef ltac:(lia))
      as [first [claim_slot [ClaimRun [ClaimLive [ClaimFresh [Scope1 Obs1]]]]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in ClaimRun.
    set (inside := single_locals (mapped_rule kind) lhs_index rhs_index witnesses_index claim_slot 0).
    assert (Lhs1 : G.E.H.resolve first lhs_index = Some lhs) by (apply (proj1 Scope1); exact LhsLive).
    assert (Rhs1 : G.E.H.resolve first rhs_index = Some rhs) by (apply (proj1 Scope1); exact RhsLive).
    assert (SameLhs1 : observed_same first lhs lhs).
    { eapply observed_comparison_observations; [|exact SameLhs]; exact Obs1. }
    assert (SameRhs1 : observed_same (compared first) rhs rhs).
    { eapply observed_comparison_observations; [|exact SameRhs].
      change (G.E.H.observations first + 2 = G.E.H.observations state + 2 + 2); rewrite Obs1; reflexivity. }
    assert (SameA1 : observed_same (compared (compared first)) a a).
    { eapply observed_comparison_observations; [|exact SameA].
      change (G.E.H.observations first + 2 + 2 = G.E.H.observations state + 2 + 2 + 2); rewrite Obs1; reflexivity. }
    assert (SameB1 : observed_same (compared (compared (compared first))) b b).
    { eapply observed_comparison_observations; [|exact SameB].
      change (G.E.H.observations first + 2 + 2 + 2 = G.E.H.observations state + 2 + 2 + 2 + 2); rewrite Obs1; reflexivity. }
    destruct (Acceptance.native_mapped_accepts first store inside kind lhs_index rhs_index (wasm_nat claim_slot)
      lhs rhs claim code a b Functions Typed Capacity Lhs1 Rhs1 ClaimLive Claim Decoded ConstructA ConstructB
      SameLhs1 SameRhs1 SameA1 SameB1 ltac:(pose proof (proj2 Scope1); lia))
      as [second [updated [MappedRun [Scope2 [Obs2 [Functions2 [Typed2 Capacity2]]]]]]].
    unfold Acceptance.Support.word, Acceptance.Support.descriptor in MappedRun.
    assert (FunctionsNext : s_funcs updated = s_funcs G.E.allocated_store) by (rewrite Functions2; exact Functions).
    assert (Scope02 : scope state second 12).
    { replace 12 with (5+7) by lia; eapply scope_trans; eassumption. }
    assert (Claim2 : G.E.H.resolve second (wasm_nat claim_slot) = Some (G.E.H.H.HTreeObj claim))
      by (apply (proj1 Scope2); exact ClaimLive).
    assert (Arity : reduce_trans
      (state, store, Build_frame initial G.E.collector_instance,
        [AI_label 0 [] (to_e_list [BI_local_get 3%N; BI_const_num (VAL_int32 (wasm_nat 1)); BI_call 26%N])])
      (state, store, Build_frame initial G.E.collector_instance, [])).
    { eapply rt_trans; [apply G.E.collector_runs_label|apply rt_step, r_simple, rs_label_const; reflexivity].
      native_pure; exact (arity_run state store initial witnesses_index witnesses 1 Functions WitnessesLive
        ltac:(rewrite Witnesses; reflexivity) ltac:(vm_compute; congruence)). }
    exists second, updated; split; [|auto].
    eapply rule_call_executes with (next := second) (updated := updated)
      (following := Build_frame inside G.E.collector_instance)
      (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn (mapped_tail kind) rule_body)));
      try exact Functions.
    { unfold rule_body at 1; rewrite native_exchange_body; cbn [nth].
      unfold initial, initial_locals, rule_arguments, result_word.
      destruct kind; cbn [mapped_rule mapped_tail Acceptance.native_constructor_index] in *.
      all: repeat native_pure.
      all: unfold initial, initial_locals, rule_arguments, result_word in Arity, ClaimRun.
      all: execute Arity.
      all: native_pure; execute ClaimRun; native_pure.
      all: repeat native_pure; cbn [lfill].
      all: eapply G.E.collector_runs_context with (prefix := [])
        (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]); apply G.E.collector_runs_label.
      all: do 3 native_pure; execute MappedRun; apply rt_refl. }
  Qed.

  Lemma native_eval_leaf state store locals lhs rhs witnesses value coupons :
    s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state lhs = Some value -> G.E.H.resolve state rhs = Some value ->
    G.E.H.resolve state witnesses = Some (G.E.H.H.HTreeObj coupons) -> get_tree_raw coupons = [] ->
    X.reflexive_eval value = true -> observed_same state value value ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list (rule_arguments 5 lhs rhs witnesses) ++ [AI_basic (BI_call 38%N)])
      (compared state, store, Build_frame locals G.E.collector_instance, v_to_e_list [result_word 1]).
  Proof.
    intros Functions Left Right Witnesses Empty Leaf Compared.
    set (inside := initial_locals 5 lhs rhs witnesses).
    eapply rule_call_executes with (next := compared state) (updated := store)
      (following := Build_frame inside G.E.collector_instance)
      (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn 16 rule_body))); try exact Functions.
    unfold rule_body at 1; rewrite native_exchange_body; cbn [nth].
      unfold inside, initial_locals, rule_arguments, result_word.
      do 12 native_pure; cbn [lfill].
      eapply G.E.collector_runs_context with (prefix := [])
        (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]); apply G.E.collector_runs_label.
      native_pure.
      execute (arity_run state store inside witnesses coupons 0 Functions Witnesses
        ltac:(rewrite Empty; reflexivity) ltac:(vm_compute; congruence)).
      execute (native_leaf_guard state store inside lhs value Functions eq_refl Left Leaf).
      do 2 native_pure; execute (same_run state store inside lhs rhs value value true Functions Left Right Compared).
      apply rt_refl.
  Qed.

  Lemma native_think_identification state store locals lhs rhs witnesses reference coupons :
    s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state lhs = Some (Thunk (G.E.H.H.Identification reference)) ->
    G.E.H.resolve state rhs = Some (Data (Ref reference)) ->
    G.E.H.resolve state witnesses = Some (G.E.H.H.HTreeObj coupons) -> get_tree_raw coupons = [] ->
    observed_same state (Thunk (G.E.H.H.Identification reference)) (Thunk (G.E.H.H.Identification reference)) ->
    G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    exists next,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list (rule_arguments 26 lhs rhs witnesses) ++ [AI_basic (BI_call 38%N)])
        (next, store, Build_frame locals G.E.collector_instance, v_to_e_list [result_word 4]) /\ scope state next 1.
  Proof.
    intros Functions Left Right Witnesses Empty Observed Room.
    set (initial := initial_locals 26 lhs rhs witnesses).
    set (value := Thunk (G.E.H.H.Identification reference)).
    assert (Values : G.E.H.host_values combination G.E.collector_instance true true handle_bytes state store
      shell_identification [descriptor rhs] = G.E.H.allocate state value).
    { unfold G.E.H.host_values, G.E.H.transform, descriptor; rewrite Right; reflexivity. }
    destruct (new_handle_run state store initial 15%N shell_identification [descriptor rhs] value
      Functions eq_refl eq_refl eq_refl ltac:(discriminate) Values Room)
      as [allocated [copy [Created [Present [Fresh [Scoped Obs]]]]]].
    set (slots := [None;None;None;None;None;Some (wasm_nat copy);None]).
    set (inside := scratch_locals 26 lhs rhs witnesses slots 0).
    assert (Left1 : G.E.H.resolve allocated lhs = Some value) by (apply (proj1 Scoped); exact Left).
      assert (Observed1 : observed_same allocated value value).
      { eapply observed_comparison_observations; [exact Obs|exact Observed]. }
      pose proof (same_run allocated store inside lhs (wasm_nat copy) value value true
        Functions Left1 Present Observed1) as Same.
      exists (compared allocated); split; [|exact Scoped].
      eapply rule_call_executes with (next := compared allocated) (updated := store)
        (following := Build_frame inside G.E.collector_instance)
        (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn 20 rule_body))); try exact Functions.
      unfold rule_body at 1; rewrite native_exchange_body; cbn [nth].
      unfold initial, initial_locals, rule_arguments, result_word.
      do 15 native_pure; cbn [lfill].
      eapply G.E.collector_runs_context with (prefix := [])
        (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]); apply G.E.collector_runs_label.
      native_pure.
      execute (arity_run state store initial witnesses coupons 0 Functions Witnesses
        ltac:(rewrite Empty; reflexivity) ltac:(vm_compute; congruence)).
      execute (native_type_query state store initial 2%N rhs (Data (Ref reference)) Functions eq_refl Right).
      do 2 native_pure.
      do 2 native_pure; execute_under [descriptor lhs] Created.
      execute Same; apply rt_refl.
  Qed.

  Lemma unary_prefix_executes state store code lhs rhs witnesses coupons bundle tag claim author :
    In code unary_codes -> s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state witnesses = Some (G.E.H.H.HTreeObj coupons) ->
    get_tree_raw coupons = [G.E.H.H.HTreeObj bundle] ->
    get_tree_raw bundle = [G.E.H.H.HTreeObj tag; G.E.H.H.HTreeObj claim] ->
    get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] -> tree_tag tag = true ->
    List.length (get_tree_raw claim) = 3 ->
    observed_same state (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim) ->
    G.E.H.D.live_count (G.E.H.slots state) + 5 <= G.E.H.capacity ->
    exists next claim_slot,
      reduce_trans
        (state, store, Build_frame (initial_locals code lhs rhs witnesses) G.E.collector_instance,
          to_e_list (firstn 48 rule_body))
        (next, store, Build_frame (single_locals code lhs rhs witnesses claim_slot 0) G.E.collector_instance, []) /\
      G.E.H.resolve next (wasm_nat claim_slot) = Some (G.E.H.H.HTreeObj claim) /\
      G.E.H.resolve state (wasm_nat claim_slot) = None /\ scope state next 5 /\
      G.E.H.observations next = G.E.H.observations state + 2.
  Proof.
    intros Code Functions Live Coupons Bundle Tag Tagged Arity Same Room.
    set (initial := initial_locals code lhs rhs witnesses).
    destruct (Acceptance.Witnesses.witness_claim_executes state store initial witnesses coupons 0 bundle tag claim author
      Functions Live ltac:(rewrite Coupons; reflexivity) ltac:(lia) Bundle Tag Tagged Arity Same ltac:(lia))
      as [next [claim_slot [ClaimRun Rest]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in ClaimRun.
    assert (ArityRun : reduce_trans
      (state, store, Build_frame initial G.E.collector_instance,
        [AI_label 0 [] (to_e_list [BI_local_get 3%N; BI_const_num (VAL_int32 (wasm_nat 1)); BI_call 26%N])])
      (state, store, Build_frame initial G.E.collector_instance, [])).
    { eapply rt_trans; [apply G.E.collector_runs_label|apply rt_step, r_simple, rs_label_const; reflexivity].
      native_pure; exact (arity_run state store initial witnesses coupons 1 Functions Live
        ltac:(rewrite Coupons; reflexivity) ltac:(vm_compute; congruence)). }
    exists next, claim_slot; split; [|exact Rest].
    unfold rule_body; rewrite native_exchange_body; cbn [nth firstn].
    unfold initial, initial_locals, single_locals, scratch_locals, optional_descriptor, rule_arguments, result_word in ArityRun, ClaimRun |- *.
    unfold unary_codes in Code; cbn in Code.
    destruct Code as [Code|[Code|[Code|[Code|[Code|[Code|[Code|[Code|[Code|[Code|[Code|[Code|[Code|[Code|[Code|Impossible]]]]]]]]]]]]]]];
      try contradiction; subst code.
    all: repeat native_pure.
    all: execute ArityRun.
    all: native_pure; execute ClaimRun; native_pure; apply rt_refl.
  Qed.

  Lemma native_direct_exchange state store locals kind lhs_index rhs_index witnesses_index lhs rhs witnesses bundle tag claim author code :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Trees.Resources.memory_capacity store ->
    G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state witnesses_index = Some (G.E.H.H.HTreeObj witnesses) ->
    get_tree_raw witnesses = [G.E.H.H.HTreeObj bundle] ->
    get_tree_raw bundle = [G.E.H.H.HTreeObj tag; G.E.H.H.HTreeObj claim] ->
    get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] -> tree_tag tag = true ->
    get_tree_raw claim = [G.E.H.H.HBlobObj code; direct_left kind lhs rhs; direct_right kind lhs rhs] ->
    decode_u64 (get_blob_data code) = Some (direct_claim kind) -> direct_data kind rhs ->
    observed_same state (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim) ->
    observed_same (compared state) (direct_left kind lhs rhs) (direct_left kind lhs rhs) ->
    observed_same (compared (compared state)) (direct_right kind lhs rhs) (direct_right kind lhs rhs) ->
    G.E.H.D.live_count (G.E.H.slots state) + 8 <= G.E.H.capacity ->
    exists next updated,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list (rule_arguments (direct_code kind) lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
        (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [result_word (direct_result kind)]) /\
      scope state next 8 /\ s_funcs updated = s_funcs store /\ store_typing updated /\ Trees.Resources.memory_capacity updated.
  Proof.
    intros Functions Typed Capacity LhsLive RhsLive WitnessesLive Witnesses Bundle Tag Tagged Claim Decoded
      Data SameRef SameLeft SameRight Room.
    set (initial := initial_locals (direct_code kind) lhs_index rhs_index witnesses_index).
    assert (Unary : In (direct_code kind) unary_codes) by (destruct kind; vm_compute; tauto).
    destruct (unary_prefix_executes state store (direct_code kind) lhs_index rhs_index witnesses_index witnesses bundle tag claim author
      Unary Functions WitnessesLive Witnesses Bundle Tag Tagged ltac:(rewrite Claim; reflexivity) SameRef ltac:(lia))
      as [first [claim_slot [Prefix [ClaimLive [ClaimFresh [Scope1 Obs1]]]]]].
    set (inside := single_locals (direct_code kind) lhs_index rhs_index witnesses_index claim_slot 0).
    assert (Left1 : G.E.H.resolve first (match kind with Symmetry => rhs_index | _ => lhs_index end) =
      Some (direct_left kind lhs rhs)) by (destruct kind; apply (proj1 Scope1); assumption).
    assert (Right1 : G.E.H.resolve first (match kind with Symmetry => lhs_index | _ => rhs_index end) =
      Some (direct_right kind lhs rhs)) by (destruct kind; apply (proj1 Scope1); assumption).
    assert (SameLeft1 : observed_same first (direct_left kind lhs rhs) (direct_left kind lhs rhs)).
    { eapply observed_comparison_observations; [|exact SameLeft]; exact Obs1. }
    assert (SameRight1 : observed_same (compared first) (direct_right kind lhs rhs) (direct_right kind lhs rhs)).
    { eapply observed_comparison_observations; [|exact SameRight].
      change (G.E.H.observations first + 2 = G.E.H.observations state + 2 + 2); rewrite Obs1; reflexivity. }
    destruct (Acceptance.Witnesses.collector_premise_executes first store inside (wasm_nat claim_slot)
      (match kind with Symmetry => rhs_index | _ => lhs_index end)
      (match kind with Symmetry => lhs_index | _ => rhs_index end)
      claim code (direct_claim kind) (direct_left kind lhs rhs) (direct_right kind lhs rhs)
      Functions Typed Capacity ClaimLive Left1 Right1 Claim Decoded SameLeft1 SameRight1
      ltac:(pose proof (proj2 Scope1); lia))
      as [second [updated [PremiseRun [Scope2 [Obs2 [Functions2 [Typed2 Capacity2]]]]]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in PremiseRun.
    assert (FunctionsNext : s_funcs updated = s_funcs G.E.allocated_store) by (rewrite Functions2; exact Functions).
    assert (Scope02 : scope state second 8).
    { replace 8 with (5+3) by lia; eapply scope_trans; eassumption. }
    assert (Claim2 : G.E.H.resolve second (wasm_nat claim_slot) = Some (G.E.H.H.HTreeObj claim))
      by (apply (proj1 Scope2); exact ClaimLive).
    exists second, updated; split; [|auto].
    eapply rule_call_executes with (next := second) (updated := updated)
      (following := Build_frame inside G.E.collector_instance)
      (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn (direct_tail kind) rule_body)));
      try exact Functions.
    rewrite <- (firstn_skipn 48 rule_body) at 1; rewrite to_e_list_cat.
    eapply rt_trans with (y := (first, store, Build_frame inside G.E.collector_instance, to_e_list (skipn 48 rule_body))).
    - change (reduce_trans
        (state, store, Build_frame initial G.E.collector_instance,
          to_e_list (firstn 48 rule_body) ++ to_e_list (skipn 48 rule_body))
        (first, store, Build_frame inside G.E.collector_instance, [] ++ to_e_list (skipn 48 rule_body))).
      apply G.E.collector_runs_context with (prefix := []) (suffix := to_e_list (skipn 48 rule_body))
        (instructions := to_e_list (firstn 48 rule_body)) (instructions' := []); exact Prefix.
    - unfold rule_body at 1; rewrite native_exchange_body; cbn [nth skipn].
      unfold inside, single_locals, scratch_locals, optional_descriptor, rule_arguments, result_word; cbn [firstn map].
      destruct kind; cbn [direct_code direct_tail direct_claim direct_left direct_right direct_result] in *.
      all: repeat native_pure; cbn [lfill].
      all: eapply G.E.collector_runs_context with (prefix := [])
        (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]); apply G.E.collector_runs_label.
      + do 3 native_pure; execute PremiseRun; apply rt_refl.
      + do 3 native_pure; execute PremiseRun; apply rt_refl.
      + destruct Data as [datum Equal]; subst rhs.
        native_pure; execute (Acceptance.native_data_executes first store inside rhs_index datum Functions
          ltac:(apply (proj1 Scope1); exact RhsLive)).
        do 3 native_pure; execute PremiseRun; apply rt_refl.
      + destruct Data as [thunk Equal]; subst rhs.
        execute (native_type_query first store inside 2%N rhs_index (Thunk thunk) Functions eq_refl Right1).
        do 2 native_pure; do 3 native_pure; execute PremiseRun; apply rt_refl.
  Qed.

  Lemma rule_prefix_continue cutoff state store frame middle updated following next final_store final_frame instructions :
    reduce_trans (state, store, frame, to_e_list (firstn cutoff rule_body)) (middle, updated, following, []) ->
    reduce_trans (middle, updated, following, to_e_list (skipn cutoff rule_body))
      (next, final_store, final_frame, instructions) ->
    reduce_trans (state, store, frame, to_e_list rule_body) (next, final_store, final_frame, instructions).
  Proof.
    intros Prefix Rest; rewrite <- (firstn_skipn cutoff rule_body) at 1; rewrite to_e_list_cat.
    eapply rt_trans with (y := (middle, updated, following, to_e_list (skipn cutoff rule_body))); [|exact Rest].
    change (reduce_trans (state, store, frame, to_e_list (firstn cutoff rule_body) ++ to_e_list (skipn cutoff rule_body))
      (middle, updated, following, [] ++ to_e_list (skipn cutoff rule_body))).
    apply G.E.collector_runs_context with (prefix := []) (suffix := to_e_list (skipn cutoff rule_body))
      (instructions := to_e_list (firstn cutoff rule_body)) (instructions' := []); exact Prefix.
  Qed.

  Lemma native_thunk_encode state store locals lhs_index rhs_index witnesses_index lhs th witnesses bundle tag claim author code (strict : bool) :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Trees.Resources.memory_capacity store ->
    G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some (Thunk th) ->
    G.E.H.resolve state witnesses_index = Some (G.E.H.H.HTreeObj witnesses) ->
    get_tree_raw witnesses = [G.E.H.H.HTreeObj bundle] ->
    get_tree_raw bundle = [G.E.H.H.HTreeObj tag; G.E.H.H.HTreeObj claim] ->
    get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] -> tree_tag tag = true ->
    get_tree_raw claim = [G.E.H.H.HBlobObj code; lhs; Encode (if strict then G.E.H.H.Strict th else G.E.H.H.Shallow th)] ->
    decode_u64 (get_blob_data code) = Some 4 ->
    observed_same state (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim) ->
    observed_same (compared state) lhs lhs ->
    observed_same (compared (compared state)) (Encode (if strict then G.E.H.H.Strict th else G.E.H.H.Shallow th))
      (Encode (if strict then G.E.H.H.Strict th else G.E.H.H.Shallow th)) ->
    G.E.H.D.live_count (G.E.H.slots state) + 9 <= G.E.H.capacity ->
    exists next updated,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list (rule_arguments (if strict then 14 else 15) lhs_index rhs_index witnesses_index) ++ [AI_basic (BI_call 38%N)])
        (next, updated, Build_frame locals G.E.collector_instance, v_to_e_list [result_word 0]) /\
      scope state next 9 /\ s_funcs updated = s_funcs store /\ store_typing updated /\ Trees.Resources.memory_capacity updated.
  Proof.
    intros Functions Typed Capacity LhsLive RhsLive WitnessesLive Witnesses Bundle Tag Tagged Claim Decoded
      SameRef SameLhs SameEncoded Room.
    set (input_code := if strict then 14 else 15).
    set (encoded := Encode (if strict then G.E.H.H.Strict th else G.E.H.H.Shallow th)).
    assert (Unary : In input_code unary_codes) by (destruct strict; vm_compute; tauto).
    destruct (unary_prefix_executes state store input_code lhs_index rhs_index witnesses_index witnesses bundle tag claim author
      Unary Functions WitnessesLive Witnesses Bundle Tag Tagged ltac:(rewrite Claim; reflexivity) SameRef ltac:(lia))
      as [first [claim_slot [Prefix [ClaimLive [ClaimFresh [Scope1 Obs1]]]]]].
    set (locals1 := single_locals input_code lhs_index rhs_index witnesses_index claim_slot 0).
    assert (Rhs1 : G.E.H.resolve first rhs_index = Some (Thunk th)) by (apply (proj1 Scope1); exact RhsLive).
    destruct (Acceptance.native_map_thunk_executes first store locals1 rhs_index th strict Functions Rhs1
      ltac:(pose proof (proj2 Scope1); lia))
      as [second [copy [MapRun [CopyLive [CopyFresh [Scope2 Obs2]]]]]].
    unfold Acceptance.Support.word, Acceptance.Support.descriptor in MapRun.
    set (slots := [Some (wasm_nat claim_slot);None;None;None;None;Some (wasm_nat copy);None]).
    set (inside := scratch_locals input_code lhs_index rhs_index witnesses_index slots 0).
    assert (Claim2 : G.E.H.resolve second (wasm_nat claim_slot) = Some (G.E.H.H.HTreeObj claim))
      by (apply (proj1 Scope2); exact ClaimLive).
    assert (Lhs2 : G.E.H.resolve second lhs_index = Some lhs)
      by (apply (proj1 Scope2), (proj1 Scope1); exact LhsLive).
    assert (SameLhs2 : observed_same second lhs lhs).
    { eapply observed_comparison_observations; [|exact SameLhs]; rewrite Obs2; exact Obs1. }
    assert (SameEncoded2 : observed_same (compared second) encoded encoded).
    { eapply observed_comparison_observations; [|exact SameEncoded].
      change (G.E.H.observations second + 2 = G.E.H.observations state + 2 + 2); rewrite Obs2, Obs1; reflexivity. }
    destruct (Acceptance.Witnesses.collector_premise_executes second store inside (wasm_nat claim_slot) lhs_index (wasm_nat copy)
      claim code 4 lhs encoded Functions Typed Capacity Claim2 Lhs2 CopyLive Claim Decoded SameLhs2 SameEncoded2
      ltac:(pose proof (proj2 Scope1); pose proof (proj2 Scope2); lia))
      as [third [updated [PremiseRun [Scope3 [Obs3 [Functions3 [Typed3 Capacity3]]]]]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in PremiseRun.
    assert (Scoped : scope state third 9).
    { replace 9 with (5+1+3) by lia; eapply scope_trans; [eapply scope_trans|]; eassumption. }
    exists third, updated; split; [|auto].
    eapply rule_call_executes with (next := third) (updated := updated)
      (following := Build_frame inside G.E.collector_instance)
      (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn (if strict then 92 else 96) rule_body)));
      try exact Functions.
    eapply rule_prefix_continue; [exact Prefix|].
    unfold rule_body at 1; rewrite native_exchange_body; cbn [nth skipn].
    unfold locals1, single_locals, rule_arguments, result_word.
    destruct strict; unfold input_code in *; repeat native_pure; cbn [lfill].
    all: eapply G.E.collector_runs_context with (prefix := [])
      (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]); apply G.E.collector_runs_label.
    all: do 3 native_pure;
      execute_under [VAL_num (VAL_int64 (wasm_nat64 4)); word claim_slot; descriptor lhs_index] MapRun.
    all: execute PremiseRun; apply rt_refl.
  Qed.

  Lemma binary_prefix_executes state store code lhs rhs witnesses coupons bundle tag claim author second_witness :
    In code binary_codes -> s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state witnesses = Some (G.E.H.H.HTreeObj coupons) ->
    get_tree_raw coupons = [G.E.H.H.HTreeObj bundle; second_witness] ->
    get_tree_raw bundle = [G.E.H.H.HTreeObj tag; G.E.H.H.HTreeObj claim] ->
    get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] -> tree_tag tag = true ->
    List.length (get_tree_raw claim) = 3 ->
    observed_same state (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim) ->
    G.E.H.D.live_count (G.E.H.slots state) + 5 <= G.E.H.capacity ->
    exists next claim_slot,
      reduce_trans
        (state, store, Build_frame (initial_locals code lhs rhs witnesses) G.E.collector_instance,
          to_e_list (firstn 48 rule_body))
        (next, store, Build_frame (single_locals code lhs rhs witnesses claim_slot 0) G.E.collector_instance, []) /\
      G.E.H.resolve next (wasm_nat claim_slot) = Some (G.E.H.H.HTreeObj claim) /\
      G.E.H.resolve state (wasm_nat claim_slot) = None /\ scope state next 5 /\
      G.E.H.observations next = G.E.H.observations state + 2.
  Proof.
    intros Code Functions Live Coupons Bundle Tag Tagged Arity Same Room.
    set (initial := initial_locals code lhs rhs witnesses).
    destruct (Acceptance.Witnesses.witness_claim_executes state store initial witnesses coupons 0 bundle tag claim author
      Functions Live ltac:(rewrite Coupons; reflexivity) ltac:(lia) Bundle Tag Tagged Arity Same ltac:(lia))
      as [next [claim_slot [ClaimRun Rest]]].
    unfold Acceptance.Witnesses.Support.word, Acceptance.Witnesses.Support.descriptor in ClaimRun.
    assert (ArityRun : reduce_trans
      (state, store, Build_frame initial G.E.collector_instance,
        [AI_label 0 [] (to_e_list [BI_local_get 3%N; BI_const_num (VAL_int32 (wasm_nat 2)); BI_call 26%N])])
      (state, store, Build_frame initial G.E.collector_instance, [])).
    { eapply rt_trans; [apply G.E.collector_runs_label|apply rt_step, r_simple, rs_label_const; reflexivity].
      native_pure; exact (arity_run state store initial witnesses coupons 2 Functions Live
        ltac:(rewrite Coupons; reflexivity) ltac:(vm_compute; congruence)). }
    exists next, claim_slot; split; [|exact Rest].
    unfold rule_body; rewrite native_exchange_body; cbn [nth firstn].
    unfold initial, initial_locals, single_locals, scratch_locals, optional_descriptor, rule_arguments, result_word in ArityRun, ClaimRun |- *.
    unfold binary_codes in Code; cbn in Code.
    destruct Code as [Code|[Code|[Code|[Code|[Code|Impossible]]]]]; try contradiction; subst code.
    all: repeat native_pure.
    all: execute ArityRun.
    all: native_pure; execute ClaimRun; native_pure; apply rt_refl.
  Qed.

  Lemma binary_endpoints_executes state store code lhs rhs witnesses coupons bundle tag claim author code_blob left right second_witness :
    In code binary_codes -> s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state witnesses = Some (G.E.H.H.HTreeObj coupons) ->
    get_tree_raw coupons = [G.E.H.H.HTreeObj bundle; second_witness] ->
    get_tree_raw bundle = [G.E.H.H.HTreeObj tag; G.E.H.H.HTreeObj claim] ->
    get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] -> tree_tag tag = true ->
    get_tree_raw claim = [G.E.H.H.HBlobObj code_blob; left; right] ->
    observed_same state (G.E.H.H.HTreeRef claim) (G.E.H.H.HTreeRef claim) ->
    G.E.H.D.live_count (G.E.H.slots state) + 7 <= G.E.H.capacity ->
    exists next claim_slot left_slot right_slot,
      reduce_trans
        (state, store, Build_frame (initial_locals code lhs rhs witnesses) G.E.collector_instance,
          to_e_list (firstn 106 rule_body))
        (next, store, Build_frame (endpoint_locals code lhs rhs witnesses claim_slot left_slot right_slot) G.E.collector_instance, []) /\
      G.E.H.resolve next (wasm_nat claim_slot) = Some (G.E.H.H.HTreeObj claim) /\
      G.E.H.resolve next (wasm_nat left_slot) = Some left /\ G.E.H.resolve next (wasm_nat right_slot) = Some right /\
      scope state next 7 /\
      G.E.H.observations next = G.E.H.observations state + 2.
  Proof.
    intros Code Functions Live Coupons Bundle Tag Tagged Claim Same Room.
    destruct (binary_prefix_executes state store code lhs rhs witnesses coupons bundle tag claim author second_witness
      Code Functions Live Coupons Bundle Tag Tagged ltac:(rewrite Claim; reflexivity) Same ltac:(lia))
      as [first [claim_slot [Prefix [ClaimLive [ClaimFresh [Scope1 Obs1]]]]]].
    set (locals1 := single_locals code lhs rhs witnesses claim_slot 0).
    destruct (Acceptance.exchange_endpoint_executes first store locals1 (wasm_nat claim_slot) claim left true
      Functions ClaimLive ltac:(rewrite Claim; reflexivity) ltac:(pose proof (proj2 Scope1); lia))
      as [second [left_slot [LeftRun [LeftLive [LeftFresh [Scope2 Obs2]]]]]].
    unfold Acceptance.Support.word, Acceptance.Support.descriptor in LeftRun.
    set (locals2 := scratch_locals code lhs rhs witnesses
      [Some (wasm_nat claim_slot);None;Some (wasm_nat left_slot);None] 0).
    assert (Claim2 : G.E.H.resolve second (wasm_nat claim_slot) = Some (G.E.H.H.HTreeObj claim))
      by (apply (proj1 Scope2); exact ClaimLive).
    destruct (Acceptance.exchange_endpoint_executes second store locals2 (wasm_nat claim_slot) claim right false
      Functions Claim2 ltac:(rewrite Claim; reflexivity)
      ltac:(pose proof (proj2 Scope1); pose proof (proj2 Scope2); lia))
      as [third [right_slot [RightRun [RightLive [RightFresh [Scope3 Obs3]]]]]].
    unfold Acceptance.Support.word, Acceptance.Support.descriptor in RightRun.
    assert (Claim3 : G.E.H.resolve third (wasm_nat claim_slot) = Some (G.E.H.H.HTreeObj claim))
      by (apply (proj1 Scope3); exact Claim2).
    assert (Left3 : G.E.H.resolve third (wasm_nat left_slot) = Some left) by (apply (proj1 Scope3); exact LeftLive).
    assert (Scoped : scope state third 7).
    { replace 7 with (5+1+1) by lia; eapply scope_trans; [eapply scope_trans|]; eassumption. }
    exists third, claim_slot, left_slot, right_slot; split.
    2: { split; [exact Claim3|]; split; [exact Left3|]; split; [exact RightLive|];
         split; [exact Scoped|rewrite Obs3, Obs2, Obs1; reflexivity]. }
    change (reduce_trans
      (state, store, Build_frame (initial_locals code lhs rhs witnesses) G.E.collector_instance,
        to_e_list (firstn 48 rule_body) ++ to_e_list (firstn 58 (skipn 48 rule_body)))
      (third, store, Build_frame (endpoint_locals code lhs rhs witnesses claim_slot left_slot right_slot) G.E.collector_instance, [])).
    eapply rt_trans with (y := (first, store, Build_frame locals1 G.E.collector_instance,
      to_e_list (firstn 58 (skipn 48 rule_body)))).
    - change (reduce_trans
        (state, store, Build_frame (initial_locals code lhs rhs witnesses) G.E.collector_instance,
          to_e_list (firstn 48 rule_body) ++ to_e_list (firstn 58 (skipn 48 rule_body)))
        (first, store, Build_frame locals1 G.E.collector_instance, [] ++ to_e_list (firstn 58 (skipn 48 rule_body)))).
      apply G.E.collector_runs_context with (prefix := []) (suffix := to_e_list (firstn 58 (skipn 48 rule_body)))
        (instructions := to_e_list (firstn 48 rule_body)) (instructions' := []); exact Prefix.
    - unfold rule_body at 1; rewrite native_exchange_body; cbn [nth skipn firstn].
      unfold locals1, single_locals, scratch_locals, optional_descriptor, rule_arguments, result_word in LeftRun |- *.
      unfold binary_codes in Code; cbn in Code.
      destruct Code as [Code|[Code|[Code|[Code|[Code|Impossible]]]]]; try contradiction; subst code.
      all: repeat native_pure; execute LeftRun; native_pure.
      all: native_pure; execute RightRun; native_pure; apply rt_refl.
  Qed.

End WasmRuleSuccess.
