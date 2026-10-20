(** Proves that each supported exchange succeeds when its premises and
    side conditions hold and the required comparisons and resources are ready. *)

From FixProof.Collector.wasm Require Import Backend Support Witnesses.
Import ListNotations.
Local Open Scope list_scope.

Module WasmAcceptance (C : COLLECTOR_CONTEXT).
  Import C S R.
  Module Support := WasmSupport C.
  Import Support.
  Module Witnesses := WasmWitnesses C.
  Import Witnesses.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.

  Local Notation ext := G.E.H.W.extern_value.
  Local Notation treeh := G.E.H.W.H.HTreeObj.
  Local Notation blobh := G.E.H.W.H.HBlobObj.
  Local Notation lower := X.D.EC.E.EP.E.lower.
  Local Notation lift := X.D.EC.E.EP.E.lift.
  Local Notation local_get_run := (G.E.collector_local_get combination true handle_bytes).

  Section Execution.
    Definition endpoint_ready witness claim := exists bundle,
      witness = treeh bundle /\ nth_error (get_tree_raw bundle) 1 = Some (treeh claim).

    (** A ready authenticated witness returns its requested claim endpoint through the
        native helper. *)
    Lemma exchange_endpoint_executes state store locals witness claim value (left : bool) :
      s_funcs store = s_funcs G.E.allocated_store ->
      endpoint_ready witness claim ->
      nth_error (get_tree_raw claim) (if left then 1 else 2) = Some value ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [ext witness] ++ [AI_basic (BI_call (if left then 28%N else 29%N))])
        (state, store, Build_frame locals G.E.collector_instance, v_to_e_list [ext value]).
    Proof.
      intros Functions [bundle [Witness Claim]] Field; subst witness.
      eapply G.E.collector_call with (code := exchange_endpoint left)
        (parameters := [T_ref T_externref]) (results := [T_ref T_externref])
        (address := if left then 28%N else 29%N) (defaults := []) (values := [ext value])
        (accepted := true) (final_frame := Build_frame [ext (treeh bundle)] G.E.collector_instance);
        try (destruct left; reflexivity).
      - rewrite Functions; destruct left; reflexivity.
      - rewrite exchange_endpoint_body.
        native_pure.
        execute (child_run state store [ext (treeh bundle)] bundle 1 (treeh claim) Functions
          ltac:(vm_compute; reflexivity) Claim).
        apply child_run; [exact Functions|destruct left; vm_compute; reflexivity|exact Field].
    Qed.

    (** The native Data guard accepts Data without changing the store. *)
    Lemma native_data_executes state store locals input :
      s_funcs store = s_funcs G.E.allocated_store ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [ext (Data input)] ++ [AI_basic (BI_call 30%N)])
        (state, store, Build_frame locals G.E.collector_instance, []).
    Proof.
      intro Functions.
      eapply G.E.collector_call_void with (code := native_data)
        (parameters := [T_ref T_externref]) (defaults := []) (address := 30%N)
        (arguments := [ext (Data input)]) (accepted := true)
        (final_frame := Build_frame [ext (Data input)] G.E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - change (reduce_trans (state, store, Build_frame [ext (Data input)] G.E.collector_instance,
          to_e_list [BI_local_get 0%N; BI_call 0%N; BI_const_num (VAL_int32 (wasm_nat 2));
            BI_relop T_i32 (Relop_i (ROI_lt SX_U)); BI_call 18%N])
          (state, store, Build_frame [ext (Data input)] G.E.collector_instance, [])).
        execute (native_type_query state store [ext (Data input)] 0%N (Data input) Functions eq_refl).
        destruct input; do 2 native_pure; apply rt_refl.
    Qed.

    (** The native constructor builds an application or selection Thunk from a Tree Object. *)
    Lemma native_map_tree_executes state store locals tree (application : bool) :
      s_funcs store = s_funcs G.E.allocated_store ->
      let mode := if application then 0 else 1 in
      let result := Thunk (if application then G.E.H.W.H.Application tree else G.E.H.W.H.Selection tree) in
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [word mode; ext (treeh tree)] ++ [AI_basic (BI_call 31%N)])
        (state, store, Build_frame locals G.E.collector_instance, v_to_e_list [ext result]).
    Proof.
      intro Functions; cbn zeta.
      set (arguments := [word (if application then 0 else 1); ext (treeh tree)]).
      eapply G.E.collector_call_return with (code := native_map)
        (parameters := [T_num T_i32; T_ref T_externref]) (results := [T_ref T_externref])
        (defaults := []) (address := 31%N) (arguments := arguments)
        (values := [ext (Thunk (if application then G.E.H.W.H.Application tree else G.E.H.W.H.Selection tree))])
        (final_frame := Build_frame arguments G.E.collector_instance) (accepted := true)
        (context := LH_rec [] 0 [] (LH_base [] [])
          (to_e_list (skipn (if application then 4 else 8) (modfunc_body native_map))));
        try (destruct application; reflexivity).
      - rewrite Functions; reflexivity.
      - unfold native_map at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
        destruct application; cbn [arguments] in *; [do 3 native_pure|do 6 native_pure].
        all: cbn [lfill].
        all: eapply G.E.collector_runs_context with (prefix := [])
          (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]);
          apply G.E.collector_runs_label.
        all: native_pure.
        all: execute (G.collector_tree_executes combination handle_bytes state store arguments (treeh tree) Functions).
        all: native_pure.
        + execute (native_value_import state store arguments 13%N shell_application [ext (treeh tree)]
            [ext (Thunk (G.E.H.W.H.Application tree))] Functions eq_refl eq_refl eq_refl
            ltac:(unfold G.E.H.host_values, ext; rewrite to_handle_to_externref; reflexivity)).
          apply rt_refl.
        + execute (native_value_import state store arguments 14%N shell_selection [ext (treeh tree)]
            [ext (Thunk (G.E.H.W.H.Selection tree))] Functions eq_refl eq_refl eq_refl
            ltac:(unfold G.E.H.host_values, ext; rewrite to_handle_to_externref; reflexivity)).
          apply rt_refl.
    Qed.

    (** The native constructor builds a strict or shallow Encode from a bare Thunk. *)
    Lemma native_map_thunk_executes state store locals th (strict : bool) :
      s_funcs store = s_funcs G.E.allocated_store ->
      let mode := if strict then 3 else 4 in
      let result := Encode (if strict then G.E.H.W.H.Strict th else G.E.H.W.H.Shallow th) in
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [word mode; ext (Thunk th)] ++ [AI_basic (BI_call 31%N)])
        (state, store, Build_frame locals G.E.collector_instance, v_to_e_list [ext result]).
    Proof.
      intro Functions; cbn zeta.
      set (arguments := [word (if strict then 3 else 4); ext (Thunk th)]).
      eapply G.E.collector_call_return with (code := native_map)
        (parameters := [T_num T_i32; T_ref T_externref]) (results := [T_ref T_externref])
        (defaults := []) (address := 31%N) (arguments := arguments)
        (values := [ext (Encode (if strict then G.E.H.W.H.Strict th else G.E.H.W.H.Shallow th))])
        (final_frame := Build_frame arguments G.E.collector_instance) (accepted := true)
        (context := LH_rec [] 0 [] (LH_base [] [])
          (to_e_list (skipn (if strict then 16 else 20) (modfunc_body native_map))));
        try (destruct strict; reflexivity).
      - rewrite Functions; reflexivity.
      - unfold native_map at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
        destruct strict; cbn [arguments] in *; [do 12 native_pure|do 15 native_pure].
        all: cbn [lfill].
        all: eapply G.E.collector_runs_context with (prefix := [])
          (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]);
          apply G.E.collector_runs_label.
        all: execute (native_type_query state store arguments 1%N (Thunk th) Functions eq_refl).
        all: do 2 native_pure.
        all: native_pure.
        + execute (native_value_import state store arguments 16%N shell_strict [ext (Thunk th)]
            [ext (Encode (G.E.H.W.H.Strict th))] Functions eq_refl eq_refl eq_refl
            ltac:(unfold G.E.H.host_values, ext; rewrite to_handle_to_externref; reflexivity)).
          apply rt_refl.
        + execute (native_value_import state store arguments 17%N shell_shallow [ext (Thunk th)]
            [ext (Encode (G.E.H.W.H.Shallow th))] Functions eq_refl eq_refl eq_refl
            ltac:(unfold G.E.H.host_values, ext; rewrite to_handle_to_externref; reflexivity)).
          apply rt_refl.
    Qed.

    (** The native constructor builds an identification Thunk from a Ref. *)
    Lemma native_map_identification_executes state store locals reference :
      s_funcs store = s_funcs G.E.allocated_store ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [word 2; ext (Data (Ref reference))] ++ [AI_basic (BI_call 31%N)])
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [ext (Thunk (G.E.H.W.H.Identification reference))]).
    Proof.
      intro Functions.
      set (arguments := [word 2; ext (Data (Ref reference))]).
      eapply G.E.collector_call_return with (code := native_map)
        (parameters := [T_num T_i32; T_ref T_externref]) (results := [T_ref T_externref])
        (defaults := []) (address := 31%N) (arguments := arguments)
        (values := [ext (Thunk (G.E.H.W.H.Identification reference))])
        (final_frame := Build_frame arguments G.E.collector_instance) (accepted := true)
        (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn 12 (modfunc_body native_map))));
        try reflexivity.
      - rewrite Functions; reflexivity.
      - unfold native_map at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
        do 9 native_pure; cbn [lfill].
        eapply G.E.collector_runs_context with (prefix := [])
          (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]);
          apply G.E.collector_runs_label.
        execute (native_type_query state store arguments 1%N (Data (Ref reference)) Functions eq_refl).
        do 3 native_pure.
        execute (native_value_import state store arguments 15%N shell_identification
          [ext (Data (Ref reference))] [ext (Thunk (G.E.H.W.H.Identification reference))]
          Functions eq_refl eq_refl eq_refl
          ltac:(unfold G.E.H.host_values, ext; rewrite to_handle_to_externref; reflexivity)).
        apply rt_refl.
    Qed.

    (** The native Ref constructor's guard accepts an Object of the corresponding kind. *)
    Lemma native_map_reference_guard state store locals object :
      s_funcs store = s_funcs G.E.allocated_store ->
      lookup_N locals 0%N = Some (word (match object with BlobObj _ => 5 | TreeObj _ => 6 end)) ->
      lookup_N locals 1%N = Some (ext (Data (Object object))) ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          to_e_list (skipn 24 (firstn 28 (modfunc_body native_map))))
        (state, store, Build_frame locals G.E.collector_instance, []).
    Proof.
      intros Functions Mode Input.
      unfold native_map; cbn [modfunc_body mod_funcs evaluation_module nth firstn skipn].
      execute (local_get_run state store locals 0%N
        (word (match object with BlobObj _ => 5 | TreeObj _ => 6 end)) Mode).
      destruct object as [blob|tree]; native_pure.
      - apply G.E.collector_if_void with (condition := true) (accepted := true).
        execute (native_data_type_query state store locals 1%N (Object (BlobObj blob)) Functions Input).
        do 2 native_pure; apply rt_refl.
      - apply G.E.collector_if_void with (condition := false) (accepted := true).
        execute (local_get_run state store locals 0%N (word 6) Mode).
        do 2 native_pure.
        execute (native_data_type_query state store locals 1%N (Object (TreeObj tree)) Functions Input).
        do 2 native_pure; apply rt_refl.
    Qed.

    (** The native constructor returns an Object's corresponding Ref. *)
    Lemma native_map_reference_executes state store locals object :
      s_funcs store = s_funcs G.E.allocated_store ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [word (match object with BlobObj _ => 5 | TreeObj _ => 6 end);
            ext (Data (Object object))] ++ [AI_basic (BI_call 31%N)])
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [ext (Data (Ref (G.E.H.W.H.data_reference (Object object))))]).
    Proof.
      intro Functions.
      set (arguments := [word (match object with BlobObj _ => 5 | TreeObj _ => 6 end);
        ext (Data (Object object))]).
      eapply G.E.collector_call with (code := native_map)
        (parameters := [T_num T_i32; T_ref T_externref]) (results := [T_ref T_externref])
        (defaults := []) (address := 31%N) (arguments := arguments)
        (values := [ext (Data (Ref (G.E.H.W.H.data_reference (Object object))))])
        (final_frame := Build_frame arguments G.E.collector_instance) (accepted := true); try reflexivity.
      - rewrite Functions; reflexivity.
      - unfold native_map at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
        destruct object as [blob|tree]; cbn [arguments] in *.
        all: do 15 native_pure.
        all: execute (native_type_query state store arguments 1%N _ Functions eq_refl).
        all: do 2 native_pure.
        + pose proof (native_map_reference_guard state store arguments (BlobObj blob) Functions eq_refl eq_refl) as Guard.
          unfold native_map in Guard; cbn [modfunc_body mod_funcs evaluation_module nth firstn skipn] in Guard.
          execute Guard.
          native_pure.
          execute (native_value_import state store arguments 11%N shell_create_ref
            [ext (Data (Object (BlobObj blob)))] [ext (Data (Ref (BlobRef blob)))]
            Functions eq_refl eq_refl eq_refl
            ltac:(unfold G.E.H.host_values, ext; rewrite to_handle_to_externref; reflexivity)).
          apply rt_refl.
        + pose proof (native_map_reference_guard state store arguments (TreeObj tree) Functions eq_refl eq_refl) as Guard.
          unfold native_map in Guard; cbn [modfunc_body mod_funcs evaluation_module nth firstn skipn] in Guard.
          execute Guard.
          native_pure.
          execute (native_value_import state store arguments 11%N shell_create_ref
            [ext (Data (Object (TreeObj tree)))] [ext (Data (Ref (TreeRef tree)))]
            Functions eq_refl eq_refl eq_refl
            ltac:(unfold G.E.H.host_values, ext; rewrite to_handle_to_externref; reflexivity)).
          apply rt_refl.
    Qed.

    Definition native_constructor_index kind := match kind with
      | X.Application => 0 | X.Selection => 1 | X.Identification => 2
      | X.Strict => 3 | X.Shallow => 4 | X.BlobReference => 5 | X.TreeReference => 6 end.

    (** Every successful abstract constructor has a matching successful native constructor
        execution. *)
    Lemma native_map_accepts state store locals kind input result :
      s_funcs store = s_funcs G.E.allocated_store -> X.construct kind input = Some result ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [word (native_constructor_index kind); ext input] ++ [AI_basic (BI_call 31%N)])
        (state, store, Build_frame locals G.E.collector_instance, v_to_e_list [ext result]).
    Proof.

      intros Functions Checked.
      destruct kind; destruct input as [[[blob|tree]|reference]|th|encoded];
        cbn [X.construct] in Checked; try discriminate; inversion Checked; subst result;
        cbn [native_constructor_index].
      - exact (native_map_tree_executes state store locals tree true Functions).
      - exact (native_map_tree_executes state store locals tree false Functions).
      - exact (native_map_identification_executes state store locals reference Functions).
      - exact (native_map_thunk_executes state store locals th true Functions).
      - exact (native_map_thunk_executes state store locals th false Functions).
      - exact (native_map_reference_executes state store locals (BlobObj blob) Functions).
      - exact (native_map_reference_executes state store locals (TreeObj tree) Functions).
    Qed.

    (** A ready identity observation executes the native exact-handle check successfully. *)
    Lemma native_same state store locals lhs rhs :
      s_funcs store = s_funcs G.E.allocated_store -> observed_same state lhs rhs ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [ext lhs; ext rhs] ++ [AI_basic (BI_call 19%N)])
        (state + 2, store, Build_frame locals G.E.collector_instance, []).
    Proof.
      intros Functions Compared; replace (state + 2) with (S (S state)) by lia.
      exact (same_run state store locals lhs rhs true Functions Compared).
    Qed.

    (** With ready comparisons and premise validation, the native mapped-congruence helper
        accepts the constructed endpoints. *)
    Lemma native_mapped_accepts state store locals kind lhs rhs witness claim a b state' store' :
      s_funcs store = s_funcs G.E.allocated_store ->
      endpoint_ready witness claim ->
      nth_error (get_tree_raw claim) 1 = Some a -> nth_error (get_tree_raw claim) 2 = Some b ->
      X.construct kind a = Some lhs -> X.construct kind b = Some rhs ->
      observed_same state lhs lhs -> observed_same (state + 2) rhs rhs ->
      premise_ready (state + 4) store witness 0 a b state' store' ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [word (native_constructor_index kind); ext lhs; ext rhs; ext witness] ++
            [AI_basic (BI_call 32%N)])
        (state', store', Build_frame locals G.E.collector_instance, []).
    Proof.

      intros Functions Ready Left Right ConstructLeft ConstructRight SameLeft SameRight Premise.
      set (arguments := [word (native_constructor_index kind); ext lhs; ext rhs; ext witness]).
      set (empty := VAL_ref (VAL_ref_null T_externref)).
      set (initial := arguments ++ [empty; empty]).
      set (with_left := arguments ++ [ext a; empty]).
      set (inside := arguments ++ [ext a; ext b]).
      eapply G.E.collector_call_void with (code := native_mapped)
        (parameters := [T_num T_i32; T_ref T_externref; T_ref T_externref; T_ref T_externref])
        (defaults := [empty; empty]) (address := 32%N) (arguments := arguments)
        (accepted := true) (final_frame := Build_frame inside G.E.collector_instance); try reflexivity.
      - rewrite Functions; reflexivity.
      - unfold native_mapped at 1; cbn [modfunc_body mod_funcs evaluation_module nth].

        native_pure.

        execute (exchange_endpoint_executes state store initial witness claim a true Functions Ready Left).

        do 2 native_pure.

        execute (exchange_endpoint_executes state store with_left witness claim b false
          Functions Ready Right).

        do 4 native_pure.

        execute_under [ext lhs] (native_map_accepts state store inside kind a lhs Functions ConstructLeft).
        execute (native_same state store inside lhs lhs Functions SameLeft).
        do 3 native_pure.
        execute_under [ext rhs] (native_map_accepts (state + 2) store inside kind b rhs Functions ConstructRight).
        execute (native_same (state + 2) store inside rhs rhs Functions SameRight).
        replace (state + 2 + 2) with (state + 4) by lia.
        do 3 native_pure.
        exact (collector_ready_premise_executes (state + 4) store witness 0 a b state' store' inside Premise).
    Qed.

    Definition native_mapped_rule kind := match kind with
      | X.Application => 4 | X.Strict => 9 | X.Selection => 17 | X.Identification => 18
      | X.Shallow => 19 | X.TreeReference => 21 | X.BlobReference => 22 end.
    Definition native_mapped_tail kind := match kind with
      | X.Application => 52 | X.Strict => 56 | X.Selection => 60 | X.Identification => 64
      | X.Shallow => 68 | X.TreeReference => 72 | X.BlobReference => 76 end.

    (** With ready witnesses, the mapped-congruence exchange returns the Equivalent claim
        code. *)
    Lemma native_mapped_exchange_accepts state store locals kind lhs rhs witnesses witness claim a b state' store' :
      s_funcs store = s_funcs G.E.allocated_store -> get_tree_raw witnesses = [witness] ->
      endpoint_ready witness claim ->
      nth_error (get_tree_raw claim) 1 = Some a -> nth_error (get_tree_raw claim) 2 = Some b ->
      X.construct kind a = Some lhs -> X.construct kind b = Some rhs ->
      observed_same state lhs lhs -> observed_same (state + 2) rhs rhs ->
      premise_ready (state + 4) store witness 0 a b state' store' ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 (native_mapped_rule kind)));
            ext lhs; ext rhs; ext (treeh witnesses)] ++ [AI_basic (BI_call 34%N)])
        (state', store', Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 0))]).
    Proof.
      intros Functions Fields Ready Left Right ConstructLeft ConstructRight SameLeft SameRight Premise.
      set (arguments := [VAL_num (VAL_int64 (wasm_nat64 (native_mapped_rule kind)));
        ext lhs; ext rhs; ext (treeh witnesses)]).
      set (empty := VAL_ref (VAL_ref_null T_externref)).
      set (initial := arguments ++ [empty; empty; empty; empty]).
      set (inside := arguments ++ [ext witness; empty; empty; empty]).
      assert (Arity : reduce_trans
        (state, store, Build_frame initial G.E.collector_instance,
          [AI_label 0 [] (to_e_list [BI_local_get 3%N; BI_const_num (VAL_int32 (wasm_nat 1)); BI_call 22%N])])
        (state, store, Build_frame initial G.E.collector_instance, [])).
      { eapply rt_trans; [apply G.E.collector_runs_label|apply rt_step, r_simple, rs_label_const; reflexivity].
        native_pure.
        exact (arity_run state store initial witnesses 1 Functions
          ltac:(rewrite Fields; reflexivity) ltac:(vm_compute; reflexivity)). }
      pose proof (native_mapped_accepts state store inside kind lhs rhs witness claim a b state' store'
        Functions Ready Left Right ConstructLeft ConstructRight SameLeft SameRight Premise) as Map.
      eapply G.E.collector_call_return with (code := native_exchange)
        (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
        (results := [T_num T_i64]) (address := 34%N) (defaults := [empty; empty; empty; empty])
        (arguments := arguments) (values := [VAL_num (VAL_int64 (wasm_nat64 0))])
        (final_frame := Build_frame inside G.E.collector_instance) (accepted := true)
        (context := LH_rec [] 0 [] (LH_base [] [])
          (to_e_list (skipn (native_mapped_tail kind) (modfunc_body native_exchange)))); try reflexivity.
      - rewrite Functions; reflexivity.
      - unfold native_exchange at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
        destruct kind; cbn [native_mapped_rule native_mapped_tail arguments] in *.
        all: repeat native_pure.
        all: execute Arity.
        all: native_pure.
        all: execute (child_run state store initial witnesses 0 witness Functions
          ltac:(vm_compute; reflexivity) ltac:(rewrite Fields; reflexivity)).
        all: native_pure.
        all: repeat native_pure.
        all: cbn [lfill].
        all: eapply G.E.collector_runs_context with (prefix := [])
          (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]);
          apply G.E.collector_runs_label.
        all: do 3 native_pure.
        all: execute Map; apply rt_refl.
    Qed.

    (** A proved unary branch execution lifts to execution of the full native exchange
        dispatcher. *)
    Lemma native_unary_dispatch state store locals index lhs rhs witnesses witness
        result state' store' (accepted : bool) :
      s_funcs store = s_funcs G.E.allocated_store -> get_tree_raw witnesses = [witness] ->
      index < 13 ->
      let arguments := [VAL_num (VAL_int64 (wasm_nat64 (nth index native_unary_codes 0)));
        ext lhs; ext rhs; ext (treeh witnesses)] in
      let empty := VAL_ref (VAL_ref_null T_externref) in
      let inside := arguments ++ [ext witness; empty; empty; empty] in
      reduce_trans
        (state, store, Build_frame inside G.E.collector_instance, to_e_list (native_unary_body index))
        (state', store', Build_frame inside G.E.collector_instance,
          if accepted then v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 result))] ++ [AI_basic BI_return]
          else [AI_trap]) ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list arguments ++ [AI_basic (BI_call 34%N)])
        (state', store', Build_frame locals G.E.collector_instance,
          if accepted then v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 result))] else [AI_trap]).
    Proof.
      intros Functions Fields Bound arguments empty inside Body.
      set (initial := arguments ++ [empty; empty; empty; empty]).
      assert (Arity : reduce_trans
        (state, store, Build_frame initial G.E.collector_instance,
          [AI_label 0 [] (to_e_list [BI_local_get 3%N; BI_const_num (VAL_int32 (wasm_nat 1)); BI_call 22%N])])
        (state, store, Build_frame initial G.E.collector_instance, [])).
      { eapply rt_trans; [apply G.E.collector_runs_label|apply rt_step, r_simple, rs_label_const; reflexivity].
        native_pure.
        exact (arity_run state store initial witnesses 1 Functions
          ltac:(rewrite Fields; reflexivity) ltac:(vm_compute; reflexivity)). }
      eapply G.E.collector_call_return with (code := native_exchange)
        (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
        (results := [T_num T_i64]) (address := 34%N) (defaults := [empty; empty; empty; empty])
        (arguments := arguments) (values := [VAL_num (VAL_int64 (wasm_nat64 result))])
        (final_frame := Build_frame inside G.E.collector_instance) (accepted := accepted)
        (context := LH_rec [] 0 [] (LH_base [] [])
          (to_e_list (skipn (52 + 4 * index) (modfunc_body native_exchange)))); try reflexivity.
      - rewrite Functions; reflexivity.
      - let finish := ltac:(
          cbn [native_unary_codes arguments initial inside nth] in *;
          unfold native_exchange at 1; cbn [modfunc_body mod_funcs evaluation_module nth];
          repeat native_pure;
          execute Arity;
          native_pure;
          execute (child_run state store initial witnesses 0 witness Functions
            ltac:(vm_compute; reflexivity) ltac:(rewrite Fields; reflexivity));
          native_pure;
          repeat native_pure;
          cbn [lfill]; rewrite ?List.app_nil_r;
          eapply G.E.collector_runs_context with (prefix := [])
            (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]);
          apply G.E.collector_runs_label; exact Body) in
        do 13 (destruct index as [|index]; [solve [finish]|]); lia.
    Qed.

    Ltac native_use_premise ready :=
      let specification := type of ready in
      lazymatch specification with
      | premise_ready ?state ?store ?witness ?kind ?lhs ?rhs ?state' ?store' =>
          lazymatch goal with
          | |- ?relation (_, _, Build_frame ?locals G.E.collector_instance, _) _ =>
              execute (collector_ready_premise_executes state store witness kind lhs rhs state' store' locals ready)
          end
      end.

    (** With a ready reversed equivalence witness, the native symmetry exchange accepts. *)
    Lemma native_symmetry state store locals lhs rhs witnesses witness state' store' :
      s_funcs store = s_funcs G.E.allocated_store -> get_tree_raw witnesses = [witness] ->
      premise_ready state store witness 0 rhs lhs state' store' ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 10)); ext lhs; ext rhs; ext (treeh witnesses)] ++
            [AI_basic (BI_call 34%N)])
        (state', store', Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 0))]).
    Proof.
      intros Functions Fields Premise.
      eapply native_unary_dispatch with (index := 8) (witness := witness) (accepted := true);
        try solve [assumption | lia].
      cbn [native_unary_body native_exchange modfunc_body mod_funcs evaluation_module nth Nat.add Nat.mul].
      do 3 native_pure; native_use_premise Premise; apply rt_refl.
    Qed.

    (** With a ready Eval witness, the native Eval-to-equivalence exchange accepts. *)
    Lemma native_eval_result_eq state store locals lhs rhs witnesses witness state' store' :
      s_funcs store = s_funcs G.E.allocated_store -> get_tree_raw witnesses = [witness] ->
      premise_ready state store witness 1 lhs rhs state' store' ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 20)); ext lhs; ext rhs; ext (treeh witnesses)] ++
            [AI_basic (BI_call 34%N)])
        (state', store', Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 0))]).
    Proof.
      intros Functions Fields Premise.
      eapply native_unary_dispatch with (index := 12) (witness := witness) (accepted := true);
        try solve [assumption | lia].
      cbn [native_unary_body native_exchange modfunc_body mod_funcs evaluation_module nth Nat.add Nat.mul].
      do 3 native_pure; native_use_premise Premise; apply rt_refl.
    Qed.

    (** With a ready Think witness returning Data, the native Think-to-Force exchange
        accepts. *)
    Lemma native_think_to_force state store locals lhs data witnesses witness state' store' :
      s_funcs store = s_funcs G.E.allocated_store -> get_tree_raw witnesses = [witness] ->
      premise_ready state store witness 4 lhs (Data data) state' store' ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 3)); ext lhs; ext (Data data); ext (treeh witnesses)] ++
            [AI_basic (BI_call 34%N)])
        (state', store', Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 3))]).
    Proof.
      intros Functions Fields Premise.
      eapply native_unary_dispatch with (index := 7) (witness := witness) (accepted := true);
        try solve [assumption | lia].
      cbn [native_unary_body native_exchange modfunc_body mod_funcs evaluation_module nth Nat.add Nat.mul].
      native_pure.
      lazymatch goal with
      | |- ?relation (_, _, Build_frame ?inside G.E.collector_instance, _) _ =>
          execute (native_data_executes state store inside data Functions)
      end.
      do 3 native_pure; native_use_premise Premise; apply rt_refl.
    Qed.

    (** With a ready Think witness returning a bare Thunk, the native Thunk-equivalence
        exchange accepts. *)
    Lemma native_thunk_eq state store locals lhs th witnesses witness state' store' :
      s_funcs store = s_funcs G.E.allocated_store -> get_tree_raw witnesses = [witness] ->
      premise_ready state store witness 4 lhs (Thunk th) state' store' ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 13)); ext lhs; ext (Thunk th); ext (treeh witnesses)] ++
            [AI_basic (BI_call 34%N)])
        (state', store', Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 0))]).
    Proof.
      intros Functions Fields Premise.
      eapply native_unary_dispatch with (index := 9) (witness := witness) (accepted := true);
        try solve [assumption | lia].
      cbn [native_unary_body native_exchange modfunc_body mod_funcs evaluation_module nth Nat.add Nat.mul].
      lazymatch goal with
      | |- ?relation (_, _, Build_frame ?inside G.E.collector_instance, _) _ =>
          execute (native_type_query state store inside 2%N (Thunk th) Functions eq_refl)
      end.
      do 5 native_pure; native_use_premise Premise; apply rt_refl.
    Qed.

    (** With a ready Think witness returning an Encode, the native exchange accepts
        equivalence to its bare Thunk. *)
    Lemma native_thunk_encode_eq state store locals lhs th witnesses witness state' store' (strict : bool) :
      s_funcs store = s_funcs G.E.allocated_store -> get_tree_raw witnesses = [witness] ->
      premise_ready state store witness 4 lhs
        (Encode (if strict then G.E.H.W.H.Strict th else G.E.H.W.H.Shallow th)) state' store' ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 (if strict then 14 else 15)));
            ext lhs; ext (Thunk th); ext (treeh witnesses)] ++ [AI_basic (BI_call 34%N)])
        (state', store', Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 0))]).
    Proof.
      intros Functions Fields Premise.
      destruct strict.
      - eapply native_unary_dispatch with (index := 10) (witness := witness) (accepted := true);
          try solve [assumption | lia].
        cbn [native_unary_body native_exchange modfunc_body mod_funcs evaluation_module nth Nat.add Nat.mul].
        do 3 native_pure.
        lazymatch goal with
        | |- ?relation (_, _, Build_frame ?inside G.E.collector_instance, _) _ =>
            execute_under [VAL_num (VAL_int64 (wasm_nat64 4)); ext witness; ext lhs]
              (native_map_thunk_executes state store inside th true Functions)
        end.
        native_use_premise Premise; apply rt_refl.
      - eapply native_unary_dispatch with (index := 11) (witness := witness) (accepted := true);
          try solve [assumption | lia].
        cbn [native_unary_body native_exchange modfunc_body mod_funcs evaluation_module nth Nat.add Nat.mul].
        do 3 native_pure.
        lazymatch goal with
        | |- ?relation (_, _, Build_frame ?inside G.E.collector_instance, _) _ =>
            execute_under [VAL_num (VAL_int64 (wasm_nat64 4)); ext witness; ext lhs]
              (native_map_thunk_executes state store inside th false Functions)
        end.
        native_use_premise Premise; apply rt_refl.
    Qed.

    (** With a ready self-comparison and no witnesses, the native reflexivity exchange
        accepts. *)
    Lemma native_reflexivity state store locals value witnesses :
      s_funcs store = s_funcs G.E.allocated_store -> get_tree_raw witnesses = [] ->
      observed_same state value value ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 12)); ext value; ext value; ext (treeh witnesses)] ++
            [AI_basic (BI_call 34%N)])
        (state + 2, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 0))]).
    Proof.
      intros Functions Fields Same.
      set (empty := VAL_ref (VAL_ref_null T_externref)).
      set (arguments := [VAL_num (VAL_int64 (wasm_nat64 12)); ext value; ext value; ext (treeh witnesses)]).
      set (inside := arguments ++ [empty; empty; empty; empty]).
      eapply G.E.collector_call_return with (code := native_exchange)
        (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
        (results := [T_num T_i64]) (address := 34%N) (defaults := [empty; empty; empty; empty])
        (arguments := arguments) (values := [VAL_num (VAL_int64 (wasm_nat64 0))])
        (final_frame := Build_frame inside G.E.collector_instance) (accepted := true)
        (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn 12 (modfunc_body native_exchange))));
        try reflexivity.
      - rewrite Functions; reflexivity.
      - change (reduce_trans (state, store, Build_frame inside G.E.collector_instance,
          to_e_list (modfunc_body native_exchange))
          (state + 2, store, Build_frame inside G.E.collector_instance,
            lfill (LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn 12 (modfunc_body native_exchange))))
              (v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 0))] ++ [AI_basic BI_return]))).
        unfold native_exchange at 1.
        cbn [modfunc_body mod_funcs evaluation_module nth].
        do 9 native_pure.
        cbn [lfill].
        eapply G.E.collector_runs_context with (prefix := [])
          (suffix := to_e_list (skipn 12 (modfunc_body native_exchange)))
          (instructions := [AI_label 0 [] (to_e_list (native_exchange_guard 2))])
          (instructions' := [AI_label 0 [] (v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 0))] ++ [AI_basic BI_return])]).
        apply G.E.collector_runs_label.
        change (reduce_trans (state, store, Build_frame inside G.E.collector_instance,
          to_e_list [BI_local_get 3%N; BI_const_num (VAL_int32 (wasm_nat 0)); BI_call 22%N;
            BI_local_get 1%N; BI_local_get 2%N; BI_call 19%N;
            BI_const_num (VAL_int64 (wasm_nat64 0)); BI_return])
          (state + 2, store, Build_frame inside G.E.collector_instance,
            v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 0))] ++ [AI_basic BI_return])).
        native_pure.
        execute (arity_run state store inside witnesses 0 Functions ltac:(rewrite Fields; reflexivity) ltac:(vm_compute; reflexivity)).
        do 2 native_pure.
        execute (same_run state store inside value value true Functions Same).
        replace (state + 2) with (S (S state)) by lia; apply rt_refl.
    Qed.

    (** The native leaf guard accepts exactly the input forms admitted by the abstract leaf
        predicate on this success path. *)
    Lemma native_leaf_guard state store locals input :
      s_funcs store = s_funcs G.E.allocated_store ->
      lookup_N locals 1%N = Some (ext input) -> X.reflexive_eval input = true ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          to_e_list [BI_local_get 1%N; BI_call 0%N; BI_const_num (VAL_int32 (wasm_nat 3));
            BI_relop T_i32 (Relop_i ROI_ne); BI_call 18%N;
            BI_local_get 1%N; BI_call 0%N; BI_testop T_i32 TO_eqz;
            BI_if (BT_valtype None)
              [BI_local_get 1%N; BI_call 1%N; BI_testop T_i32 TO_eqz; BI_call 18%N] []])
        (state, store, Build_frame locals G.E.collector_instance, []).
    Proof.
      intros Functions Input Leaf.
      destruct input as [[[blob|tree]|reference]|th|encoded]; cbn [X.reflexive_eval] in Leaf; try discriminate.
      all: execute (native_type_query state store locals 1%N _ Functions Input).
      all: do 2 native_pure.
      all: execute (native_type_query state store locals 1%N _ Functions Input).
      all: native_pure.
      - apply G.E.collector_if_void with (condition := true) (accepted := true).
        execute (native_data_type_query state store locals 1%N (Object (BlobObj blob)) Functions Input).
        do 2 native_pure; apply rt_refl.
      - native_pure; apply rt_refl.
      - native_pure; apply rt_refl.
    Qed.

    (** With a ready self-comparison, the native leaf exchange certifies unchanged leaf
        evaluation. *)
    Lemma native_eval_leaf state store locals value witnesses :
      s_funcs store = s_funcs G.E.allocated_store -> get_tree_raw witnesses = [] ->
      X.reflexive_eval value = true -> observed_same state value value ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 5)); ext value; ext value; ext (treeh witnesses)] ++
            [AI_basic (BI_call 34%N)])
        (state + 2, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 1))]).
    Proof.
      intros Functions Fields Leaf Same.
      set (empty := VAL_ref (VAL_ref_null T_externref)).
      set (arguments := [VAL_num (VAL_int64 (wasm_nat64 5)); ext value; ext value; ext (treeh witnesses)]).
      set (inside := arguments ++ [empty; empty; empty; empty]).
      eapply G.E.collector_call_return with (code := native_exchange)
        (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
        (results := [T_num T_i64]) (address := 34%N) (defaults := [empty; empty; empty; empty])
        (arguments := arguments) (values := [VAL_num (VAL_int64 (wasm_nat64 1))])
        (final_frame := Build_frame inside G.E.collector_instance) (accepted := true)
        (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn 16 (modfunc_body native_exchange))));
        try reflexivity.
      - rewrite Functions; reflexivity.
      - unfold native_exchange at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
        do 12 native_pure; cbn [lfill].
        eapply G.E.collector_runs_context with (prefix := [])
          (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]);
          apply G.E.collector_runs_label.
        native_pure.
        execute (arity_run state store inside witnesses 0 Functions ltac:(rewrite Fields; reflexivity) ltac:(vm_compute; reflexivity)).
        execute (native_leaf_guard state store inside value Functions eq_refl Leaf).
        do 2 native_pure.
        execute (native_same state store inside value value Functions Same).
        apply rt_refl.
    Qed.

    (** With a ready source comparison, the native identification exchange accepts the Ref
        result. *)
    Lemma native_think_identification state store locals reference witnesses :
      s_funcs store = s_funcs G.E.allocated_store -> get_tree_raw witnesses = [] ->
      observed_same state (Thunk (G.E.H.W.H.Identification reference))
        (Thunk (G.E.H.W.H.Identification reference)) ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 26)); ext (Thunk (G.E.H.W.H.Identification reference));
            ext (Data (Ref reference)); ext (treeh witnesses)] ++ [AI_basic (BI_call 34%N)])
        (state + 2, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 4))]).
    Proof.
      intros Functions Fields Same.
      set (empty := VAL_ref (VAL_ref_null T_externref)).
      set (th := Thunk (G.E.H.W.H.Identification reference)).
      set (arguments := [VAL_num (VAL_int64 (wasm_nat64 26)); ext th;
        ext (Data (Ref reference)); ext (treeh witnesses)]).
      set (inside := arguments ++ [empty; empty; empty; empty]).
      eapply G.E.collector_call_return with (code := native_exchange)
        (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
        (results := [T_num T_i64]) (address := 34%N) (defaults := [empty; empty; empty; empty])
        (arguments := arguments) (values := [VAL_num (VAL_int64 (wasm_nat64 4))])
        (final_frame := Build_frame inside G.E.collector_instance) (accepted := true)
        (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn 20 (modfunc_body native_exchange))));
        try reflexivity.
      - rewrite Functions; reflexivity.
      - unfold native_exchange at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
        do 15 native_pure; cbn [lfill].
        eapply G.E.collector_runs_context with (prefix := [])
          (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]);
          apply G.E.collector_runs_label.
        native_pure.
        execute (arity_run state store inside witnesses 0 Functions ltac:(rewrite Fields; reflexivity) ltac:(vm_compute; reflexivity)).
        execute (native_type_query state store inside 2%N (Data (Ref reference)) Functions eq_refl).
        do 4 native_pure.
        execute_under [ext th] (native_value_import state store inside 15%N shell_identification
          [ext (Data (Ref reference))] [ext th] Functions eq_refl eq_refl eq_refl
          ltac:(unfold th, G.E.H.host_values, ext; rewrite to_handle_to_externref; reflexivity)).
        execute (native_same state store inside th th Functions Same).
        apply rt_refl.
    Qed.

    (** A ready premise starts with the collector's expected function store. *)
    Lemma premise_ready_functions state store witness kind lhs rhs state' store' :
      premise_ready state store witness kind lhs rhs state' store' ->
      s_funcs store = s_funcs G.E.allocated_store.
    Proof.
      unfold premise_ready; intro Ready.
      repeat lazymatch type of Ready with ex _ => destruct Ready as [? Ready] end.
      exact (proj1 (proj2 Ready)).
    Qed.

    (** A proved endpoint-based branch lifts to the full native exchange dispatcher,
        including its success or trap result. *)
    Lemma native_endpoint_dispatch state store locals index lhs rhs witnesses witness claim a b
        result state' store' (accepted : bool) :
      s_funcs store = s_funcs G.E.allocated_store -> index < 7 ->
      List.length (get_tree_raw witnesses) = (if (index =? 1) || (index =? 4) then 1 else 2) ->
      nth_error (get_tree_raw witnesses) 0 = Some witness ->
      endpoint_ready witness claim ->
      nth_error (get_tree_raw claim) 1 = Some a -> nth_error (get_tree_raw claim) 2 = Some b ->
      let arguments := [VAL_num (VAL_int64 (wasm_nat64 (nth index native_endpoint_codes 0)));
        ext lhs; ext rhs; ext (treeh witnesses)] in
      let empty := VAL_ref (VAL_ref_null T_externref) in
      let inside := arguments ++ [ext witness; empty; ext a; ext b] in
      reduce_trans
        (state, store, Build_frame inside G.E.collector_instance, to_e_list (native_endpoint_body index))
        (state', store', Build_frame inside G.E.collector_instance,
          if accepted then v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 result))] ++ [AI_basic BI_return]
          else [AI_trap]) ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list arguments ++ [AI_basic (BI_call 34%N)])
        (state', store', Build_frame locals G.E.collector_instance,
          if accepted then v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 result))] else [AI_trap]).
    Proof.
      intros Functions Bound Count Witness Ready Left Right arguments empty inside Body.
      set (initial := arguments ++ [empty; empty; empty; empty]).
      set (with_p := arguments ++ [ext witness; empty; empty; empty]).
      set (with_a := arguments ++ [ext witness; empty; ext a; empty]).
      assert (Arity : reduce_trans
        (state, store, Build_frame initial G.E.collector_instance,
          [AI_label 0 [] (to_e_list [BI_local_get 3%N;
            BI_const_num (VAL_int32 (wasm_nat (if (index =? 1) || (index =? 4) then 1 else 2))); BI_call 22%N])])
        (state, store, Build_frame initial G.E.collector_instance, [])).
      { eapply rt_trans; [apply G.E.collector_runs_label|apply rt_step, r_simple, rs_label_const; reflexivity].
        native_pure.
        exact (arity_run state store initial witnesses _ Functions Count
          ltac:(destruct index as [|[|[|[|[|index]]]]]; vm_compute; reflexivity)). }
      eapply G.E.collector_call_return with (code := native_exchange)
        (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
        (results := [T_num T_i64]) (address := 34%N) (defaults := [empty; empty; empty; empty])
        (arguments := arguments) (values := [VAL_num (VAL_int64 (wasm_nat64 result))])
        (final_frame := Build_frame inside G.E.collector_instance) (accepted := accepted)
        (context := LH_rec [] 0 [] (LH_base [] [])
          (to_e_list (skipn (110 + 4 * index) (modfunc_body native_exchange)))); try reflexivity.
      - rewrite Functions; reflexivity.
      - let finish := ltac:(
          cbn [native_endpoint_codes arguments initial with_p with_a inside nth] in *;
          unfold native_exchange at 1; cbn [modfunc_body mod_funcs evaluation_module nth];
          repeat native_pure;
          execute Arity;
          native_pure;
          execute (child_run state store initial witnesses 0 witness Functions
            ltac:(vm_compute; reflexivity) Witness);
          repeat native_pure;
          execute (exchange_endpoint_executes state store with_p witness claim a true Functions Ready Left);
          do 2 native_pure;
          execute (exchange_endpoint_executes state store with_a witness claim b false Functions Ready Right);
          repeat native_pure;
          cbn [lfill]; rewrite ?List.app_nil_r;
          eapply G.E.collector_runs_context with (prefix := [])
            (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]);
          apply G.E.collector_runs_label; exact Body) in
        do 7 (destruct index as [|index]; [solve [finish]|]); lia.
    Qed.

    Ltac native_binary_premises first_premise second_premise fields :=
      do 3 native_pure; native_use_premise first_premise; native_pure;
      let tree := lazymatch type of fields with get_tree_raw ?tree = _ => tree end in
      lazymatch type of second_premise with
      | premise_ready ?state ?store ?witness ?kind ?lhs ?rhs ?state' ?store' =>
        lazymatch goal with
        | |- ?relation (_, _, Build_frame ?inside G.E.collector_instance, _) _ =>
          execute_under [VAL_num (VAL_int64 (wasm_nat64 kind))]
            (child_run state store inside tree 1 witness
              (premise_ready_functions _ _ _ _ _ _ _ _ second_premise)
              ltac:(vm_compute; reflexivity) ltac:(rewrite fields; reflexivity))
        end
      end;
      do 2 native_pure; native_use_premise second_premise; apply rt_refl.

    (** Ready ordered witnesses sharing the middle endpoint make the native transitivity
        exchange accept. *)
    Lemma native_transitivity state store locals lhs rhs witnesses witness second claim a middle
        state1 store1 state2 store2 :
      s_funcs store = s_funcs G.E.allocated_store -> get_tree_raw witnesses = [witness; second] ->
      endpoint_ready witness claim ->
      nth_error (get_tree_raw claim) 1 = Some a -> nth_error (get_tree_raw claim) 2 = Some middle ->
      premise_ready state store witness 0 lhs middle state1 store1 ->
      premise_ready state1 store1 second 0 middle rhs state2 store2 ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 11)); ext lhs; ext rhs; ext (treeh witnesses)] ++
            [AI_basic (BI_call 34%N)])
        (state2, store2, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 0))]).
    Proof.
      intros Functions Fields Ready Left Right First Second.
      eapply native_endpoint_dispatch with (index := 3) (witness := witness) (claim := claim)
        (a := a) (b := middle) (accepted := true);
        try solve [assumption | lia | rewrite Fields; reflexivity].
      cbn [native_endpoint_body native_exchange modfunc_body mod_funcs evaluation_module nth Nat.add Nat.mul].
      native_binary_premises First Second Fields.
    Qed.

    (** Ready Eval and equivalence witnesses make the native evaluation-substitution
        exchange accept. *)
    Lemma native_eval_eq state store locals lhs rhs witnesses witness second claim a middle
        state1 store1 state2 store2 :
      s_funcs store = s_funcs G.E.allocated_store -> get_tree_raw witnesses = [witness; second] ->
      endpoint_ready witness claim ->
      nth_error (get_tree_raw claim) 1 = Some a -> nth_error (get_tree_raw claim) 2 = Some middle ->
      premise_ready state store witness 1 a rhs state1 store1 ->
      premise_ready state1 store1 second 0 a lhs state2 store2 ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 8)); ext lhs; ext rhs; ext (treeh witnesses)] ++
            [AI_basic (BI_call 34%N)])
        (state2, store2, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 1))]).
    Proof.
      intros Functions Fields Ready Left Right First Second.
      eapply native_endpoint_dispatch with (index := 2) (witness := witness) (claim := claim)
        (a := a) (b := middle) (accepted := true);
        try solve [assumption | lia | rewrite Fields; reflexivity].
      cbn [native_endpoint_body native_exchange modfunc_body mod_funcs evaluation_module nth Nat.add Nat.mul].
      native_binary_premises First Second Fields.
    Qed.

    (** Ready equivalence and Force witnesses make the native forcing-substitution exchange
        accept. *)
    Lemma native_force_eq state store locals lhs rhs witnesses witness second claim a middle
        state1 store1 state2 store2 :
      s_funcs store = s_funcs G.E.allocated_store -> get_tree_raw witnesses = [witness; second] ->
      endpoint_ready witness claim ->
      nth_error (get_tree_raw claim) 1 = Some a -> nth_error (get_tree_raw claim) 2 = Some middle ->
      premise_ready state store witness 0 a lhs state1 store1 ->
      premise_ready state1 store1 second 3 a rhs state2 store2 ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 24)); ext lhs; ext rhs; ext (treeh witnesses)] ++
            [AI_basic (BI_call 34%N)])
        (state2, store2, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 3))]).
    Proof.
      intros Functions Fields Ready Left Right First Second.
      eapply native_endpoint_dispatch with (index := 5) (witness := witness) (claim := claim)
        (a := a) (b := middle) (accepted := true);
        try solve [assumption | lia | rewrite Fields; reflexivity].
      cbn [native_endpoint_body native_exchange modfunc_body mod_funcs evaluation_module nth Nat.add Nat.mul].
      native_binary_premises First Second Fields.
    Qed.

    (** Ready Eval and Apply witnesses make the native Think-application exchange accept. *)
    Lemma native_think_application state store locals input ready result witnesses witness second claim
        state1 store1 state2 store2 :
      s_funcs store = s_funcs G.E.allocated_store -> get_tree_raw witnesses = [witness; second] ->
      endpoint_ready witness claim ->
      nth_error (get_tree_raw claim) 1 = Some (treeh input) -> nth_error (get_tree_raw claim) 2 = Some ready ->
      observed_same state (Thunk (G.E.H.W.H.Application input)) (Thunk (G.E.H.W.H.Application input)) ->
      premise_ready (state + 2) store witness 1 (treeh input) ready state1 store1 ->
      premise_ready state1 store1 second 2 ready result state2 store2 ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 2)); ext (Thunk (G.E.H.W.H.Application input));
            ext result; ext (treeh witnesses)] ++ [AI_basic (BI_call 34%N)])
        (state2, store2, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 4))]).
    Proof.
      intros Functions Fields Ready Left Right Same First Second.
      eapply native_endpoint_dispatch with (index := 0) (witness := witness) (claim := claim)
        (a := treeh input) (b := ready) (accepted := true);
        try solve [assumption | lia | rewrite Fields; reflexivity].
      cbn [native_endpoint_body native_exchange modfunc_body mod_funcs evaluation_module nth Nat.add Nat.mul].
      do 2 native_pure.
      lazymatch goal with
      | |- ?relation (_, _, Build_frame ?inside G.E.collector_instance, _) _ =>
          execute_under [ext (Thunk (G.E.H.W.H.Application input))]
            (native_map_tree_executes state store inside input true Functions);
          execute (native_same state store inside (Thunk (G.E.H.W.H.Application input))
            (Thunk (G.E.H.W.H.Application input)) Functions Same)
      end.
      do 3 native_pure; native_use_premise First.
      native_pure.
      lazymatch goal with
      | |- ?relation (_, _, Build_frame ?inside G.E.collector_instance, _) _ =>
          execute_under [VAL_num (VAL_int64 (wasm_nat64 2))]
            (child_run state1 store1 inside witnesses 1 second
              (premise_ready_functions _ _ _ _ _ _ _ _ Second)
              ltac:(vm_compute; reflexivity) ltac:(rewrite Fields; reflexivity))
      end.
      do 2 native_pure; native_use_premise Second; apply rt_refl.
    Qed.

    (** Ready Eval and Select witnesses make the native Think-selection exchange accept. *)
    Lemma native_think_selection state store locals input ready result witnesses witness second claim
        state1 store1 state2 store2 :
      s_funcs store = s_funcs G.E.allocated_store -> get_tree_raw witnesses = [witness; second] ->
      endpoint_ready witness claim ->
      nth_error (get_tree_raw claim) 1 = Some (treeh input) -> nth_error (get_tree_raw claim) 2 = Some ready ->
      observed_same state (Thunk (G.E.H.W.H.Selection input)) (Thunk (G.E.H.W.H.Selection input)) ->
      premise_ready (state + 2) store witness 1 (treeh input) ready state1 store1 ->
      premise_ready state1 store1 second 6 ready result state2 store2 ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 25)); ext (Thunk (G.E.H.W.H.Selection input));
            ext result; ext (treeh witnesses)] ++ [AI_basic (BI_call 34%N)])
        (state2, store2, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 4))]).
    Proof.
      intros Functions Fields Ready Left Right Same First Second.
      eapply native_endpoint_dispatch with (index := 6) (witness := witness) (claim := claim)
        (a := treeh input) (b := ready) (accepted := true);
        try solve [assumption | lia | rewrite Fields; reflexivity].
      cbn [native_endpoint_body native_exchange modfunc_body mod_funcs evaluation_module nth Nat.add Nat.mul].
      do 2 native_pure.
      lazymatch goal with
      | |- ?relation (_, _, Build_frame ?inside G.E.collector_instance, _) _ =>
          execute_under [ext (Thunk (G.E.H.W.H.Selection input))]
            (native_map_tree_executes state store inside input false Functions);
          execute (native_same state store inside (Thunk (G.E.H.W.H.Selection input))
            (Thunk (G.E.H.W.H.Selection input)) Functions Same)
      end.
      do 3 native_pure; native_use_premise First.
      native_pure.
      lazymatch goal with
      | |- ?relation (_, _, Build_frame ?inside G.E.collector_instance, _) _ =>
          execute_under [VAL_num (VAL_int64 (wasm_nat64 6))]
            (child_run state1 store1 inside witnesses 1 second
              (premise_ready_functions _ _ _ _ _ _ _ _ Second)
              ltac:(vm_compute; reflexivity) ltac:(rewrite Fields; reflexivity))
      end.
      do 2 native_pure; native_use_premise Second; apply rt_refl.
    Qed.

    (** Ready Force, Force, and result-equivalence witnesses make the native
        Force-result-equivalence exchange accept. *)
    Lemma native_force_results state store locals lhs rhs witnesses first second third first_claim second_claim
        a b state1 store1 state2 store2 state3 store3 :
      s_funcs store = s_funcs G.E.allocated_store -> get_tree_raw witnesses = [first; second; third] ->
      endpoint_ready first first_claim -> endpoint_ready second second_claim ->
      nth_error (get_tree_raw first_claim) 2 = Some a -> nth_error (get_tree_raw second_claim) 2 = Some b ->
      premise_ready state store first 3 lhs a state1 store1 ->
      premise_ready state1 store1 second 3 rhs b state2 store2 ->
      premise_ready state2 store2 third 0 a b state3 store3 ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 1)); ext lhs; ext rhs; ext (treeh witnesses)] ++
            [AI_basic (BI_call 34%N)])
        (state3, store3, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 0))]).
    Proof.
      intros Functions Fields FirstReady SecondReady Left Right First Second Third.
      set (empty := VAL_ref (VAL_ref_null T_externref)).
      set (arguments := [VAL_num (VAL_int64 (wasm_nat64 1)); ext lhs; ext rhs; ext (treeh witnesses)]).
      set (initial := arguments ++ [empty; empty; empty; empty]).
      set (with_p := arguments ++ [ext first; empty; empty; empty]).
      set (with_q := arguments ++ [ext first; ext second; empty; empty]).
      set (with_a := arguments ++ [ext first; ext second; ext a; empty]).
      set (inside := arguments ++ [ext first; ext second; ext a; ext b]).
      eapply G.E.collector_call_return with (code := native_exchange)
        (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
        (results := [T_num T_i64]) (address := 34%N) (defaults := [empty; empty; empty; empty])
        (arguments := arguments) (values := [VAL_num (VAL_int64 (wasm_nat64 0))])
        (final_frame := Build_frame inside G.E.collector_instance) (accepted := true)
        (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn 24 (modfunc_body native_exchange))));
        try reflexivity.
      - rewrite Functions; reflexivity.
      - unfold native_exchange at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
        do 18 native_pure; cbn [lfill].
        eapply G.E.collector_runs_context with (prefix := [])
          (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]);
          apply G.E.collector_runs_label.
        native_pure.
        execute (arity_run state store initial witnesses 3 Functions
          ltac:(rewrite Fields; reflexivity) ltac:(vm_compute; reflexivity)).
        native_pure.
        execute (child_run state store initial witnesses 0 first Functions
          ltac:(vm_compute; reflexivity) ltac:(rewrite Fields; reflexivity)).
        do 2 native_pure.
        execute (child_run state store with_p witnesses 1 second Functions
          ltac:(vm_compute; reflexivity) ltac:(rewrite Fields; reflexivity)).
        do 2 native_pure.
        execute (exchange_endpoint_executes state store with_q first first_claim a false Functions FirstReady Left).
        do 2 native_pure.
        execute (exchange_endpoint_executes state store with_a second second_claim b false Functions SecondReady Right).
        do 4 native_pure; native_use_premise First.
        do 3 native_pure; native_use_premise Second.
        native_pure.
        execute_under [VAL_num (VAL_int64 (wasm_nat64 0))]
          (child_run state2 store2 inside witnesses 2 third
            (premise_ready_functions _ _ _ _ _ _ _ _ Third)
            ltac:(vm_compute; reflexivity) ltac:(rewrite Fields; reflexivity)).
        do 2 native_pure; native_use_premise Third; apply rt_refl.
    Qed.

    (** The native lowering helper returns the lowered form of Data. *)
    Lemma native_lower_executes state store locals input :
      s_funcs store = s_funcs G.E.allocated_store ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [ext (Data input)] ++ [AI_basic (BI_call 24%N)])
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [ext (lower (Data input))]).
    Proof.
      intro Functions; rewrite X.D.EC.E.EP.E.lower_data_reference.
      destruct input as [object|reference].
      - set (arguments := [ext (Data (Object object))]).
        eapply G.E.collector_call_return with (code := native_lower)
          (parameters := [T_ref T_externref]) (results := [T_ref T_externref])
          (address := 24%N) (defaults := []) (arguments := arguments)
          (values := [ext (Data (Ref (G.E.H.W.H.data_reference (Object object))))])
          (final_frame := Build_frame arguments G.E.collector_instance) (accepted := true)
          (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn 4 (modfunc_body native_lower))));
          try reflexivity.
        + rewrite Functions; reflexivity.
        + unfold native_lower at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
          execute (native_type_query state store arguments 0%N (Data (Object object)) Functions eq_refl).
          do 2 native_pure; cbn [lfill].
          eapply G.E.collector_runs_context with (prefix := [])
            (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]);
            apply G.E.collector_runs_label.
          native_pure.
          execute (native_value_import state store arguments 11%N shell_create_ref
            [ext (Data (Object object))] [ext (Data (Ref (G.E.H.W.H.data_reference (Object object))))]
            Functions eq_refl eq_refl eq_refl
            ltac:(unfold G.E.H.host_values, ext; rewrite to_handle_to_externref; reflexivity)).
          apply rt_refl.
      - set (arguments := [ext (Data (Ref reference))]).
        eapply G.E.collector_call with (code := native_lower)
          (parameters := [T_ref T_externref]) (results := [T_ref T_externref])
          (address := 24%N) (defaults := []) (arguments := arguments)
          (values := [ext (Data (Ref reference))])
          (final_frame := Build_frame arguments G.E.collector_instance) (accepted := true);
          try reflexivity.
        + rewrite Functions; reflexivity.
        + unfold native_lower at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
          execute (native_type_query state store arguments 0%N (Data (Ref reference)) Functions eq_refl).
          do 2 native_pure.
          execute (native_type_query state store arguments 0%N (Data (Ref reference)) Functions eq_refl).
          do 3 native_pure; apply rt_refl.
    Qed.

    (** A ready reference comparison lets the native helper authenticate the supplied
        Object. *)
    Lemma native_object_executes state store locals reference object :
      s_funcs store = s_funcs G.E.allocated_store ->
      observed_same state reference (Data (Ref (G.E.H.W.H.data_reference (Object object)))) ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [ext reference; ext (Data (Object object))] ++ [AI_basic (BI_call 25%N)])
        (state + 2, store, Build_frame locals G.E.collector_instance, []).
    Proof.
      intros Functions [left [right [Left [Right Compared]]]].
      replace (state + 2) with (S (S state)) by lia.
      eapply G.collector_object_executes with (equal := true); eassumption.
    Qed.

    (** The native strict-result check accepts the lifted Data result under the stated
        local and comparison readiness conditions. *)
    Lemma native_strict_result_executes state store locals input :
      s_funcs store = s_funcs G.E.allocated_store ->
      lookup_N locals 2%N = Some (ext (lift (Data input))) ->
      lookup_N locals 7%N = Some (ext (Data input)) -> observed_same state (Data input) (Data input) ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          to_e_list [BI_local_get 7%N; BI_call 0%N; BI_testop T_i32 TO_eqz;
            BI_if (BT_valtype None)
              [BI_local_get 2%N; BI_local_get 7%N; BI_call 19%N]
              [BI_local_get 7%N; BI_local_get 2%N; BI_call 25%N]])
        (state + 2, store, Build_frame locals G.E.collector_instance, []).
    Proof.
      intros Functions Lifted Input Same; destruct input as [object|[blob|tree]].
      all: execute (native_type_query state store locals 7%N _ Functions Input).
      all: do 2 native_pure.
      all: eapply rt_trans;
        [apply G.E.collector_runs_label|apply rt_step, r_simple, rs_label_const; reflexivity].
      - execute (local_get_run state store locals 2%N (ext (Data (Object object))) Lifted).
        execute_under [ext (Data (Object object))]
          (local_get_run state store locals 7%N (ext (Data (Object object))) Input).
        exact (native_same state store locals _ _ Functions Same).
      - execute (local_get_run state store locals 7%N (ext (Data (Ref (BlobRef blob)))) Input).
        execute_under [ext (Data (Ref (BlobRef blob)))]
          (local_get_run state store locals 2%N (ext (blobh blob)) Lifted).
        exact (native_object_executes state store locals _ (BlobObj blob) Functions Same).
      - execute (local_get_run state store locals 7%N (ext (Data (Ref (TreeRef tree)))) Input).
        execute_under [ext (Data (Ref (TreeRef tree)))]
          (local_get_run state store locals 2%N (ext (treeh tree)) Lifted).
        exact (native_object_executes state store locals _ (TreeObj tree) Functions Same).
    Qed.

    (** A ready Force witness and endpoint comparisons make the native strict-Encode
        equivalence exchange accept. *)
    Lemma native_force_strict state store locals th data witnesses witness claim state' store' :
      s_funcs store = s_funcs G.E.allocated_store -> get_tree_raw witnesses = [witness] ->
      endpoint_ready witness claim ->
      nth_error (get_tree_raw claim) 1 = Some (Thunk th) ->
      nth_error (get_tree_raw claim) 2 = Some (Data data) ->
      observed_same state (Encode (G.E.H.W.H.Strict th)) (Encode (G.E.H.W.H.Strict th)) ->
      observed_same (state + 2) (Data data) (Data data) ->
      premise_ready (state + 4) store witness 3 (Thunk th) (Data data) state' store' ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 7)); ext (Encode (G.E.H.W.H.Strict th));
            ext (lift (Data data)); ext (treeh witnesses)] ++ [AI_basic (BI_call 34%N)])
        (state', store', Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 0))]).
    Proof.
      intros Functions Fields Ready Left Right Same Result Premise.
      eapply native_endpoint_dispatch with (index := 1) (witness := witness) (claim := claim)
        (a := Thunk th) (b := Data data) (accepted := true);
        try solve [assumption | lia | rewrite Fields; reflexivity].
      cbn [native_endpoint_body native_exchange modfunc_body mod_funcs evaluation_module nth Nat.add Nat.mul].
      do 2 native_pure.
      lazymatch goal with
      | |- ?relation (_, _, Build_frame ?inside G.E.collector_instance, _) _ =>
          execute_under [ext (Encode (G.E.H.W.H.Strict th))]
            (native_map_thunk_executes state store inside th true Functions);
          execute (native_same state store inside (Encode (G.E.H.W.H.Strict th))
            (Encode (G.E.H.W.H.Strict th)) Functions Same)
      end.
      native_pure.
      lazymatch goal with
      | |- ?relation (_, _, Build_frame ?inside G.E.collector_instance, _) _ =>
          execute (native_data_executes (state + 2) store inside data Functions);
          execute (native_strict_result_executes (state + 2) store inside data Functions eq_refl eq_refl Result)
      end.
      replace (state + 2 + 2) with (state + 4) by lia.
      do 3 native_pure; native_use_premise Premise; apply rt_refl.
    Qed.

    (** A ready Force witness and endpoint comparisons make the native shallow-Encode
        equivalence exchange accept. *)
    Lemma native_force_shallow state store locals th data witnesses witness claim state' store' :
      s_funcs store = s_funcs G.E.allocated_store -> get_tree_raw witnesses = [witness] ->
      endpoint_ready witness claim ->
      nth_error (get_tree_raw claim) 1 = Some (Thunk th) ->
      nth_error (get_tree_raw claim) 2 = Some (Data data) ->
      observed_same state (Encode (G.E.H.W.H.Shallow th)) (Encode (G.E.H.W.H.Shallow th)) ->
      observed_same (state + 2) (lower (Data data)) (lower (Data data)) ->
      premise_ready (state + 4) store witness 3 (Thunk th) (Data data) state' store' ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 16)); ext (Encode (G.E.H.W.H.Shallow th));
            ext (lower (Data data)); ext (treeh witnesses)] ++ [AI_basic (BI_call 34%N)])
        (state', store', Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 0))]).
    Proof.
      intros Functions Fields Ready Left Right Same Result Premise.
      eapply native_endpoint_dispatch with (index := 4) (witness := witness) (claim := claim)
        (a := Thunk th) (b := Data data) (accepted := true);
        try solve [assumption | lia | rewrite Fields; reflexivity].
      cbn [native_endpoint_body native_exchange modfunc_body mod_funcs evaluation_module nth Nat.add Nat.mul].
      do 2 native_pure.
      lazymatch goal with
      | |- ?relation (_, _, Build_frame ?inside G.E.collector_instance, _) _ =>
          execute_under [ext (Encode (G.E.H.W.H.Shallow th))]
            (native_map_thunk_executes state store inside th false Functions);
          execute (native_same state store inside (Encode (G.E.H.W.H.Shallow th))
            (Encode (G.E.H.W.H.Shallow th)) Functions Same)
      end.
      do 2 native_pure.
      lazymatch goal with
      | |- ?relation (_, _, Build_frame ?inside G.E.collector_instance, _) _ =>
          execute_under [ext (lower (Data data))] (native_lower_executes (state + 2) store inside data Functions);
          execute (native_same (state + 2) store inside (lower (Data data)) (lower (Data data)) Functions Result)
      end.
      replace (state + 2 + 2) with (state + 4) by lia.
      do 3 native_pure; native_use_premise Premise; apply rt_refl.
    Qed.

    (** The native Tag query returns the modeled marker of a Tree Object. *)
    Lemma native_tag_query state store locals index tree :
      s_funcs store = s_funcs G.E.allocated_store -> lookup_N locals index = Some (ext (treeh tree)) ->
      reduce_trans
        (state, store, Build_frame locals G.E.collector_instance,
          to_e_list [BI_local_get index; BI_call 3%N])
        (state, store, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int32 (wasm_bool (tree_tag tree))) ]).
    Proof.
      intros Functions Local.
      eapply G.collector_query_local with (operation := shell_is_tag) (value := treeh tree);
        try exact Local; try reflexivity.
      - rewrite Functions; reflexivity.
      - unfold G.E.H.host_values, ext; rewrite to_handle_to_externref.
        unfold G.E.H.W.Api.is_tag, G.E.H.W.H.HTreeObj, G.E.H.W.bool_result.
        destruct (tree_tag tree); reflexivity.
    Qed.

    Section TreeRules.
      Variable kind : nat.
      Variable lhs rhs witnesses : TreeName.
      Variable states : nat -> nat.
      Variable stores : nat -> store_record.
      Let count := List.length (get_tree_raw lhs).
      (** The number of Tree children fits the native unsigned 32-bit loop counter. *)
      Hypothesis count_bound : (Z.of_nat count < 2 ^ 32)%Z.
      (** The right Tree has the same number of children as the left Tree. *)
      Hypothesis right_length : List.length (get_tree_raw rhs) = count.
      (** There is exactly one ordered witness for each pair of Tree children. *)
      Hypothesis witness_length : List.length (get_tree_raw witnesses) = count.
      (** The two operand Trees have the same Tag marker. *)
      Hypothesis markers : tree_tag lhs = tree_tag rhs.
      (** Each pair of corresponding Tree children has a ready witness validation with the
          specified intermediate stores. *)
      Hypothesis entries_ready : forall index, index < count -> exists witness left right,
        nth_error (get_tree_raw witnesses) index = Some witness /\
        nth_error (get_tree_raw lhs) index = Some left /\
        nth_error (get_tree_raw rhs) index = Some right /\
        premise_ready (states index) (stores index) witness kind left right (states (S index)) (stores (S index)).

      Let tree_arguments :=
        [VAL_num (VAL_int64 (wasm_nat64 kind)); ext (treeh lhs); ext (treeh rhs); ext (treeh witnesses)].
      Let tree_locals index := tree_arguments ++ [word count; word index].
      Let tree_frame index := Build_frame (tree_locals index) G.E.collector_instance.

      (** Updating the native Tree-loop counter advances its local-state invariant by one. *)
      Lemma tree_counter_update index :
        seq.set_nth (word (S index)) (tree_locals index) 5 (word (S index)) = tree_locals (S index).
      Proof. reflexivity. Qed.

      (** One native Tree-loop iteration validates the next child witness and advances the
          counter and store. *)
      Lemma tree_iteration index : index < count ->
        reduce_trans
          (states index, stores index, tree_frame index, to_e_list native_tree_loop)
          (states (S index), stores (S index), tree_frame (S index), [AI_basic (BI_br 0%N)]).
      Proof.
        intro Bound.
        destruct (entries_ready index Bound) as [witness [left [right [Witness [Left [Right Ready]]]]]].
        assert (Functions : s_funcs (stores index) = s_funcs G.E.allocated_store)
          by exact (premise_ready_functions _ _ _ _ _ _ _ _ Ready).
        assert (IndexBound : (Z.of_nat index < 2 ^ 32)%Z) by lia.
        pose proof (counter_test_run (states index) (stores index) (tree_locals index) 5%N 4%N
          index count IndexBound count_bound eq_refl eq_refl) as Test.
        assert (Continue : Nat.eqb index count = false) by (apply Nat.eqb_neq; lia).
        rewrite Continue in Test.
        unfold tree_frame at 1; unfold native_tree_loop.
        execute Test; do 3 native_pure.
        execute_under [VAL_num (VAL_int64 (wasm_nat64 kind))]
          (child_run (states index) (stores index) (tree_locals index) witnesses index witness Functions IndexBound Witness).
        do 2 native_pure.
        execute_under [VAL_num (VAL_int64 (wasm_nat64 kind)); ext witness]
          (child_run (states index) (stores index) (tree_locals index) lhs index left Functions IndexBound Left).
        do 2 native_pure.
        execute_under [VAL_num (VAL_int64 (wasm_nat64 kind)); ext witness; ext left]
          (child_run (states index) (stores index) (tree_locals index) rhs index right Functions IndexBound Right).
        native_use_premise Ready; native_pure.
        assert (Increment : reduce_trans
          (states (S index), stores (S index), tree_frame index,
            [v_to_e (word index); AI_basic (BI_const_num (VAL_int32 (wasm_nat 1)));
             AI_basic (BI_binop T_i32 (Binop_i BOI_add))])
          (states (S index), stores (S index), tree_frame index, v_to_e_list [word (S index)])).
        { apply rt_step, r_simple, wasm_nat_increment_step; exact IndexBound. }
        execute Increment.
        pose proof (local_set_run (states (S index)) (stores (S index)) (tree_locals index) 5%N
          (word (S index)) ltac:(unfold tree_locals, tree_arguments; cbn; lia)) as Update.
        change (reduce_trans
          (states (S index), stores (S index), tree_frame index,
            [v_to_e (word (S index)); AI_basic (BI_local_set 5%N)])
          (states (S index), stores (S index),
            Build_frame (seq.set_nth (word (S index)) (tree_locals index) 5 (word (S index)))
              G.E.collector_instance, [])) in Update.
        rewrite tree_counter_update in Update; execute Update; apply rt_refl.
      Qed.

      (** The native Tree loop validates every corresponding child witness and exits with
          the resulting store. *)
      Lemma tree_loop_executes : reduce_trans
        (states 0, stores 0, tree_frame 0,
          to_e_list [BI_block (BT_valtype None) [BI_loop (BT_valtype None) native_tree_loop]])
        (states count, stores count, tree_frame count, []).
      Proof.
        eapply G.E.collector_loop_executes with (count := count) (states := states) (stores := stores) (frames := tree_frame).
        - exact tree_iteration.
        - exists (to_e_list (skipn 4 native_tree_loop)).
          pose proof (counter_test_run (states count) (stores count) (tree_locals count) 5%N 4%N
            count count count_bound count_bound eq_refl eq_refl) as Test.
          rewrite Nat.eqb_refl in Test.
          change (reduce_trans
            (states count, stores count, tree_frame count,
              to_e_list (firstn 4 native_tree_loop) ++ to_e_list (skipn 4 native_tree_loop))
            (states count, stores count, tree_frame count,
              [AI_basic (BI_br 1%N)] ++ to_e_list (skipn 4 native_tree_loop))).
          eapply G.E.collector_runs_context with (prefix := [])
            (instructions := to_e_list (firstn 4 native_tree_loop)) (instructions' := [AI_basic (BI_br 1%N)]).
          exact Test.
      Qed.

      (** The native Tree-rule helper accepts ready child witnesses with matching Tree
          markers and lengths. *)
      Lemma native_tree_rule_executes locals :
        s_funcs (stores 0) = s_funcs G.E.allocated_store ->
        reduce_trans
          (states 0, stores 0, Build_frame locals G.E.collector_instance,
            v_to_e_list tree_arguments ++ [AI_basic (BI_call 33%N)])
          (states count, stores count, Build_frame locals G.E.collector_instance, []).
      Proof.
        intro Functions; set (initial := tree_arguments ++ [word 0; word 0]).
        eapply G.E.collector_call_void with (code := native_tree_rule)
          (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
          (address := 33%N) (defaults := [word 0; word 0]) (arguments := tree_arguments)
          (accepted := true) (final_frame := tree_frame count); try reflexivity.
        - rewrite Functions; reflexivity.
        - unfold native_tree_rule at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
          native_pure.
          execute (G.collector_size_executes combination handle_bytes (states 0) (stores 0) initial lhs Functions count_bound).
          do 3 native_pure.
          execute (arity_run (states 0) (stores 0) (tree_locals 0) rhs count Functions right_length count_bound).
          do 2 native_pure.
          execute (arity_run (states 0) (stores 0) (tree_locals 0) witnesses count Functions witness_length count_bound).
          execute (native_tag_query (states 0) (stores 0) (tree_locals 0) 1%N lhs Functions eq_refl).
          pose proof (native_tag_query (states 0) (stores 0) (tree_locals 0) 2%N rhs Functions eq_refl) as RightTag.
          rewrite <- markers in RightTag.
          execute_under [VAL_num (VAL_int32 (wasm_bool (tree_tag lhs)))] RightTag.
          destruct (tree_tag lhs); do 2 native_pure; exact tree_loop_executes.
      Qed.
    End TreeRules.

    (** Ready pointwise Tree witnesses make the native Tree exchange return its requested
        claim code. *)
    Lemma native_tree_exchange (evaluation : bool) lhs rhs witnesses states stores locals :
      let kind := if evaluation then 1 else 0 in
      let count := List.length (get_tree_raw lhs) in
      s_funcs (stores 0) = s_funcs G.E.allocated_store ->
      (Z.of_nat count < 2 ^ 32)%Z ->
      List.length (get_tree_raw rhs) = count ->
      List.length (get_tree_raw witnesses) = count ->
      tree_tag lhs = tree_tag rhs ->
      (forall index, index < count -> exists witness left right,
        nth_error (get_tree_raw witnesses) index = Some witness /\
        nth_error (get_tree_raw lhs) index = Some left /\
        nth_error (get_tree_raw rhs) index = Some right /\
        premise_ready (states index) (stores index) witness kind left right (states (S index)) (stores (S index))) ->
      reduce_trans
        (states 0, stores 0, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 (if evaluation then 6 else 0)));
            ext (treeh lhs); ext (treeh rhs); ext (treeh witnesses)] ++ [AI_basic (BI_call 34%N)])
        (states count, stores count, Build_frame locals G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 kind))]).
    Proof.
      cbn zeta; intros Functions Bound RightLength WitnessLength Markers Entries.
      set (empty := VAL_ref (VAL_ref_null T_externref)).
      set (arguments := [VAL_num (VAL_int64 (wasm_nat64 (if evaluation then 6 else 0)));
        ext (treeh lhs); ext (treeh rhs); ext (treeh witnesses)]).
      set (inside := arguments ++ [empty; empty; empty; empty]).
      eapply G.E.collector_call_return with (code := native_exchange)
        (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
        (results := [T_num T_i64]) (address := 34%N) (defaults := [empty; empty; empty; empty])
        (arguments := arguments) (values := [VAL_num (VAL_int64 (wasm_nat64 (if evaluation then 1 else 0)))])
        (final_frame := Build_frame inside G.E.collector_instance) (accepted := true)
        (context := LH_rec [] 0 [] (LH_base [] [])
          (to_e_list (skipn (if evaluation then 8 else 4) (modfunc_body native_exchange)))); try reflexivity.
      - rewrite Functions; reflexivity.
      - assert (Run := native_tree_rule_executes (if evaluation then 1 else 0) lhs rhs witnesses states stores
          Bound RightLength WitnessLength Markers Entries inside Functions).
        destruct evaluation; unfold native_exchange at 1;
          cbn [modfunc_body mod_funcs evaluation_module nth];
          [do 6 native_pure|do 3 native_pure]; cbn [lfill];
          eapply G.E.collector_runs_context with (prefix := [])
            (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] _]);
          apply G.E.collector_runs_label; do 3 native_pure; execute Run; apply rt_refl.
    Qed.

    (** The collector entry-point header reads the expected request fields into its local
        state. *)
    Lemma collector_entry_header state initial_store opcode rule before page metadata callable value result witnesses :
      s_funcs initial_store = s_funcs G.E.allocated_store ->
      s_mems initial_store = [before] ->
      G.E.H.attachment_allowed before (get_blob_data opcode) = true ->
      attachment_page (get_blob_data opcode) = Some page ->
      decode_u64 (get_blob_data opcode) = Some rule ->
      get_tree_raw combination = [metadata; callable; blobh opcode; value; result; treeh witnesses] ->
      reduce_trans
        (state, initial_store,
          Build_frame [ext (treeh combination); VAL_ref (VAL_ref_null T_externref);
            VAL_ref (VAL_ref_null T_externref); VAL_num (VAL_int64 (wasm_nat64 0))] G.E.collector_instance,
          to_e_list (firstn 20 (modfunc_body evaluation_apply)))
        (state, upd_s_mem initial_store [page],
          Build_frame [ext (treeh combination); ext value; ext result;
            VAL_num (VAL_int64 (wasm_nat64 0))] G.E.collector_instance,
          v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 rule)); ext value; ext result; ext (treeh witnesses)]).
    Proof.
      intros Functions Memory Allowed Page Decoded Fields.
      set (empty := VAL_ref (VAL_ref_null T_externref)).
      set (initial := [ext (treeh combination); empty; empty; VAL_num (VAL_int64 (wasm_nat64 0))]).
      set (selected := [ext (treeh combination); ext value; ext result; VAL_num (VAL_int64 (wasm_nat64 0))]).
        unfold evaluation_apply at 1; cbn [modfunc_body mod_funcs evaluation_module nth firstn].
          native_pure.
          execute (arity_run state initial_store initial combination 6 Functions
            ltac:(rewrite Fields; reflexivity) ltac:(vm_compute; reflexivity)).
          native_pure.
          execute (child_run state initial_store initial combination 3 value Functions
            ltac:(vm_compute; reflexivity) ltac:(rewrite Fields; reflexivity)).
          do 2 native_pure.
          lazymatch goal with
          | |- ?relation (_, _, Build_frame ?inside G.E.collector_instance, _) _ =>
            execute (child_run state initial_store inside combination 4 result Functions
              ltac:(vm_compute; reflexivity) ltac:(rewrite Fields; reflexivity))
          end.
          execute (local_set_run state initial_store
            [ext (treeh combination); ext value; empty; VAL_num (VAL_int64 (wasm_nat64 0))]
            2%N (ext result) ltac:(cbn; lia)).
          native_pure.
          execute (child_run state initial_store selected combination 2 (blobh opcode) Functions
            ltac:(vm_compute; reflexivity) ltac:(rewrite Fields; reflexivity)).
          execute (collector_integer_executes state initial_store selected opcode rule before page
            Functions Memory Allowed Page Decoded).
          do 3 native_pure.
          execute_under [VAL_num (VAL_int64 (wasm_nat64 rule)); ext value; ext result]
            (child_run state (upd_s_mem initial_store [page]) selected combination 5 (treeh witnesses) Functions
              ltac:(vm_compute; reflexivity) ltac:(rewrite Fields; reflexivity)).
      apply rt_refl.
    Qed.

  End Execution.
End WasmAcceptance.
