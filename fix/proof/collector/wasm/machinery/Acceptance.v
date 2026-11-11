Module WasmAcceptance.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Support := WasmSupport.
  Import Support.
  Module Witnesses := WasmWitnesses.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Lemma exchange_endpoint_executes state store locals root claim value (left : bool) :
    s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state root = Some (treeh claim) ->
    nth_error (get_tree_raw claim) (if left then 1 else 2) = Some value ->
    G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    exists next output,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [descriptor root] ++ [AI_basic (BI_call (if left then 32%N else 33%N))])
        (next, store, Build_frame locals G.E.collector_instance, v_to_e_list [word output]) /\
      G.E.H.resolve next (wasm_nat output) = Some value /\
      G.E.H.resolve state (wasm_nat output) = None /\ scope state next 1 /\
      G.E.H.observations next = G.E.H.observations state.
  Proof.
    intros Functions Live Field Room.
    destruct (child_run state store [descriptor root] root claim (if left then 1 else 2) value
      Functions Live Field ltac:(destruct left; lia) Room)
      as [next [output [Run Rest]]].
    exists next, output; split; [|exact Rest].
    eapply G.E.collector_call with (code := exchange_endpoint left)
      (parameters := [T_num T_i32]) (results := [T_num T_i32])
      (address := if left then 32%N else 33%N) (defaults := []) (values := [word output])
      (accepted := true) (arguments := [descriptor root])
      (final_frame := Build_frame [descriptor root] G.E.collector_instance);
      try (destruct left; reflexivity).
    - rewrite Functions; destruct left; reflexivity.
    - rewrite exchange_endpoint_body; native_pure; exact Run.
  Qed.

  Lemma native_data_executes state store locals index input :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some (Data input) ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [descriptor index] ++ [AI_basic (BI_call 34%N)])
      (state, store, Build_frame locals G.E.collector_instance, []).
  Proof.
    intros Functions Live.
    eapply G.E.collector_call_void with (code := native_data)
      (parameters := [T_num T_i32]) (defaults := []) (address := 34%N)
      (arguments := [descriptor index]) (accepted := true)
      (final_frame := Build_frame [descriptor index] G.E.collector_instance); try reflexivity.
    - rewrite Functions; reflexivity.
    - rewrite native_data_body.
      execute (native_type_query state store [descriptor index] 0%N index (Data input) Functions eq_refl Live).
      destruct input; do 2 native_pure; apply rt_refl.
  Qed.

  Lemma native_map_tree_executes state store locals index tree (application : bool) :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some (treeh tree) ->
    G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    let mode := if application then 0 else 1 in
    let result := Thunk (if application then G.E.H.H.Application tree else G.E.H.H.Selection tree) in
    exists next output,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [word mode; descriptor index] ++ [AI_basic (BI_call 35%N)])
        (next, store, Build_frame locals G.E.collector_instance, v_to_e_list [word output]) /\
      G.E.H.resolve next (wasm_nat output) = Some result /\
      G.E.H.resolve state (wasm_nat output) = None /\ scope state next 1 /\
      G.E.H.observations next = G.E.H.observations state.
  Proof.
    intros Functions Live Room; cbn zeta.
    set (arguments := [word (if application then 0 else 1); descriptor index]).
    set (result := Thunk (if application then G.E.H.H.Application tree else G.E.H.H.Selection tree)).
    assert (Values : G.E.H.host_values combination G.E.collector_instance true true handle_bytes
      state store (if application then shell_application else shell_selection) [descriptor index] =
      G.E.H.allocate state result).
    { destruct application; unfold G.E.H.host_values, G.E.H.transform, descriptor;
        rewrite Live; reflexivity. }
    destruct (new_handle_run state store arguments (if application then 13%N else 14%N)
      (if application then shell_application else shell_selection) [descriptor index] result
      Functions ltac:(destruct application; reflexivity) ltac:(destruct application; reflexivity)
      ltac:(destruct application; reflexivity) ltac:(destruct application; discriminate) Values Room)
      as [next [output [Run Rest]]].
    exists next, output; split; [|exact Rest].
    eapply G.E.collector_call_return with (code := native_map)
      (parameters := [T_num T_i32; T_num T_i32]) (results := [T_num T_i32])
      (defaults := []) (address := 35%N) (arguments := arguments) (values := [word output])
      (final_frame := Build_frame arguments G.E.collector_instance) (accepted := true)
      (context := LH_rec [] 0 [] (LH_base [] [])
        (to_e_list (skipn (if application then 4 else 8) (modfunc_body native_map))));
      try (destruct application; reflexivity).
    - rewrite Functions; reflexivity.
    - rewrite native_map_body.
      destruct application; cbn [arguments] in *; [do 3 native_pure|do 6 native_pure].
      all: cbn [lfill].
      all: eapply G.E.collector_runs_context with (prefix := [])
        (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]);
        apply G.E.collector_runs_label.
      all: native_pure.
      all: execute (G.collector_tree_executes combination handle_bytes (address := index)
        state store arguments (treeh tree) Functions Live).
      all: native_pure; execute Run; apply rt_refl.
  Qed.

  Lemma native_map_thunk_executes state store locals index th (strict : bool) :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some (Thunk th) ->
    G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    let mode := if strict then 3 else 4 in
    let result := Encode (if strict then G.E.H.H.Strict th else G.E.H.H.Shallow th) in
    exists next output,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [word mode; descriptor index] ++ [AI_basic (BI_call 35%N)])
        (next, store, Build_frame locals G.E.collector_instance, v_to_e_list [word output]) /\
      G.E.H.resolve next (wasm_nat output) = Some result /\
      G.E.H.resolve state (wasm_nat output) = None /\ scope state next 1 /\
      G.E.H.observations next = G.E.H.observations state.
  Proof.
    intros Functions Live Room; cbn zeta.
    set (arguments := [word (if strict then 3 else 4); descriptor index]).
    set (result := Encode (if strict then G.E.H.H.Strict th else G.E.H.H.Shallow th)).
    assert (Values : G.E.H.host_values combination G.E.collector_instance true true handle_bytes
      state store (if strict then shell_strict else shell_shallow) [descriptor index] =
      G.E.H.allocate state result).
    { destruct strict; unfold G.E.H.host_values, G.E.H.transform, descriptor;
        rewrite Live; reflexivity. }
    destruct (new_handle_run state store arguments (if strict then 16%N else 17%N)
      (if strict then shell_strict else shell_shallow) [descriptor index] result
      Functions ltac:(destruct strict; reflexivity) ltac:(destruct strict; reflexivity)
      ltac:(destruct strict; reflexivity) ltac:(destruct strict; discriminate) Values Room)
      as [next [output [Run Rest]]].
    exists next, output; split; [|exact Rest].
    eapply G.E.collector_call_return with (code := native_map)
      (parameters := [T_num T_i32; T_num T_i32]) (results := [T_num T_i32])
      (defaults := []) (address := 35%N) (arguments := arguments) (values := [word output])
      (final_frame := Build_frame arguments G.E.collector_instance) (accepted := true)
      (context := LH_rec [] 0 [] (LH_base [] [])
        (to_e_list (skipn (if strict then 16 else 20) (modfunc_body native_map))));
      try (destruct strict; reflexivity).
    - rewrite Functions; reflexivity.
    - rewrite native_map_body.
      destruct strict; cbn [arguments] in *; [do 12 native_pure|do 15 native_pure].
      all: cbn [lfill].
      all: eapply G.E.collector_runs_context with (prefix := [])
        (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]);
        apply G.E.collector_runs_label.
      all: execute (native_type_query state store arguments 1%N index (Thunk th) Functions eq_refl Live).
      all: do 3 native_pure; execute Run; apply rt_refl.
  Qed.

  Lemma native_map_identification_executes state store locals index reference :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some (Data (Ref reference)) ->
    G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    exists next output,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [word 2; descriptor index] ++ [AI_basic (BI_call 35%N)])
        (next, store, Build_frame locals G.E.collector_instance, v_to_e_list [word output]) /\
      G.E.H.resolve next (wasm_nat output) = Some (Thunk (G.E.H.H.Identification reference)) /\
      G.E.H.resolve state (wasm_nat output) = None /\ scope state next 1 /\
      G.E.H.observations next = G.E.H.observations state.
  Proof.
    intros Functions Live Room.
    set (arguments := [word 2; descriptor index]).
    assert (Values : G.E.H.host_values combination G.E.collector_instance true true handle_bytes
      state store shell_identification [descriptor index] =
      G.E.H.allocate state (Thunk (G.E.H.H.Identification reference))).
    { unfold G.E.H.host_values, G.E.H.transform, descriptor; rewrite Live; reflexivity. }
    destruct (new_handle_run state store arguments 15%N shell_identification [descriptor index]
      (Thunk (G.E.H.H.Identification reference)) Functions eq_refl eq_refl eq_refl
      ltac:(discriminate) Values Room) as [next [output [Run Rest]]].
    exists next, output; split; [|exact Rest].
    eapply G.E.collector_call_return with (code := native_map)
      (parameters := [T_num T_i32; T_num T_i32]) (results := [T_num T_i32])
      (defaults := []) (address := 35%N) (arguments := arguments) (values := [word output])
      (final_frame := Build_frame arguments G.E.collector_instance) (accepted := true)
      (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn 12 (modfunc_body native_map))));
      try reflexivity.
    - rewrite Functions; reflexivity.
    - rewrite native_map_body.
      do 9 native_pure; cbn [lfill].
      eapply G.E.collector_runs_context with (prefix := [])
        (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]);
        apply G.E.collector_runs_label.
      execute (native_type_query state store arguments 1%N index (Data (Ref reference)) Functions eq_refl Live).
      do 3 native_pure; execute Run; apply rt_refl.
  Qed.

  Lemma native_map_reference_guard state store locals index object :
    s_funcs store = s_funcs G.E.allocated_store ->
    lookup_N locals 0%N = Some (word (match object with BlobObj _ => 5 | TreeObj _ => 6 end)) ->
    lookup_N locals 1%N = Some (descriptor index) ->
    G.E.H.resolve state index = Some (Data (Object object)) ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        to_e_list (skipn 24 (firstn 28 (modfunc_body native_map))))
      (state, store, Build_frame locals G.E.collector_instance, []).
  Proof.
    intros Functions Mode Input Live.
    rewrite native_map_body; cbn [firstn skipn].
    execute (G.E.collector_local_get combination true true handle_bytes state store locals 0%N
      (word (match object with BlobObj _ => 5 | TreeObj _ => 6 end)) Mode).
    destruct object as [blob|tree]; native_pure.
    - apply G.E.collector_if_void with (condition := true) (accepted := true).
      execute (native_data_type_query state store locals 1%N index (Object (BlobObj blob)) Functions Input Live).
      do 2 native_pure; apply rt_refl.
    - apply G.E.collector_if_void with (condition := false) (accepted := true).
      execute (G.E.collector_local_get combination true true handle_bytes state store locals 0%N (word 6) Mode).
      do 2 native_pure.
      execute (native_data_type_query state store locals 1%N index (Object (TreeObj tree)) Functions Input Live).
      do 2 native_pure; apply rt_refl.
  Qed.

  Lemma native_map_reference_executes state store locals index object :
    s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state index = Some (Data (Object object)) ->
    G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    exists next output,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [word (match object with BlobObj _ => 5 | TreeObj _ => 6 end); descriptor index] ++
            [AI_basic (BI_call 35%N)])
        (next, store, Build_frame locals G.E.collector_instance, v_to_e_list [word output]) /\
      G.E.H.resolve next (wasm_nat output) = Some (Data (Ref (G.E.H.H.data_reference (Object object)))) /\
      G.E.H.resolve state (wasm_nat output) = None /\ scope state next 1 /\
      G.E.H.observations next = G.E.H.observations state.
  Proof.
    intros Functions Live Room.
    set (arguments := [word (match object with BlobObj _ => 5 | TreeObj _ => 6 end); descriptor index]).
    assert (Values : G.E.H.host_values combination G.E.collector_instance true true handle_bytes
      state store shell_create_ref [descriptor index] =
      G.E.H.allocate state (Data (Ref (G.E.H.H.data_reference (Object object))))).
    { unfold G.E.H.host_values, G.E.H.transform, descriptor; rewrite Live; reflexivity. }
    destruct (new_handle_run state store arguments 11%N shell_create_ref [descriptor index]
      (Data (Ref (G.E.H.H.data_reference (Object object)))) Functions eq_refl eq_refl eq_refl
      ltac:(discriminate) Values Room) as [next [output [Run Rest]]].
    exists next, output; split; [|exact Rest].
    eapply G.E.collector_call with (code := native_map)
      (parameters := [T_num T_i32; T_num T_i32]) (results := [T_num T_i32])
      (defaults := []) (address := 35%N) (arguments := arguments) (values := [word output])
      (final_frame := Build_frame arguments G.E.collector_instance) (accepted := true); try reflexivity.
    - rewrite Functions; reflexivity.
    - rewrite native_map_body.
      destruct object as [blob|tree]; cbn [arguments] in *.
      all: do 15 native_pure.
      all: execute (native_type_query state store arguments 1%N index _ Functions eq_refl Live).
      all: do 2 native_pure.
      + pose proof (native_map_reference_guard state store arguments index (BlobObj blob)
          Functions eq_refl eq_refl Live) as Guard.
        rewrite native_map_body in Guard; cbn [firstn skipn] in Guard.
        execute Guard; native_pure; execute Run; apply rt_refl.
      + pose proof (native_map_reference_guard state store arguments index (TreeObj tree)
          Functions eq_refl eq_refl Live) as Guard.
        rewrite native_map_body in Guard; cbn [firstn skipn] in Guard.
        execute Guard; native_pure; execute Run; apply rt_refl.
  Qed.

  Definition native_constructor_index kind := match kind with
    | X.Application => 0 | X.Selection => 1 | X.Identification => 2
    | X.Strict => 3 | X.Shallow => 4 | X.BlobReference => 5 | X.TreeReference => 6 end.

  Lemma native_map_accepts state store locals index kind input result :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some input ->
    X.construct kind input = Some result -> G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    exists next output,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [word (native_constructor_index kind); descriptor index] ++ [AI_basic (BI_call 35%N)])
        (next, store, Build_frame locals G.E.collector_instance, v_to_e_list [word output]) /\
      G.E.H.resolve next (wasm_nat output) = Some result /\
      G.E.H.resolve state (wasm_nat output) = None /\ scope state next 1 /\
      G.E.H.observations next = G.E.H.observations state.
  Proof.
    intros Functions Live Checked Room.
    destruct kind; destruct input as [[[blob|tree]|reference]|th|encoded];
      cbn [X.construct] in Checked; try discriminate; inversion Checked; subst result;
      cbn [native_constructor_index].
    - exact (native_map_tree_executes state store locals index tree true Functions Live Room).
    - exact (native_map_tree_executes state store locals index tree false Functions Live Room).
    - exact (native_map_identification_executes state store locals index reference Functions Live Room).
    - exact (native_map_thunk_executes state store locals index th true Functions Live Room).
    - exact (native_map_thunk_executes state store locals index th false Functions Live Room).
    - exact (native_map_reference_executes state store locals index (BlobObj blob) Functions Live Room).
    - exact (native_map_reference_executes state store locals index (TreeObj tree) Functions Live Room).
  Qed.

  Lemma native_lower_executes state store locals index input :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some (Data input) ->
    G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    exists next output,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [descriptor index] ++ [AI_basic (BI_call 28%N)])
        (next, store, Build_frame locals G.E.collector_instance, v_to_e_list [descriptor output]) /\
      G.E.H.resolve next output = Some (Data (Ref (G.E.H.H.data_reference input))) /\
      scope state next 1 /\ G.E.H.observations next = G.E.H.observations state.
  Proof.
    intros Functions Live Room; set (arguments := [descriptor index]); destruct input as [object|reference].
    - assert (Values : G.E.H.host_values combination G.E.collector_instance true true handle_bytes
        state store shell_create_ref [descriptor index] =
        G.E.H.allocate state (Data (Ref (G.E.H.H.data_reference (Object object))))).
      { unfold G.E.H.host_values, G.E.H.transform, descriptor; rewrite Live; reflexivity. }
      destruct (new_handle_run state store arguments 11%N shell_create_ref [descriptor index]
        (Data (Ref (G.E.H.H.data_reference (Object object)))) Functions eq_refl eq_refl eq_refl
        ltac:(discriminate) Values Room)
        as [next [output [Run [Present [Fresh [Scoped Observations]]]]]].
      exists next, (wasm_nat output); split; [|auto].
      eapply G.E.collector_call_return with (code := native_lower)
        (parameters := [T_num T_i32]) (results := [T_num T_i32])
        (defaults := []) (address := 28%N) (arguments := arguments) (values := [word output])
        (final_frame := Build_frame arguments G.E.collector_instance) (accepted := true)
        (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn 4 (modfunc_body native_lower))));
        try reflexivity.
      + rewrite Functions; reflexivity.
      + rewrite native_lower_body.
        execute (native_type_query state store arguments 0%N index (Data (Object object)) Functions eq_refl Live).
        do 2 native_pure; cbn [lfill].
        eapply G.E.collector_runs_context with (prefix := [])
          (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]);
          apply G.E.collector_runs_label.
        native_pure; execute Run; apply rt_refl.
    - exists state, index; split.
      + eapply G.E.collector_call with (code := native_lower)
          (parameters := [T_num T_i32]) (results := [T_num T_i32])
          (defaults := []) (address := 28%N) (arguments := arguments) (values := [descriptor index])
          (final_frame := Build_frame arguments G.E.collector_instance) (accepted := true); try reflexivity.
        * rewrite Functions; reflexivity.
        * rewrite native_lower_body.
          execute (native_type_query state store arguments 0%N index (Data (Ref reference)) Functions eq_refl Live).
          do 2 native_pure.
          execute (native_type_query state store arguments 0%N index (Data (Ref reference)) Functions eq_refl Live).
          do 3 native_pure; apply rt_refl.
      + split; [exact Live|]; split; [eapply scope_weaken; [apply scope_refl|lia]|reflexivity].
  Qed.

  Lemma native_mapped_accepts state store locals kind lhs_index rhs_index claim_index lhs rhs claim code a b :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Witnesses.Resources.memory_capacity store ->
    G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state claim_index = Some (treeh claim) ->
    get_tree_raw claim = [G.E.H.H.HBlobObj code; a; b] -> decode_u64 (get_blob_data code) = Some 0 ->
    X.construct kind a = Some lhs -> X.construct kind b = Some rhs ->
    observed_same state lhs lhs -> observed_same (compared state) rhs rhs ->
    observed_same (compared (compared state)) a a ->
    observed_same (compared (compared (compared state))) b b ->
    G.E.H.D.live_count (G.E.H.slots state) + 7 <= G.E.H.capacity ->
    exists next updated,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [word (native_constructor_index kind); descriptor lhs_index; descriptor rhs_index;
            descriptor claim_index] ++ [AI_basic (BI_call 36%N)])
        (next, updated, Build_frame locals G.E.collector_instance, []) /\
      scope state next 7 /\ G.E.H.observations next = G.E.H.observations state + 8 /\
      s_funcs updated = s_funcs store /\ store_typing updated /\ Witnesses.Resources.memory_capacity updated.
  Proof.
    intros Functions Typed Capacity LhsLive RhsLive ClaimLive Fields Decoded ConstructA ConstructB
      SameLhs SameRhs SameA SameB Room.
    set (arguments := [word (native_constructor_index kind); descriptor lhs_index;
      descriptor rhs_index; descriptor claim_index]).
    set (initial := arguments ++ [word 0; word 0]).
    destruct (exchange_endpoint_executes state store initial claim_index claim a true Functions ClaimLive
      ltac:(rewrite Fields; reflexivity) ltac:(lia))
      as [first [a_slot [ARun [ALive [AFresh [Scope1 Obs1]]]]]].
    pose proof (proj2 Scope1) as Count1.
    set (locals1 := arguments ++ [word a_slot; word 0]).
    assert (Claim1 : G.E.H.resolve first claim_index = Some (treeh claim)) by (apply (proj1 Scope1); exact ClaimLive).
    destruct (exchange_endpoint_executes first store locals1 claim_index claim b false Functions Claim1
      ltac:(rewrite Fields; reflexivity) ltac:(lia))
      as [second [b_slot [BRun [BLive [BFresh [Scope2 Obs2]]]]]].
    pose proof (proj2 Scope2) as Count2.
    set (locals2 := arguments ++ [word a_slot; word b_slot]).
    assert (A2 : G.E.H.resolve second (wasm_nat a_slot) = Some a) by (apply (proj1 Scope2); exact ALive).
    assert (Scope02 : scope state second 2).
    { replace 2 with (1+1) by lia; eapply scope_trans; eassumption. }
    destruct (native_map_accepts second store locals2 (wasm_nat a_slot) kind a lhs Functions A2 ConstructA ltac:(lia))
      as [third [left_copy [MapA [MapALive [MapAFresh [Scope3 Obs3]]]]]].
    set (locals3 := locals2).
    assert (Scope03 : scope state third 3).
    { replace 3 with (2+1) by lia; eapply scope_trans; eassumption. }
    assert (Lhs3 : G.E.H.resolve third lhs_index = Some lhs) by (apply (proj1 Scope03); exact LhsLive).
    assert (SameLhs3 : observed_same third lhs lhs).
    { eapply observed_comparison_observations; [rewrite Obs3, Obs2, Obs1; reflexivity|exact SameLhs]. }
    pose proof (same_run third store locals3 lhs_index (wasm_nat left_copy) lhs lhs true
      Functions Lhs3 MapALive SameLhs3) as CompareA.
    assert (Scope3Compared : scope second (compared third) 1) by exact Scope3.
    pose (fourth := compared third).
    assert (Scope4 : scope second fourth 1) by exact Scope3.
    assert (Obs4 : G.E.H.observations fourth = G.E.H.observations third + 2) by reflexivity.
    pose proof (proj2 Scope4) as Count4.
    assert (B4 : G.E.H.resolve fourth (wasm_nat b_slot) = Some b) by (apply (proj1 Scope4); exact BLive).
    destruct (native_map_accepts fourth store locals3 (wasm_nat b_slot) kind b rhs Functions B4 ConstructB ltac:(lia))
      as [fifth [right_copy [MapB [MapBLive [MapBFresh [Scope5 Obs5]]]]]].
    set (inside := locals2).
    assert (Scope25 : scope second fifth 2).
    { replace 2 with (1+1) by lia; eapply scope_trans; eassumption. }
    assert (Rhs5 : G.E.H.resolve fifth rhs_index = Some rhs).
    { apply (proj1 Scope25), (proj1 Scope02); exact RhsLive. }
    assert (SameRhs5 : observed_same fifth rhs rhs).
    { eapply observed_comparison_observations; [|exact SameRhs].
      rewrite Obs5, Obs4; change (G.E.H.observations third + 2 = G.E.H.observations state + 2).
      rewrite Obs3, Obs2, Obs1; reflexivity. }
    pose proof (same_run fifth store inside rhs_index (wasm_nat right_copy) rhs rhs true
      Functions Rhs5 MapBLive SameRhs5) as CompareB.
    assert (Scope5Compared : scope fourth (compared fifth) 1) by exact Scope5.
    pose (sixth := compared fifth).
    assert (Scope6 : scope fourth sixth 1) by exact Scope5.
    assert (Obs6 : G.E.H.observations sixth = G.E.H.observations fifth + 2) by reflexivity.
    assert (Scope26 : scope second sixth 2).
    { replace 2 with (1+1) by lia; eapply scope_trans; eassumption. }
    assert (Scope06 : scope state sixth 4).
    { replace 4 with (2+2) by lia; eapply scope_trans; eassumption. }
    pose proof (proj2 Scope06) as Count6.
    assert (Claim6 : G.E.H.resolve sixth claim_index = Some (treeh claim))
      by (apply (proj1 Scope06); exact ClaimLive).
    assert (A6 : G.E.H.resolve sixth (wasm_nat a_slot) = Some a) by (apply (proj1 Scope26); exact A2).
    assert (B6 : G.E.H.resolve sixth (wasm_nat b_slot) = Some b) by (apply (proj1 Scope26); exact BLive).
    assert (ObsSix : G.E.H.observations sixth = G.E.H.observations state + 4).
    { rewrite Obs6; change (G.E.H.observations fifth + 2 = G.E.H.observations state + 4).
      rewrite Obs5, Obs4; change (G.E.H.observations third + 2 + 2 = G.E.H.observations state + 4).
      rewrite Obs3, Obs2, Obs1; lia. }
    assert (SameA6 : observed_same sixth a a).
    { eapply observed_comparison_observations; [|exact SameA].
      change (G.E.H.observations sixth = G.E.H.observations state + 2 + 2).
      rewrite ObsSix; lia. }
    assert (SameB6 : observed_same (compared sixth) b b).
    { eapply observed_comparison_observations; [|exact SameB].
      change (G.E.H.observations sixth + 2 = G.E.H.observations state + 2 + 2 + 2).
      rewrite ObsSix; lia. }
    destruct (Witnesses.collector_premise_executes sixth store inside claim_index (wasm_nat a_slot)
      (wasm_nat b_slot) claim code 0 a b Functions Typed Capacity Claim6 A6 B6 Fields Decoded SameA6 SameB6 ltac:(lia))
      as [seventh [updated [PremiseRun [Scope7 [Obs7 [Functions7 [Typed7 Capacity7]]]]]]].
    assert (FunctionsNext : s_funcs updated = s_funcs G.E.allocated_store) by (rewrite Functions7; exact Functions).
    assert (Scope27 : scope second seventh 5).
    { replace 5 with (2+3) by lia; eapply scope_trans; eassumption. }
    assert (Scope07 : scope state seventh 7).
    { replace 7 with (2+5) by lia; eapply scope_trans; eassumption. }
    exists seventh, updated; split.
    2: { split; [exact Scope07|]; split; [rewrite Obs7, ObsSix; lia|auto]. }
    eapply G.E.collector_call_void with (code := native_mapped)
      (parameters := [T_num T_i32; T_num T_i32; T_num T_i32; T_num T_i32])
      (defaults := [word 0; word 0]) (address := 36%N) (arguments := arguments)
      (accepted := true) (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
    - rewrite Functions; reflexivity.
    - change (reduce_trans (state, store, Build_frame initial G.E.collector_instance,
        to_e_list (modfunc_body native_mapped)) (seventh, updated, Build_frame inside G.E.collector_instance, [])).
      rewrite native_mapped_body.
      native_pure; execute ARun; native_pure.
      native_pure; execute BRun; native_pure.
      do 3 native_pure; execute_under [descriptor lhs_index] MapA; execute CompareA.
      do 3 native_pure; execute_under [descriptor rhs_index] MapB; execute CompareB.
      do 3 native_pure; exact PremiseRun.
  Qed.
End WasmAcceptance.
