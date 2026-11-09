From Wasm Require Import typing properties.
From FixProof.Collector.wasm Require Import Backend Support Witnesses Refinement.
Import ListNotations.
Local Open Scope list_scope.

Module WasmAuthority (C : COLLECTOR_CONTEXT).
  Import C S R.
  Module V := WasmRefinement C.
  Import V V.Support V.Witnesses V.Resources.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation ext := G.E.H.W.extern_value.
  Local Notation treeh := G.E.H.W.H.HTreeObj.
  Local Notation empty := (VAL_ref (VAL_ref_null T_externref)).
  Local Notation local_get_run := (G.E.collector_local_get combination true true handle_bytes).

  Lemma kernel_author_tree tree :
    treeh tree = X.kernel_author <-> tree_tag tree = false /\ get_tree_raw tree = [].
  Proof.
    split.
    - intro Equal; unfold X.kernel_author, treeh in Equal; inversion Equal; subst tree.
      split; [apply tree_tag_create_tree|apply get_tree_raw_create_tree].
    - intros [Untagged Fields].
      pose proof (create_tree_get_tree_raw tree) as Rebuilt.
      rewrite Untagged, Fields in Rebuilt; unfold X.kernel_author, create_tree, treeh.
      now rewrite Rebuilt.
  Qed.

  Lemma ready_author state witness claim :
    claim_ready state witness claim ->
    exists bundle tag author,
      witness = treeh bundle /\ get_tree_raw bundle = [treeh tag; treeh claim] /\
      get_tree_raw tag = [author; G.E.H.W.H.HTreeRef claim] /\
      X.Wire.author witness = Some author.
  Proof.
    intros [bundle [tag [author [reference [Witness [Bundle [Tag [Tagged [Arity Same]]]]]]]]].
    apply observed_same_identifies in Same; subst reference.
    exists bundle, tag, author; repeat split; try assumption.
    subst witness; unfold X.Wire.author; cbn [treeh]; rewrite Bundle.
    cbn [treeh]; rewrite Tagged, Tag.
    destruct (get_tree_raw claim) as [|code [|lhs [|rhs rest]]] eqn:Claim;
      cbn in Arity; try discriminate.
    destruct rest; [|cbn in Arity; discriminate].
    rewrite X.API.handle_eqb_refl; reflexivity.
  Qed.
  Definition authority_accept := nth 14 native_authority_loop BI_unreachable.

  Definition authority_reserve := nth 12 (modfunc_body native_authority) BI_unreachable.

  Lemma authority_reserve_body : authority_reserve =
    BI_if (BT_valtype None)
      [BI_ref_null T_externref; BI_local_get 5%N; BI_table_size 9%N;
       BI_binop T_i32 (Binop_i BOI_sub); BI_table_grow 9%N;
       BI_const_num (VAL_int32 int32_minus_one); BI_relop T_i32 (Relop_i ROI_ne); BI_call 21%N] [].
  Proof. reflexivity. Qed.

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
       BI_local_get 1%N; BI_local_get 4%N; BI_call 22%N].
  Proof. reflexivity. Qed.

  Lemma authority_zero64 state store locals length :
    (Z.of_nat length < 2 ^ 64)%Z ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        [v_to_e (VAL_num (VAL_int64 (wasm_nat64 length))); AI_basic (BI_testop T_i64 TO_eqz)])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Nat.eqb length 0)))]) .
  Proof.
    intro Bound.
    pose proof (wasm_nat64_equal 0 length ltac:(vm_compute; reflexivity) Bound) as Same.
    rewrite Nat.eqb_sym in Same.
    change (app_testop_i i64m TO_eqz (wasm_nat64 length) = Nat.eqb length 0) in Same.
    apply rt_step, r_simple.
    rewrite <- Same; apply rs_testop_i64.
  Qed.

  Lemma authority_tree_length state store locals tree :
    s_funcs store = s_funcs G.E.allocated_store ->
    (Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 64)%Z ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [ext (treeh tree)] ++ [AI_basic (BI_call 6%N)])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 (List.length (get_tree_raw tree))))]).
  Proof.
    intros Functions Bound.
    eapply native_value_import with (operation := shell_len); try reflexivity.
    - exact Functions.
    - unfold G.E.H.host_values, ext; rewrite to_handle_to_externref.
      change (G.E.H.length_result (List.length (get_tree_raw tree)) =
        Some [VAL_num (VAL_int64 (wasm_nat64 (List.length (get_tree_raw tree))))]).
      apply G.E.H.length_result_small; exact Bound.
  Qed.

  Lemma authority_zero_bound : (Z.of_nat 0 < 2 ^ 64)%Z.
  Proof. vm_compute; reflexivity. Qed.

  Ltac authority_query functions bound :=
    lazymatch goal with
    | |- ?relation (?state, ?store, Build_frame ?locals G.E.collector_instance,
        ?left :: ?head :: AI_basic (BI_call 6%N) :: ?tail) _ =>
      let prefix := eval cbn [e_to_v_opt option_map v_to_e word G.E.H.W.extern_value] in (e_to_v_opt left) in
      let parsed := eval cbn [e_to_v_opt e_to_vref_opt option_map v_to_e vref_to_e G.E.H.W.extern_value G.E.H.W.H.HTreeObj G.E.H.W.H.HBlobObj X.D.EC.E.EP.E.H.HTreeObj] in (e_to_v_opt head) in
      lazymatch prefix with Some ?value =>
      lazymatch parsed with Some (VAL_ref (VAL_ref_extern (to_externref ?input))) =>
      let input := eval hnf in input in
      lazymatch input with ?data (?object (?constructor ?tree)) =>
        execute_under [value] (authority_tree_length state store locals tree functions
          ltac:(first [exact bound | assumption | repeat match goal with Hyp : ?left = ?right |- _ => progress rewrite Hyp end; first [assumption|exact authority_zero_bound] | lia]))
      end end end
    | |- ?relation (?state, ?store, Build_frame ?locals G.E.collector_instance,
        ?head :: AI_basic (BI_call ?index) :: ?tail) _ =>
      let parsed := eval cbn [e_to_v_opt e_to_vref_opt option_map v_to_e vref_to_e G.E.H.W.extern_value G.E.H.W.H.HTreeObj G.E.H.W.H.HBlobObj X.D.EC.E.EP.E.H.HTreeObj] in (e_to_v_opt head) in
      lazymatch parsed with
      | Some (VAL_ref (VAL_ref_extern (to_externref ?input))) =>
      let operation := lazymatch index with
        | 0%N => constr:(shell_get_type) | 1%N => constr:(shell_get_data_type)
        | 3%N => constr:(shell_is_tag) end in
      let answer := lazymatch index with
        | 0%N => constr:([word (G.E.H.W.H.value_type input)])
        | 1%N => eval cbn [G.E.H.data_type] in
            (match G.E.H.data_type input with Some n => [word n] | None => [] end)
        | 3%N => constr:([VAL_num (VAL_int32 (wasm_bool (G.E.H.W.Api.is_tag input)))])
        end in
      execute (native_value_import state store locals index operation [ext input] answer
        functions eq_refl eq_refl eq_refl
        ltac:(unfold G.E.H.host_values, ext; rewrite to_handle_to_externref;
          first [reflexivity | destruct (G.E.H.W.Api.is_tag input); reflexivity]))
      end
    end.

  Lemma authority_block state store frame body :
    reduce_trans (state, store, frame, [AI_basic (BI_block (BT_valtype None) body)])
      (state, store, frame, [AI_label 0 [] (to_e_list body)]).
  Proof.
    apply rt_step; eapply r_block with (vs := []) (n := 0%nat) (t1s := []) (t2s := []); reflexivity.
  Qed.

  Lemma authority_branch state store frame index (condition : bool) :
    reduce_trans
      (state, store, frame, v_to_e_list [VAL_num (VAL_int32 (wasm_bool condition))] ++
        [AI_basic (BI_br_if index)])
      (state, store, frame, if condition then [AI_basic (BI_br index)] else []).
  Proof.
    destruct condition; apply rt_step, r_simple.
    - apply rs_br_if_true; vm_compute; discriminate.
    - apply rs_br_if_false; reflexivity.
  Qed.

  Lemma authority_empty_label state store frame :
    reduce_trans (state, store, frame, [AI_label 0 [] []]) (state, store, frame, []).
  Proof. apply rt_step, r_simple, rs_label_const; reflexivity. Qed.

  Ltac authority_step functions bound :=
    cbn [List.app seq.cat to_e_list v_to_e_list List.map seq.map G.E.H.W.Api.is_tag G.E.H.W.H.value_type G.E.H.data_type G.E.H.W.H.HTreeObj X.D.EC.E.EP.E.H.HTreeObj];
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
        let prefix := eval cbn [e_to_v_opt option_map v_to_e word G.E.H.W.extern_value] in (e_to_v_opt left) in
        let parsed := eval cbn [e_to_v_opt option_map v_to_e word G.E.H.W.extern_value] in (e_to_v_opt head) in
        lazymatch prefix with Some ?value =>
        lazymatch parsed with Some (VAL_num (VAL_int64 (wasm_nat64 ?length))) =>
          execute_under [value] (authority_zero64 state store locals length ltac:(first [exact bound | assumption | repeat match goal with Hyp : ?left = ?right |- _ => progress rewrite Hyp end; first [assumption|exact authority_zero_bound] | lia]))
        end end
      | |- ?relation (?state, ?store, Build_frame ?locals G.E.collector_instance,
          ?head :: AI_basic (BI_testop T_i64 TO_eqz) :: ?tail) _ =>
        let parsed := eval cbn [e_to_v_opt option_map v_to_e word G.E.H.W.extern_value] in (e_to_v_opt head) in
        lazymatch parsed with
        | Some (VAL_num (VAL_int64 (wasm_nat64 ?length))) =>
          execute (authority_zero64 state store locals length ltac:(first [exact bound | assumption | repeat match goal with Hyp : ?left = ?right |- _ => progress rewrite Hyp end; first [assumption|exact authority_zero_bound] | lia]))
        end
      | |- ?relation (?state, ?store, ?frame, AI_basic (BI_block (BT_valtype None) ?body) :: ?tail) _ =>
        execute (authority_block state store frame body)
      | |- ?relation (?state, ?store, ?frame, ?head :: AI_basic (BI_br_if ?index) :: ?tail) _ =>
        let parsed := eval cbn [e_to_v_opt option_map v_to_e word G.E.H.W.extern_value] in (e_to_v_opt head) in
        lazymatch parsed with Some (VAL_num (VAL_int32 ?operand)) =>
          let zero := eval vm_compute in (Wasm_int.int_eq i32m operand (wasm_nat 0)) in
          let condition := eval cbn in (negb zero) in
          execute (authority_branch state store frame index condition)
        end
      | |- ?relation (?state, ?store, ?frame, AI_label 0 [] [] :: ?tail) _ =>
        execute (authority_empty_label state store frame)
      | |- ?relation (?state, ?store, ?frame, AI_label 0 [] ?body :: ?tail) _ =>
        eapply rt_trans; [eapply G.E.collector_runs_context with (prefix := []) (suffix := tail)
          (instructions := [AI_label 0 [] body]);
          apply G.E.collector_runs_label; authority_step functions bound; apply rt_refl|]
      end].

  Lemma authority_author state store prefix0 prefix2 prefix3 collector author count index :
    s_funcs store = s_funcs G.E.allocated_store -> lengths_bounded 1 author ->
    (forall index value, exists bytes, handle_bytes index value = Some bytes) ->
    let locals := [prefix0; ext collector; prefix2; prefix3; ext author; word count; word index] in
    exists state' (accepted : bool),
      reduce_trans (state, store, Build_frame locals G.E.collector_instance,
        to_e_list [authority_accept])
        (state', store, Build_frame locals G.E.collector_instance,
          if accepted then [] else [AI_trap]) /\
      (accepted = true -> author = collector \/ author = X.kernel_author).
  Proof.
    intros Functions Bound Bytes locals.
    destruct (ClassicalDescription.excluded_middle_informative (author = X.kernel_author)) as [Kernel|Other].
    - subst author; exists state, true; split; [|auto].
      assert (Tagged : tree_tag (create_tree []) = false) by apply tree_tag_create_tree.
      assert (Fields : get_tree_raw (create_tree []) = []) by apply get_tree_raw_create_tree.
      assert (Length : (Z.of_nat (List.length (get_tree_raw (create_tree []))) < 2 ^ 64)%Z)
        by (rewrite Fields; vm_compute; reflexivity).
      unfold locals, X.kernel_author; rewrite authority_accept_body.
      cbn [to_e_list List.map seq.map].
      repeat authority_step Functions Length.
      apply rt_step, r_simple; eapply rs_br with (vs := []) (i := 2%nat)
        (lh := LH_rec [] 0 [] (LH_rec [] 0 [] (LH_base [] []) [])
          (to_e_list [BI_local_get 1%N; BI_local_get 4%N; BI_call 22%N])); reflexivity.
    - destruct (observed_comparison_total state collector author Bytes) as [equal Compared].
      exists (Datatypes.S (Datatypes.S state)), equal; split.
      + { pose proof (same_run state store locals collector author equal Functions Compared) as Same.
        unfold locals in Same |- *; rewrite authority_accept_body; cbn [to_e_list List.map seq.map].
        destruct author as [[[blob|tree]|reference]|thunk|encoded].
        all: try (destruct (tree_tag tree) eqn:Tagged;
          destruct (get_tree_raw tree) as [|first rest] eqn:Fields;
          try solve [exfalso; apply Other; apply kernel_author_tree; auto]).
        all: try (pose proof (proj1 Bound) as Length; rewrite Fields in Length).
        all: repeat authority_step Functions ltac:(first [exact Length|exact Bound]).
        all: eapply rt_trans; [apply G.E.collector_runs_label; exact Same|].
        all: destruct equal; [apply rt_step, r_simple, rs_label_const; reflexivity|
          apply rt_step, r_simple, rs_label_trap]. }
      + intro Accepted; subst equal; apply observed_same_identifies in Compared; auto.
  Qed.

  Lemma authority_loop_body : native_authority_loop =
    [BI_local_get 6%N; BI_local_get 5%N; BI_relop T_i32 (Relop_i ROI_eq); BI_br_if 1%N;
     BI_local_get 6%N; BI_table_get 5%N; BI_local_set 3%N;
     BI_local_get 6%N; BI_local_get 3%N; BI_call 29%N; BI_table_set 9%N;
     BI_const_num (VAL_int32 (wasm_nat 0)); BI_table_get 6%N; BI_local_set 4%N;
     authority_accept;
     BI_local_get 6%N; BI_const_num (VAL_int32 (wasm_nat 1)); BI_binop T_i32 (Binop_i BOI_add);
     BI_local_set 6%N; BI_br 0%N].
  Proof. reflexivity. Qed.

  Lemma checked_claim state store locals witness :
    s_funcs store = s_funcs G.E.allocated_store -> claim_views_ready store -> lengths_bounded 2 witness ->
    (forall index value, exists bytes, handle_bytes index value = Some bytes) ->
    checked
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [ext witness] ++ [AI_basic (BI_call 29%N)])
      (fun final => let '(_, store', frame', instructions) := final in
        frame' = Build_frame locals G.E.collector_instance /\
        exists claim, instructions = v_to_e_list [ext (treeh claim)] /\
          claim_ready state witness claim /\ claim_store_transition store witness claim store').
  Proof.
    intros Functions Views Bounds Bytes.
    destruct (collector_claim_execution state store witness Functions Views Bounds Bytes)
      as [state' [store' [returned [Run Ready]]]].
    destruct returned as [claim|].
    - eapply checked_result; [exact (Run locals)|].
      split; [reflexivity|]; exists claim; split; [reflexivity|].
      exact (proj2 (Ready claim eq_refl)).
    - eapply checked_trap; exact (Run locals).
  Qed.

  Section WitnessLoop.
    Variable collector : handle.
    Variable coupons : TreeName.
    Variable witnesses : list handle.
    Hypothesis coupon_fields : get_tree_raw coupons = witnesses.
    Hypothesis count_bound : (Z.of_nat (List.length witnesses) < 2 ^ 32)%Z.
    Hypothesis witness_bounds : Forall (lengths_bounded 3) witnesses.
    Hypothesis bytes_available : forall state value, exists bytes, handle_bytes state value = Some bytes.

    Definition authority_empty_claims := Build_tableinst
      (Build_table_type (Build_limits 0%N None) T_externref) [].

    Lemma authority_initial_claims store : view_capacity store 9%N 0 ->
      lookup_N (s_tables store) 9%N = Some authority_empty_claims.
    Proof.
      intros [table [Entry [Element [Maximum [Minimum Size]]]]].
      change (lookup_N (s_tables store) 9%N = Some table) in Entry.
      destruct table as [[limits element] entries]; destruct limits as [minimum maximum].
      cbn in *; subst element maximum.
      assert (minimum = 0%N) by lia; subst minimum.
      assert (entries = []) by (apply List.length_zero_iff_nil; lia); subst entries.
      exact Entry.
    Qed.

    Definition authority_claim_values := map supplied_claim witnesses.
    Definition authority_claim_type := Build_table_type
      (Build_limits (N.of_nat (List.length witnesses)) None) T_externref.
    Definition authority_claim_table index := Build_tableinst authority_claim_type
      (G.E.H.Tables.copied_entries [] authority_claim_values index).
    Definition authority_claims_mapped store index :=
      lookup_N (s_tables store) 9%N = Some (authority_claim_table index).

    Lemma authority_claim_table_size index : index <= List.length witnesses ->
      tab_size (authority_claim_table index) = List.length witnesses.
    Proof.
      intro Bound; unfold tab_size, authority_claim_table.
      cbn [tableinst_elem]; rewrite G.E.H.Tables.copied_entries_length.
      - unfold authority_claim_values; rewrite List.length_map; reflexivity.
      - unfold authority_claim_values; rewrite List.length_map; exact Bound.
    Qed.

    Lemma authority_grow_claims :
      growtable authority_empty_claims (N.of_nat (List.length witnesses)) (VAL_ref_null T_externref) =
        Some (authority_claim_table 0).
    Proof.
      assert (Bound : (N.of_nat (List.length authority_claim_values) < u32_bound)%N).
      { unfold authority_claim_values; rewrite List.length_map; unfold u32_bound; cbn; lia. }
      pose proof (G.E.H.Tables.grow_copied_entries [] authority_claim_values 0%N
        ltac:(cbn; exact Bound)) as Grown.
      cbn [List.length Nat.add] in Grown.
      unfold authority_claim_table, authority_claim_type, authority_empty_claims.
      unfold authority_claim_values in Grown |- *; rewrite List.length_map in Grown.
      exact Grown.
    Qed.

    Lemma authority_claim_table_typing store index : index <= List.length witnesses ->
      tableinst_typing store (authority_claim_table index) = Some authority_claim_type.
    Proof.
      intro Bound.
      assert (Valid : tabletype_valid authority_claim_type = true).
      { unfold tabletype_valid, authority_claim_type, limit_valid_range.
        cbn -[N.leb table_limit_bound].
        assert (Fits : N.leb (N.of_nat (List.length witnesses)) table_limit_bound = true).
        { apply N.leb_le; unfold table_limit_bound, u32_bound; cbn; lia. }
        rewrite Fits; reflexivity. }
      assert (Entries : seq.all
        (fun reference => value_typing store (VAL_ref reference) (T_ref T_externref))
        (G.E.H.Tables.copied_entries [] authority_claim_values index) = true).
      { assert (Externs : forall values, seq.all
          (fun reference => value_typing store (VAL_ref reference) (T_ref T_externref))
          (map G.E.H.Tables.externref values) = true).
        { intro values; induction values; cbn [List.map seq.all]; [reflexivity|].
          change ((true && seq.all
            (fun reference => value_typing store (VAL_ref reference) (T_ref T_externref))
            (map G.E.H.Tables.externref values))%bool = true).
          rewrite IHvalues; reflexivity. }
        assert (Nulls : forall count, seq.all
          (fun reference => value_typing store (VAL_ref reference) (T_ref T_externref))
          (repeat (VAL_ref_null T_externref) count) = true).
        { intro count; induction count; cbn [List.repeat seq.all]; [reflexivity|].
          change ((true && seq.all
            (fun reference => value_typing store (VAL_ref reference) (T_ref T_externref))
            (repeat (VAL_ref_null T_externref) count))%bool = true).
          rewrite IHcount; reflexivity. }
        unfold G.E.H.Tables.copied_entries; rewrite seq.all_cat, Externs, Nulls; reflexivity. }
      unfold tableinst_typing, authority_claim_table; cbn -[authority_claim_type tabletype_valid].
      rewrite Valid.
      change ((if ssrnat.N_eqb
        (N.of_nat (List.length (G.E.H.Tables.copied_entries [] authority_claim_values index)))
        (N.of_nat (List.length witnesses)) then
          if seq.all (fun reference => value_typing store (VAL_ref reference) (T_ref T_externref))
            (G.E.H.Tables.copied_entries [] authority_claim_values index)
          then Some authority_claim_type else None else None) = Some authority_claim_type).
      pose proof (authority_claim_table_size index Bound) as Size.
      change (List.length (G.E.H.Tables.copied_entries [] authority_claim_values index) =
        List.length witnesses) in Size.
      rewrite Size, Entries.
      assert (Same : ssrnat.N_eqb (N.of_nat (List.length witnesses))
        (N.of_nat (List.length witnesses)) = true).
      { destruct (N.of_nat (List.length witnesses)); cbn [ssrnat.N_eqb];
          [reflexivity|apply Pos.eqb_refl]. }
      rewrite Same; reflexivity.
    Qed.

    Lemma authority_write_claim state store locals index witness :
      index < List.length witnesses -> authority_claims_mapped store index ->
      nth_error witnesses index = Some witness ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [word index; ext (supplied_claim witness)] ++ [AI_basic (BI_table_set 9%N)])
        (state, table_replacement store 9 (authority_claim_table (S index)),
          Build_frame locals G.E.collector_instance, []).
    Proof.
      intros Index Mapped Entry.
      assert (IndexBound : (Z.of_nat index < 2 ^ 32)%Z) by lia.
      assert (Value : nth_error authority_claim_values index = Some (supplied_claim witness)).
      { unfold authority_claim_values; rewrite List.nth_error_map, Entry; reflexivity. }
      apply rt_step, r_table_set_success.
      cbn [f_inst]; unfold stab_update.
      assert (Address : lookup_N (inst_tables G.E.collector_instance) 9%N = Some 9%N)
        by reflexivity.
      rewrite Address.
      unfold authority_claims_mapped in Mapped; rewrite Mapped, (wasm_nat_to_N index IndexBound).
      rewrite authority_claim_table_size by lia.
      assert (Fits : N.ltb (N.of_nat index) (N.of_nat (List.length witnesses)) = true)
        by (apply N.ltb_lt; lia).
      rewrite Fits; unfold table_replacement, authority_claim_table.
      cbn [tableinst_elem tableinst_type].
      rewrite Nat2N.id.
      pose proof (G.E.H.Tables.copied_entries_write [] authority_claim_values index
        (supplied_claim witness) Value) as Written.
      cbn [List.length Nat.add] in Written.
      unfold G.E.H.Tables.externref in Written.
      rewrite Written.
      reflexivity.
    Qed.

    Lemma authority_claims_complete store :
      authority_claims_mapped store (List.length witnesses) -> retained_claims store witnesses.
    Proof.
      intros Mapped index witness Entry.
      unfold stab_elem; rewrite (eq_refl : lookup_N (inst_tables G.E.collector_instance) 9%N = Some 9%N).
      unfold authority_claims_mapped in Mapped; rewrite Mapped.
      unfold authority_claim_table; cbn [tableinst_elem].
      replace (List.length witnesses) with (List.length authority_claim_values)
        by (unfold authority_claim_values; rewrite List.length_map; reflexivity).
      rewrite G.E.H.Tables.copied_entries_complete; cbn.
      unfold lookup_N; rewrite Nat2N.id, List.nth_error_map.
      unfold authority_claim_values; rewrite List.nth_error_map, Entry; reflexivity.
    Qed.

    Lemma authority_write_claims_mapped store index :
      authority_claims_mapped
        (table_replacement store 9 (authority_claim_table (S index))) (S index).
    Proof.
      unfold authority_claims_mapped, table_replacement, lookup_N; cbn [upd_s_table].
      apply nth_error_set_eq.
    Qed.

    Lemma claim_transition_private_table store witness claim next :
      claim_store_transition store witness claim next ->
      stab next G.E.collector_instance 9%N = stab store G.E.collector_instance 9%N.
    Proof.
      intros [bundle [tag [first [_ [_ [First Second]]]]]].
      rewrite (G.E.H.tree_attachment_result_other_view _ _ _ _ _ 9%N 6%N 9%N Second
        eq_refl eq_refl ltac:(discriminate)).
      exact (G.E.H.tree_attachment_result_other_view _ _ _ _ _ 9%N 3%N 9%N First
        eq_refl eq_refl ltac:(discriminate)).
    Qed.

    Lemma claim_transition_resources other_tables state store witness claim next :
      V.resources other_tables store -> claim_ready state witness claim ->
      claim_store_transition store witness claim next -> V.resources other_tables next.
    Proof.
      intros Before Ready [bundle [tag [first [Witness [Bundle [First Second]]]]]].
      destruct Ready as [ready_bundle [ready_tag [author [reference
        [ReadyWitness [ReadyBundle [Tag [Tagged [Arity Same]]]]]]]]].
      rewrite Witness in ReadyWitness; inversion ReadyWitness; subst ready_bundle.
      rewrite Bundle in ReadyBundle; inversion ReadyBundle; subst ready_tag.
      eapply V.resources_after_attachment with (store := first) (node := tag) (index := 6).
      - eapply V.resources_after_attachment with (store := store) (node := bundle) (index := 3).
        + exact Before.
        + lia.
        + lia.
        + intros _; rewrite Bundle; reflexivity.
        + discriminate.
        + discriminate.
        + exact First.
      - lia.
      - lia.
      - discriminate.
      - discriminate.
      - intros _; rewrite Tag; reflexivity.
      - exact Second.
    Qed.

    Lemma claim_transition_witness_view store witness claim next :
      claim_store_transition store witness claim next ->
      tree_view store 5%N coupons -> tree_view next 5%N coupons.
    Proof.
      intros [bundle [tag [first [_ [_ [First Second]]]]]] View.
      eapply tree_attachment_other_view with (index := 6) (other := 5); try exact Second; try lia.
      eapply tree_attachment_other_view with (index := 3) (other := 5); try exact First; try lia; exact View.
    Qed.

    Lemma authority_claim_tables_extension before after :
      before <= List.length witnesses -> after <= List.length witnesses ->
      table_extension (authority_claim_table before) (authority_claim_table after) = true.
    Proof.
      intros Before After.
      unfold table_extension; rewrite !authority_claim_table_size by assumption.
      assert (Types : table_type_extension authority_claim_type authority_claim_type = true).
      { unfold table_type_extension, limits_extension, authority_claim_type; cbn -[N.leb].
        rewrite N.leb_refl; reflexivity. }
      change ((table_type_extension authority_claim_type authority_claim_type &&
        ssrnat.leq (List.length witnesses) (List.length witnesses))%bool = true).
      rewrite Types; exact (ssrnat.leqnn (List.length witnesses)).
    Qed.

    Lemma authority_replace_resources other_tables store before replacement typ :
      V.resources other_tables store -> nth_error (s_tables store) 9 = Some before ->
      table_extension before replacement = true ->
      tableinst_typing (table_replacement store 9 replacement) replacement = Some typ ->
      V.resources other_tables (table_replacement store 9 replacement).
    Proof.
      intros Before Present Extension Replacement.
      assert (TableBound : 9 < List.length (s_tables store)).
      { apply (proj1 (List.nth_error_Some _ _)); rewrite Present; discriminate. }
      assert (Typed : store_typing (table_replacement store 9 replacement)).
      { eapply table_replacement_typing; [exact Present|exact (V.resources_typing _ _ Before)| |exact Replacement].
        eapply table_replacement_extension; eassumption. }
      eapply V.resources_frame; [exact Before|reflexivity|exact Typed| | | | | |].
      - apply table_replacement_memory_capacity; exact (V.resources_memory _ _ Before).
      - apply table_replacement_first_two_tables; lia.
      - unfold stab; rewrite (eq_refl : lookup_N (inst_tables G.E.collector_instance) 2%N = Some 2%N); apply table_replacement_lookup with (other := 2); lia.
      - unfold stab; rewrite (eq_refl : lookup_N (inst_tables G.E.collector_instance) 3%N = Some 3%N); apply table_replacement_lookup with (other := 3); lia.
      - unfold stab; rewrite (eq_refl : lookup_N (inst_tables G.E.collector_instance) 4%N = Some 4%N); apply table_replacement_lookup with (other := 4); lia.
      - unfold stab; rewrite (eq_refl : lookup_N (inst_tables G.E.collector_instance) 6%N = Some 6%N); apply table_replacement_lookup with (other := 6); lia.
    Qed.

    Definition authority_reserved store :=
      if Nat.eqb (List.length witnesses) 0 then store
      else table_replacement store 9 (authority_claim_table 0).

    Lemma authority_empty_extension : table_extension authority_empty_claims (authority_claim_table 0) = true.
    Proof.
      unfold table_extension.
      rewrite authority_claim_table_size by lia.
      change ((table_type_extension (Build_table_type (Build_limits 0%N None) T_externref)
        authority_claim_type && ssrnat.leq 0 (List.length witnesses))%bool = true).
      unfold table_type_extension, limits_extension, authority_claim_type; cbn.
      assert (Lower : N.leb 0 (N.of_nat (List.length witnesses)) = true)
        by (apply N.leb_le; lia).
      rewrite Lower; exact (ssrnat.leq0n (List.length witnesses)).
    Qed.

    Lemma authority_reserved_resources other_tables store :
      V.resources other_tables store -> V.fresh_tree_views store ->
      view_capacity store 5%N 0 -> view_capacity store 9%N 0 ->
      V.resources other_tables (authority_reserved store) /\
      V.fresh_tree_views (authority_reserved store) /\
      view_capacity (authority_reserved store) 5%N 0 /\
      authority_claims_mapped (authority_reserved store) 0.
    Proof.
      intros Before Fresh View Empty.
      pose proof (authority_initial_claims store Empty) as Present.
      change (nth_error (s_tables store) 9 = Some authority_empty_claims) in Present.
      unfold authority_reserved; destruct (Nat.eqb (List.length witnesses) 0) eqn:Zero.
      - apply Nat.eqb_eq in Zero; apply List.length_zero_iff_nil in Zero.
        split; [exact Before|]; split; [exact Fresh|]; split; [exact View|].
        unfold authority_claims_mapped, authority_claim_table, authority_claim_type, authority_claim_values.
        rewrite Zero; exact Present.
      - assert (TableBound : 9 < List.length (s_tables store)).
        { apply (proj1 (List.nth_error_Some _ _)); exact (ltac:(rewrite Present; discriminate)). }
        split.
        + eapply authority_replace_resources; [exact Before|exact Present|exact authority_empty_extension|].
          apply authority_claim_table_typing; lia.
        + split.
          * destruct Fresh as [Left Right]; split;
              [eapply table_replacement_capacity with (other := 7)|
               eapply table_replacement_capacity with (other := 8)];
              try exact Left; try exact Right; lia.
          * split.
            -- eapply table_replacement_capacity with (other := 5); try exact View; lia.
            -- unfold authority_claims_mapped, table_replacement, lookup_N; cbn [upd_s_table]; apply nth_error_set_eq.
    Qed.

    Lemma authority_write_resources other_tables store index :
      V.resources other_tables store -> authority_claims_mapped store index ->
      index < List.length witnesses ->
      V.resources other_tables (table_replacement store 9 (authority_claim_table (S index))).
    Proof.
      intros Before Mapped Index.
      eapply authority_replace_resources; [exact Before|exact Mapped| |].
      - apply authority_claim_tables_extension; lia.
      - apply authority_claim_table_typing; lia.
    Qed.

    Definition authority_state other_tables store index :=
      V.resources other_tables store /\ V.fresh_tree_views store /\
      tree_view store 5%N coupons /\ authority_claims_mapped store index.

    Lemma claim_transition_fresh store witness claim next :
      claim_store_transition store witness claim next ->
      V.fresh_tree_views store -> V.fresh_tree_views next.
    Proof.
      intros [bundle [tag [first [_ [_ [First Second]]]]]] [Left Right]; split.
      - eapply tree_attachment_other_capacity with (index := 6) (other := 7); try exact Second; try lia.
        eapply tree_attachment_other_capacity with (index := 3) (other := 7); try exact First; try lia; exact Left.
      - eapply tree_attachment_other_capacity with (index := 6) (other := 8); try exact Second; try lia.
        eapply tree_attachment_other_capacity with (index := 3) (other := 8); try exact First; try lia; exact Right.
    Qed.

    Lemma claim_transition_authority_state other_tables state store index witness claim next :
      authority_state other_tables store index -> claim_ready state witness claim ->
      claim_store_transition store witness claim next -> authority_state other_tables next index.
    Proof.
      intros [Resources [Fresh [View Mapped]]] Ready Transition.
      split; [eapply claim_transition_resources; eassumption|].
      split; [eapply claim_transition_fresh; eassumption|].
      split; [eapply claim_transition_witness_view; eassumption|].
      change (stab next G.E.collector_instance 9%N = Some (authority_claim_table index)).
      rewrite (claim_transition_private_table _ _ _ _ Transition); exact Mapped.
    Qed.

    Lemma authority_write_state other_tables store index :
      authority_state other_tables store index -> index < List.length witnesses ->
      authority_state other_tables
        (table_replacement store 9 (authority_claim_table (S index))) (S index).
    Proof.
      intros [Resources [Fresh [View Mapped]]] Index.
      assert (TableBound : 9 < List.length (s_tables store)).
      { apply (proj1 (List.nth_error_Some _ _)); change (nth_error (s_tables store) 9 = Some (authority_claim_table index)) in Mapped;
          rewrite Mapped; discriminate. }
      split; [eapply authority_write_resources; eassumption|].
      split.
      - destruct Fresh as [Left Right]; split;
          [eapply table_replacement_capacity with (other := 7)|
           eapply table_replacement_capacity with (other := 8)];
          try exact Left; try exact Right; lia.
      - split.
        + eapply table_replacement_view with (other := 5); try exact View; lia.
        + apply authority_write_claims_mapped.
    Qed.

    Lemma claim_transition_author state store witness claim next :
      claim_ready state witness claim -> claim_store_transition store witness claim next ->
      exists bundle tag author,
        witness = treeh bundle /\ get_tree_raw bundle = [treeh tag; treeh claim] /\
        get_tree_raw tag = [author; G.E.H.W.H.HTreeRef claim] /\
        X.Wire.author witness = Some author /\
        stab_elem next G.E.collector_instance 6%N 0%N = Some (G.E.H.Tables.externref author).
    Proof.
      intros Ready [stored_bundle [stored_tag [first [StoredWitness [StoredBundle [First Second]]]]]].
      destruct (ready_author _ _ _ Ready) as [bundle [tag [author [Witness [Bundle [Tag Author]]]]]].
      rewrite Witness in StoredWitness; inversion StoredWitness; subst stored_bundle.
      rewrite Bundle in StoredBundle; inversion StoredBundle; subst stored_tag.
      exists bundle, tag, author; repeat split; try assumption.
      eapply G.E.H.tree_attachment_result_elements with (offset := 0) in Second;
        [exact Second|rewrite Tag; reflexivity].
    Qed.

    Definition authority_locals index previous_witness previous_author :=
      [ext (treeh combination); ext collector; ext (treeh coupons); previous_witness; previous_author;
       word (List.length witnesses); word index].
    Definition authority_frame index previous_witness previous_author :=
      Build_frame (authority_locals index previous_witness previous_author) G.E.collector_instance.

    Lemma authority_count_positive :
      app_relop (Relop_i (ROI_lt SX_U)) (VAL_int32 (wasm_nat 0))
        (VAL_int32 (wasm_nat (List.length witnesses))) = negb (Nat.eqb (List.length witnesses) 0).
    Proof.
      change (Wasm_int.Int32.ltu (wasm_nat 0) (wasm_nat (List.length witnesses)) =
        negb (Nat.eqb (List.length witnesses) 0)).
      unfold Wasm_int.Int32.ltu.
      rewrite (wasm_nat_unsigned 0 ltac:(vm_compute; congruence)),
        (wasm_nat_unsigned (List.length witnesses) count_bound).
      change ((if Coqlib.zlt 0 (Z.of_nat (List.length witnesses)) then true else false) =
        negb (Nat.eqb (List.length witnesses) 0)).
      destruct (Coqlib.zlt 0 (Z.of_nat (List.length witnesses)));
        symmetry; [apply Bool.negb_true_iff, Nat.eqb_neq|apply Bool.negb_false_iff, Nat.eqb_eq]; lia.
    Qed.

    Lemma authority_reserve_executes state store locals :
      s_funcs store = s_funcs G.E.allocated_store -> view_capacity store 9%N 0 ->
      lookup_N locals 5%N = Some (word (List.length witnesses)) ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          to_e_list [BI_table_size 9%N; BI_local_get 5%N;
            BI_relop T_i32 (Relop_i (ROI_lt SX_U)); authority_reserve])
        (state, authority_reserved store, Build_frame locals G.E.collector_instance, []).
    Proof.
      intros Functions Empty Count.
      pose proof (authority_initial_claims store Empty) as Present.
      assert (Size : reduce_trans
        (state, store, Build_frame locals G.E.collector_instance, [AI_basic (BI_table_size 9%N)])
        (state, store, Build_frame locals G.E.collector_instance, v_to_e_list [word 0])).
      { apply rt_step; eapply r_table_size with (tab := authority_empty_claims) (sz := 0);
          [exact Present|reflexivity]. }
      execute Size.
      execute_under [word 0] (local_get_run state store locals 5%N (word (List.length witnesses)) Count).
      pose proof (native_relop state store locals (VAL_int32 (wasm_nat 0))
        (VAL_int32 (wasm_nat (List.length witnesses))) T_i32 (Relop_i (ROI_lt SX_U)) eq_refl) as Compare.
      rewrite authority_count_positive in Compare; execute Compare.
      rewrite authority_reserve_body; unfold authority_reserved.
      destruct (Nat.eqb (List.length witnesses) 0) eqn:Zero.
      - apply G.E.collector_if_skip.
      - eapply G.E.collector_if_void with (condition := true) (accepted := true).
        execute_under [VAL_ref (VAL_ref_null T_externref)]
          (local_get_run state store locals 5%N (word (List.length witnesses)) Count).
        execute_under [VAL_ref (VAL_ref_null T_externref); word (List.length witnesses)] Size.
        assert (Subtract : app_binop (Binop_i BOI_sub)
          (VAL_int32 (wasm_nat (List.length witnesses))) (VAL_int32 (wasm_nat 0)) =
            Some (VAL_int32 (wasm_nat (List.length witnesses)))).
        { change (Some (VAL_int32 (Wasm_int.Int32.sub (wasm_nat (List.length witnesses)) Wasm_int.Int32.zero)) =
            Some (VAL_int32 (wasm_nat (List.length witnesses)))).
          rewrite Wasm_int.Int32.sub_zero_l; reflexivity. }
        execute_under [VAL_ref (VAL_ref_null T_externref)]
          (native_binop state store locals (VAL_int32 (wasm_nat (List.length witnesses)))
            (VAL_int32 (wasm_nat 0)) T_i32 (Binop_i BOI_sub)
            (VAL_int32 (wasm_nat (List.length witnesses))) eq_refl Subtract).
        assert (Grow : reduce_trans
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [VAL_ref (VAL_ref_null T_externref); word (List.length witnesses)] ++
              [AI_basic (BI_table_grow 9%N)])
          (state, table_replacement store 9 (authority_claim_table 0),
            Build_frame locals G.E.collector_instance, v_to_e_list [word 0])).
        { apply rt_step, r_table_grow_success; cbn [f_inst]; unfold stab_grow.
          rewrite (eq_refl : lookup_N (inst_tables G.E.collector_instance) 9%N = Some 9%N), Present,
            (wasm_nat_to_N (List.length witnesses) count_bound), authority_grow_claims.
          reflexivity. }
        execute Grow.
        pose proof (native_relop state (table_replacement store 9 (authority_claim_table 0)) locals
          (VAL_int32 (wasm_nat 0)) (VAL_int32 int32_minus_one) T_i32 (Relop_i ROI_ne) eq_refl) as Success.
        assert (Succeeded : app_relop (Relop_i ROI_ne) (VAL_int32 (wasm_nat 0))
          (VAL_int32 int32_minus_one) = true) by reflexivity.
        rewrite Succeeded in Success; execute Success.
        apply G.E.collector_require_executes with (condition := true).
        change (lookup_N (s_funcs store) 21%N =
          Some (FC_func_native (Tf [T_num T_i32] []) G.E.collector_instance evaluation_require)).
        rewrite Functions; reflexivity.
    Qed.

    Lemma checked_authority_iteration other_tables state store index previous_witness previous_author :
      index < List.length witnesses -> authority_state other_tables store index ->
      checked
        (state, store, authority_frame index previous_witness previous_author, to_e_list native_authority_loop)
        (fun final => let '(_, store', frame', instructions) := final in
          authority_state other_tables store' (S index) /\ instructions = [AI_basic (BI_br 0%N)] /\
          exists witness author claim,
            frame' = authority_frame (S index) (ext witness) (ext author) /\
            nth_error witnesses index = Some witness /\ X.authenticated_by collector witness = true /\
            authenticated_claim witness claim).
    Proof.
      intros Index Before.
      destruct Before as [Resources [Fresh [View Mapped]]].
      assert (Before : authority_state other_tables store index) by
        (exact (conj Resources (conj Fresh (conj View Mapped)))).
      pose proof (V.resources_functions _ _ Resources) as Functions.
      assert (IndexBound : (Z.of_nat index < 2 ^ 32)%Z) by lia.
      destruct (nth_error witnesses index) as [witness|] eqn:Entry;
        [|apply nth_error_None in Entry; lia].
      assert (WitnessBound : lengths_bounded 3 witness).
      { apply Forall_forall with (x := witness) in witness_bounds;
          [exact witness_bounds|eapply nth_error_In; exact Entry]. }
      pose proof (counter_test_run state store (authority_locals index previous_witness previous_author)
        6%N 5%N index (List.length witnesses) IndexBound count_bound eq_refl eq_refl) as Test.
      rewrite (proj2 (Nat.eqb_neq index (List.length witnesses)) ltac:(lia)) in Test.
      unfold authority_frame at 1; rewrite authority_loop_body.
      checked_advance Test; checked_pure.
      checked_advance (tree_view_run state store (authority_locals index previous_witness previous_author)
        coupons 5%N index witness IndexBound View ltac:(rewrite coupon_fields; exact Entry)).
      do 3 checked_pure.
      checked_use_under [word index]
        (checked_claim state store (authority_locals index (ext witness) previous_author)
          witness Functions (V.resources_claim_ready _ _ Resources)
          (lengths_bounded_weaken 2 witness WitnessBound) bytes_available).
      intros [[[claim_state claim_store] frame'] instructions] [Frame [claim [Code [Ready Transition]]]];
        subst frame' instructions.
      pose proof (claim_ready_authenticates _ _ _ Ready) as Authenticated.
      pose proof (authenticated_claim_supplied _ _ Authenticated) as Supplied.
      assert (AfterClaim : authority_state other_tables claim_store index).
      { eapply claim_transition_authority_state; eassumption. }
      destruct (claim_transition_author _ _ _ _ _ Ready Transition)
        as [bundle [tag [author [Witness [Bundle [Tag [Decoded AuthorEntry]]]]]]].
      assert (AuthorBound : lengths_bounded 1 author).
      { eapply lengths_bounded_child with (tree := tag).
        - eapply lengths_bounded_child with (tree := bundle); [rewrite <- Witness; exact WitnessBound|rewrite Bundle; cbn; auto].
        - rewrite Tag; cbn; auto. }
      set (written := table_replacement claim_store 9 (authority_claim_table (S index))).
      assert (WrittenState : authority_state other_tables written (S index)).
      { unfold written; apply authority_write_state; assumption. }
      assert (WriteRun : reduce_trans
        (claim_state, claim_store, authority_frame index (ext witness) previous_author,
          v_to_e_list [word index; ext (treeh claim)] ++ [AI_basic (BI_table_set 9%N)])
        (claim_state, written, authority_frame index (ext witness) previous_author, [])).
      { rewrite <- Supplied; apply authority_write_claim; try exact Entry; try exact Index.
        exact (proj2 (proj2 (proj2 AfterClaim))). }
      checked_advance WriteRun.
      assert (TableBound : 9 < List.length (s_tables claim_store)).
      { apply (proj1 (List.nth_error_Some _ _));
          change (lookup_N (s_tables claim_store) 9%N <> None).
        rewrite (proj2 (proj2 (proj2 AfterClaim))); discriminate. }
      assert (WrittenAuthor : stab_elem written G.E.collector_instance 6%N 0%N =
        Some (G.E.H.Tables.externref author)).
      { unfold written; rewrite (table_replacement_element _ 9 _ 6 0%N TableBound ltac:(lia) ltac:(lia)); exact AuthorEntry. }
      assert (AuthorRead : reduce_trans
        (claim_state, written, Build_frame (authority_locals index (ext witness) previous_author)
          G.E.collector_instance,
          [v_to_e (word 0); AI_basic (BI_table_get 6%N)])
        (claim_state, written, Build_frame (authority_locals index (ext witness) previous_author)
          G.E.collector_instance,
          v_to_e_list [ext author])).
      { exact (table_get_run claim_state written (authority_locals index (ext witness) previous_author)
          6%N (wasm_nat 0) _ WrittenAuthor). }
      checked_advance AuthorRead.
      checked_pure.
      pose proof (V.resources_functions _ _ (proj1 WrittenState)) as WrittenFunctions.
      destruct (authority_author claim_state written (ext (treeh combination)) (ext (treeh coupons))
        (ext witness) collector author (List.length witnesses) index
        WrittenFunctions AuthorBound bytes_available) as [author_state [accepted [Run Allowed]]].
      destruct accepted.
      - checked_advance Run; checked_pure.
        assert (Increment : reduce_trans
          (author_state, written, authority_frame index (ext witness) (ext author),
            [v_to_e (word index); v_to_e (word 1); AI_basic (BI_binop T_i32 (Binop_i BOI_add))])
          (author_state, written, authority_frame index (ext witness) (ext author), v_to_e_list [word (S index)])).
        { apply rt_step, r_simple, wasm_nat_increment_step; exact IndexBound. }
        checked_advance Increment.
        checked_advance (local_set_run author_state written
          (authority_locals index (ext witness) (ext author)) 6%N (word (S index)) ltac:(cbn; lia)).
        eapply checked_result; [apply rt_refl|].
        split; [exact WrittenState|]; split; [reflexivity|].
        exists witness, author, claim; split; [reflexivity|].
        split; [reflexivity|]; split; [|exact Authenticated].
        unfold X.authenticated_by; rewrite Decoded.
        destruct (Allowed eq_refl) as [Equal|Kernel];
          subst author; rewrite X.API.handle_eqb_refl; apply orb_true_l || apply orb_true_r.
      - eapply checked_trap.
        eapply G.E.collector_trap_suffix with (instructions := to_e_list [authority_accept])
          (suffix := to_e_list [BI_local_get 6%N; BI_const_num (VAL_int32 (wasm_nat 1));
            BI_binop T_i32 (Binop_i BOI_add); BI_local_set 6%N; BI_br 0%N]).
        exact Run.
    Qed.

    Lemma checked_authority_loop other_tables remaining index state store previous_witness previous_author :
      index + remaining = List.length witnesses -> authority_state other_tables store index ->
      checked
        (state, store, authority_frame index previous_witness previous_author,
          G.E.collector_loop_state native_authority_loop (to_e_list native_authority_loop))
        (fun final => let '(_, store', _, instructions) := final in
          authority_state other_tables store' (List.length witnesses) /\ instructions = [] /\
          forallb (X.authenticated_by collector) (skipn index witnesses) = true /\
          Forall (fun witness => exists claim, authenticated_claim witness claim) (skipn index witnesses)).
    Proof.
      revert index state store previous_witness previous_author.
      induction remaining as [|remaining IH]; intros index state store previous_witness previous_author End Before.
      - pose proof (V.resources_functions _ _ (proj1 Before)) as Functions.
        assert (Index : index = List.length witnesses) by lia; subst index.
        pose proof (counter_test_run state store
          (authority_locals (List.length witnesses) previous_witness previous_author)
          6%N 5%N (List.length witnesses) (List.length witnesses)
          count_bound count_bound eq_refl eq_refl) as Test.
        rewrite Nat.eqb_refl in Test.
        eapply checked_result with (final := (state, store,
          authority_frame (List.length witnesses) previous_witness previous_author, [])).
        + eapply G.E.collector_loop_exit with (accepted := true)
            (suffix := to_e_list (skipn 4 native_authority_loop)).
          change (reduce_trans
            (state, store, authority_frame (List.length witnesses) previous_witness previous_author,
              to_e_list (firstn 4 native_authority_loop) ++ to_e_list (skipn 4 native_authority_loop))
            (state, store, authority_frame (List.length witnesses) previous_witness previous_author,
              [AI_basic (BI_br 1%N)] ++ to_e_list (skipn 4 native_authority_loop))).
          eapply G.E.collector_runs_context with (prefix := [])
            (suffix := to_e_list (skipn 4 native_authority_loop))
            (instructions := to_e_list (firstn 4 native_authority_loop))
            (instructions' := [AI_basic (BI_br 1%N)]); exact Test.
        + split; [exact Before|]; split; [reflexivity|].
          rewrite skipn_all; split; [reflexivity|constructor].
      - eapply checked_context with
          (initial := (state, store, authority_frame index previous_witness previous_author,
            to_e_list native_authority_loop))
          (context := LH_rec [] 0 [] (LH_rec [] 0 (G.E.collector_loop_cont native_authority_loop)
            (LH_base [] []) []) [])
          (before := fun final => let '(_, store', frame', instructions) := final in
            authority_state other_tables store' (S index) /\ instructions = [AI_basic (BI_br 0%N)] /\
            exists witness author claim,
              frame' = authority_frame (S index) (ext witness) (ext author) /\
              nth_error witnesses index = Some witness /\ X.authenticated_by collector witness = true /\
              authenticated_claim witness claim).
        + apply checked_authority_iteration; [lia|exact Before].
        + intros [[[state' store'] frame'] instructions]
            [After [Code [witness [author [claim [Frame [Entry [Allowed Authenticated]]]]]]]];
            subst frame' instructions.
          eapply checked_progress; [apply checked_loop_restart|].
          eapply checked_weaken; [apply IH; [lia|exact After]|].
          intros [[[state'' store''] frame''] instructions] [Final [Code [Tail AuthenticatedTail]]].
          split; [exact Final|]; split; [exact Code|].
          rewrite (remaining_child witnesses index witness Entry); split.
          * cbn [forallb]; rewrite Allowed; exact Tail.
          * constructor; [exists claim; exact Authenticated|exact AuthenticatedTail].
    Qed.
  End WitnessLoop.

  Lemma authority_body : modfunc_body native_authority =
    [BI_const_num (VAL_int32 (wasm_nat 1)); BI_table_get 2%N; BI_local_set 1%N;
     BI_const_num (VAL_int32 (wasm_nat 5)); BI_table_get 2%N; BI_local_set 2%N;
     BI_local_get 2%N; BI_call 24%N; BI_local_set 5%N;
     BI_table_size 9%N; BI_local_get 5%N; BI_relop T_i32 (Relop_i (ROI_lt SX_U)); authority_reserve;
     BI_local_get 2%N; BI_const_num (VAL_int32 (wasm_nat 5)); BI_call 8%N;
     BI_block (BT_valtype None) [BI_loop (BT_valtype None) native_authority_loop]].
  Proof. reflexivity. Qed.

  Lemma checked_authority_body other_tables state store metadata callable opcode lhs rhs coupons witnesses :
    V.resources other_tables store -> V.fresh_tree_views store ->
    view_capacity store 5%N 0 -> view_capacity store 9%N 0 ->
    get_tree_raw combination = [metadata; callable; opcode; lhs; rhs; treeh coupons] ->
    get_tree_raw coupons = witnesses -> lengths_bounded 1 (treeh coupons) ->
    Forall (lengths_bounded 3) witnesses ->
    (forall index value, exists bytes, handle_bytes index value = Some bytes) ->
    checked
      (state, store, Build_frame [ext (treeh combination); empty; empty; empty; empty; word 0; word 0]
        G.E.collector_instance, to_e_list (modfunc_body native_authority))
      (fun final => let '(_, next, _, instructions) := final in
        instructions = [] /\ V.resources other_tables next /\ V.fresh_tree_views next /\
        retained_claims next witnesses /\
        Forall (fun witness => exists claim, authenticated_claim witness claim) witnesses /\
        X.reconstruction_authority callable witnesses = true).
  Proof.
    intros Resources Fresh View5 View9 Combination Coupons CouponBound WitnessBounds Bytes.
    pose proof (V.resources_functions _ _ Resources) as Functions.
    pose proof (V.resources_combination _ _ Resources) as CombinationView.
    rewrite authority_body.
    checked_advance (tree_view_run state store
      [ext (treeh combination); empty; empty; empty; empty; word 0; word 0]
      combination 2%N 1 callable ltac:(vm_compute; congruence) CombinationView
      ltac:(rewrite Combination; reflexivity)).
    checked_pure.
    checked_advance (tree_view_run state store
      [ext (treeh combination); ext callable; empty; empty; empty; word 0; word 0]
      combination 2%N 5 (treeh coupons) ltac:(vm_compute; congruence) CombinationView
      ltac:(rewrite Combination; reflexivity)).
    do 2 checked_pure.
    checked_use (V.checked_size state store
      [ext (treeh combination); ext callable; ext (treeh coupons); empty; empty; word 0; word 0]
      (treeh coupons) Functions CouponBound).
    intros [[[size_state size_store] size_frame] instructions] [State [Store [Frame [t [Same [Count [NativeCount Code]]]]]]];
      subst size_state size_store size_frame instructions.
    inversion Same; subst t; rewrite Coupons in Count, NativeCount.
    rewrite Coupons.
    checked_pure.
    set (locals0 := [ext (treeh combination); ext callable; ext (treeh coupons); empty; empty;
      word (List.length witnesses); word 0]).
    set (reserved := authority_reserved witnesses store).
    destruct (authority_reserved_resources witnesses Count other_tables store Resources Fresh View5 View9)
      as [ReservedResources [ReservedFresh [ReservedView ReservedClaims]]].
    checked_advance (authority_reserve_executes witnesses Count state store locals0 Functions View9 eq_refl).
    assert (Available : exists next, G.E.H.tree_attachment_result G.E.collector_instance reserved
      [ext (treeh coupons); word 5] = Some (next, result_values [])).
    { eapply tree_attachment_available with (index := 5) (capacity := 0);
        [exact (V.resources_typing _ _ ReservedResources)|exact ReservedView|lia| |].
      - vm_compute; congruence.
      - rewrite Coupons; exact NativeCount. }
    destruct Available as [attached Attached].
    checked_pure.
    checked_advance (attach_tree_run state reserved locals0 coupons 5 attached
      (V.resources_functions _ _ ReservedResources) Attached).
    assert (AttachedResources : V.resources other_tables attached).
    { eapply V.resources_after_attachment with (index := 5); try exact ReservedResources;
        try exact Attached; try lia; intros; discriminate. }
    assert (AttachedFresh : V.fresh_tree_views attached).
    { destruct ReservedFresh as [Left Right]; split;
        [eapply tree_attachment_other_capacity with (index := 5) (other := 7)|
         eapply tree_attachment_other_capacity with (index := 5) (other := 8)];
        try exact Attached; try exact Left; try exact Right; lia. }
    assert (AttachedView : tree_view attached 5%N coupons)
      by (eapply tree_attachment_view with (index := 5); [vm_compute; congruence|exact Attached]).
    assert (AttachedClaims : authority_claims_mapped witnesses attached 0).
    { change (stab attached G.E.collector_instance 9%N = Some (authority_claim_table witnesses 0)).
      rewrite (G.E.H.tree_attachment_result_other_view _ _ _ _ _ 9%N 5%N 9%N Attached
        eq_refl eq_refl ltac:(discriminate)); exact ReservedClaims. }
    eapply checked_progress; [apply G.E.collector_loop_enter|].
    eapply checked_weaken.
    - apply checked_authority_loop with (remaining := List.length witnesses) (other_tables := other_tables);
        try assumption; [lia|].
      split; [exact AttachedResources|]; split; [exact AttachedFresh|]; split; assumption.
    - intros [[[state' store'] frame'] instructions] [Final [Code [Allowed Authenticated]]].
      split; [exact Code|].
      destruct Final as [FinalResources [FinalFresh [FinalView FinalClaims]]].
      split; [exact FinalResources|]; split; [exact FinalFresh|].
      split; [apply authority_claims_complete; exact FinalClaims|].
      cbn [skipn] in Authenticated, Allowed; split; assumption.
  Qed.

  Lemma collector_authority_execution other_tables state store metadata callable opcode lhs rhs coupons witnesses :
    V.resources other_tables store -> V.fresh_tree_views store ->
    view_capacity store 5%N 0 -> view_capacity store 9%N 0 ->
    get_tree_raw combination = [metadata; callable; opcode; lhs; rhs; treeh coupons] ->
    get_tree_raw coupons = witnesses -> lengths_bounded 1 (treeh coupons) ->
    Forall (lengths_bounded 3) witnesses ->
    (forall index value, exists bytes, handle_bytes index value = Some bytes) ->
    exists state' store' (accepted : bool),
      (forall caller, reduce_trans
        (state, store, Build_frame caller G.E.collector_instance,
          v_to_e_list [ext (treeh combination)] ++ [AI_basic (BI_call 39%N)])
        (state', store', Build_frame caller G.E.collector_instance, if accepted then [] else [AI_trap])) /\
      (accepted = true -> V.resources other_tables store' /\ V.fresh_tree_views store' /\
        retained_claims store' witnesses /\
        Forall (fun witness => exists claim, authenticated_claim witness claim) witnesses /\
        X.reconstruction_authority callable witnesses = true).
  Proof.
    intros Resources Fresh View5 View9 Combination Coupons CouponBound WitnessBounds Bytes.
    pose proof (V.resources_functions _ _ Resources) as Functions.
    destruct (checked_authority_body other_tables state store metadata callable opcode lhs rhs coupons witnesses
      Resources Fresh View5 View9 Combination Coupons CouponBound WitnessBounds Bytes)
      as [[[[state' store'] frame'] instructions] [Run [Trap|[Code Post]]]].
    - subst instructions; exists state', store', false; split; [|discriminate].
      intro caller; eapply G.E.collector_call_void with (code := native_authority)
        (parameters := [T_ref T_externref]) (defaults := [empty; empty; empty; empty; word 0; word 0])
        (address := 39%N) (arguments := [ext (treeh combination)])
        (accepted := false) (final_frame := frame'); try reflexivity.
      + rewrite Functions; reflexivity.
      + exact Run.
    - subst instructions; exists state', store', true; split; [|intro Accepted; exact Post].
      intro caller; eapply G.E.collector_call_void with (code := native_authority)
        (parameters := [T_ref T_externref]) (defaults := [empty; empty; empty; empty; word 0; word 0])
        (address := 39%N) (arguments := [ext (treeh combination)])
        (accepted := true) (final_frame := frame'); try reflexivity.
      + rewrite Functions; reflexivity.
      + exact Run.
  Qed.

End WasmAuthority.
