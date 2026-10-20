(** Instantiates the collector module and proves reusable Wasm reduction
    rules for calls, frames, instruction contexts, loops, and traps. *)

From Wasm Require Import datatypes operations opsem properties instantiation_func interp_instantiate_sound.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Collector Require Import Matching.
From FixProof.Shell Require Import ABI Externref Host.
From FixProof.Collector.wasm Require Import Binary Layout.
From Stdlib Require Import List NArith String Lia Relation_Operators.
Import ListNotations.
Local Open Scope list_scope.

Module WasmExecution (S : STORAGE) (P : PROGRAM S) (R : EXTERNREFS S).
  Module H := ShellHost S P R.

  Section Execution.
    Context `{mem : BlockUpdateMemory}.

    Definition imported_functions := map snd shell_function_imports.
    Definition host_store : store_record := Build_store_record
      (map (fun operation => FC_func_host (shell_signature operation) operation) imported_functions)
      [] [] [] [] [].
    Definition import_values := map (fun index => EV_func (N.of_nat index)) (seq 0 18).
    Definition module_allocation := interp_alloc_module host_store evaluation_module import_values [] [].
    Definition allocated_store := fst module_allocation.
    Definition collector_instance := snd module_allocation.

    Variable combination : S.TreeName.
    Variable privileged : bool.
    Variable handle_bytes : nat -> S.handle -> option S.BlobName.
    Local Instance collector_host : host :=
      H.shell_host combination collector_instance privileged handle_bytes.

    (** The supplied host imports instantiate the parsed collector module with the modeled
        initial store and instance. *)
    Lemma collector_module_instantiated :
      interp_instantiate 0 host_store evaluation_module import_values =
      (Some (0, allocated_store, Build_frame [] collector_instance, []), ""%string).
    Proof. vm_compute; reflexivity. Qed.

    (** The modeled collector instantiation satisfies WasmCert's declarative instantiation
        relation. *)
    Lemma collector_module_instantiation_sound :
      instantiation_spec.instantiate host_store evaluation_module import_values
        (allocated_store, Build_frame [] collector_instance, []).
    Proof. eapply interp_instantiate_imp_instantiate; exact collector_module_instantiated. Qed.

    (** The instantiated collector uses the expected function, table, and memory indices. *)
    Lemma collector_resource_indices :
      inst_tables collector_instance = [0%N; 1%N; 2%N] /\
      inst_mems collector_instance = [0%N].
    Proof. split; reflexivity. Qed.

    (** A multi-step execution remains valid under a mapping that preserves individual
        reduction steps. *)
    Lemma collector_runs_map transform initial final :
      (forall before after, reduce_tuple before after ->
        reduce_tuple (transform before) (transform after)) ->
      reduce_trans initial final -> reduce_trans (transform initial) (transform final).
    Proof.
      intros Step Run; induction Run.
      - apply rt_step; apply Step; assumption.
      - apply rt_refl.
      - eapply rt_trans; eassumption.
    Qed.

    Definition in_label arity continuation
      (configuration : nat * store_record * frame * list administrative_instruction) :=
      let '(state, store, frame, instructions) := configuration in
      (state, store, frame, [AI_label arity continuation instructions]).
    Definition in_frame arity (caller : frame)
      (configuration : nat * store_record * frame * list administrative_instruction) :=
      let '(state, store, frame, instructions) := configuration in
      (state, store, caller, [AI_frame arity frame instructions]).

    (** A multi-step execution can be placed inside the same Wasm label context. *)
    Lemma collector_runs_label state store frame instructions state' store' frame' instructions'
        arity continuation :
      reduce_trans (state, store, frame, instructions) (state', store', frame', instructions') ->
      reduce_trans (state, store, frame, [AI_label arity continuation instructions])
        (state', store', frame', [AI_label arity continuation instructions']).
    Proof.
      intro Run; change (reduce_trans
        (in_label arity continuation (state, store, frame, instructions))
        (in_label arity continuation (state', store', frame', instructions'))).
      eapply collector_runs_map; [|exact Run].
      intros [[[hs s] f] es] [[[hs' s'] f'] es'] Step.
      cbn in *; eapply r_label with (lh := LH_rec [] arity continuation (LH_base [] []) []);
        [exact Step|cbn; now rewrite List.app_nil_r|cbn; now rewrite List.app_nil_r].
    Qed.

    (** A multi-step execution can be placed inside the same Wasm call frame. *)
    Lemma collector_runs_frame state store frame instructions state' store' frame' instructions'
        arity caller :
      reduce_trans (state, store, frame, instructions) (state', store', frame', instructions') ->
      reduce_trans (state, store, caller, [AI_frame arity frame instructions])
        (state', store', caller, [AI_frame arity frame' instructions']).
    Proof.
      intro Run; change (reduce_trans
        (in_frame arity caller (state, store, frame, instructions))
        (in_frame arity caller (state', store', frame', instructions'))).
      eapply collector_runs_map; [|exact Run].
      intros [[[hs s] f] es] [[[hs' s'] f'] es'] Step.
      cbn in *; apply r_frame; exact Step.
    Qed.

    Definition in_context prefix suffix
      (configuration : nat * store_record * frame * list administrative_instruction) :=
      let '(state, store, frame, instructions) := configuration in
      (state, store, frame, v_to_e_list prefix ++ instructions ++ suffix).

    (** A multi-step execution can be placed inside the surrounding instruction context. *)
    Lemma collector_runs_context state store frame instructions state' store' frame' instructions'
        prefix suffix :
      reduce_trans (state, store, frame, instructions) (state', store', frame', instructions') ->
      reduce_trans (state, store, frame, v_to_e_list prefix ++ instructions ++ suffix)
        (state', store', frame', v_to_e_list prefix ++ instructions' ++ suffix).
    Proof.
      intro Run; change (reduce_trans
        (in_context prefix suffix (state, store, frame, instructions))
        (in_context prefix suffix (state', store', frame', instructions'))).
      eapply collector_runs_map; [|exact Run].
      intros [[[hs s] f] es] [[[hs' s'] f'] es'] Step.
      cbn in *; eapply r_label with (lh := LH_base prefix suffix);
        [exact Step|reflexivity|reflexivity].
    Qed.

    (** A trap propagates past a trailing instruction suffix. *)
    Lemma collector_trap_suffix state store frame instructions state' store' frame' suffix :
      reduce_trans (state, store, frame, instructions) (state', store', frame', [AI_trap]) ->
      reduce_trans (state, store, frame, instructions ++ suffix) (state', store', frame', [AI_trap]).
    Proof.
      intro Run; eapply rt_trans with (y := (state', store', frame', AI_trap :: suffix)).
      - exact (collector_runs_context _ _ _ _ _ _ _ _ [] suffix Run).
      - destruct suffix as [|first suffix]; [apply rt_refl|].
        apply rt_step, r_simple; eapply rs_trap with (lh := LH_base [] (first :: suffix));
          [discriminate|reflexivity].
    Qed.

    Definition collector_loop_cont code := [AI_basic (BI_loop (BT_valtype None) code)].
    Definition collector_loop_state code instructions :=
      [AI_label 0 [] [AI_label 0 (collector_loop_cont code) instructions]].

    (** Entering a Wasm loop establishes the loop label and its body. *)
    Lemma collector_loop_enter code state store frame :
      reduce_trans
        (state, store, frame, [AI_basic (BI_block (BT_valtype None) [BI_loop (BT_valtype None) code])])
        (state, store, frame, collector_loop_state code (to_e_list code)).
    Proof.
      eapply rt_trans with (y := (state, store, frame, [AI_label 0 [] (collector_loop_cont code)])).
      - apply rt_step; eapply r_block with (vs := []) (n := 0%nat) (t1s := []) (t2s := []); reflexivity.
      - apply collector_runs_label, rt_step;
          eapply r_loop with (vs := []) (n := 0%nat) (t1s := []) (t2s := []); reflexivity.
    Qed.

    (** A branch back to the loop label restarts the loop with the updated state. *)
    Lemma collector_loop_continue code state store frame state' store' frame' :
      reduce_trans (state, store, frame, to_e_list code)
        (state', store', frame', [AI_basic (BI_br 0%N)]) ->
      reduce_trans (state, store, frame, collector_loop_state code (to_e_list code))
        (state', store', frame', collector_loop_state code (to_e_list code)).
    Proof.
      intro Step.
      eapply rt_trans with (y := (state', store', frame', collector_loop_state code [AI_basic (BI_br 0%N)])).
      - apply collector_runs_label, collector_runs_label; exact Step.
      - eapply rt_trans with (y := (state', store', frame', [AI_label 0 [] (collector_loop_cont code)])).
        + apply collector_runs_label, rt_step, r_simple;
            eapply rs_br with (vs := []) (i := 0%nat) (lh := LH_base [] []); reflexivity.
        + apply collector_runs_label, rt_step;
            eapply r_loop with (vs := []) (n := 0%nat) (t1s := []) (t2s := []); reflexivity.
    Qed.

    (** A branch out of the loop exits to the surrounding context. *)
    Lemma collector_loop_exit code state store frame state' store' frame' (accepted : bool) suffix :
      reduce_trans (state, store, frame, to_e_list code)
        (state', store', frame', if accepted then AI_basic (BI_br 1%N) :: suffix else [AI_trap]) ->
      reduce_trans (state, store, frame, collector_loop_state code (to_e_list code))
        (state', store', frame', if accepted then [] else [AI_trap]).
    Proof.
      intro Done.
      eapply rt_trans with (y := (state', store', frame',
        collector_loop_state code (if accepted then AI_basic (BI_br 1%N) :: suffix else [AI_trap]))).
      - apply collector_runs_label, collector_runs_label; exact Done.
      - destruct accepted.
        + apply rt_step, r_simple; eapply rs_br with (vs := []) (i := 1%nat)
            (lh := LH_rec [] 0 (collector_loop_cont code) (LH_base [] suffix) []); reflexivity.
        + eapply rt_trans with (y := (state', store', frame', [AI_label 0 [] [AI_trap]])).
          * apply collector_runs_label, rt_step, r_simple, rs_label_trap.
          * apply rt_step, r_simple, rs_label_trap.
    Qed.

    (** A finite sequence of loop iterations followed by an exit or trap gives the
        corresponding complete loop execution. *)
    Lemma collector_loop_result code count states stores frames state' store' frame' (accepted : bool) :
      (forall index, (index < count)%nat ->
        reduce_trans (states index, stores index, frames index, to_e_list code)
          (states (S index), stores (S index), frames (S index), [AI_basic (BI_br 0%N)])) ->
      (exists suffix, reduce_trans (states count, stores count, frames count, to_e_list code)
        (state', store', frame', if accepted then AI_basic (BI_br 1%N) :: suffix else [AI_trap])) ->
      reduce_trans
        (states 0, stores 0, frames 0,
          [AI_basic (BI_block (BT_valtype None) [BI_loop (BT_valtype None) code])])
        (state', store', frame', if accepted then [] else [AI_trap]).
    Proof.
      intros Step [suffix Done].
      assert (Loop : forall remaining index, index + remaining = count ->
        reduce_trans (states index, stores index, frames index,
          collector_loop_state code (to_e_list code))
          (state', store', frame', if accepted then [] else [AI_trap])).
      { induction remaining as [|remaining IH]; intros index End.
        - replace index with count by lia.
          eapply collector_loop_exit; exact Done.
        - eapply rt_trans; [apply collector_loop_continue; apply Step; lia|].
          apply (IH (S index)); lia. }
      eapply rt_trans; [apply collector_loop_enter|apply (Loop count 0); reflexivity].
    Qed.

    (** Finite loop iterations satisfying the supplied invariant yield the stated loop
        execution. *)
    Lemma collector_loop_executes code count states stores frames :
      (forall index, (index < count)%nat ->
        reduce_trans (states index, stores index, frames index, to_e_list code)
          (states (S index), stores (S index), frames (S index), [AI_basic (BI_br 0%N)])) ->
      (exists suffix, reduce_trans (states count, stores count, frames count, to_e_list code)
        (states count, stores count, frames count, AI_basic (BI_br 1%N) :: suffix)) ->
      reduce_trans
        (states 0, stores 0, frames 0,
          [AI_basic (BI_block (BT_valtype None) [BI_loop (BT_valtype None) code])])
        (states count, stores count, frames count, []).
    Proof.
      intros Step [suffix Done].
      eapply collector_loop_result with (accepted := true); [exact Step|exists suffix; exact Done].
    Qed.

    (** A valid local-variable lookup pushes its stored Wasm value. *)
    Lemma collector_local_get state store locals index value :
      lookup_N locals index = Some value ->
      reduce_trans
        (state, store, Build_frame locals collector_instance, [AI_basic (BI_local_get index)])
        (state, store, Build_frame locals collector_instance, v_to_e_list [value]).
    Proof. intro Local; apply rt_step, r_local_get; exact Local. Qed.

    (** A Boolean condition enters a label containing exactly the selected Wasm branch. *)
    Lemma collector_if_enter state store frame condition yes no :
      reduce_trans
        (state, store, frame,
          [v_to_e (VAL_num (VAL_int32 (wasm_bool condition))); AI_basic (BI_if (BT_valtype None) yes no)])
        (state, store, frame, [AI_label 0 [] (to_e_list (if condition then yes else no))]).
    Proof.
      eapply rt_trans with (y := (state, store, frame,
        [AI_basic (BI_block (BT_valtype None) (if condition then yes else no))])).
      - apply rt_step, r_simple; destruct condition;
          [apply rs_if_true; vm_compute; discriminate|apply rs_if_false; reflexivity].
      - apply rt_step; eapply r_block with (vs := []) (n := 0%nat) (t1s := []) (t2s := []); reflexivity.
    Qed.

    (** A false condition selects the else branch of the modeled Wasm conditional. *)
    Lemma collector_if_skip state store frame yes :
      reduce_trans
        (state, store, frame,
          [v_to_e (VAL_num (VAL_int32 (wasm_bool false))); AI_basic (BI_if (BT_valtype None) yes [])])
        (state, store, frame, []).
    Proof.
      eapply rt_trans with (y := (state, store, frame, [AI_label 0 [] []])).
      - apply collector_if_enter with (condition := false).
      - apply rt_step, r_simple, rs_label_const; reflexivity.
    Qed.

    (** A conditional with no result type executes its selected branch without adding
        results. *)
    Lemma collector_if_void state store frame (condition : bool) yes no state' store' frame' (accepted : bool) :
      reduce_trans (state, store, frame, to_e_list (if condition then yes else no))
        (state', store', frame', if accepted then [] else [AI_trap]) ->
      reduce_trans
        (state, store, frame,
          [v_to_e (VAL_num (VAL_int32 (wasm_bool condition))); AI_basic (BI_if (BT_valtype None) yes no)])
        (state', store', frame', if accepted then [] else [AI_trap]).
    Proof.
      intro Body.
      eapply rt_trans; [apply collector_if_enter|].
      eapply rt_trans; [apply collector_runs_label; exact Body|].
      destruct accepted; apply rt_step, r_simple;
        [apply rs_label_const; reflexivity|apply rs_label_trap].
    Qed.

    (** Calling a defined collector function establishes its parameters, locals, and call
        frame. *)
    Lemma collector_call_enter state store caller index address code parameters results arguments defaults :
      lookup_N (inst_funcs (f_inst caller)) index = Some address ->
      lookup_N (s_funcs store) address =
        Some (FC_func_native (Tf parameters results) collector_instance code) ->
      List.length arguments = List.length parameters ->
      default_vals (modfunc_locals code) = Some defaults ->
      reduce_trans (state, store, caller, v_to_e_list arguments ++ [AI_basic (BI_call index)])
        (state, store, caller,
          [AI_frame (List.length results) (Build_frame (arguments ++ defaults) collector_instance)
            [AI_label (List.length results) [] (to_e_list (modfunc_body code))]]).
    Proof.
      intros Index Closure Arity Defaults.
      destruct code as [type_index local_types instructions]; cbn in Defaults.
      eapply rt_trans with (y := (state, store, caller, v_to_e_list arguments ++ [AI_invoke address])).
      - apply rt_step; eapply r_label with (lh := LH_base arguments []);
          [apply r_call; exact Index|reflexivity|reflexivity].
      - apply rt_step; eapply r_invoke_native with (vs := arguments) (ves := v_to_e_list arguments)
          (inst := collector_instance) (es := instructions) (ts1 := parameters) (ts2 := results)
          (ts := local_types) (defaults := defaults) (n := List.length arguments) (m := List.length results)
          (k := List.length local_types); try eassumption; try reflexivity.
        symmetry; exact Arity.
    Qed.

    (** A proved collector function body execution gives the corresponding call execution. *)
    Lemma collector_call state store caller index address code parameters results arguments defaults values
        state' store' final_frame (accepted : bool) :
      lookup_N (inst_funcs (f_inst caller)) index = Some address ->
      lookup_N (s_funcs store) address =
        Some (FC_func_native (Tf parameters results) collector_instance code) ->
      List.length arguments = List.length parameters ->
      default_vals (modfunc_locals code) = Some defaults ->
      List.length values = List.length results ->
      reduce_trans
        (state, store, Build_frame (arguments ++ defaults) collector_instance, to_e_list (modfunc_body code))
        (state', store', final_frame, if accepted then v_to_e_list values else [AI_trap]) ->
      reduce_trans (state, store, caller, v_to_e_list arguments ++ [AI_basic (BI_call index)])
        (state', store', caller, if accepted then v_to_e_list values else [AI_trap]).
    Proof.
      intros Index Closure Arity Defaults Results Body.
      eapply rt_trans with (y := (state, store, caller,
        [AI_frame (List.length results) (Build_frame (arguments ++ defaults) collector_instance)
          [AI_label (List.length results) [] (to_e_list (modfunc_body code))]])).
      - eapply collector_call_enter with (code := code) (address := address) (parameters := parameters); eassumption.
      - eapply rt_trans with (y := (state', store', caller,
          [AI_frame (List.length results) final_frame
            [AI_label (List.length results) [] (if accepted then v_to_e_list values else [AI_trap])]])).
        + apply collector_runs_frame, collector_runs_label; exact Body.
        + destruct accepted.
          * eapply rt_trans with (y := (state', store', caller,
              [AI_frame (List.length results) final_frame (v_to_e_list values)])).
            -- apply rt_step, r_frame, r_simple, rs_label_const; apply v_to_e_const.
            -- apply rt_step, r_simple, rs_local_const.
               ++ apply v_to_e_const.
               ++ rewrite v_to_e_length; exact Results.
          * eapply rt_trans with (y := (state', store', caller,
              [AI_frame (List.length results) final_frame [AI_trap]])).
            -- apply rt_step, r_frame, r_simple, rs_label_trap.
            -- apply rt_step, r_simple, rs_local_trap.
    Qed.

    (** A trap in a nested execution propagates through the surrounding Wasm context. *)
    Lemma collector_trap_context state store frame depth (context : lholed depth) :
      reduce_trans (state, store, frame, lfill context [AI_trap])
        (state, store, frame, [AI_trap]).
    Proof.
      assert (Base : forall prefix suffix,
        reduce_trans (state, store, frame, v_to_e_list prefix ++ AI_trap :: suffix)
          (state, store, frame, [AI_trap])).
      { intros [|first prefix] suffix.
        - destruct suffix as [|next suffix]; [apply rt_refl|].
          apply rt_step, r_simple; eapply rs_trap with (lh := LH_base [] (next :: suffix));
            [discriminate|reflexivity].
        - apply rt_step, r_simple; eapply rs_trap with (lh := LH_base (first :: prefix) suffix);
            [destruct first as [number|vector|reference]; try discriminate;
             destruct reference; discriminate|reflexivity]. }
      induction context as [prefix suffix|depth prefix arity continuation inner IH suffix].
      - apply Base.
      - cbn [lfill].
        eapply rt_trans with (y := (state, store, frame,
          v_to_e_list prefix ++ [AI_label arity continuation [AI_trap]] ++ suffix)).
        + apply collector_runs_context, collector_runs_label; exact IH.
        + eapply rt_trans with (y := (state, store, frame,
            v_to_e_list prefix ++ [AI_trap] ++ suffix)).
          * apply collector_runs_context, rt_step, r_simple, rs_label_trap.
          * apply Base.
    Qed.

    (** A function-body return delivers its result values to the caller. *)
    Lemma collector_call_return state store caller index address code parameters results arguments defaults values
        state' store' final_frame depth (context : lholed depth) (accepted : bool) :
      lookup_N (inst_funcs (f_inst caller)) index = Some address ->
      lookup_N (s_funcs store) address =
        Some (FC_func_native (Tf parameters results) collector_instance code) ->
      List.length arguments = List.length parameters ->
      default_vals (modfunc_locals code) = Some defaults ->
      List.length values = List.length results ->
      reduce_trans
        (state, store, Build_frame (arguments ++ defaults) collector_instance, to_e_list (modfunc_body code))
        (state', store', final_frame,
          lfill context (if accepted then v_to_e_list values ++ [AI_basic BI_return] else [AI_trap])) ->
      reduce_trans (state, store, caller, v_to_e_list arguments ++ [AI_basic (BI_call index)])
        (state', store', caller, if accepted then v_to_e_list values else [AI_trap]).
    Proof.
      intros Index Closure Arity Defaults Results Body.
      destruct accepted.
      2: {
        eapply collector_call with (code := code) (parameters := parameters) (results := results)
          (address := address) (defaults := defaults) (values := values) (accepted := false);
          try eassumption.
        eapply rt_trans; [exact Body|apply collector_trap_context].
      }
      eapply rt_trans with (y := (state, store, caller,
        [AI_frame (List.length results) (Build_frame (arguments ++ defaults) collector_instance)
          [AI_label (List.length results) [] (to_e_list (modfunc_body code))]])).
      - eapply collector_call_enter with (code := code) (address := address) (parameters := parameters); eassumption.
      - eapply rt_trans with (y := (state', store', caller,
          [AI_frame (List.length results) final_frame
            [AI_label (List.length results) [] (lfill context (v_to_e_list values ++ [AI_basic BI_return]))]])).
        + apply collector_runs_frame, collector_runs_label; exact Body.
        + apply rt_step, r_simple; eapply rs_return with (vs := v_to_e_list values)
            (lh := LH_rec [] (List.length results) [] context []);
            [apply v_to_e_const|rewrite v_to_e_length; exact Results|reflexivity].
    Qed.

    (** A function body returning no values completes the corresponding call. *)
    Lemma collector_call_void state store caller index address code parameters arguments defaults
        state' store' final_frame (accepted : bool) :
      lookup_N (inst_funcs (f_inst caller)) index = Some address ->
      lookup_N (s_funcs store) address =
        Some (FC_func_native (Tf parameters []) collector_instance code) ->
      List.length arguments = List.length parameters ->
      default_vals (modfunc_locals code) = Some defaults ->
      reduce_trans
        (state, store, Build_frame (arguments ++ defaults) collector_instance, to_e_list (modfunc_body code))
        (state', store', final_frame, if accepted then [] else [AI_trap]) ->
      reduce_trans (state, store, caller, v_to_e_list arguments ++ [AI_basic (BI_call index)])
        (state', store', caller, if accepted then [] else [AI_trap]).
    Proof.
      intros Index Closure Arity Defaults Body.
      eapply collector_call with (results := []) (values := []);
        eassumption || reflexivity.
    Qed.

    (** The native require call accepts a true condition and traps on a false condition. *)
    Lemma collector_require_executes state store locals (condition : bool) :
      lookup_N (s_funcs store) 18%N =
        Some (FC_func_native (Tf [T_num T_i32] []) collector_instance evaluation_require) ->
      reduce_trans
        (state, store, Build_frame locals collector_instance,
          [v_to_e (VAL_num (VAL_int32 (wasm_bool condition))); AI_basic (BI_call 18%N)])
        (state, store, Build_frame locals collector_instance, if condition then [] else [AI_trap]).
    Proof.
      intro Closure; eapply collector_call_void with (code := evaluation_require)
        (parameters := [T_num T_i32]) (defaults := []) (index := 18%N) (address := 18%N)
        (arguments := [VAL_num (VAL_int32 (wasm_bool condition))]) (accepted := condition)
        (final_frame := Build_frame [VAL_num (VAL_int32 (wasm_bool condition))] collector_instance);
        try reflexivity; try exact Closure.
      apply evaluation_require_executes; reflexivity.
    Qed.

    (** A successful modeled Shell host call gives the corresponding Wasm import execution. *)
    Lemma collector_import_executes state store next frame index address operation arguments result :
      lookup_N (inst_funcs (f_inst frame)) index = Some address ->
      lookup_N (s_funcs store) address =
        Some (FC_func_host (shell_signature operation) operation) ->
      (match shell_signature operation with Tf parameters _ =>
        List.length arguments = List.length parameters end) ->
      H.host_result combination collector_instance privileged handle_bytes state store
        (shell_signature operation) operation arguments = (next, result) ->
      reduce_trans
        (state, store, frame, v_to_e_list arguments ++ [AI_basic (BI_call index)])
        (H.next_state state operation, next, frame, result_to_stack result).
    Proof.
      intros Index Closure Arity Result.
      eapply rt_trans with (y := (state, store, frame, v_to_e_list arguments ++ [AI_invoke address])).
      - apply rt_step; eapply r_label with (lh := LH_base arguments []);
          [apply r_call; exact Index|reflexivity|reflexivity].
      - apply rt_step; destruct (shell_signature operation) as [parameters results] eqn:Signature.
        eapply r_invoke_host_success with (h := operation) (r := result) (vcs := arguments)
          (n := List.length parameters) (m := List.length results);
          try reflexivity; try eassumption.
        split; [reflexivity|].
        change (Some (next, result) = Some (H.host_result combination collector_instance
          privileged handle_bytes state store (Tf parameters results) operation arguments)).
        rewrite Result; reflexivity.
    Qed.
  End Execution.
End WasmExecution.

Ltac collector_execute_under lift stack run :=
  try fold reduce_trans;
  let Run := type of run in
  let Run := eval cbv beta iota zeta in Run in
  lazymatch Run with
  | ?reduction (_, _, _, ?before) (?state', ?store', ?frame', ?after) =>
    lazymatch goal with
    | |- ?relation (_, _, _, ?whole) _ =>
      let tail := eval cbv beta iota zeta delta
        [List.skipn List.length List.map List.app seq.map Nat.add to_e_list v_to_e_list]
        in (skipn (List.length stack + List.length before) whole) in
      eapply rt_trans with (y := (state', store', frame', v_to_e_list stack ++ after ++ tail));
      [ eapply lift with (prefix := stack) (suffix := tail)
          (instructions := before) (instructions' := after); exact run
      | cbn [List.app v_to_e_list seq.map]; fold reduce_trans ]
    end
  end.
