Module WasmChecked.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Support := WasmSupport.
  Import Support.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
      Local Notation configuration := (G.E.H.state * store_record * frame * list administrative_instruction)%type.

      Definition checked (initial : configuration) (post : configuration -> Prop) :=
        exists final, reduce_trans initial final /\
          ((let '(_, _, _, instructions) := final in instructions = [AI_trap]) \/ post final).

      Definition in_context depth (context : lholed depth) (configuration : configuration) :=
        let '(state, store, frame, instructions) := configuration in
        (state, store, frame, lfill context instructions).

      Ltac normalize_checked := cbn [in_context lfill List.app seq.cat v_to_e_list seq.map].

      (** A multi-step reduction remains valid inside the same surrounding Wasm instruction
          context. *)
      Lemma runs_in_context depth (context : lholed depth) initial final :
        reduce_trans initial final -> reduce_trans (in_context depth context initial) (in_context depth context final).
      Proof.
        destruct initial as [[[state store] frame] instructions].
        destruct final as [[[state' store'] frame'] instructions']; intro Run.
        induction context as [prefix suffix|depth prefix arity continuation inner IH suffix]; cbn [in_context lfill].
        - apply G.E.collector_runs_context; exact Run.
        - apply G.E.collector_runs_context, G.E.collector_runs_label; exact IH.
      Qed.

      (** A reduction to a state satisfying the postcondition establishes the
          checked-execution contract. *)
      Lemma checked_result initial final post :
        reduce_trans initial final -> post final -> checked initial post.
      Proof. intros Run Post; exists final; split; [exact Run|right; exact Post]. Qed.

      (** A reduction to a trap satisfies the checked-execution contract without a success
          postcondition. *)
      Lemma checked_trap initial state store frame post :
        reduce_trans initial (state, store, frame, [AI_trap]) -> checked initial post.
      Proof. intro Run; exists (state, store, frame, [AI_trap]); split; [exact Run|left; reflexivity]. Qed.

      (** A finite reduction prefix can be prepended to a checked execution. *)
      Lemma checked_progress initial middle post :
        reduce_trans initial middle -> checked middle post -> checked initial post.
      Proof.
        intros Prefix [final [Suffix Post]]; exists final; split; [eapply rt_trans; eassumption|exact Post].
      Qed.

      (** A checked execution remains checked under a weaker success postcondition. *)
      Lemma checked_weaken initial before after :
        checked initial before -> (forall final, before final -> after final) -> checked initial after.
      Proof.
        intros [final [Run [Trap|Post]]] Weaken; exists final; split; [exact Run|left; exact Trap|exact Run|].
        right; apply Weaken; exact Post.
      Qed.

      (** A checked computation can be composed with its surrounding instruction context
          and continuation postcondition. *)
      Lemma checked_context depth (context : lholed depth) initial before after :
        checked initial before ->
        (forall final, before final -> checked (in_context depth context final) after) ->
        checked (in_context depth context initial) after.
      Proof.
        intros [final [Run [Trap|Post]]] Next.
        - destruct final as [[[state store] frame] instructions]; subst instructions.
          eapply checked_trap, rt_trans; [apply runs_in_context; exact Run|].
          apply G.E.collector_trap_context.
        - eapply checked_progress; [apply runs_in_context; exact Run|apply Next; exact Post].
      Qed.

      Definition returned values (configuration : configuration) :=
        let '(_, _, _, instructions) := configuration in
        instructions = v_to_e_list values \/
          exists depth (context : lholed depth),
            instructions = lfill context (v_to_e_list values ++ [AI_basic BI_return]).

      (** A checked function-body execution establishes the corresponding checked function
          call. *)
      Lemma checked_call state store caller index address code parameters results arguments defaults (dummy : list value) post :
        lookup_N (inst_funcs (f_inst caller)) index = Some address ->
        lookup_N (s_funcs store) address =
          Some (FC_func_native (Tf parameters results) G.E.collector_instance code) ->
        List.length arguments = List.length parameters ->
        default_vals (modfunc_locals code) = Some defaults ->
        List.length dummy = List.length results ->
        checked
          (state, store, Build_frame (arguments ++ defaults) G.E.collector_instance,
            to_e_list (modfunc_body code))
          (fun final => exists values, returned values final /\
            List.length values = List.length results /\
            let '(state', store', _, _) := final in post (state', store', caller, v_to_e_list values)) ->
        checked (state, store, caller, v_to_e_list arguments ++ [AI_basic (BI_call index)]) post.
      Proof.
        intros Index Closure Arity Defaults Dummy [final [Run [Trap|Accepted]]].
        - destruct final as [[[state' store'] frame'] instructions]; subst instructions.
          eapply checked_trap, G.E.collector_call with (code := code) (parameters := parameters)
            (results := results) (address := address) (defaults := defaults) (values := dummy)
            (final_frame := frame') (accepted := false); try eassumption.
        - destruct final as [[[state' store'] frame'] instructions].
          destruct Accepted as [values [[Normal|[depth [context Return]]] [Length Post]]].
          + subst instructions; eapply checked_result; [|exact Post].
            eapply G.E.collector_call with (code := code) (parameters := parameters)
              (results := results) (address := address) (defaults := defaults) (values := values)
              (final_frame := frame') (accepted := true); eassumption.
          + subst instructions; eapply checked_result; [|exact Post].
            eapply G.E.collector_call_return with (code := code) (parameters := parameters)
              (results := results) (address := address) (defaults := defaults) (values := values)
              (final_frame := frame') (context := context) (accepted := true); eassumption.
      Qed.

      Ltac checked_advance_under stack run :=
        normalize_checked;
        let Run := type of run in
        let Run := eval cbv beta iota zeta in Run in
        lazymatch Run with
        | ?reduction (_, _, _, ?before) (?state', ?store', ?frame', ?after) =>
          lazymatch goal with
          | |- checked (_, _, _, ?whole) _ =>
            let tail := eval cbv beta iota zeta delta
              [List.skipn List.length List.map List.app seq.cat seq.map Nat.add to_e_list v_to_e_list]
              in (skipn (List.length stack + List.length before) whole) in
            eapply checked_progress with (middle := (state', store', frame', v_to_e_list stack ++ after ++ tail));
            [eapply G.E.collector_runs_context with (prefix := stack) (suffix := tail)
              (instructions := before) (instructions' := after); exact run
            |cbn [List.app seq.cat v_to_e_list seq.map]]
          end
        end.

      Ltac checked_advance run := checked_advance_under (@nil value) run.
      Ltac checked_pure := normalize_checked; eapply checked_progress; [native_pure; apply rt_refl|].

      Ltac checked_use_under stack run :=
        normalize_checked;
        let Run := type of run in
        let Run := eval cbv beta iota zeta in Run in
        lazymatch Run with
        | checked (?at_state, ?at_store, ?at_frame, ?source) ?condition =>
          lazymatch goal with
          | |- checked (_, _, _, ?whole) _ =>
            let tail := eval cbv beta iota zeta delta
              [List.skipn List.length List.map List.app seq.cat seq.map Nat.add to_e_list v_to_e_list]
              in (skipn (List.length stack + List.length source) whole) in
            eapply checked_context with (initial := (at_state, at_store, at_frame, source))
              (context := LH_base stack tail) (before := condition);
              [exact run|]
          end
        end.
      Ltac checked_use run := checked_use_under (@nil value) run.

      Lemma wasm_nat64_equal n m : (Z.of_nat n < 2 ^ 64)%Z -> (Z.of_nat m < 2 ^ 64)%Z ->
        app_relop (Relop_i ROI_eq) (VAL_int64 (wasm_nat64 n)) (VAL_int64 (wasm_nat64 m)) = Nat.eqb n m.
      Proof.
        intros NBound MBound; change (Wasm_int.Int64.eq (wasm_nat64 n) (wasm_nat64 m) = Nat.eqb n m).
        unfold Wasm_int.Int64.eq; rewrite (wasm_nat64_unsigned n NBound), (wasm_nat64_unsigned m MBound).
        destruct (Coqlib.zeq (Z.of_nat n) (Z.of_nat m)) as [Equal|Different].
        - symmetry; apply Nat.eqb_eq; lia.
        - symmetry; apply Nat.eqb_neq; lia.
      Qed.

End WasmChecked.
