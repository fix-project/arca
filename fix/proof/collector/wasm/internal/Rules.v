Module WasmRules.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Support := WasmSupport.
  Import Support.
  Module Acceptance := WasmAcceptance.
  Module Trees := WasmTreeLoop.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.

  Definition rule_body := modfunc_body native_exchange.
  Definition result_word kind := VAL_num (VAL_int64 (wasm_nat64 kind)).
  Definition rule_arguments code lhs rhs witnesses := [result_word code; descriptor lhs; descriptor rhs; descriptor witnesses].
  Definition initial_locals code lhs rhs witnesses := rule_arguments code lhs rhs witnesses ++ repeat (word 0) 4.
  Definition optional_descriptor slot := descriptor (match slot with Some index => index | None => wasm_nat 0 end).
  Definition scratch_locals code lhs rhs witnesses slots (_ : nat) := rule_arguments code lhs rhs witnesses ++
    map optional_descriptor (firstn 4 slots).
  Definition single_locals code lhs rhs witnesses claim kind :=
    scratch_locals code lhs rhs witnesses [Some (wasm_nat claim);None;None;None] kind.
  Definition endpoint_locals code lhs rhs witnesses claim a b :=
    scratch_locals code lhs rhs witnesses [Some (wasm_nat claim);None;Some (wasm_nat a);Some (wasm_nat b)] 0.

  Ltac scratch_pure := try unfold optional_descriptor; try unfold result_word; native_pure.

  Definition mapped_rule kind := match kind with
    | X.Application => 4 | X.Strict => 9 | X.Selection => 17 | X.Identification => 18
    | X.Shallow => 19 | X.TreeReference => 21 | X.BlobReference => 22 end.
  Definition mapped_tail kind := match kind with
    | X.Application => 52 | X.Strict => 56 | X.Selection => 60 | X.Identification => 64
    | X.Shallow => 68 | X.TreeReference => 72 | X.BlobReference => 76 end.
  Definition unary_codes := [3;4;7;9;10;13;14;15;16;17;18;19;20;21;22].
  Inductive direct_rule := Symmetry | EvalResult | ThinkData | ThunkStep.
  Definition direct_code kind := match kind with Symmetry => 10 | EvalResult => 20 | ThinkData => 3 | ThunkStep => 13 end.
  Definition direct_tail kind := match kind with Symmetry => 84 | EvalResult => 100 | ThinkData => 80 | ThunkStep => 88 end.
  Definition direct_claim kind := match kind with Symmetry => 0 | EvalResult => 1 | ThinkData | ThunkStep => 4 end.
  Definition direct_result kind := match kind with ThinkData => 3 | _ => 0 end.
  Definition direct_left kind (lhs rhs : handle) := match kind with Symmetry => rhs | _ => lhs end.
  Definition direct_right kind (lhs rhs : handle) := match kind with Symmetry => lhs | _ => rhs end.
  Definition direct_data kind rhs := match kind with
    | ThinkData => exists datum, rhs = Data datum
    | ThunkStep => exists thunk, rhs = Thunk thunk | _ => True end.
  Definition binary_codes := [2;8;11;24;25].

  Lemma native_leaf_guard state store locals index input :
    s_funcs store = s_funcs G.E.allocated_store -> lookup_N locals 1%N = Some (descriptor index) ->
    G.E.H.resolve state index = Some input -> X.reflexive_eval input = true ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        to_e_list [BI_local_get 1%N; BI_call 0%N; BI_const_num (VAL_int32 (wasm_nat 3));
          BI_relop T_i32 (Relop_i ROI_ne); BI_call 22%N;
          BI_local_get 1%N; BI_call 0%N; BI_testop T_i32 TO_eqz;
          BI_if (BT_valtype None) [BI_local_get 1%N; BI_call 1%N; BI_testop T_i32 TO_eqz; BI_call 22%N] []])
      (state, store, Build_frame locals G.E.collector_instance, []).
  Proof.
    intros Functions Input Live Leaf.
    destruct input as [[[blob|tree]|reference]|th|encoded]; cbn [X.reflexive_eval] in Leaf; try discriminate.
    all: execute (native_type_query state store locals 1%N index _ Functions Input Live).
    all: do 2 native_pure.
    all: execute (native_type_query state store locals 1%N index _ Functions Input Live).
    all: native_pure.
    - apply G.E.collector_if_void with (condition := true) (accepted := true).
      execute (native_data_type_query state store locals 1%N index (Object (BlobObj blob)) Functions Input Live).
      do 2 native_pure; apply rt_refl.
    - native_pure; apply rt_refl.
    - native_pure; apply rt_refl.
  Qed.

End WasmRules.
