From Wasm Require Import typing memory.
From FixProof.Collector.wasm Require Import Backend Support Witnesses Acceptance Output Refinement Resolution Authority.
Import ListNotations.
Local Open Scope list_scope.

Module WasmEntry (C : COLLECTOR_CONTEXT).
  Import C S R.
  Module Authority := WasmAuthority C.
  Module V := Authority.V.
  Module Selection := WasmResolution C.
  Import V V.Support V.Output V.Resources.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation ext := G.E.H.W.extern_value.
  Local Notation treeh := G.E.H.W.H.HTreeObj.
  Local Notation blobh := G.E.H.W.H.HBlobObj.
  Local Notation empty := (VAL_ref (VAL_ref_null T_externref)).

  Definition tree_request code lhs rhs :=
    Nat.eqb code 5 && G.E.H.W.Api.is_tag lhs && negb (G.E.H.W.Api.is_tag rhs).

  Lemma entry_tag_query state store locals index value :
    s_funcs store = s_funcs G.E.allocated_store -> lookup_N locals index = Some (ext value) ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        to_e_list [BI_local_get index; BI_call 3%N])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool (G.E.H.W.Api.is_tag value)))]).
  Proof.
    intros Functions Local.
    eapply G.collector_query_local with (operation := shell_is_tag) (value := value);
      try exact Local; try reflexivity.
    - rewrite Functions; reflexivity.
    - unfold G.E.H.host_values, ext; rewrite to_handle_to_externref.
      destruct (G.E.H.W.Api.is_tag value); reflexivity.
  Qed.

  Lemma collector_entry_header initial_state state store combined validated copied opcode code page metadata callable lhs rhs coupons :
    s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.tree_attachment_result G.E.collector_instance store
      [ext (treeh combination); word 2] = Some (combined, result_values []) ->
    s_funcs validated = s_funcs G.E.allocated_store -> tree_view validated 2%N combination ->
    s_funcs copied = s_funcs G.E.allocated_store -> tree_view copied 2%N combination ->
    G.E.H.blob_read_result G.E.collector_instance validated
      [ext (blobh opcode); word 0; word 0; word 8] = Some (copied, result_values [word 8]) ->
    s_mems copied = [page] -> load page 0%N 0%N 8%N = Some (get_blob_data opcode) ->
    decode_u64 (get_blob_data opcode) = Some code ->
    get_tree_raw combination = [metadata; callable; blobh opcode; lhs; rhs; treeh coupons] ->
    reduce_trans
      (initial_state, combined, Build_frame [ext (treeh combination); empty; empty] G.E.collector_instance,
        v_to_e_list [ext (treeh combination)] ++ [AI_basic (BI_call 39%N)])
      (state, validated, Build_frame [ext (treeh combination); empty; empty] G.E.collector_instance, []) ->
    reduce_trans
      (initial_state, store, Build_frame [ext (treeh combination); empty; empty] G.E.collector_instance,
        to_e_list (firstn 26 (modfunc_body evaluation_entry)))
      (state, copied, Build_frame [ext (treeh combination); ext lhs; ext rhs] G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool (tree_request code lhs rhs)))]).
  Proof.
    intros Functions CombinationAttached ValidatedFunctions ValidatedView CopiedFunctions CopiedView
      Read Memory Loaded Code Fields Validation.
    set (initial := [ext (treeh combination); empty; empty]).
    set (selected := [ext (treeh combination); ext lhs; ext rhs]).
    unfold evaluation_entry at 1; cbn [modfunc_body mod_funcs evaluation_module nth firstn].
    native_pure.
    execute (arity_run initial_state store initial combination 6 Functions
      ltac:(rewrite Fields; reflexivity) ltac:(vm_compute; reflexivity)).
    native_pure.
    execute (attach_tree_run initial_state store initial combination 2 combined Functions CombinationAttached).
    native_pure.
    execute Validation.
    execute (tree_view_run state validated initial combination 2%N 3 lhs
      ltac:(vm_compute; reflexivity) ValidatedView ltac:(rewrite Fields; reflexivity)).
    native_pure.
    execute (tree_view_run state validated [ext (treeh combination); ext lhs; empty] combination 2%N 4 rhs
      ltac:(vm_compute; reflexivity) ValidatedView ltac:(rewrite Fields; reflexivity)).
    native_pure.
    execute (tree_view_run state validated selected combination 2%N 2 (blobh opcode)
      ltac:(vm_compute; reflexivity) ValidatedView ltac:(rewrite Fields; reflexivity)).
    execute (collector_integer_executes state validated selected opcode code copied page
      ValidatedFunctions Read CopiedFunctions Memory Loaded Code).
    native_pure.
    match goal with
    | |- context [app_relop ?operation ?left ?right] =>
      assert (Comparison : app_relop operation left right = Nat.eqb code 5)
        by (change (app_relop (Relop_i ROI_eq) (VAL_int64 (wasm_nat64 code))
              (VAL_int64 (wasm_nat64 5)) = Nat.eqb code 5);
            apply wasm_nat64_equal; [eapply decode_u64_bound; exact Code|vm_compute; reflexivity]);
      rewrite Comparison
    end.
    destruct (Nat.eqb code 5) eqn:Test;
      destruct (G.E.H.W.Api.is_tag lhs) eqn:LeftTag;
      destruct (G.E.H.W.Api.is_tag rhs) eqn:RightTag.
    all: unfold tree_request; rewrite Test, LeftTag, RightTag; cbn [andb negb].
    all: pose proof (entry_tag_query state copied selected 1%N lhs CopiedFunctions eq_refl) as Left;
      rewrite LeftTag in Left;
      first [execute_under [word 1] Left | execute_under [word 0] Left].
    all: native_pure.
    all: pose proof (entry_tag_query state copied selected 2%N rhs CopiedFunctions eq_refl) as Right;
      rewrite RightTag in Right;
      first [execute_under [word 1] Right | execute_under [word 0] Right].
    all: do 2 native_pure; apply rt_refl.
  Qed.

  Lemma collector_entry_exchange initial_state state store combined validated copied opcode code page metadata callable lhs rhs coupons
      state' store' final_frame result (accepted : bool) caller :
    s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.tree_attachment_result G.E.collector_instance store
      [ext (treeh combination); word 2] = Some (combined, result_values []) ->
    s_funcs validated = s_funcs G.E.allocated_store -> tree_view validated 2%N combination ->
    s_funcs copied = s_funcs G.E.allocated_store -> tree_view copied 2%N combination ->
    G.E.H.blob_read_result G.E.collector_instance validated
      [ext (blobh opcode); word 0; word 0; word 8] = Some (copied, result_values [word 8]) ->
    s_mems copied = [page] -> load page 0%N 0%N 8%N = Some (get_blob_data opcode) ->
    decode_u64 (get_blob_data opcode) = Some code ->
    get_tree_raw combination = [metadata; callable; blobh opcode; lhs; rhs; treeh coupons] ->
    reduce_trans
      (initial_state, combined, Build_frame [ext (treeh combination); empty; empty] G.E.collector_instance,
        v_to_e_list [ext (treeh combination)] ++ [AI_basic (BI_call 39%N)])
      (state, validated, Build_frame [ext (treeh combination); empty; empty] G.E.collector_instance, []) ->
    tree_request code lhs rhs = false ->
    reduce_trans
      (state, copied, Build_frame [ext (treeh combination); ext lhs; ext rhs] G.E.collector_instance,
        v_to_e_list [ext (treeh combination)] ++ [AI_basic (BI_call 38%N)])
      (state', store', final_frame, if accepted then v_to_e_list [ext result] else [AI_trap]) ->
    reduce_trans
      (initial_state, store, Build_frame caller G.E.collector_instance,
        v_to_e_list [ext (treeh combination)] ++ [AI_basic (BI_call 42%N)])
      (state', store', Build_frame caller G.E.collector_instance,
        if accepted then v_to_e_list [ext result] else [AI_trap]).
  Proof.
    intros Functions CombinationAttached ValidatedFunctions ValidatedView CopiedFunctions CopiedView
      Read Memory Loaded Code Fields Validation Ordinary Run.
    eapply G.E.collector_call with (code := evaluation_entry) (parameters := [T_ref T_externref])
      (results := [T_ref T_externref]) (address := 42%N) (defaults := [empty; empty])
      (arguments := [ext (treeh combination)]) (values := [ext result])
      (accepted := accepted) (final_frame := final_frame); try reflexivity.
    - rewrite Functions; reflexivity.
    - rewrite <- (firstn_skipn 26 (modfunc_body evaluation_entry)) at 1.
      unfold to_e_list; rewrite List.map_app.
      execute (collector_entry_header initial_state state store combined validated copied opcode code page
        metadata callable lhs rhs coupons Functions CombinationAttached ValidatedFunctions ValidatedView
        CopiedFunctions CopiedView Read Memory Loaded Code Fields Validation).
      rewrite Ordinary.
      cbn [evaluation_entry modfunc_body mod_funcs evaluation_module nth firstn skipn List.length List.app List.map seq.map].
      repeat native_pure; exact Run.
  Qed.

  Lemma collector_entry_rejects_author state state' store combined final_store caller metadata callable opcode lhs rhs coupons :
    s_funcs store = s_funcs G.E.allocated_store ->
    G.E.H.tree_attachment_result G.E.collector_instance store
      [ext (treeh combination); word 2] = Some (combined, result_values []) ->
    get_tree_raw combination = [metadata; callable; opcode; lhs; rhs; treeh coupons] ->
    (forall locals, reduce_trans
      (state, combined, Build_frame locals G.E.collector_instance,
        v_to_e_list [ext (treeh combination)] ++ [AI_basic (BI_call 39%N)])
      (state', final_store, Build_frame locals G.E.collector_instance, [AI_trap])) ->
    reduce_trans
      (state, store, Build_frame caller G.E.collector_instance,
        v_to_e_list [ext (treeh combination)] ++ [AI_basic (BI_call 42%N)])
      (state', final_store, Build_frame caller G.E.collector_instance, [AI_trap]).
  Proof.
    intros Functions CombinationAttached Fields Rejected.
    set (initial := [ext (treeh combination); empty; empty]).
    eapply G.E.collector_call with (code := evaluation_entry) (parameters := [T_ref T_externref])
      (results := [T_ref T_externref]) (address := 42%N) (defaults := [empty; empty])
      (arguments := [ext (treeh combination)]) (values := [ext lhs])
      (accepted := false) (final_frame := Build_frame initial G.E.collector_instance); try reflexivity.
    - rewrite Functions; reflexivity.
    - unfold evaluation_entry at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
      native_pure.
      execute (arity_run state store initial combination 6 Functions
        ltac:(rewrite Fields; reflexivity) ltac:(vm_compute; reflexivity)).
      native_pure.
      execute (attach_tree_run state store initial combination 2 combined Functions CombinationAttached).
      native_pure.
      eapply G.E.collector_trap_suffix with
        (instructions := v_to_e_list [ext (treeh combination)] ++ [AI_basic (BI_call 39%N)]).
      apply Rejected.
  Qed.

  Section TreeExchange.
    Variable other_tables : list tableinst.
    Variable witnesses : list handle.
    Hypothesis witness_bounds : Forall (V.Witnesses.lengths_bounded 3) witnesses.
    Hypothesis bytes_available : forall state value, exists bytes, handle_bytes state value = Some bytes.
    Variable output_type assertion_type : table_type.
    Hypothesis output_element : tt_elem_type output_type = T_externref.
    Hypothesis assertion_element : tt_elem_type assertion_type = T_externref.
    Hypothesis scratch_tables : other_tables =
      [Build_tableinst output_type (repeat (VAL_ref_null T_externref) 2);
       Build_tableinst assertion_type (repeat (VAL_ref_null T_externref) 3)].
    Variable trusted : handle -> Prop.

    Record entry_resources store : Prop := {
      entry_functions : s_funcs store = s_funcs G.E.allocated_store;
      entry_typing : store_typing store;
      entry_memory : memory_capacity store;
      entry_tables : firstn 2 (s_tables store) = other_tables;
      entry_combination_capacity : view_capacity store 2%N 0;
      entry_claim_capacity : view_capacity store 3%N 2;
      entry_premise_capacity : view_capacity store 4%N 3;
      entry_witnesses_capacity : view_capacity store 5%N 0;
      entry_coupon_capacity : view_capacity store 6%N 2;
      entry_tree_views : V.fresh_tree_views store;
      entry_retained_capacity : view_capacity store 9%N 0;
    }.

    Definition initial_empty_view :=
      Build_tableinst (Build_table_type (Build_limits 0%N None) T_externref) [].

    Lemma allocated_view_capacity index capacity :
      2 <= index -> index < 10 ->
      view_capacity G.E.allocated_store (N.of_nat index) capacity.
    Proof.
      intros Lower Upper; exists initial_empty_view; split.
      - unfold stab; rewrite (collector_table_address index Upper).
        change (lookup_N
          [Build_tableinst (Build_table_type (Build_limits 2%N None) T_externref)
             (repeat (VAL_ref_null T_externref) 2);
           Build_tableinst (Build_table_type (Build_limits 3%N None) T_externref)
             (repeat (VAL_ref_null T_externref) 3);
           initial_empty_view; initial_empty_view; initial_empty_view; initial_empty_view;
           initial_empty_view; initial_empty_view; initial_empty_view; initial_empty_view]
          (N.of_nat index) = Some initial_empty_view).
        unfold lookup_N; rewrite Nat2N.id.
        do 10 (destruct index as [|index]; [try lia; reflexivity|]); lia.
      - split; [reflexivity|]; split; [reflexivity|]; split.
        + apply N.le_0_l.
        + change (0 <= capacity); lia.
    Qed.

    Lemma allocated_entry_resources :
      store_typing G.E.allocated_store ->
      firstn 2 (s_tables G.E.allocated_store) = other_tables ->
      entry_resources G.E.allocated_store.
    Proof.
      intros Typed Tables; constructor.
      - reflexivity.
      - exact Typed.
      - exists (Build_meminst (Build_limits 1%N None) (mem_make page_size)); split; [reflexivity|].
        split; [reflexivity|]; apply mem_make_length.
      - exact Tables.
      - apply allocated_view_capacity with (index := 2); lia.
      - apply allocated_view_capacity with (index := 3); lia.
      - apply allocated_view_capacity with (index := 4); lia.
      - apply allocated_view_capacity with (index := 5); lia.
      - apply allocated_view_capacity with (index := 6); lia.
      - split; [apply allocated_view_capacity with (index := 7)|apply allocated_view_capacity with (index := 8)]; lia.
      - apply allocated_view_capacity with (index := 9); lia.
    Qed.

    Lemma allocated_entry_ready :
      firstn 2 (s_tables G.E.allocated_store) = other_tables ->
      entry_resources G.E.allocated_store.
    Proof.
      intro Tables; apply allocated_entry_resources; [|exact Tables].
      exact (G.E.collector_allocated_store_typing combination true true handle_bytes).
    Qed.

    Lemma entry_combination_available store :
      entry_resources store -> List.length (get_tree_raw combination) = 6 ->
      exists combined,
        G.E.H.tree_attachment_result G.E.collector_instance store
          [ext (treeh combination); word 2] = Some (combined, result_values []) /\
        V.resources other_tables combined /\ V.fresh_tree_views combined /\
        view_capacity combined 5%N 0 /\ view_capacity combined 9%N 0.
    Proof.
      intros Before Arity.
      destruct (tree_attachment_available store combination 2 0
        (entry_typing _ Before) (entry_combination_capacity _ Before)
        ltac:(lia) ltac:(vm_compute; congruence) ltac:(rewrite Arity; vm_compute; congruence))
        as [combined Attached].
      exists combined; split; [exact Attached|]; split.
      - constructor.
        + rewrite (G.E.H.tree_attachment_result_functions _ _ _ _ _ Attached); exact (entry_functions _ Before).
        + apply (proj1 (proj2 (G.E.H.tree_attachment_result_correct _ _ _ _ _ Attached))).
          exact (entry_typing _ Before).
        + eapply tree_attachment_memory_capacity; [exact Attached|exact (entry_memory _ Before)].
        + rewrite (tree_attachment_first_two_tables store combination 2 combined ltac:(lia) ltac:(lia) Attached).
          exact (entry_tables _ Before).
        + eapply tree_attachment_other_capacity with (index := 2) (other := 3); try exact Attached; try exact (entry_claim_capacity _ Before); lia.
        + eapply tree_attachment_other_capacity with (index := 2) (other := 4); try exact Attached; try exact (entry_premise_capacity _ Before); lia.
        + eapply tree_attachment_other_capacity with (index := 2) (other := 6); try exact Attached; try exact (entry_coupon_capacity _ Before); lia.
        + eapply tree_attachment_view with (index := 2); [vm_compute; congruence|exact Attached].
      - split.
        + destruct (entry_tree_views _ Before) as [Left Right]; split.
          * eapply tree_attachment_other_capacity with (index := 2) (other := 7); try exact Attached; try exact Left; lia.
          * eapply tree_attachment_other_capacity with (index := 2) (other := 8); try exact Attached; try exact Right; lia.
        + split.
          * eapply tree_attachment_other_capacity with (index := 2) (other := 5); try exact Attached; try exact (entry_witnesses_capacity _ Before); lia.
          * eapply tree_attachment_other_capacity with (index := 2) (other := 9); try exact Attached; try exact (entry_retained_capacity _ Before); lia.
    Qed.

    Lemma issue_eval state store locals lhs rhs issuing_author :
      V.prepared other_tables witnesses store witnesses ->
      G.E.H.Tables.T.A.author_at 1 (treeh combination) = Some issuing_author ->
      exists state' store' tag claim,
        reduce_trans
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [ext (treeh combination); ext lhs; ext rhs; VAL_num (VAL_int64 (wasm_nat64 1))] ++
              [AI_basic (BI_call 41%N)])
          (state', store', Build_frame locals G.E.collector_instance,
            if is_comparable issuing_author then v_to_e_list [ext (treeh tag)] else [AI_trap]) /\
        X.Wire.encode Eval issuing_author lhs rhs = Some (treeh (create_tree [treeh tag; treeh claim])).
    Proof.
      intros Prepared Author.
      pose proof (V.prepared_functions other_tables witnesses store witnesses Prepared) as Functions.
      pose proof (V.prepared_tables other_tables witnesses store witnesses Prepared) as Tables.
      pose proof (V.prepared_combination other_tables witnesses store witnesses Prepared) as CombinationView.
      assert (Ready : coupon_output_store store output_type assertion_type lhs rhs issuing_author 1 0 0 = store).
      { unfold coupon_output_store, G.E.H.Tables.copied_entries.
        change (upd_s_table store
          ([Build_tableinst output_type (repeat (VAL_ref_null T_externref) 2);
            Build_tableinst assertion_type (repeat (VAL_ref_null T_externref) 3)] ++
           skipn 2 (s_tables store)) = store).
        rewrite <- scratch_tables, <- Tables, firstn_skipn; apply V.table_update_identity. }
      assert (Selected : nth_error (get_tree_raw combination) 1 = Some issuing_author).
      { change (match nth_error (get_tree_raw combination) 1 with
          | Some child => Some child | None => None end = Some issuing_author) in Author.
        destruct (nth_error (get_tree_raw combination) 1) as [selected|] eqn:Entry;
          inversion Author; subst; reflexivity. }
      assert (AuthorView : stab_elem store G.E.collector_instance 2%N 1%N =
          Some (G.E.H.Tables.externref issuing_author)).
      { exact (CombinationView 1 issuing_author Selected). }
      pose proof (collector_issue_executes store output_type assertion_type lhs rhs issuing_author 1 state locals
        ltac:(vm_compute; reflexivity) Functions output_element assertion_element Author AuthorView) as Run.
      rewrite Ready in Run.
      exists state, (coupon_output_store store output_type assertion_type lhs rhs issuing_author 1 3 2),
        (issued_tag issuing_author lhs rhs 1), (issued_claim lhs rhs 1).
      split; [exact Run|]; apply collector_output_encodes; reflexivity.
    Qed.

    Theorem exported_exchange_refinement state store caller opcode code selected_code rule metadata callable lhs rhs coupons issuing_author :
      get_tree_raw combination = [metadata; callable; blobh opcode; lhs; rhs; treeh coupons] ->
      decode_u64 (get_blob_data opcode) = Some code -> X.select_exchange code lhs = Some selected_code ->
      X.decode_exchange selected_code = Some rule -> tree_request code lhs rhs = false ->
      get_tree_raw coupons = witnesses -> entry_resources store ->
      V.Witnesses.lengths_bounded 1 lhs -> V.Witnesses.lengths_bounded 1 rhs ->
      V.Witnesses.lengths_bounded 1 (treeh coupons) ->
      G.E.H.Tables.T.A.author_at 1 (treeh combination) = Some issuing_author ->
      exists state' store' instructions,
        reduce_trans
          (state, store, Build_frame caller G.E.collector_instance,
            v_to_e_list [ext (treeh combination)] ++ [AI_basic (BI_call 42%N)])
          (state', store', Build_frame caller G.E.collector_instance, instructions) /\
        (instructions = [AI_trap] \/ exists claim_kind tag claim,
          instructions = v_to_e_list [ext (treeh tag)] /\
          X.check rule witnesses lhs rhs = Some claim_kind /\
          X.issue issuing_author claim_kind lhs rhs witnesses (treeh (create_tree [treeh tag; treeh claim]))).
    Proof.
      intros Combination Code Selected Rule Ordinary Coupons Initial LeftBound RightBound CouponsBound Author.
      pose proof (entry_functions _ Initial) as Functions.
      assert (Issuer : issuing_author = callable).
      { change (match nth_error (get_tree_raw combination) 1 with
          | Some child => Some child | None => None end = Some issuing_author) in Author.
        rewrite Combination in Author; cbn in Author; congruence. }
      subst issuing_author.
      destruct (entry_combination_available store Initial ltac:(rewrite Combination; reflexivity))
        as [combined [CombinationAttached [CombinedResources [CombinedFresh [WitnessCapacity RetainedCapacity]]]]].
      destruct (Authority.collector_authority_execution other_tables state combined metadata callable (blobh opcode)
        lhs rhs coupons witnesses CombinedResources CombinedFresh WitnessCapacity RetainedCapacity
        Combination Coupons CouponsBound witness_bounds bytes_available)
        as [guard_state [guard_store [accepted [Validation Policy]]]].
      destruct accepted.
      2: { exists guard_state, guard_store, [AI_trap]; split; [|left; reflexivity].
        eapply collector_entry_rejects_author; eassumption. }
      destruct (Policy eq_refl) as [GuardResources [GuardFresh [Retained [Authenticated AllowedWitnesses]]]].
      assert (GuardPrepared : V.prepared other_tables witnesses guard_store []).
      { constructor; assumption. }
      pose proof (V.Witnesses.memory_capacity_integer_reads guard_store
        (V.resources_typing _ _ GuardResources) (V.resources_memory _ _ GuardResources)) as Reads.
      destruct (Reads opcode ltac:(eapply decode_u64_width; exact Code))
        as [copied [page [Read [Memory Loaded]]]].
      assert (After : V.prepared other_tables witnesses copied []).
      { eapply V.prepared_after_read; [exact GuardPrepared|exact Read]. }
      assert (AfterFresh : V.fresh_tree_views copied).
      { eapply V.fresh_after_read; [exact GuardFresh|exact Read]. }
      pose proof (V.prepared_functions other_tables witnesses guard_store [] GuardPrepared) as GuardFunctions.
      pose proof (V.prepared_combination other_tables witnesses guard_store [] GuardPrepared) as GuardView.
      pose proof (V.prepared_functions other_tables witnesses copied [] After) as CopiedFunctions.
      pose proof (V.prepared_combination other_tables witnesses copied [] After) as CopiedView.
      assert (Native : exists state' store' instructions,
        reduce_trans
          (guard_state, copied, Build_frame [ext (treeh combination); ext lhs; ext rhs] G.E.collector_instance,
            v_to_e_list [ext (treeh combination)] ++ [AI_basic (BI_call 38%N)])
          (state', store', Build_frame [ext (treeh combination); ext lhs; ext rhs] G.E.collector_instance, instructions) /\
        (instructions = [AI_trap] \/ exists claim_kind tag claim,
          instructions = v_to_e_list [ext (treeh tag)] /\
          X.check rule witnesses lhs rhs = Some claim_kind /\
          X.Wire.encode claim_kind callable lhs rhs = Some (treeh (create_tree [treeh tag; treeh claim])))).
      { eapply V.collector_refinement with (selected_code := selected_code) (rule := rule)
          (other_tables := other_tables) (witnesses := witnesses)
          (output_type := output_type) (assertion_type := assertion_type); try eassumption.
        intros at_state at_store locals AtFunctions.
        eapply Selection.resolve_validation; [eapply decode_u64_bound; exact Code|exact Selected|exact AtFunctions]. }
      destruct Native as [state' [store' [instructions [Run [Trap|[kind [tag [claim [Result [Checked Encoded]]]]]]]]]].
      - subst instructions; exists state', store', [AI_trap]; split; [|left; reflexivity].
        eapply collector_entry_exchange with (state := guard_state) (combined := combined)
          (validated := guard_store) (copied := copied) (opcode := opcode) (code := code)
          (page := page) (metadata := metadata) (callable := callable) (coupons := coupons)
          (accepted := false) (result := lhs); eauto; exact (Validation _).
      - subst instructions; exists state', store', (v_to_e_list [ext (treeh tag)]); split.
        + eapply collector_entry_exchange with (state := guard_state) (combined := combined)
            (validated := guard_store) (copied := copied) (opcode := opcode) (code := code)
            (page := page) (metadata := metadata) (callable := callable) (coupons := coupons)
            (accepted := true); eauto; exact (Validation _).
        + right; exists kind, tag, claim; repeat split; assumption || reflexivity.
    Qed.

    Theorem exported_tree_exchange_sound state store caller opcode metadata callable original fields coupons author body :
      trusted callable -> Forall (X.stable_conditional trusted) witnesses ->
      get_tree_raw combination = [metadata; callable; blobh opcode; treeh original; treeh fields; treeh coupons] ->
      decode_u64 (get_blob_data opcode) = Some 5 ->
      tree_tag original = true -> tree_tag fields = false -> get_tree_raw fields = author :: body ->
      get_tree_raw coupons = witnesses -> entry_resources store ->
      V.Witnesses.lengths_bounded 1 (treeh original) -> V.Witnesses.lengths_bounded 1 (treeh fields) ->
      V.Witnesses.lengths_bounded 1 (treeh coupons) ->
      G.E.H.Tables.T.A.author_at 1 (treeh combination) = Some callable ->
      V.checked
        (state, store, Build_frame caller G.E.collector_instance,
          v_to_e_list [ext (treeh combination)] ++ [AI_basic (BI_call 42%N)])
        (fun final => let '(_, _, frame', instructions) := final in
          frame' = Build_frame caller G.E.collector_instance /\
          exists tag claim result, instructions = v_to_e_list [ext (treeh tag)] /\
            result = create_tree_tagged true (get_tree_raw fields) /\
            X.D.coupon_eval (treeh original) (treeh result) /\
            X.issue callable Eval (treeh original) (treeh result) witnesses (treeh (create_tree [treeh tag; treeh claim]))).
    Proof.
      intros Trusted Conditional Combination Code Tagged Untagged Fields Coupons Initial LeftBound RightBound CouponsBound Author.
      pose proof (entry_functions _ Initial) as Functions.
      destruct (entry_combination_available store Initial ltac:(rewrite Combination; reflexivity))
        as [combined [CombinationAttached [CombinedResources [CombinedFresh [WitnessCapacity RetainedCapacity]]]]].
      destruct (Authority.collector_authority_execution other_tables state combined metadata callable (blobh opcode)
        (treeh original) (treeh fields) coupons witnesses CombinedResources CombinedFresh WitnessCapacity RetainedCapacity
        Combination Coupons CouponsBound witness_bounds bytes_available)
        as [guard_state [guard_store [accepted [Validation Policy]]]].
      destruct accepted.
      2: { eapply V.checked_trap; eapply collector_entry_rejects_author; eassumption. }
      destruct (Policy eq_refl) as [GuardResources [GuardFresh [Retained [Authenticated AllowedWitnesses]]]].
      assert (GuardPrepared : V.prepared other_tables witnesses guard_store []).
      { constructor; assumption. }
      pose proof (V.Witnesses.memory_capacity_integer_reads guard_store
        (V.resources_typing _ _ GuardResources) (V.resources_memory _ _ GuardResources)) as Reads.
      destruct (Reads opcode ltac:(eapply decode_u64_width; exact Code))
        as [copied [page [Read [Memory Loaded]]]].
      assert (After : V.prepared other_tables witnesses copied []).
      { eapply V.prepared_after_read; [exact GuardPrepared|exact Read]. }
      assert (AfterFresh : V.fresh_tree_views copied).
      { eapply V.fresh_after_read; [exact GuardFresh|exact Read]. }
      pose proof (V.prepared_functions other_tables witnesses guard_store [] GuardPrepared) as GuardFunctions.
      pose proof (V.prepared_combination other_tables witnesses guard_store [] GuardPrepared) as GuardView.
      pose proof (V.prepared_functions other_tables witnesses copied [] After) as CopiedFunctions.
      pose proof (V.prepared_combination other_tables witnesses copied [] After) as CopiedView.
      assert (Selected : tree_request 5 (treeh original) (treeh fields) = true).
      { change ((true && tree_tag original && negb (tree_tag fields)) = true).
        rewrite Tagged, Untagged; reflexivity. }
      eapply V.checked_call with (code := evaluation_entry) (parameters := [T_ref T_externref])
        (results := [T_ref T_externref]) (address := 42%N) (defaults := [empty; empty])
        (arguments := [ext (treeh combination)]) (dummy := [ext (treeh original)]); try reflexivity.
      - rewrite Functions; reflexivity.
      - rewrite <- (firstn_skipn 26 (modfunc_body evaluation_entry)) at 1.
        unfold to_e_list; rewrite List.map_app.
        checked_advance (collector_entry_header state guard_state store combined guard_store copied opcode 5 page
          metadata callable (treeh original) (treeh fields) coupons Functions CombinationAttached GuardFunctions GuardView
          CopiedFunctions CopiedView Read Memory Loaded Code Combination (Validation _)).
        rewrite Selected.
        cbn [evaluation_entry modfunc_body mod_funcs evaluation_module nth firstn skipn List.length List.app List.map seq.map].
        checked_pure.
        eapply V.checked_context with (depth := 1)
          (initial := (guard_state, copied,
            Build_frame [ext (treeh combination); ext (treeh original); ext (treeh fields)] G.E.collector_instance,
            to_e_list [BI_local_get 0%N; BI_call 40%N; BI_local_set 2%N;
              BI_local_get 0%N; BI_local_get 1%N; BI_local_get 2%N;
              BI_const_num (VAL_int64 (wasm_nat64 1)); BI_call 41%N; BI_return]))
          (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list [BI_local_get 0%N; BI_call 38%N]))
          (before := fun final => let '(_, _, _, instructions) := final in
            exists tag claim result, instructions = v_to_e_list [ext (treeh tag)] ++ [AI_basic BI_return] /\
              result = create_tree_tagged true (get_tree_raw fields) /\
              X.D.coupon_eval (treeh original) (treeh result) /\
              X.issue callable Eval (treeh original) (treeh result) witnesses (treeh (create_tree [treeh tag; treeh claim]))).
        + checked_pure.
          checked_use (V.collector_reconstruction other_tables witnesses witness_bounds bytes_available Authenticated
            guard_state (copied) [ext (treeh combination); ext (treeh original); ext (treeh fields)]
            metadata callable (blobh opcode) original fields coupons author body
            Combination Tagged Fields Coupons After AfterFresh LeftBound RightBound CouponsBound).
          intros [[[state' store'] frame'] instructions] [Frame [Prepared' [result [Instructions [Shape Checked]]]]];
            subst frame' instructions.
          do 4 checked_pure.
          let locals := match goal with |- V.checked (_, _, Build_frame ?locals _, _) _ => constr:(locals) end in
          destruct (issue_eval state' store' locals (treeh original) (treeh result) callable Prepared' Author)
            as [state'' [store'' [tag [claim [Run Issued]]]]].
          destruct (is_comparable callable) eqn:Comparable.
          2: { checked_advance Run.
            eapply V.checked_trap.
            eapply G.E.collector_trap_suffix with
              (instructions := [AI_trap]) (suffix := [AI_basic BI_return]); apply rt_refl. }
          checked_advance Run.
          assert (IssuedCoupon : X.issue callable Eval (treeh original) (treeh result) witnesses
              (treeh (create_tree [treeh tag; treeh claim]))) by (split; assumption).
          assert (Evaluated : X.D.coupon_eval (treeh original) (treeh result)).
          { eapply X.reconstruction_validated_sound with (trusted := trusted)
              (collector := callable) (coupons := witnesses); eassumption. }
          eapply V.checked_result; [apply rt_refl|].
          exists tag, claim, result; repeat split; try assumption; try reflexivity.
        + intros [[[state' store'] frame'] instructions] [tag [claim [result [Instructions [Shape [Eval Issued]]]]]];
            subst instructions.
          eapply V.checked_result; [apply rt_refl|].
          exists [ext (treeh tag)]; split.
          * right; exists 1, (LH_rec [] 0 [] (LH_base [] []) (to_e_list [BI_local_get 0%N; BI_call 38%N])); reflexivity.
          * split; [reflexivity|]; split; [reflexivity|]; exists tag, claim, result; auto.
    Qed.
  End TreeExchange.
End WasmEntry.
