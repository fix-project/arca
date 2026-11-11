  Lemma local_set_run state store locals index value :
    (N.to_nat index < List.length locals)%nat ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        [v_to_e value; AI_basic (BI_local_set index)])
      (state, store, Build_frame (seq.set_nth value locals (N.to_nat index) value)
        G.E.collector_instance, []).
  Proof.
    intro Bound; apply rt_step; eapply r_local_set with (vd := value);
      [reflexivity|apply (ssrbool.introT ssrnat.ltP); exact Bound|reflexivity].
  Qed.

  Lemma local_tee_run state store locals index value :
    (N.to_nat index < List.length locals)%nat ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        [v_to_e value; AI_basic (BI_local_tee index)])
      (state, store, Build_frame (seq.set_nth value locals (N.to_nat index) value) G.E.collector_instance,
        v_to_e_list [value]).
  Proof.
    intro Bound; eapply rt_trans with (y := (state, store, Build_frame locals G.E.collector_instance,
      [v_to_e value; v_to_e value; AI_basic (BI_local_set index)])).
    - apply rt_step, r_simple, rs_local_tee.
    - change (reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [value] ++ [v_to_e value; AI_basic (BI_local_set index)] ++ [])
        (state, store, Build_frame (seq.set_nth value locals (N.to_nat index) value) G.E.collector_instance,
          v_to_e_list [value] ++ [] ++ [])).
      apply G.E.collector_runs_context with (prefix := [value]) (suffix := [])
        (instructions := [v_to_e value; AI_basic (BI_local_set index)]) (instructions' := []).
      exact (local_set_run state store locals index value Bound).
  Qed.

  Lemma native_relop state store locals lhs rhs ty op :
    relop_typecheck lhs rhs ty op = true ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num lhs; VAL_num rhs] ++ [AI_basic (BI_relop ty op)])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool (app_relop op lhs rhs)))]).
  Proof. intro Typed; apply rt_step, r_simple, rs_relop; exact Typed. Qed.

  Lemma equal64_run state store locals value :
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int64 value); VAL_num (VAL_int64 value)] ++
          [AI_basic (BI_relop T_i64 (Relop_i ROI_eq))])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool true))]).
  Proof.
    pose proof (native_relop state store locals (VAL_int64 value) (VAL_int64 value)
      T_i64 (Relop_i ROI_eq) eq_refl) as Run.
    change (reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int64 value); VAL_num (VAL_int64 value)] ++
          [AI_basic (BI_relop T_i64 (Relop_i ROI_eq))])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Wasm_int.Int64.eq value value)))])) in Run.
    rewrite Wasm_int.Int64.eq_true in Run; exact Run.
  Qed.

  (** A native integer binary instruction computes the modeled arithmetic result. *)
  Lemma native_binop state store locals lhs rhs ty op result :
    binop_typecheck lhs rhs ty op = true -> app_binop op lhs rhs = Some result ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num lhs; VAL_num rhs] ++ [AI_basic (BI_binop ty op)])
      (state, store, Build_frame locals G.E.collector_instance, v_to_e_list [VAL_num result]).
  Proof. intros Typed Result; apply rt_step, r_simple; eapply rs_binop_success; eassumption. Qed.

  (** A native i32 test computes the modeled Boolean result. *)
  Lemma native_test32 state store locals operand op :
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 operand)] ++ [AI_basic (BI_testop T_i32 op)])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool (app_testop_i i32m op operand)))]).
  Proof. apply rt_step, r_simple, rs_testop_i32. Qed.

  (** A successful value-returning Shell import executes without changing the store. *)

  Lemma native_value_import state store next locals index operation arguments values :
    s_funcs store = s_funcs G.E.allocated_store ->
    lookup_N (s_funcs G.E.allocated_store) index =
      Some (FC_func_host (shell_signature operation) operation) ->
    lookup_N (inst_funcs G.E.collector_instance) index = Some index ->
    (match shell_signature operation with Tf parameters _ =>
      List.length arguments = List.length parameters end) ->
    operation <> shell_read_blob ->
    G.E.H.host_values combination G.E.collector_instance true true handle_bytes state store
      operation arguments = Some (next, values) ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list arguments ++ [AI_basic (BI_call index)])
      (next, store, Build_frame locals G.E.collector_instance, v_to_e_list values).
  Proof.
    intros Functions Closure Index Arity Other Values.
    eapply G.E.collector_import_executes with (operation := operation) (address := index)
      (result := result_values values); try eassumption.
    - rewrite Functions; exact Closure.
    - apply G.E.H.host_values_result; assumption.
  Qed.

  Lemma new_handle_run state store locals index operation arguments value :
    s_funcs store = s_funcs G.E.allocated_store ->
    lookup_N (s_funcs G.E.allocated_store) index =
      Some (FC_func_host (shell_signature operation) operation) ->
    lookup_N (inst_funcs G.E.collector_instance) index = Some index ->
    (match shell_signature operation with Tf parameters _ =>
      List.length arguments = List.length parameters end) ->
    operation <> shell_read_blob ->
    G.E.H.host_values combination G.E.collector_instance true true handle_bytes state store operation arguments =
      G.E.H.allocate state value ->
    G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    exists next output,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list arguments ++ [AI_basic (BI_call index)])
        (next, store, Build_frame locals G.E.collector_instance, v_to_e_list [word output]) /\
      G.E.H.resolve next (wasm_nat output) = Some value /\
      G.E.H.resolve state (wasm_nat output) = None /\ scope state next 1 /\
      G.E.H.observations next = G.E.H.observations state.
  Proof.
    intros Functions Closure Index Arity Other Values Room.
    destruct (G.E.H.allocate_available state value Room) as [next [values Allocated]].
    destruct (allocate_scope _ _ _ _ Allocated) as [output [ValuesOut [_ [Fresh [Present Scoped]]]]].
    assert (Observations : G.E.H.observations next = G.E.H.observations state).
    { destruct (G.E.H.allocate_resolves _ _ _ _ Allocated)
        as [i [_ [_ [_ [_ [_ Same]]]]]]; exact Same. }
    subst values; exists next, output; split.
    - eapply native_value_import; try eassumption; rewrite Values; exact Allocated.
    - auto.
  Qed.

  Lemma child_run state store locals root tree index child :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state root = Some (treeh tree) ->
    nth_error (get_tree_raw tree) index = Some child -> (Z.of_nat index < 2 ^ 32)%Z ->
    G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    exists next output,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [descriptor root; word index] ++ [AI_basic (BI_call 7%N)])
        (next, store, Build_frame locals G.E.collector_instance, v_to_e_list [word output]) /\
      G.E.H.resolve next (wasm_nat output) = Some child /\
      G.E.H.resolve state (wasm_nat output) = None /\ scope state next 1 /\
      G.E.H.observations next = G.E.H.observations state.
  Proof.
    intros Functions Live Field Bound Room.
    eapply new_handle_run with (operation := shell_get_tree_element); try eassumption; try reflexivity;
      [discriminate|].
    unfold G.E.H.host_values, G.E.H.transform, descriptor, word.
    rewrite Live; cbn [G.E.H.H.obind G.E.H.Api.get_tree_data_api G.E.H.H.HTreeObj].
    rewrite wasm_nat_to_nat by exact Bound; rewrite Field; reflexivity.
  Qed.

  Lemma native_type_query state store locals local index input :
    s_funcs store = s_funcs G.E.allocated_store ->
    lookup_N locals local = Some (descriptor index) ->
    G.E.H.resolve state index = Some input ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        to_e_list [BI_local_get local; BI_call 0%N])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [word (G.E.H.H.value_type input)]).
  Proof.
    intros Functions Local Live.
    eapply G.collector_query_local with (address := index) (operation := shell_get_type);
      try exact Local; try reflexivity.
    - rewrite Functions; reflexivity.
    - unfold G.E.H.host_values, G.descriptor_value; rewrite Live; reflexivity.
  Qed.

  Lemma native_data_type_query state store locals local index input :
    s_funcs store = s_funcs G.E.allocated_store ->
    lookup_N locals local = Some (descriptor index) -> G.E.H.resolve state index = Some (Data input) ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        to_e_list [BI_local_get local; BI_call 1%N])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [word (match input with Object (TreeObj _) | Ref (TreeRef _) => 1 | _ => 0 end)]).
  Proof.
    intros Functions Local Live.
    eapply G.collector_query_local with (address := index) (operation := shell_get_data_type);
      try exact Local; try reflexivity.
    - rewrite Functions; reflexivity.
    - unfold G.E.H.host_values, G.descriptor_value; rewrite Live; destruct input as [[b|t]|[b|t]]; reflexivity.
  Qed.

  Lemma native_require state store locals (condition : bool) :
    s_funcs store = s_funcs G.E.allocated_store ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool condition))] ++ [AI_basic (BI_call 22%N)])
      (state, store, Build_frame locals G.E.collector_instance, if condition then [] else [AI_trap]).
  Proof. intro Functions; apply G.E.collector_require_executes; rewrite Functions; reflexivity. Qed.

  Ltac execute_under stack run := collector_execute_under G.E.collector_runs_context stack run.
  Ltac execute run := execute_under (@nil value) run.

  Ltac local_value locals index :=
    let found := eval hnf in (lookup_N locals index) in
    lazymatch found with
    | Some ?entry => let entry := eval hnf in entry in constr:(Some entry)
    | None => constr:(@None value)
    end.

  Lemma drop_run state store locals value :
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance, [v_to_e value; AI_basic BI_drop])
      (state, store, Build_frame locals G.E.collector_instance, []).
  Proof. apply rt_step, r_simple, rs_drop. Qed.

  Ltac native_pure_at state store locals stack instruction :=
    lazymatch instruction with
    | BI_local_get ?index =>
        let found := eval hnf in (lookup_N locals index) in
        lazymatch found with
        | Some ?value => execute_under stack (local_get_run state store locals index value eq_refl)
        | _ => fail "cannot resolve local" index found
        end
    | BI_local_set ?index =>
        let prefix := eval cbv beta iota zeta delta [firstn List.length List.app Nat.sub] in (firstn (List.length stack - 1) stack) in
        let found := eval cbv beta iota zeta delta [nth_error List.length List.app Nat.sub] in (nth_error stack (List.length stack - 1)) in
        lazymatch found with
        | Some ?value => execute_under prefix
            (local_set_run state store locals index value ltac:(lazy; lia))
        end
    | BI_drop =>
        let prefix := eval cbv beta iota zeta delta [firstn List.length List.app Nat.sub] in (firstn (List.length stack - 1) stack) in
        let found := eval cbv beta iota zeta delta [nth_error List.length List.app Nat.sub] in (nth_error stack (List.length stack - 1)) in
        lazymatch found with
        | Some ?value => execute_under prefix (drop_run state store locals value)
        end
    | BI_local_tee ?index =>
        let prefix := eval cbv beta iota zeta delta [firstn List.length List.app Nat.sub] in (firstn (List.length stack - 1) stack) in
        let found := eval cbv beta iota zeta delta [nth_error List.length List.app Nat.sub] in (nth_error stack (List.length stack - 1)) in
        lazymatch found with
        | Some ?value => execute_under prefix (local_tee_run state store locals index value ltac:(lazy; lia))
        end
    | BI_relop ?ty ?op =>
        let prefix := eval cbv beta iota zeta delta [firstn List.length List.app Nat.sub] in (firstn (List.length stack - 2) stack) in
        let pair := eval cbv beta iota zeta delta [skipn List.length List.app Nat.sub] in (skipn (List.length stack - 2) stack) in
        lazymatch pair with
        | [VAL_num ?lhs; VAL_num ?rhs] => execute_under prefix
            (native_relop state store locals lhs rhs ty op ltac:(reflexivity))
        | _ => fail "relop stack does not have two numeric operands" pair
        end
    | BI_binop ?ty ?op =>
        let prefix := eval cbv beta iota zeta delta [firstn List.length List.app Nat.sub] in (firstn (List.length stack - 2) stack) in
        let pair := eval cbv beta iota zeta delta [skipn List.length List.app Nat.sub] in (skipn (List.length stack - 2) stack) in
        lazymatch pair with
        | [VAL_num ?lhs; VAL_num ?rhs] =>
            let computed := eval vm_compute in (app_binop op lhs rhs) in
            lazymatch computed with
            | Some ?result => execute_under prefix
                (native_binop state store locals lhs rhs ty op result ltac:(reflexivity) eq_refl)
            end
        end
    | BI_testop T_i32 ?op =>
        let prefix := eval cbv beta iota zeta delta [firstn List.length List.app Nat.sub] in (firstn (List.length stack - 1) stack) in
        let last := eval cbv beta iota zeta delta [nth_error List.length List.app Nat.sub] in (nth_error stack (List.length stack - 1)) in
        lazymatch last with
        | Some (VAL_num (VAL_int32 ?operand)) => execute_under prefix
            (native_test32 state store locals operand op)
        end
    | BI_call 22%N =>
        let prefix := eval cbv beta iota zeta delta [firstn List.length List.app Nat.sub] in (firstn (List.length stack - 1) stack) in
        let last := eval cbv beta iota zeta delta [nth_error List.length List.app Nat.sub] in (nth_error stack (List.length stack - 1)) in
        lazymatch last with
        | Some (VAL_num (VAL_int32 ?operand)) =>
            let zero := eval vm_compute in (Wasm_int.int_eq i32m operand (wasm_nat 0)) in
            let condition := eval cbn in (negb zero) in
            lazymatch goal with
            | Functions : s_funcs store = s_funcs G.E.allocated_store |- _ =>
                execute_under prefix (native_require state store locals condition Functions)
            end
        end
    | BI_if (BT_valtype None) ?yes ?no =>
        let prefix := eval cbv beta iota zeta delta [firstn List.length List.app Nat.sub] in (firstn (List.length stack - 1) stack) in
        let last := eval cbv beta iota zeta delta [nth_error List.length List.app Nat.sub] in (nth_error stack (List.length stack - 1)) in
        lazymatch last with
        | Some (VAL_num (VAL_int32 ?operand)) =>
            let zero := eval vm_compute in (Wasm_int.int_eq i32m operand (wasm_nat 0)) in
            lazymatch zero with
            | true => lazymatch no with
                | [] => execute_under prefix
                    (G.E.collector_if_skip combination true true handle_bytes state store
                      (Build_frame locals G.E.collector_instance) yes)
                | _ => execute_under prefix
                    (G.E.collector_if_enter combination true true handle_bytes state store
                      (Build_frame locals G.E.collector_instance) false yes no)
                end
            | false => execute_under prefix
                (G.E.collector_if_enter combination true true handle_bytes state store
                  (Build_frame locals G.E.collector_instance) true yes no)
            end
        end
    | _ => fail "not a pure instruction" instruction
    end.

  Ltac native_pure_scan state store locals stack instructions :=
    lazymatch instructions with
    | ?head :: ?tail =>
        let head := eval hnf in head in
        let converted := eval cbn [e_to_v_opt e_to_vref_opt option_map v_to_e vref_to_e word descriptor G.E.H.H.value_type] in (e_to_v_opt head) in
        lazymatch converted with
        | Some ?value =>
            let stack := eval cbn [List.app] in (stack ++ [value]) in
            native_pure_scan state store locals stack tail
        | None => lazymatch head with
            | AI_basic ?instruction => native_pure_at state store locals stack instruction
            | _ => fail "not a basic instruction" head
            end
        | _ => fail "value conversion did not simplify" converted
        end
    | _ => fail "no next instruction" instructions
    end.

  Ltac native_pure :=
    try fold reduce_trans;
    lazymatch goal with
    | |- ?relation (?state, ?store, Build_frame ?locals G.E.collector_instance, ?instructions) _ =>
        let instructions := eval cbn [to_e_list v_to_e_list List.app seq.map v_to_e vref_to_e descriptor G.E.H.H.HTreeObj G.E.H.H.HBlobObj] in instructions in
        native_pure_scan state store locals (@nil value) instructions
    | _ => fail "not a reduction goal with an explicit frame"
    end.


  Lemma native_length_query state store locals local index input :
    s_funcs store = s_funcs G.E.allocated_store -> lookup_N locals local = Some (descriptor index) ->
    G.E.H.resolve state index = Some (Data input) ->
    (Z.of_nat (G.E.H.T.O.data_length input) < 2 ^ 64)%Z ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        to_e_list [BI_local_get local; BI_call 6%N])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 (G.E.H.T.O.data_length input))) ]).
  Proof.
    intros Functions Local Live Bound.
    eapply G.collector_query_local with (address := index) (operation := shell_len);
      try exact Local; try reflexivity.
    - rewrite Functions; reflexivity.
    - unfold G.E.H.host_values, G.descriptor_value; rewrite Live.
      cbn [G.E.H.H.obind]; unfold G.E.H.length_result.
      assert (Fits : Z.ltb (Z.of_nat (G.E.H.T.O.data_length input)) (2 ^ 64) = true)
        by (apply Z.ltb_lt; exact Bound).
      rewrite Fits; reflexivity.
  Qed.

  Lemma read_blob_run state store locals index next count :
    s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.blob_read_result G.E.collector_instance state store
      [descriptor index; word 0; word 0; word 8] = Some (next, [word count]) ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [descriptor index; word 0; word 0; word 8] ++ [AI_basic (BI_call 8%N)])
      (state, next, Build_frame locals G.E.collector_instance, v_to_e_list [word count]).
  Proof.
    intros Functions Read; eapply G.E.collector_import_executes with (operation := shell_read_blob)
      (address := 8%N) (result := result_values [word count]); try reflexivity.
    - rewrite Functions; reflexivity.
    - unfold G.E.H.host_result, G.E.H.candidate.
      destruct (function_type_eq_dec _ _); [|contradiction].
      destruct (shell_function_eq_dec shell_read_blob shell_read_blob); [|contradiction].
      rewrite Read; reflexivity.
  Qed.

  Lemma collector_integer_body_executes state store index blob number next page :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some (blobh blob) ->
    G.E.H.blob_read_result G.E.collector_instance state store
      [descriptor index; word 0; word 0; word 8] = Some (next, [word 8]) ->
    s_mems next = [page] -> load page 0%N 0%N 8%N = Some (get_blob_data blob) ->
    decode_u64 (get_blob_data blob) = Some number ->
    reduce_trans
      (state, store, Build_frame [descriptor index] G.E.collector_instance,
        to_e_list (modfunc_body evaluation_integer))
      (state, next, Build_frame [descriptor index] G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 number))]).
  Proof.
    intros Functions Live Read Memory Loaded Decoded.
    assert (Width : List.length (get_blob_data blob) = 8) by (eapply decode_u64_width; exact Decoded).
    assert (Length : G.E.H.T.O.data_length (Object (BlobObj blob)) = 8) by exact Width.
    assert (FunctionsNext : s_funcs next = s_funcs G.E.allocated_store).
    { rewrite (G.E.H.blob_read_result_functions _ _ _ _ _ _ Read); exact Functions. }
    set (locals := [descriptor index]).
    rewrite evaluation_integer_body.
    execute (native_type_query state store locals 0%N index (Data (Object (BlobObj blob))) Functions eq_refl Live).
    do 2 native_pure.
    execute (native_data_type_query state store locals 0%N index (Object (BlobObj blob)) Functions eq_refl Live).
    do 2 native_pure.
    execute (native_length_query state store locals 0%N index (Object (BlobObj blob)) Functions eq_refl Live
      ltac:(change (Z.of_nat (List.length (get_blob_data blob)) < 2 ^ 64)%Z; rewrite Width; lia)).
    rewrite Length.
    do 2 native_pure.
    native_pure.
    execute (read_blob_run state store locals index next 8 Functions Read).
    do 2 native_pure.
    pose proof (wasm_deserialise_u64 _ _ Decoded) as Number.
    change (wasm_deserialise (get_blob_data blob) T_i64 = VAL_int64 (wasm_nat64 number)) in Number.
    rewrite <- Number.
    apply rt_step; eapply r_load_success with (m := page) (bs := get_blob_data blob).
    - change (lookup_N (s_mems next) 0%N = Some page); rewrite Memory; reflexivity.
    - exact Loaded.
  Qed.

  Lemma collector_integer_executes state store locals index blob number next page :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some (blobh blob) ->
    G.E.H.blob_read_result G.E.collector_instance state store
      [descriptor index; word 0; word 0; word 8] = Some (next, [word 8]) ->
    s_mems next = [page] -> load page 0%N 0%N 8%N = Some (get_blob_data blob) ->
    decode_u64 (get_blob_data blob) = Some number ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [descriptor index] ++ [AI_basic (BI_call 27%N)])
      (state, next, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 number))]).
  Proof.
    intros Functions Live Read Memory Loaded Decoded.
    eapply G.E.collector_call with (code := evaluation_integer) (parameters := [T_num T_i32])
      (results := [T_num T_i64]) (defaults := []) (address := 27%N)
      (arguments := [descriptor index]) (values := [VAL_num (VAL_int64 (wasm_nat64 number))])
      (accepted := true) (final_frame := Build_frame [descriptor index] G.E.collector_instance);
      try reflexivity.
    - rewrite Functions; reflexivity.
    - eapply collector_integer_body_executes; eassumption.
  Qed.
