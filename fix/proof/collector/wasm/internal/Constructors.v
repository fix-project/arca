Module WasmConstructors.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module V := WasmValidation.
  Import V V.V V.V.Support.
  Module A := WasmAcceptance.
  Local Notation native_constructor_index := A.native_constructor_index.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.

  Definition map_guard mode :=
    match nth (3 + 4 * mode) (modfunc_body native_map) BI_unreachable with
    | BI_if _ body _ => body | _ => [] end.

  Lemma map_guard_rejects state store locals mode index :
    mode < 5 -> s_funcs store = s_funcs G.E.allocated_store ->
    reduce_trans
      (state, store, Build_frame [word mode; descriptor index] G.E.collector_instance, to_e_list (map_guard mode))
      (state, store, Build_frame [word mode; descriptor index] G.E.collector_instance, [AI_trap]) ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [word mode; descriptor index] ++ [AI_basic (BI_call 35%N)])
      (state, store, Build_frame locals G.E.collector_instance, [AI_trap]).
  Proof.
    intros Mode Functions Body.
    eapply G.E.collector_call_return with (code := native_map)
      (parameters := [T_num T_i32; T_num T_i32]) (results := [T_num T_i32])
      (defaults := []) (address := 35%N) (arguments := [word mode; descriptor index])
      (values := [descriptor index]) (final_frame := Build_frame [word mode; descriptor index] G.E.collector_instance)
      (accepted := false)
      (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn (4 * S mode) (modfunc_body native_map))));
      try reflexivity.
    - rewrite Functions; reflexivity.
    - let finish := ltac:(
        unfold native_map at 1; cbn [modfunc_body mod_funcs evaluation_module nth];
        repeat native_pure; cbn [lfill];
        eapply G.E.collector_runs_context with (prefix := [])
          (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] [AI_trap]]);
        apply G.E.collector_runs_label; exact Body) in
      do 5 (destruct mode as [|mode]; [solve [finish]|]); lia.
  Qed.

  Lemma map_value_rejects state store locals index node :
    s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state index = Some (G.E.H.H.HTreeObj node) -> tree_value node = false ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [word 6; descriptor index] ++ [AI_basic (BI_call 35%N)])
      (state, store, Build_frame locals G.E.collector_instance, [AI_trap]).
  Proof.
    intros Functions Live Value.
    set (arguments := [word 6; descriptor index]).
    eapply G.E.collector_call with (code := native_map)
      (parameters := [T_num T_i32; T_num T_i32]) (results := [T_num T_i32])
      (defaults := []) (address := 35%N) (arguments := arguments) (values := [word 0])
      (final_frame := Build_frame arguments G.E.collector_instance) (accepted := false); try reflexivity.
    - rewrite Functions; reflexivity.
    - rewrite native_map_body; do 15 native_pure.
      execute (native_type_query state store arguments 1%N index _ Functions eq_refl Live).
      do 2 native_pure.
      pose proof (A.native_map_reference_guard state store arguments index (TreeObj node)
        Functions eq_refl eq_refl Live) as Guard.
      rewrite native_map_body in Guard; cbn [firstn skipn] in Guard.
      execute Guard.
      execute (native_value_query state store arguments 1%N index _ Functions eq_refl Live).
      change (tree_value node) with (G.E.H.H.is_value (G.E.H.H.HTreeObj node)) in Value.
      rewrite Value; native_pure.
      eapply G.E.collector_trap_suffix with (instructions := [AI_trap]); apply rt_refl.
  Qed.

  Lemma map_rejects state store locals index kind input :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some input ->
    X.construct kind input = None ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [word (native_constructor_index kind); descriptor index] ++ [AI_basic (BI_call 35%N)])
      (state, store, Build_frame locals G.E.collector_instance, [AI_trap]).
  Proof.
    intros Functions Live Invalid; unfold X.construct in Invalid.
    destruct kind; destruct input as [[[blob|node]|reference]|thunk|encoded];
      cbn [X.construct native_constructor_index] in *; try discriminate.
    all: try solve [
      change ((if tree_value node then Some (G.E.H.H.HTreeRef node) else None) = None) in Invalid;
      destruct (tree_value node) eqn:Value; [discriminate|];
      eapply map_value_rejects; eassumption].
    all: first
      [ solve [
          first [eapply map_guard_rejects with (mode := 0)|eapply map_guard_rejects with (mode := 1)];
            [lia|exact Functions|];
          unfold map_guard; cbn [native_map modfunc_body mod_funcs evaluation_module nth Nat.add Nat.mul];
          native_pure;
          eapply G.E.collector_trap_suffix with
            (instructions := v_to_e_list [descriptor index] ++ [AI_basic (BI_call 24%N)]);
          exact (G.collector_tree_executes combination handle_bytes (address := index) _ _ _ _ Functions Live)]
      | solve [
          first [eapply map_guard_rejects with (mode := 2)|eapply map_guard_rejects with (mode := 3)
                |eapply map_guard_rejects with (mode := 4)]; [lia|exact Functions|];
          unfold map_guard; cbn [native_map modfunc_body mod_funcs evaluation_module nth Nat.add Nat.mul];
          lazymatch goal with
          | |- ?relation (_, _, Build_frame ?inside G.E.collector_instance, _) _ =>
            execute (native_type_query state store inside 1%N index _ Functions eq_refl Live)
          end;
          do 2 native_pure;
          eapply G.E.collector_trap_suffix with (instructions := [AI_trap]); apply rt_refl]
      | solve [
          lazymatch goal with
          | |- reduce_trans (_, _, _, v_to_e_list [word ?mode; descriptor ?slot] ++ _) _ =>
            eapply G.E.collector_call with (code := native_map)
              (parameters := [T_num T_i32; T_num T_i32]) (results := [T_num T_i32])
              (defaults := []) (address := 35%N) (arguments := [word mode; descriptor slot])
              (values := [descriptor slot]) (final_frame := Build_frame [word mode; descriptor slot] G.E.collector_instance)
              (accepted := false); try reflexivity;
            [rewrite Functions; reflexivity|];
            unfold native_map at 1; cbn [modfunc_body mod_funcs evaluation_module nth];
            do 15 native_pure;
            execute (native_type_query state store [word mode; descriptor slot] 1%N slot _ Functions eq_refl Live);
            do 2 native_pure;
            eapply G.E.collector_trap_suffix with (instructions := [AI_trap]); apply rt_refl
          end]
      | solve [
          lazymatch goal with
          | |- reduce_trans (_, _, _, v_to_e_list [word ?mode; descriptor ?slot] ++ _) _ =>
            eapply G.E.collector_call_return with (code := native_map)
              (parameters := [T_num T_i32; T_num T_i32]) (results := [T_num T_i32])
              (defaults := []) (address := 35%N) (arguments := [word mode; descriptor slot])
              (values := [descriptor slot]) (final_frame := Build_frame [word mode; descriptor slot] G.E.collector_instance)
              (accepted := false)
              (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn 28 (modfunc_body native_map))));
              try reflexivity;
            [rewrite Functions; reflexivity|];
            unfold native_map at 1; cbn [modfunc_body mod_funcs evaluation_module nth];
            do 15 native_pure;
            execute (native_type_query state store [word mode; descriptor slot] 1%N slot _ Functions eq_refl Live);
            do 5 native_pure; cbn [lfill];
            eapply G.E.collector_runs_context with (prefix := [])
              (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] [AI_trap]]);
            apply G.E.collector_runs_label;
            first [execute (native_data_type_query state store [word mode; descriptor slot] 1%N slot
                (Object (TreeObj node)) Functions eq_refl Live)
              |do 3 native_pure; execute (native_data_type_query state store [word mode; descriptor slot] 1%N slot
                (Object (BlobObj blob)) Functions eq_refl Live)];
            do 2 native_pure; apply rt_refl
          end] ].
  Qed.

  Lemma checked_map state store locals index kind input :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some input ->
    G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list [word (native_constructor_index kind); descriptor index] ++ [AI_basic (BI_call 35%N)])
      (fun final => exists next output result,
        final = (next, store, Build_frame locals G.E.collector_instance, v_to_e_list [word output]) /\
        X.construct kind input = Some result /\ G.E.H.resolve next (wasm_nat output) = Some result /\
        G.E.H.resolve state (wasm_nat output) = None /\ scope state next 1 /\
        G.E.H.observations next = G.E.H.observations state).
  Proof.
    intros Functions Live Room; destruct (X.construct kind input) as [result|] eqn:Constructed.
    - destruct (A.native_map_accepts state store locals index kind input result Functions Live Constructed Room)
        as [next [output [Run [Present [Fresh [Scoped Observations]]]]]].
      eapply checked_result; [exact Run|]; exists next, output, result.
      split; [reflexivity|]; split; [reflexivity|]; split; [exact Present|]; split; [exact Fresh|]; auto.
    - eapply checked_trap; eapply map_rejects; eassumption.
  Qed.

  Lemma checked_lower state store locals index input :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some input ->
    G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor index] ++ [AI_basic (BI_call 28%N)])
      (fun final => exists next output datum,
        final = (next, store, Build_frame locals G.E.collector_instance, v_to_e_list [descriptor output]) /\
        input = Data datum /\ G.E.H.resolve next output = Some (X.D.Context.EC.E.EP.E.lower input) /\
        scope state next 1 /\
        G.E.H.observations next = G.E.H.observations state).
  Proof.
    intros Functions Live Room; destruct input as [datum|thunk|encoded].
    { destruct (A.native_lower_executes state store locals index datum Functions Live Room)
        as [next [output [Run [Present [Scoped Observations]]]]].
      eapply checked_result; [exact Run|]; exists next, output, datum.
      split; [reflexivity|]; split; [reflexivity|]; split.
      - destruct datum as [[blob|node]|reference]; exact Present.
      -  auto. }
    all: eapply checked_trap; eapply G.E.collector_call with (code := native_lower)
      (parameters := [T_num T_i32]) (results := [T_num T_i32]) (defaults := [])
      (address := 28%N) (arguments := [descriptor index]) (values := [word 0])
      (accepted := false) (final_frame := Build_frame [descriptor index] G.E.collector_instance); try reflexivity.
    all: try solve [rewrite Functions; reflexivity].
    all: rewrite native_lower_body;
      execute (native_type_query state store [descriptor index] 0%N index _ Functions eq_refl Live);
      do 2 native_pure;
      execute (native_type_query state store [descriptor index] 0%N index _ Functions eq_refl Live);
      do 2 native_pure;
      eapply G.E.collector_trap_suffix with (instructions := [AI_trap]); apply rt_refl.
  Qed.
End WasmConstructors.
