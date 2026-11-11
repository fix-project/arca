  Lemma observed_comparison_total state lhs rhs :
    (forall index value, exists bytes, handle_bytes index value = Some bytes) ->
    exists equal, observed_comparison state lhs rhs equal.
  Proof.
    intro Bytes.
    destruct (Bytes (G.E.H.observations state) lhs) as [left Left].
    destruct (Bytes (Datatypes.S (G.E.H.observations state)) rhs) as [right Right].
    destruct (proj2 (G.E.H.T.O.compare_handles_defined (blobh left) (blobh right))
      (conj eq_refl eq_refl)) as [equal Compared].
    exists equal, left, right; auto.
  Qed.

  Lemma observed_same_identifies state lhs rhs : observed_same state lhs rhs -> lhs = rhs.
  Proof.
    intros [left [right [Left [Right Compared]]]].
    eapply bytes_identify; [exact Left|exact Right|].
    apply G.E.H.T.O.compare_handles_equal in Compared as [_ [_ Related]].
    apply G.E.H.T.O.indistinguishable_blobs in Related; exact Related.
  Qed.

  Lemma same_run state store locals lhs_index rhs_index lhs rhs equal :
    s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    observed_comparison state lhs rhs equal ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [descriptor lhs_index; descriptor rhs_index] ++ [AI_basic (BI_call 23%N)])
      (compared state, store, Build_frame locals G.E.collector_instance,
        if equal then [] else [AI_trap]).
  Proof.
    intros Functions LiveLeft LiveRight [left [right [Left [Right Compared]]]].
    set (inside := [descriptor lhs_index; descriptor rhs_index]).
    eapply G.E.collector_call_void with (code := evaluation_same)
      (parameters := [T_num T_i32; T_num T_i32]) (defaults := []) (address := 23%N)
      (arguments := inside) (accepted := equal) (final_frame := Build_frame inside G.E.collector_instance);
      try reflexivity.
    - rewrite Functions; reflexivity.
    - rewrite evaluation_same_body.
      native_pure; native_pure.
      assert (Import : G.E.H.host_values combination G.E.collector_instance true true handle_bytes
        state store shell_same_handle_bytes inside =
          Some (compared state, [VAL_num (VAL_int32 (wasm_bool equal))])).
      { unfold G.E.H.host_values, inside, descriptor.
        rewrite LiveLeft, LiveRight; cbn [G.E.H.H.obind].
        rewrite Left, Right; cbn [G.E.H.H.obind]; rewrite Compared; destruct equal; reflexivity. }
      execute (native_value_import state store (compared state) inside 4%N shell_same_handle_bytes
        inside [VAL_num (VAL_int32 (wasm_bool equal))] Functions eq_refl eq_refl eq_refl
        ltac:(discriminate) Import).
      apply native_require; exact Functions.
  Qed.

  Definition native_object := nth 7 (mod_funcs evaluation_module) (Build_module_func 0%N [] []).
  Lemma native_object_body : modfunc_body native_object =
    ltac:(let code := eval cbv [modfunc_body native_object mod_funcs evaluation_module nth] in
      (modfunc_body native_object) in exact code).
  Proof. reflexivity. Qed.

  Lemma allocate_observations before value after values : G.E.H.allocate before value = Some (after, values) ->
    G.E.H.observations after = G.E.H.observations before.
  Proof.
    intro Allocated; destruct (G.E.H.allocate_resolves _ _ _ _ Allocated)
      as [index [_ [_ [_ [_ [_ Observations]]]]]]; exact Observations.
  Qed.

  Lemma observed_comparison_observations before after lhs rhs equal :
    G.E.H.observations after = G.E.H.observations before ->
    observed_comparison before lhs rhs equal -> observed_comparison after lhs rhs equal.
  Proof. intros Same; unfold observed_comparison; rewrite Same; auto. Qed.

  Lemma object_run state store locals reference_index object_index reference object :
    s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.resolve state reference_index = Some reference ->
    G.E.H.resolve state object_index = Some (Data (Object object)) ->
    observed_same state reference (Data (Ref (G.E.H.H.data_reference (Object object)))) ->
    G.E.H.D.live_count (G.E.H.slots state) < G.E.H.capacity ->
    exists next,
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [descriptor reference_index; descriptor object_index] ++ [AI_basic (BI_call 29%N)])
        (next, store, Build_frame locals G.E.collector_instance, []) /\ scope state next 1 /\
      G.E.H.observations next = G.E.H.observations state + 2.
  Proof.
    intros Functions Reference ObjectLive Observed Room.
    set (value := Data (Ref (G.E.H.H.data_reference (Object object)))).
    set (inside := [descriptor reference_index; descriptor object_index]).
    assert (Values : G.E.H.host_values combination G.E.collector_instance true true handle_bytes
      state store shell_create_ref [descriptor object_index] = G.E.H.allocate state value).
    { unfold G.E.H.host_values, G.E.H.transform, descriptor, value.
      rewrite ObjectLive; reflexivity. }
    destruct (new_handle_run state store inside 11%N shell_create_ref [descriptor object_index] value
      Functions eq_refl eq_refl eq_refl ltac:(discriminate) Values Room)
      as [allocated [copy [Created [Present [Fresh [Scoped Observations]]]]]].
    assert (ReferenceNext : G.E.H.resolve allocated reference_index = Some reference)
      by (apply (proj1 Scoped); exact Reference).
    assert (ObservedNext : observed_same allocated reference value).
    { eapply observed_comparison_observations; eassumption. }
    exists (compared allocated); split.
    - eapply G.E.collector_call_void with (code := native_object)
        (parameters := [T_num T_i32; T_num T_i32]) (defaults := [])
        (address := 29%N) (arguments := inside) (accepted := true)
        (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
      + rewrite Functions; reflexivity.
      + rewrite native_object_body.
        execute (native_type_query state store inside 1%N object_index (Data (Object object)) Functions eq_refl ObjectLive).
        do 4 native_pure.
        execute_under [descriptor reference_index] Created.
        exact (same_run allocated store inside reference_index (wasm_nat copy) reference value true
          Functions ReferenceNext Present ObservedNext).
    - split; [exact Scoped|].
      change (G.E.H.observations allocated + 2 = G.E.H.observations state + 2).
      rewrite Observations; reflexivity.
  Qed.

  Lemma fresh_distinct state fresh other value :
    G.E.H.resolve state fresh = None -> G.E.H.resolve state other = Some value ->
    G.E.H.nat_of_i32 fresh <> G.E.H.nat_of_i32 other.
  Proof.
    unfold G.E.H.resolve; intros Fresh Present Same; rewrite Same, Present in Fresh; discriminate.
  Qed.

  Lemma arity_run state store locals index tree length :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some (treeh tree) ->
    List.length (get_tree_raw tree) = length -> native_size_bound length ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [descriptor index; word length] ++ [AI_basic (BI_call 26%N)])
      (state, store, Build_frame locals G.E.collector_instance, []).
  Proof.
    intros Functions Live Length Bound.
    pose proof (G.collector_arity_executes combination handle_bytes (address := index)
      state store locals tree (wasm_nat length) Functions ltac:(rewrite Length; exact Bound) Live) as Run.
    assert (Size : G.E.H.T.O.data_length (Object (TreeObj tree)) = length) by exact Length.
    rewrite Size in Run.
    assert (Same : app_relop (Relop_i ROI_eq) (VAL_int32 (wasm_nat length)) (VAL_int32 (wasm_nat length)) = true).
    { change (Wasm_int.int_eq i32m (wasm_nat length) (wasm_nat length) = true).
      rewrite wasm_nat_equal by (apply native_size_bound_word; exact Bound); apply Nat.eqb_refl. }
    change (reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [descriptor index; word length] ++ [AI_basic (BI_call 26%N)])
      (state, store, Build_frame locals G.E.collector_instance,
        if app_relop (Relop_i ROI_eq) (VAL_int32 (wasm_nat length)) (VAL_int32 (wasm_nat length))
        then [] else [AI_trap])) in Run.
    rewrite Same in Run; exact Run.
  Qed.

  Lemma tag_run state store locals index value tagged :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some value ->
    G.E.H.Api.is_tag value = tagged ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [descriptor index] ++ [AI_basic (BI_call 3%N)])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool tagged))]).
  Proof.
    intros Functions Live Tagged.
    eapply native_value_import with (operation := shell_is_tag); try exact Functions; try reflexivity;
      [discriminate|].
    unfold G.E.H.host_values, descriptor; rewrite Live; cbn [G.E.H.H.obind].
    rewrite Tagged; destruct tagged; reflexivity.
  Qed.


  Lemma counter_equal_run state store locals left right :
    (Z.of_nat left < 2 ^ 32)%Z -> (Z.of_nat right < 2 ^ 32)%Z ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [word left; word right] ++ [AI_basic (BI_relop T_i32 (Relop_i ROI_eq))])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Nat.eqb left right)))]).
  Proof.
    intros Left Right; pose proof (native_relop state store locals (VAL_int32 (wasm_nat left))
      (VAL_int32 (wasm_nat right)) T_i32 (Relop_i ROI_eq) eq_refl) as Run.
    change (reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [word left; word right] ++ [AI_basic (BI_relop T_i32 (Relop_i ROI_eq))])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Wasm_int.int_eq i32m (wasm_nat left) (wasm_nat right))))])) in Run.
    rewrite wasm_nat_equal in Run by assumption; exact Run.
  Qed.

  Lemma branch_if_run state store frame index (condition : bool) :
    reduce_trans
      (state, store, frame, v_to_e_list [VAL_num (VAL_int32 (wasm_bool condition))] ++
        [AI_basic (BI_br_if index)])
      (state, store, frame, if condition then [AI_basic (BI_br index)] else []).
  Proof.
    destruct condition; apply rt_step, r_simple.
    - apply rs_br_if_true; vm_compute; discriminate.
    - apply rs_br_if_false; reflexivity.
  Qed.

  Lemma counter_increment_run state store locals index :
    (Z.of_nat index < 2 ^ 32)%Z ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [word index; word 1] ++ [AI_basic (BI_binop T_i32 (Binop_i BOI_add))])
      (state, store, Build_frame locals G.E.collector_instance, v_to_e_list [word (Datatypes.S index)]).
  Proof. intro Bound; apply rt_step, r_simple; exact (wasm_nat_increment_step index Bound). Qed.
