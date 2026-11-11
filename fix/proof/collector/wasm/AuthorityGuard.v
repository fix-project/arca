Module WasmAuthorityGuard.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Support := WasmSupport.
  Import Support.
  Module Checked := WasmChecked.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Definition authority_loop := match nth 15 (modfunc_body native_authority) BI_unreachable with
    | BI_block _ [BI_loop _ body] => body | _ => [] end.
  Definition authority_accept := nth 17 authority_loop BI_unreachable.

  Lemma authority_accept_body : authority_accept =
    BI_block (BT_valtype None)
      [BI_local_get 4%N; BI_call 0%N; BI_testop T_i32 TO_eqz;
       BI_if (BT_valtype None)
         [BI_local_get 4%N; BI_call 1%N; BI_const_num (VAL_int32 (wasm_nat 1));
          BI_relop T_i32 (Relop_i ROI_eq);
          BI_if (BT_valtype None)
            [BI_local_get 4%N; BI_call 3%N; BI_testop T_i32 TO_eqz;
             BI_local_get 4%N; BI_call 6%N; BI_testop T_i64 TO_eqz;
             BI_binop T_i32 (Binop_i BOI_and); BI_br_if 2%N] []] [];
       BI_local_get 1%N; BI_local_get 4%N; BI_call 23%N].
  Proof. reflexivity. Qed.

  Lemma kernel_author_tree tree :
    treeh tree = X.kernel_author <-> tree_tag tree = false /\ get_tree_raw tree = [].
  Proof.
    split.
    - intro Equal; unfold X.kernel_author, treeh in Equal; inversion Equal; subst tree.
      split; [apply tree_tag_create_tree|apply get_tree_raw_create_tree].
    - intros [Untagged Fields]; pose proof (create_tree_get_tree_raw tree) as Rebuilt.
      rewrite Untagged, Fields in Rebuilt; unfold X.kernel_author, create_tree, treeh; now rewrite Rebuilt.
  Qed.

  Lemma authority_zero64 state store locals length :
    (Z.of_nat length < 2 ^ 64)%Z ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        [v_to_e (VAL_num (VAL_int64 (wasm_nat64 length))); AI_basic (BI_testop T_i64 TO_eqz)])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Nat.eqb length 0)))]) .
  Proof.
    intro Bound; pose proof (Checked.wasm_nat64_equal 0 length ltac:(vm_compute; reflexivity) Bound) as Same.
    rewrite Nat.eqb_sym in Same.
    change (app_testop_i i64m TO_eqz (wasm_nat64 length) = Nat.eqb length 0) in Same.
    apply rt_step, r_simple; rewrite <- Same; apply rs_testop_i64.
  Qed.

  Lemma authority_tree_length state store locals index tree :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state index = Some (treeh tree) ->
    (Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 64)%Z ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [descriptor index] ++ [AI_basic (BI_call 6%N)])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 (List.length (get_tree_raw tree))))]).
  Proof.
    intros Functions Live Bound.
    eapply native_value_import with (operation := shell_len); try exact Functions; try reflexivity; [discriminate|].
    unfold G.E.H.host_values, descriptor; rewrite Live; cbn [G.E.H.H.obind].
    unfold G.E.H.length_result.
    assert (Fits : Z.ltb (Z.of_nat (List.length (get_tree_raw tree))) (2 ^ 64) = true)
      by (apply Z.ltb_lt; exact Bound).
    change ((if Z.ltb (Z.of_nat (List.length (get_tree_raw tree))) (2 ^ 64) then
      Some (state, [VAL_num (VAL_int64 (wasm_nat64 (List.length (get_tree_raw tree))))]) else None) =
      Some (state, [VAL_num (VAL_int64 (wasm_nat64 (List.length (get_tree_raw tree))))])).
    rewrite Fits; reflexivity.
  Qed.

  Lemma authority_zero_bound : (Z.of_nat 0 < 2 ^ 64)%Z.
  Proof. vm_compute; reflexivity. Qed.

  Ltac authority_query functions bound :=
    lazymatch goal with
    | |- ?relation (?state, ?store, Build_frame ?locals G.E.collector_instance,
        ?left :: ?head :: AI_basic (BI_call 6%N) :: ?tail) _ =>
      let prefix := eval cbn [e_to_v_opt option_map v_to_e word descriptor] in (e_to_v_opt left) in
      let parsed := eval cbn [e_to_v_opt option_map v_to_e descriptor] in (e_to_v_opt head) in
      lazymatch prefix with Some ?value =>
      lazymatch parsed with Some (VAL_num (VAL_int32 ?index)) =>
      lazymatch goal with Live : G.E.H.resolve state index = Some (Data (Object (TreeObj ?tree))) |- _ =>
        execute_under [value] (authority_tree_length state store locals index tree functions Live
          ltac:(first [exact bound | assumption | repeat match goal with Hyp : ?left = ?right |- _ => progress rewrite Hyp end; first [assumption|exact authority_zero_bound] | lia]))
      end end end
    | |- ?relation (?state, ?store, Build_frame ?locals G.E.collector_instance,
        ?head :: AI_basic (BI_call ?index) :: ?tail) _ =>
      let parsed := eval cbn [e_to_v_opt option_map v_to_e descriptor] in (e_to_v_opt head) in
      lazymatch parsed with Some (VAL_num (VAL_int32 ?slot)) =>
      lazymatch goal with Live : G.E.H.resolve state slot = Some ?input |- _ =>
      let operation := lazymatch index with
        | 0%N => constr:(shell_get_type) | 1%N => constr:(shell_get_data_type) | 3%N => constr:(shell_is_tag) end in
      let answer := lazymatch index with
        | 0%N => constr:([word (G.E.H.H.value_type input)])
        | 1%N => eval cbn [G.E.H.data_type] in (match G.E.H.data_type input with Some n => [word n] | None => [] end)
        | 3%N => constr:([VAL_num (VAL_int32 (wasm_bool (G.E.H.Api.is_tag input)))]) end in
      lazymatch index with
      | 3%N => execute (tag_run state store locals slot input (G.E.H.Api.is_tag input) functions Live eq_refl)
      | _ => execute (native_value_import state store state locals index operation [descriptor slot] answer
          functions eq_refl eq_refl eq_refl ltac:(discriminate)
          ltac:(unfold G.E.H.host_values, descriptor; rewrite Live; reflexivity))
      end
      end end
    end.

  Lemma authority_block state store frame body :
    reduce_trans (state, store, frame, [AI_basic (BI_block (BT_valtype None) body)])
      (state, store, frame, [AI_label 0 [] (to_e_list body)]).
  Proof. apply rt_step; eapply r_block with (vs := []) (n := 0%nat) (t1s := []) (t2s := []); reflexivity. Qed.

  Lemma authority_empty_label state store frame :
    reduce_trans (state, store, frame, [AI_label 0 [] []]) (state, store, frame, []).
  Proof. apply rt_step, r_simple, rs_label_const; reflexivity. Qed.
  Ltac authority_step functions bound :=
    cbn [List.app seq.cat to_e_list v_to_e_list List.map seq.map G.E.H.Api.is_tag G.E.H.H.value_type G.E.H.data_type G.E.H.H.HTreeObj X.D.EC.E.EP.E.H.HTreeObj];
    repeat match goal with
    | Tagged : tree_tag ?tree = ?tag |- context [tree_tag ?tree] => rewrite Tagged
    | Fields : get_tree_raw ?tree = ?fields |- context [get_tree_raw ?tree] => rewrite Fields
    end;
    repeat match goal with Hyp : ?left = ?right |- _ => progress rewrite Hyp end;
    try fold reduce_trans;
    first [native_pure | authority_query functions bound |
      lazymatch goal with
      | |- ?relation (?state, ?store, Build_frame ?locals G.E.collector_instance,
          ?left :: ?head :: AI_basic (BI_testop T_i64 TO_eqz) :: ?tail) _ =>
        let prefix := eval cbn [e_to_v_opt option_map v_to_e word descriptor] in (e_to_v_opt left) in
        let parsed := eval cbn [e_to_v_opt option_map v_to_e word descriptor] in (e_to_v_opt head) in
        lazymatch prefix with Some ?value =>
        lazymatch parsed with Some (VAL_num (VAL_int64 (wasm_nat64 ?length))) =>
          execute_under [value] (authority_zero64 state store locals length ltac:(first [exact bound | assumption | repeat match goal with Hyp : ?left = ?right |- _ => progress rewrite Hyp end; first [assumption|exact authority_zero_bound] | lia]))
        end end
      | |- ?relation (?state, ?store, Build_frame ?locals G.E.collector_instance,
          ?head :: AI_basic (BI_testop T_i64 TO_eqz) :: ?tail) _ =>
        let parsed := eval cbn [e_to_v_opt option_map v_to_e word descriptor] in (e_to_v_opt head) in
        lazymatch parsed with
        | Some (VAL_num (VAL_int64 (wasm_nat64 ?length))) =>
          execute (authority_zero64 state store locals length ltac:(first [exact bound | assumption | repeat match goal with Hyp : ?left = ?right |- _ => progress rewrite Hyp end; first [assumption|exact authority_zero_bound] | lia]))
        end
      | |- ?relation (?state, ?store, ?frame, AI_basic (BI_block (BT_valtype None) ?body) :: ?tail) _ =>
        execute (authority_block state store frame body)
      | |- ?relation (?state, ?store, ?frame, ?head :: AI_basic (BI_br_if ?index) :: ?tail) _ =>
        let parsed := eval cbn [e_to_v_opt option_map v_to_e word descriptor] in (e_to_v_opt head) in
        lazymatch parsed with Some (VAL_num (VAL_int32 ?operand)) =>
          let zero := eval vm_compute in (Wasm_int.int_eq i32m operand (wasm_nat 0)) in
          let condition := eval cbn in (negb zero) in
          execute (branch_if_run state store frame index condition)
        end
      | |- ?relation (?state, ?store, ?frame, AI_label 0 [] [] :: ?tail) _ =>
        execute (authority_empty_label state store frame)
      | |- ?relation (?state, ?store, ?frame, AI_label 0 [] ?body :: ?tail) _ =>
        eapply rt_trans; [eapply G.E.collector_runs_context with (prefix := []) (suffix := tail)
          (instructions := [AI_label 0 [] body]);
          apply G.E.collector_runs_label; authority_step functions bound; apply rt_refl|]
      end].


  Lemma comparison_unique state left right first second :
    observed_comparison state left right first -> observed_comparison state left right second -> first = second.
  Proof.
    intros [l [r [Left [Right Compared]]]] [l2 [r2 [Left2 [Right2 Compared2]]]].
    rewrite Left in Left2; inversion Left2; subst l2.
    rewrite Right in Right2; inversion Right2; subst r2; congruence.
  Qed.

  Definition author_bounded author := match author with
    | Data (Object (TreeObj node)) => (Z.of_nat (List.length (get_tree_raw node)) < 2 ^ 64)%Z
    | _ => True end.

  Lemma authority_author state store prefix0 prefix2 prefix3 collector_slot author_slot
      collector author count index :
    s_funcs store = s_funcs G.E.allocated_store -> author_bounded author ->
    (forall index value, exists bytes, handle_bytes index value = Some bytes) ->
    G.E.H.resolve state collector_slot = Some collector -> G.E.H.resolve state author_slot = Some author ->
    let locals := [prefix0; descriptor collector_slot; prefix2; prefix3;
      descriptor author_slot; word count; word index] in
    exists next (accepted : bool),
      reduce_trans (state, store, Build_frame locals G.E.collector_instance, to_e_list [authority_accept])
        (next, store, Build_frame locals G.E.collector_instance, if accepted then [] else [AI_trap]) /\
      scope state next 0 /\ (accepted = true -> author = collector \/ author = X.kernel_author) /\
      (author = X.kernel_author \/ observed_same state collector author -> accepted = true).
  Proof.
    intros Functions Bound Bytes CollectorLive AuthorLive locals.
    destruct (ClassicalDescription.excluded_middle_informative (author = X.kernel_author)) as [Kernel|Other].
    - subst author; exists state, true; split.
      2: { split; [apply scope_refl|]; auto. }
      assert (Tagged : tree_tag (create_tree []) = false) by apply tree_tag_create_tree.
      assert (Fields : get_tree_raw (create_tree []) = []) by apply get_tree_raw_create_tree.
      assert (Length : (Z.of_nat (List.length (get_tree_raw (create_tree []))) < 2 ^ 64)%Z)
        by (rewrite Fields; vm_compute; reflexivity).
      unfold locals, X.kernel_author in AuthorLive |- *; rewrite authority_accept_body.
      change (G.E.H.resolve state author_slot = Some (Data (Object (TreeObj (create_tree []))))) in AuthorLive.
      cbn [to_e_list List.map seq.map].
      repeat authority_step Functions Length.
      apply rt_step, r_simple; eapply rs_br with (vs := []) (i := 2%nat)
        (lh := LH_rec [] 0 [] (LH_rec [] 0 [] (LH_base [] []) [])
          (to_e_list [BI_local_get 1%N; BI_local_get 4%N; BI_call 23%N])); reflexivity.
    - destruct (observed_comparison_total state collector author Bytes) as [equal Compared].
      exists (compared state), equal; split.
      + { pose proof (same_run state store locals collector_slot author_slot collector author equal
          Functions CollectorLive AuthorLive Compared) as Same.
        unfold locals in Same |- *; rewrite authority_accept_body; cbn [to_e_list List.map seq.map].
        destruct author as [[[blob|tree]|reference]|thunk|encoded].
        all: try (destruct (tree_tag tree) eqn:Tagged;
          destruct (get_tree_raw tree) as [|first rest] eqn:Fields;
          try solve [exfalso; apply Other; apply kernel_author_tree; auto]).
        all: try (pose proof Bound as Length; cbn [author_bounded] in Length; rewrite Fields in Length).
        all: repeat authority_step Functions ltac:(first [exact Length|exact Bound]).
        all: eapply rt_trans; [apply G.E.collector_runs_label; exact Same|].
        all: destruct equal; [apply rt_step, r_simple, rs_label_const; reflexivity|
          apply rt_step, r_simple, rs_label_trap]. }
      + split; [apply compared_scope|]; split.
        * intro Accepted; subst equal; apply observed_same_identifies in Compared; auto.
        * intros [Kernel|Ready]; [contradiction|].
          exact (comparison_unique state collector author equal true Compared Ready).
  Qed.

  Lemma authority_author_accepts state store prefix0 prefix2 prefix3 collector_slot author_slot
      collector author count index :
    s_funcs store = s_funcs G.E.allocated_store -> author_bounded author ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.resolve state collector_slot = Some collector -> G.E.H.resolve state author_slot = Some author ->
    (author = X.kernel_author \/ observed_same state collector author) ->
    let locals := [prefix0; descriptor collector_slot; prefix2; prefix3;
      descriptor author_slot; word count; word index] in
    exists next,
      reduce_trans (state, store, Build_frame locals G.E.collector_instance, to_e_list [authority_accept])
        (next, store, Build_frame locals G.E.collector_instance, []) /\ scope state next 0.
  Proof.
    intros Functions Bound Bytes CollectorLive AuthorLive Ready locals.
    destruct (authority_author state store prefix0 prefix2 prefix3 collector_slot author_slot
      collector author count index Functions Bound Bytes CollectorLive AuthorLive)
      as [next [accepted [Run [Scoped [Sound Complete]]]]].
    specialize (Complete Ready); subst accepted; exists next; auto.
  Qed.
  Definition witness_limit : Z := 1048576.
  Definition witnesses_bounded witnesses := (Z.of_nat (List.length (get_tree_raw witnesses)) <= witness_limit)%Z.

  Lemma witness_limit_comparison count : (Z.of_nat count < 2 ^ 32)%Z ->
    app_relop (Relop_i (ROI_le SX_U)) (VAL_int32 (wasm_nat count))
      (VAL_int32 (Wasm_int.int_of_Z i32m 1048576)) = Z.leb (Z.of_nat count) witness_limit.
  Proof.
    intro Bound.
    change (negb (Wasm_int.Int32.ltu (Wasm_int.Int32.repr 1048576) (wasm_nat count)) =
      Z.leb (Z.of_nat count) witness_limit).
    unfold Wasm_int.Int32.ltu.
    rewrite wasm_nat_unsigned by exact Bound.
    rewrite Wasm_int.Int32.unsigned_repr by (vm_compute; intuition congruence).
    unfold witness_limit.
    destruct (Coqlib.zlt 1048576 (Z.of_nat count));
      destruct (Z.leb (Z.of_nat count) 1048576) eqn:Compared;
      try reflexivity; first [apply Z.leb_le in Compared|apply Z.leb_gt in Compared]; lia.
  Qed.

  Lemma witness_limit_run state store locals count : (Z.of_nat count < 2 ^ 32)%Z ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [word count; VAL_num (VAL_int32 (Wasm_int.int_of_Z i32m 1048576))] ++
          [AI_basic (BI_relop T_i32 (Relop_i (ROI_le SX_U)))])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Z.leb (Z.of_nat count) witness_limit))) ]).
  Proof.
    intro Bound; pose proof (native_relop state store locals (VAL_int32 (wasm_nat count))
      (VAL_int32 (Wasm_int.int_of_Z i32m 1048576)) T_i32 (Relop_i (ROI_le SX_U)) eq_refl) as Run.
    rewrite witness_limit_comparison in Run by exact Bound; exact Run.
  Qed.
End WasmAuthorityGuard.
