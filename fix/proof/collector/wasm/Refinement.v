From Wasm Require Import typing memory.
(** Connects execution of the collector binary to the abstract checker and
    issuer: it returns the specified certificate or traps. Also proves
    successful execution under explicit readiness and resource conditions. *)

From FixProof.Collector.wasm Require Import Backend Support Witnesses Acceptance Output Resources.
Import ListNotations.
Local Open Scope list_scope.

Module WasmRefinement (C : COLLECTOR_CONTEXT).
  Import C S R.
  Module Support := WasmSupport C.
  Import Support.
  Module Witnesses := WasmWitnesses C.
  Import Witnesses.
  Module Acceptance := WasmAcceptance C.
  Import Acceptance.
  Module Output := WasmOutput C.
  Import Output.
  Module Resources := WasmResources C.
  Import Resources.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.

  Local Notation ext := G.E.H.W.extern_value.
  Local Notation treeh := G.E.H.W.H.HTreeObj.
  Local Notation blobh := G.E.H.W.H.HBlobObj.
  Local Notation lower := X.D.EC.E.EP.E.lower.
  Local Notation lift := X.D.EC.E.EP.E.lift.
  Local Notation local_get_run := (G.E.collector_local_get combination true true handle_bytes).

  Section Execution.
    Section Refinement.
      Local Notation configuration := (nat * store_record * frame * list administrative_instruction)%type.

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

      Variable other_tables : list tableinst.
      Variable witnesses : list handle.
      (** Every supplied witness satisfies the required bounded-depth length constraints. *)
      Hypothesis witness_bounds : Forall (lengths_bounded 3) witnesses.
      (** Privileged inspection can supply handle bytes for every requested handle at every
          observation index. *)
      Hypothesis bytes_available : forall state value, exists bytes, handle_bytes state value = Some bytes.

      Hypothesis witnesses_authenticated : Forall (fun witness =>
        exists claim, authenticated_claim witness claim) witnesses.

      Record resources store : Prop := {
        resources_functions : s_funcs store = s_funcs G.E.allocated_store;
        resources_typing : store_typing store;
        resources_memory : memory_capacity store;
        resources_tables : firstn 2 (s_tables store) = other_tables;
        resources_claim_view : view_capacity store 3%N 2;
        resources_premise_view : view_capacity store 4%N 3;
        resources_coupon_view : view_capacity store 6%N 2;
        resources_combination : tree_view store 2%N combination;
      }.

      Record prepared store (_used : list handle) : Prop := {
        prepared_resources : resources store;
        prepared_retained : retained_claims store witnesses;
      }.

      Lemma prepared_functions store used : prepared store used ->
        s_funcs store = s_funcs G.E.allocated_store.
      Proof. intro Prepared; exact (resources_functions _ (prepared_resources _ _ Prepared)). Qed.
      Lemma prepared_typing store used : prepared store used -> store_typing store.
      Proof. intro Prepared; exact (resources_typing _ (prepared_resources _ _ Prepared)). Qed.
      Lemma prepared_memory store used : prepared store used -> memory_capacity store.
      Proof. intro Prepared; exact (resources_memory _ (prepared_resources _ _ Prepared)). Qed.
      Lemma prepared_tables store used : prepared store used -> firstn 2 (s_tables store) = other_tables.
      Proof. intro Prepared; exact (resources_tables _ (prepared_resources _ _ Prepared)). Qed.
      Lemma prepared_claim_view store used : prepared store used -> view_capacity store 3%N 2.
      Proof. intro Prepared; exact (resources_claim_view _ (prepared_resources _ _ Prepared)). Qed.
      Lemma prepared_premise_view store used : prepared store used -> view_capacity store 4%N 3.
      Proof. intro Prepared; exact (resources_premise_view _ (prepared_resources _ _ Prepared)). Qed.
      Lemma prepared_coupon_view store used : prepared store used -> view_capacity store 6%N 2.
      Proof. intro Prepared; exact (resources_coupon_view _ (prepared_resources _ _ Prepared)). Qed.
      Lemma prepared_combination store used : prepared store used -> tree_view store 2%N combination.
      Proof. intro Prepared; exact (resources_combination _ (prepared_resources _ _ Prepared)). Qed.

      Definition fresh_tree_views store :=
        view_capacity store 7%N 0 /\ view_capacity store 8%N 0.

      Lemma resources_frame before after :
        resources before -> s_funcs after = s_funcs before -> store_typing after ->
        memory_capacity after -> firstn 2 (s_tables after) = firstn 2 (s_tables before) ->
        stab after G.E.collector_instance 2%N = stab before G.E.collector_instance 2%N ->
        stab after G.E.collector_instance 3%N = stab before G.E.collector_instance 3%N ->
        stab after G.E.collector_instance 4%N = stab before G.E.collector_instance 4%N ->
        stab after G.E.collector_instance 6%N = stab before G.E.collector_instance 6%N ->
        resources after.
      Proof.
        intros Before Functions Typed Memory Prefix Combination Claim Premise Coupon.
        destruct Before as [BeforeFunctions BeforeTyped BeforeMemory BeforePrefix BeforeClaim BeforePremise BeforeCoupon BeforeCombination].
        constructor.
        - rewrite Functions; exact BeforeFunctions.
        - exact Typed.
        - exact Memory.
        - rewrite Prefix; exact BeforePrefix.
        - destruct BeforeClaim as [table [Entry Shape]]; exists table; split; [rewrite Claim; exact Entry|exact Shape].
        - destruct BeforePremise as [table [Entry Shape]]; exists table; split; [rewrite Premise; exact Entry|exact Shape].
        - destruct BeforeCoupon as [table [Entry Shape]]; exists table; split; [rewrite Coupon; exact Entry|exact Shape].
        - intros offset value Entry.
          change (match stab after G.E.collector_instance 2%N with
            | Some table => lookup_N (tableinst_elem table) (N.of_nat offset) | None => None end =
            Some (G.E.H.Tables.externref value)).
          rewrite Combination; exact (BeforeCombination offset value Entry).
      Qed.

      Lemma resources_after_read store arguments next result :
        resources store ->
        G.E.H.blob_read_result G.E.collector_instance store arguments = Some (next, result) ->
        resources next.
      Proof.
        intros Before Read; constructor.
        - rewrite (G.E.H.blob_read_result_functions _ _ _ _ _ Read); exact (resources_functions _ Before).
        - apply (proj1 (proj2 (G.E.H.blob_read_result_correct _ _ _ _ _ Read))).
          exact (resources_typing _ Before).
        - eapply blob_read_memory_capacity; [exact Read|exact (resources_memory _ Before)].
        - rewrite (G.E.H.blob_read_result_tables _ _ _ _ _ Read); exact (resources_tables _ Before).
        - eapply blob_read_capacity; [exact Read|exact (resources_claim_view _ Before)].
        - eapply blob_read_capacity; [exact Read|exact (resources_premise_view _ Before)].
        - eapply blob_read_capacity; [exact Read|exact (resources_coupon_view _ Before)].
        - eapply blob_read_view; [exact Read|exact (resources_combination _ Before)].
      Qed.

      Lemma resources_after_attachment store node index next :
        resources store -> 3 <= index -> index < 9 ->
        (index = 3 -> List.length (get_tree_raw node) = 2) ->
        (index = 4 -> List.length (get_tree_raw node) = 3) ->
        (index = 6 -> List.length (get_tree_raw node) = 2) ->
        G.E.H.tree_attachment_result G.E.collector_instance store
          [ext (treeh node); word index] = Some (next, result_values []) ->
        resources next.
      Proof.
        intros Before Lower Upper ClaimArity PremiseArity CouponArity Attached; constructor.
        - rewrite (G.E.H.tree_attachment_result_functions _ _ _ _ _ Attached); exact (resources_functions _ Before).
        - apply (proj1 (proj2 (G.E.H.tree_attachment_result_correct _ _ _ _ _ Attached))).
          exact (resources_typing _ Before).
        - eapply tree_attachment_memory_capacity; [exact Attached|exact (resources_memory _ Before)].
        - rewrite (tree_attachment_first_two_tables store node index next ltac:(lia) ltac:(lia) Attached).
          exact (resources_tables _ Before).
        - destruct (Nat.eq_dec index 3) as [Same|Different].
          + subst index.
            pose proof (tree_attachment_capacity store node 3 2 next ltac:(vm_compute; congruence)
              Attached (resources_claim_view _ Before)) as Capacity.
            rewrite (ClaimArity eq_refl) in Capacity; exact Capacity.
          + eapply tree_attachment_other_capacity with (index := index) (other := 3); try exact Attached; try lia; exact (resources_claim_view _ Before).
        - destruct (Nat.eq_dec index 4) as [Same|Different].
          + subst index.
            pose proof (tree_attachment_capacity store node 4 3 next ltac:(vm_compute; congruence)
              Attached (resources_premise_view _ Before)) as Capacity.
            rewrite (PremiseArity eq_refl) in Capacity; exact Capacity.
          + eapply tree_attachment_other_capacity with (index := index) (other := 4); try exact Attached; try lia; exact (resources_premise_view _ Before).
        - destruct (Nat.eq_dec index 6) as [Same|Different].
          + subst index.
            pose proof (tree_attachment_capacity store node 6 2 next ltac:(vm_compute; congruence)
              Attached (resources_coupon_view _ Before)) as Capacity.
            rewrite (CouponArity eq_refl) in Capacity; exact Capacity.
          + eapply tree_attachment_other_capacity with (index := index) (other := 6); try exact Attached; try lia; exact (resources_coupon_view _ Before).
        - eapply tree_attachment_other_view with (index := index) (other := 2); try exact Attached; try lia; exact (resources_combination _ Before).
      Qed.

      Lemma resources_claim_ready store : resources store -> claim_views_ready store.
      Proof.
        intros Before witness WitnessArity.
        destruct (tree_attachment_available store witness 3 2
          (resources_typing _ Before) (resources_claim_view _ Before)
          ltac:(rewrite WitnessArity; lia) ltac:(vm_compute; congruence)
          ltac:(rewrite WitnessArity; vm_compute; congruence)) as [first First].
        assert (FirstResources : resources first).
        { eapply resources_after_attachment; try exact Before; try exact First; try lia; intros; try discriminate; exact WitnessArity. }
        exists first; split; [exact First|]; intros tag TagArity.
        destruct (tree_attachment_available first tag 6 2
          (resources_typing _ FirstResources) (resources_coupon_view _ FirstResources)
          ltac:(rewrite TagArity; lia) ltac:(vm_compute; congruence)
          ltac:(rewrite TagArity; vm_compute; congruence)) as [second Second].
        exists second; exact Second.
      Qed.

      Lemma prepared_after_premise state store witness kind lhs rhs state' next used :
        prepared store used -> premise_ready state store witness kind lhs rhs state' next ->
        prepared next (used ++ [witness]).
      Proof.
        intros Before Ready; constructor.
        - destruct Ready as [claim [code [claimed_lhs [claimed_rhs [attached [page Ready]]]]]].
          destruct Ready as [_ [_ [Fields [_ [Attached [Read _]]]]]].
          eapply resources_after_read; [|exact Read].
          eapply resources_after_attachment; try exact Attached; try lia;
            try exact (prepared_resources _ _ Before); intros; try discriminate;
            rewrite Fields; reflexivity.
        - eapply premise_ready_retained; [exact Ready|exact (prepared_retained _ _ Before)].
      Qed.

      Lemma prepared_after_operand_attachment store node index next used :
        prepared store used -> (index = 7 \/ index = 8) ->
        G.E.H.tree_attachment_result G.E.collector_instance store
          [ext (treeh node); word index] = Some (next, result_values []) ->
        prepared next used.
      Proof.
        intros Before Index Attached; constructor.
        - eapply resources_after_attachment; try exact Attached; try exact (prepared_resources _ _ Before);
            destruct Index; subst index; try lia; intros; discriminate.
        - eapply tree_attachment_retained_claims; try exact Attached; try exact (prepared_retained _ _ Before);
            destruct Index; subst index; lia.
      Qed.

      Lemma prepared_premise_ready store used : prepared store used -> premise_views_ready store.
      Proof.
        intros Prepared claim Arity.
        destruct (tree_attachment_available store claim 4 3
          (prepared_typing _ _ Prepared) (prepared_premise_view _ _ Prepared)
          ltac:(rewrite Arity; lia) ltac:(vm_compute; congruence)
          ltac:(rewrite Arity; vm_compute; congruence)) as [attached Attached].
        exists attached; split; [exact Attached|].
        apply memory_capacity_integer_reads.
        - apply (proj1 (proj2 (G.E.H.tree_attachment_result_correct _ _ _ _ _ Attached))).
          exact (prepared_typing _ _ Prepared).
        - eapply tree_attachment_memory_capacity; [exact Attached|].
          exact (prepared_memory _ _ Prepared).
      Qed.

      Lemma prepared_operand_views store lhs rhs used :
        prepared store used -> fresh_tree_views store ->
        (N.of_nat (List.length (get_tree_raw lhs)) <= native_table_limit)%N ->
        List.length (get_tree_raw rhs) = List.length (get_tree_raw lhs) ->
        exists first next,
          G.E.H.tree_attachment_result G.E.collector_instance store
            [ext (treeh lhs); word 7] = Some (first, result_values []) /\
          G.E.H.tree_attachment_result G.E.collector_instance first
            [ext (treeh rhs); word 8] = Some (next, result_values []) /\
          prepared next used /\ tree_view next 7%N lhs /\ tree_view next 8%N rhs.
      Proof.
        intros Before [LeftCapacity RightCapacity] Bound Length.
        destruct (tree_attachment_available store lhs 7 0 (prepared_typing _ _ Before)
          LeftCapacity ltac:(lia) ltac:(vm_compute; congruence)
          Bound) as [first Left].
        assert (FirstPrepared : prepared first used).
        { eapply prepared_after_operand_attachment; [exact Before|left; reflexivity|exact Left]. }
        assert (FirstRightCapacity : view_capacity first 8%N 0).
        { eapply tree_attachment_other_capacity with (index := 7) (other := 8); try exact Left; try exact RightCapacity; lia. }
        destruct (tree_attachment_available first rhs 8 0 (prepared_typing _ _ FirstPrepared)
          FirstRightCapacity ltac:(lia) ltac:(vm_compute; congruence)
          ltac:(rewrite Length; exact Bound)) as [next Right].
        exists first, next; split; [exact Left|]; split; [exact Right|]; split.
        - eapply prepared_after_operand_attachment; [exact FirstPrepared|right; reflexivity|exact Right].
        - split.
          + eapply tree_attachment_other_view with (index := 8) (other := 7); try exact Right; try lia.
            eapply tree_attachment_view with (index := 7); [vm_compute; congruence|exact Left].
          + eapply tree_attachment_view with (index := 8); [vm_compute; congruence|exact Right].
      Qed.

      Lemma ready_premise_functions state store witness kind lhs rhs state' store' :
        premise_ready state store witness kind lhs rhs state' store' -> s_funcs store' = s_funcs store.
      Proof. apply collector_ready_premise_functions. Qed.

      Lemma checked_premise_preserving state store locals used witness rest kind claim_kind lhs rhs (invariant : store_record -> Prop) :
        used ++ witness :: rest = witnesses -> prepared store used ->
        Claims.Code.statement_kind kind = Some claim_kind ->
        invariant store ->
        (forall state' store', premise_ready state store witness kind lhs rhs state' store' -> invariant store') ->
        checked
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 kind)); ext (supplied_claim witness); ext lhs; ext rhs] ++
              [AI_basic (BI_call 30%N)])
          (fun final => let '(_, store', frame', instructions) := final in
            instructions = [] /\ frame' = Build_frame locals G.E.collector_instance /\ prepared store' (used ++ [witness]) /\ invariant store' /\
            X.read claim_kind witness = Some (lhs, rhs)).
      Proof.
        intros Order Prepared Kind Invariant Preserve.
        pose proof (prepared_functions _ _ Prepared) as Functions.
        assert (Authenticated : exists claim, authenticated_claim witness claim).
        { rewrite <- Order in witnesses_authenticated.
          apply Forall_app in witnesses_authenticated as [_ Tail]; exact (Forall_inv Tail). }
        assert (Bounds : lengths_bounded 3 witness).
        { rewrite <- Order in witness_bounds; apply Forall_app in witness_bounds as [_ Tail]; exact (Forall_inv Tail). }
        assert (KindBound : (Z.of_nat kind < 2 ^ 64)%Z).
        { pose proof (Claims.Code.statement_code_bound _ _ (Claims.Code.statement_kind_roundtrip _ _ Kind)); cbn; lia. }
        destruct (collector_premise_execution state store witness kind claim_kind lhs rhs
          Functions Bounds bytes_available Authenticated (prepared_premise_ready _ _ Prepared) KindBound Kind)
          as [state' [store' [accepted [Run Ready]]]].
        destruct accepted.
        - specialize (Ready eq_refl).
          destruct (collector_ready_premise_decodes state store witness kind lhs rhs state' store'
            claim_kind Ready Kind) as [authors Decoded].
          eapply checked_result; [exact (Run locals)|].
          do 2 (split; [reflexivity|]); split.
          + eapply prepared_after_premise; eassumption.
          + split; [eapply Preserve; exact Ready|].
            change (X.Wire.decode witness = Some (claim_kind, authors, lhs, rhs)) in Decoded.
            unfold X.read; rewrite Decoded.
            destruct (X.API.statement_kind_eq_dec claim_kind claim_kind); [reflexivity|contradiction].
        - eapply checked_trap; exact (Run locals).
      Qed.

      Lemma checked_premise state store locals used witness rest kind claim_kind lhs rhs :
        used ++ witness :: rest = witnesses -> prepared store used ->
        Claims.Code.statement_kind kind = Some claim_kind ->
        checked
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 kind)); ext (supplied_claim witness); ext lhs; ext rhs] ++
              [AI_basic (BI_call 30%N)])
          (fun final => let '(_, store', frame', instructions) := final in
            instructions = [] /\ frame' = Build_frame locals G.E.collector_instance /\ prepared store' (used ++ [witness]) /\
            X.read claim_kind witness = Some (lhs, rhs)).
      Proof.
        intros Order Prepared Kind.
        eapply checked_weaken with (before := fun final =>
          let '(_, store', frame', instructions) := final in
          instructions = [] /\ frame' = Build_frame locals G.E.collector_instance /\
          prepared store' (used ++ [witness]) /\ True /\ X.read claim_kind witness = Some (lhs, rhs)).
        - eapply checked_premise_preserving with (invariant := fun _ => True); eauto.
        - intros [[[state' store'] frame'] instructions] [Code [Frame [Prepared' [_ Read]]]].
          exact (conj Code (conj Frame (conj Prepared' Read))).
      Qed.

      (** Exact-handle validation either traps or establishes identity of the supplied
          handles. *)
      Lemma checked_same state store locals lhs rhs :
        s_funcs store = s_funcs G.E.allocated_store ->
        checked
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [ext lhs; ext rhs] ++ [AI_basic (BI_call 22%N)])
          (fun final => let '(_, store', frame', instructions) := final in
            store' = store /\ frame' = Build_frame locals G.E.collector_instance /\
            instructions = [] /\ lhs = rhs).
      Proof.
        intro Functions.
        destruct (observed_comparison_total state lhs rhs bytes_available) as [equal Compared].
        pose proof (same_run state store locals lhs rhs equal Functions Compared) as Run.
        destruct equal.
        - eapply checked_result; [exact Run|]; repeat split; try reflexivity.
          eapply observed_same_identifies; exact Compared.
        - eapply checked_trap; exact Run.
      Qed.

      (** Endpoint lookup either traps or returns the requested endpoint from an
          authenticated claim. *)
      Lemma checked_endpoint state store locals witness (left : bool) :
        s_funcs store = s_funcs G.E.allocated_store ->
        checked
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [ext witness] ++ [AI_basic (BI_call (if left then 31%N else 32%N))])
          (fun final => let '(_, store', frame', instructions) := final in
            store' = store /\ frame' = Build_frame locals G.E.collector_instance /\
            exists field, instructions = v_to_e_list [ext field]).
      Proof.
        intro Functions.
        eapply checked_call with (code := exchange_endpoint left)
          (parameters := [T_ref T_externref]) (results := [T_ref T_externref])
          (address := if left then 31%N else 32%N) (defaults := [])
          (arguments := [ext witness]) (dummy := [ext witness]); try (destruct left; reflexivity).
        - rewrite Functions; destruct left; reflexivity.
        - rewrite exchange_endpoint_body.
          checked_pure.
          checked_advance (G.collector_tree_element_executes combination handle_bytes state store
            [ext witness] witness (wasm_nat (if left then 1 else 2)) Functions).
          destruct (G.E.H.W.Api.get_tree_data_api witness
            (G.E.H.W.nat_of_i32 (wasm_nat (if left then 1 else 2)))) as [field|];
            [|eapply checked_trap; apply rt_refl].
          eapply checked_result; [apply rt_refl|].
          exists [ext field]; split; [left; reflexivity|].
          do 3 (split; [reflexivity|]); exists field; reflexivity.
      Qed.

      (** Object validation either traps or establishes that the supplied Object backs the
          supplied Ref. *)
      Lemma checked_object state store locals reference value :
        s_funcs store = s_funcs G.E.allocated_store ->
        checked
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [ext reference; ext value] ++ [AI_basic (BI_call 28%N)])
          (fun final => let '(_, store', frame', instructions) := final in
            store' = store /\ frame' = Build_frame locals G.E.collector_instance /\ instructions = [] /\
            exists object, value = Data (Object object) /\ reference = Data (Ref (G.E.H.W.H.data_reference (Object object)))).
      Proof.
        intro Functions.
        destruct value as [[object|ref]|th|encoded].
        - destruct (observed_comparison_total state reference
            (Data (Ref (G.E.H.W.H.data_reference (Object object)))) bytes_available)
            as [equal [reference_bytes [object_bytes [Reference [Object Compared]]]]].
          pose proof (G.collector_object_executes combination handle_bytes state store locals reference object
            reference_bytes object_bytes equal Functions Reference Object Compared) as Run.
          destruct equal.
          + eapply checked_result; [exact Run|]; do 3 (split; [reflexivity|]).
            exists object; split; [reflexivity|].
            eapply (G.collector_object_comparison_sound handle_bytes C.bytes_identify); eassumption.
          + eapply checked_trap; exact Run.
        - eapply checked_trap, G.collector_object_rejects; [exact Functions|intros; discriminate].
        - eapply checked_trap, G.collector_object_rejects; [exact Functions|intros; discriminate].
        - eapply checked_trap, G.collector_object_rejects; [exact Functions|intros; discriminate].
      Qed.

      (** The size helper either traps or returns a Tree length fitting the unsigned 32-bit
          bound. *)
      Lemma checked_size state store locals input :
        s_funcs store = s_funcs G.E.allocated_store -> lengths_bounded 1 input ->
        checked
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [ext input] ++ [AI_basic (BI_call 24%N)])
          (fun final => let '(state', store', frame', instructions) := final in
            state' = state /\ store' = store /\ frame' = Build_frame locals G.E.collector_instance /\
            exists tree, input = treeh tree /\
              (Z.of_nat (List.length (get_tree_raw tree)) < 2 ^ 32)%Z /\
              (N.of_nat (List.length (get_tree_raw tree)) <= native_table_limit)%N /\
              instructions = v_to_e_list [word (List.length (get_tree_raw tree))]).
      Proof.
        intros Functions Bounds.
        destruct input as [[[blob|tree]|reference]|th|encoded];
          try solve [eapply checked_trap, G.collector_size_rejects_non_tree;
            [exact Functions|intros; discriminate]].
        destruct Bounds as [Wide _].
        destruct (Z_lt_ge_dec (Z.of_nat (List.length (get_tree_raw tree))) (Z.of_N native_table_limit + 1)%Z) as [Bound|Large].
        - eapply checked_result; [apply G.collector_size_executes; eassumption|].
          do 3 (split; [reflexivity|]).
          exists tree; repeat split; try reflexivity.
          + apply native_size_bound_word; exact Bound.
          + apply native_size_bound_N; exact Bound.
        - eapply checked_trap, G.collector_size_rejects_large; [exact Functions|lia].
      Qed.

      (** The arity helper either traps or establishes the exact requested Tree length. *)
      Lemma checked_arity state store locals input expected :
        s_funcs store = s_funcs G.E.allocated_store -> lengths_bounded 1 input ->
        (Z.of_nat expected < 2 ^ 32)%Z ->
        checked
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [ext input; word expected] ++ [AI_basic (BI_call 25%N)])
          (fun final => let '(state', store', frame', instructions) := final in
            state' = state /\ store' = store /\ frame' = Build_frame locals G.E.collector_instance /\
            instructions = [] /\ exists tree, input = treeh tree /\ List.length (get_tree_raw tree) = expected).
      Proof.
        intros Functions Bounds Expected.
        destruct input as [[[blob|tree]|reference]|th|encoded];
          try solve [eapply checked_trap, G.collector_arity_rejects_non_tree;
            [exact Functions|intros; discriminate]].
        destruct Bounds as [Wide _].
        destruct (Z_lt_ge_dec (Z.of_nat (List.length (get_tree_raw tree))) (Z.of_N native_table_limit + 1)%Z) as [Bound|Large].
        - destruct (Nat.eq_dec (List.length (get_tree_raw tree)) expected) as [Length|Different].
          + eapply checked_result; [eapply arity_run; try eassumption; rewrite <- Length; exact Bound|].
            do 4 (split; [reflexivity|]).
            exists tree; split; [reflexivity|exact Length].
          + eapply checked_trap, G.collector_arity_rejects_length; eassumption.
        - eapply checked_trap, G.collector_arity_rejects_large; [exact Functions|lia].
      Qed.

      (** The Data guard either traps or establishes that its input is Data. *)
      Lemma checked_data state store locals input :
        s_funcs store = s_funcs G.E.allocated_store ->
        checked
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [ext input] ++ [AI_basic (BI_call 33%N)])
          (fun final => let '(state', store', frame', instructions) := final in
            state' = state /\ store' = store /\ frame' = Build_frame locals G.E.collector_instance /\
            instructions = [] /\ exists data, input = Data data).
      Proof.
        intro Functions; destruct input as [data|th|encoded].
        - eapply checked_result; [apply native_data_executes; exact Functions|].
          do 4 (split; [reflexivity|]).
          exists data; reflexivity.
        - eapply checked_trap, G.E.collector_call_void with (code := native_data)
            (parameters := [T_ref T_externref]) (defaults := []) (address := 33%N)
            (arguments := [ext (Thunk th)]) (accepted := false)
            (final_frame := Build_frame [ext (Thunk th)] G.E.collector_instance); try reflexivity.
          + rewrite Functions; reflexivity.
          + unfold native_data; cbn [modfunc_body mod_funcs evaluation_module nth].
            execute (native_type_query state store [ext (Thunk th)] 0%N (Thunk th) Functions eq_refl).
            do 2 native_pure; apply rt_refl.
        - eapply checked_trap, G.E.collector_call_void with (code := native_data)
            (parameters := [T_ref T_externref]) (defaults := []) (address := 33%N)
            (arguments := [ext (Encode encoded)]) (accepted := false)
            (final_frame := Build_frame [ext (Encode encoded)] G.E.collector_instance); try reflexivity.
          + rewrite Functions; reflexivity.
          + unfold native_data; cbn [modfunc_body mod_funcs evaluation_module nth].
            execute (native_type_query state store [ext (Encode encoded)] 0%N (Encode encoded) Functions eq_refl).
            do 2 native_pure; apply rt_refl.
      Qed.

      (** The lowering helper either traps or returns the lowered form of its Data input. *)
      Lemma checked_lower state store locals input :
        s_funcs store = s_funcs G.E.allocated_store ->
        checked
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [ext input] ++ [AI_basic (BI_call 27%N)])
          (fun final => let '(state', store', frame', instructions) := final in
            state' = state /\ store' = store /\ frame' = Build_frame locals G.E.collector_instance /\
            instructions = v_to_e_list [ext (lower input)] /\ exists data, input = Data data).
      Proof.
        intro Functions; destruct input as [data|th|encoded].
        - eapply checked_result; [apply native_lower_executes; exact Functions|].
          do 4 (split; [reflexivity|]).
          exists data; reflexivity.
        - eapply checked_trap, G.E.collector_call with (code := native_lower)
            (parameters := [T_ref T_externref]) (results := [T_ref T_externref])
            (address := 27%N) (defaults := []) (arguments := [ext (Thunk th)])
            (values := [ext (Thunk th)]) (final_frame := Build_frame [ext (Thunk th)] G.E.collector_instance)
            (accepted := false); try reflexivity.
          + rewrite Functions; reflexivity.
          + unfold native_lower at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
            execute (native_type_query state store [ext (Thunk th)] 0%N (Thunk th) Functions eq_refl).
            do 2 native_pure.
            execute (native_type_query state store [ext (Thunk th)] 0%N (Thunk th) Functions eq_refl).
            do 2 native_pure.
            eapply G.E.collector_trap_suffix with (instructions := [AI_trap]); apply rt_refl.
        - eapply checked_trap, G.E.collector_call with (code := native_lower)
            (parameters := [T_ref T_externref]) (results := [T_ref T_externref])
            (address := 27%N) (defaults := []) (arguments := [ext (Encode encoded)])
            (values := [ext (Encode encoded)]) (final_frame := Build_frame [ext (Encode encoded)] G.E.collector_instance)
            (accepted := false); try reflexivity.
          + rewrite Functions; reflexivity.
          + unfold native_lower at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
            execute (native_type_query state store [ext (Encode encoded)] 0%N (Encode encoded) Functions eq_refl).
            do 2 native_pure.
            execute (native_type_query state store [ext (Encode encoded)] 0%N (Encode encoded) Functions eq_refl).
            do 2 native_pure.
            eapply G.E.collector_trap_suffix with (instructions := [AI_trap]); apply rt_refl.
      Qed.

      Definition map_guard index :=
        match nth (3 + 4 * index) (modfunc_body native_map) BI_unreachable with
        | BI_if _ body _ => body | _ => [] end.

      (** A rejecting constructor guard propagates to a trap from the full native
          constructor call. *)
      Lemma native_map_guard_rejects state store locals index input :
        index < 5 -> s_funcs store = s_funcs G.E.allocated_store ->
        reduce_trans
          (state, store, Build_frame [word index; ext input] G.E.collector_instance, to_e_list (map_guard index))
          (state, store, Build_frame [word index; ext input] G.E.collector_instance, [AI_trap]) ->
        reduce_trans
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [word index; ext input] ++ [AI_basic (BI_call 34%N)])
          (state, store, Build_frame locals G.E.collector_instance, [AI_trap]).
      Proof.
        intros Index Functions Body.
        eapply G.E.collector_call_return with (code := native_map)
          (parameters := [T_num T_i32; T_ref T_externref]) (results := [T_ref T_externref])
          (defaults := []) (address := 34%N) (arguments := [word index; ext input])
          (values := [ext input]) (final_frame := Build_frame [word index; ext input] G.E.collector_instance)
          (accepted := false)
          (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn (4 * S index) (modfunc_body native_map))));
          try reflexivity.
        - rewrite Functions; reflexivity.
        - let finish := ltac:(
            unfold native_map at 1; cbn [modfunc_body mod_funcs evaluation_module nth];
            repeat native_pure; cbn [lfill];
            eapply G.E.collector_runs_context with (prefix := [])
              (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] [AI_trap]]);
            apply G.E.collector_runs_label; exact Body) in
          do 5 (destruct index as [|index]; [solve [finish]|]); lia.
      Qed.

      (** When an abstract constructor is undefined, the corresponding native constructor
          traps. *)
      Lemma native_map_rejects state store locals kind input :
        s_funcs store = s_funcs G.E.allocated_store -> X.construct kind input = None ->
        reduce_trans
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [word (native_constructor_index kind); ext input] ++ [AI_basic (BI_call 34%N)])
          (state, store, Build_frame locals G.E.collector_instance, [AI_trap]).
      Proof.
        intros Functions Invalid.
        destruct kind; destruct input as [[[blob|tree]|reference]|th|encoded];
          cbn [X.construct native_constructor_index] in *; try discriminate.
        all: first
          [ solve [
              first [eapply native_map_guard_rejects with (index := 0)
                    |eapply native_map_guard_rejects with (index := 1)];
                [lia|exact Functions|];
              unfold map_guard; cbn [native_map modfunc_body mod_funcs evaluation_module nth Nat.add Nat.mul];
              native_pure;
              eapply G.E.collector_trap_suffix with (instructions := v_to_e_list [_] ++ [AI_basic (BI_call 23%N)]);
              apply G.collector_tree_executes; exact Functions]
          | solve [
              first [eapply native_map_guard_rejects with (index := 2)
                    |eapply native_map_guard_rejects with (index := 3)
                    |eapply native_map_guard_rejects with (index := 4)];
                [lia|exact Functions|];
              unfold map_guard; cbn [native_map modfunc_body mod_funcs evaluation_module nth Nat.add Nat.mul];
              lazymatch goal with
              | |- ?relation (_, _, Build_frame ?inside G.E.collector_instance, _) _ =>
                let found := local_value inside 1%N in lazymatch found with
                | Some (VAL_ref (VAL_ref_extern (to_externref ?value))) =>
                  execute (native_type_query state store inside 1%N value Functions eq_refl)
                end
              end;
              do 2 native_pure;
              eapply G.E.collector_trap_suffix with (instructions := [AI_trap]); apply rt_refl]
          | solve [
              lazymatch goal with
              | |- reduce_trans (_, _, _, v_to_e_list [word ?mode; ext ?value] ++ _) _ =>
                eapply G.E.collector_call with (code := native_map)
                  (parameters := [T_num T_i32; T_ref T_externref]) (results := [T_ref T_externref])
                  (defaults := []) (address := 34%N) (arguments := [word mode; ext value])
                  (values := [ext value]) (final_frame := Build_frame [word mode; ext value] G.E.collector_instance)
                  (accepted := false); try reflexivity;
                [rewrite Functions; reflexivity|];
                unfold native_map at 1; cbn [modfunc_body mod_funcs evaluation_module nth];
                do 15 native_pure;
                execute (native_type_query state store [word mode; ext value] 1%N value Functions eq_refl);
                do 2 native_pure;
                eapply G.E.collector_trap_suffix with (instructions := [AI_trap]); apply rt_refl
              end]
          | solve [
              lazymatch goal with
              | |- reduce_trans (_, _, _, v_to_e_list [word ?mode; ext ?value] ++ _) _ =>
                eapply G.E.collector_call_return with (code := native_map)
                  (parameters := [T_num T_i32; T_ref T_externref]) (results := [T_ref T_externref])
                  (defaults := []) (address := 34%N) (arguments := [word mode; ext value])
                  (values := [ext value]) (final_frame := Build_frame [word mode; ext value] G.E.collector_instance)
                  (accepted := false)
                  (context := LH_rec [] 0 [] (LH_base [] []) (to_e_list (skipn 28 (modfunc_body native_map))));
                  try reflexivity;
                [rewrite Functions; reflexivity|];
                unfold native_map at 1; cbn [modfunc_body mod_funcs evaluation_module nth];
                do 15 native_pure;
                execute (native_type_query state store [word mode; ext value] 1%N value Functions eq_refl);
                do 5 native_pure; cbn [lfill];
                eapply G.E.collector_runs_context with (prefix := [])
                  (instructions := [AI_label 0 [] _]) (instructions' := [AI_label 0 [] [AI_trap]]);
                apply G.E.collector_runs_label;
                first [execute (native_data_type_query state store [word mode; ext value] 1%N
                    (Object (TreeObj tree)) Functions eq_refl)
                  |do 3 native_pure; execute (native_data_type_query state store [word mode; ext value] 1%N
                    (Object (BlobObj blob)) Functions eq_refl)];
                do 2 native_pure; apply rt_refl
              end] ].
      Qed.

      (** The native constructor either traps or returns exactly the abstract constructor's
          result. *)
      Lemma checked_map state store locals kind input :
        s_funcs store = s_funcs G.E.allocated_store ->
        checked
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [word (native_constructor_index kind); ext input] ++ [AI_basic (BI_call 34%N)])
          (fun final => let '(state', store', frame', instructions) := final in
            state' = state /\ store' = store /\ frame' = Build_frame locals G.E.collector_instance /\
            exists result, X.construct kind input = Some result /\ instructions = v_to_e_list [ext result]).
      Proof.
        intro Functions; destruct (X.construct kind input) as [result|] eqn:Constructed.
        - eapply checked_result; [eapply native_map_accepts; eassumption|].
          do 3 (split; [reflexivity|]).
          exists result; split; [reflexivity|reflexivity].
        - eapply checked_trap, native_map_rejects; eassumption.
      Qed.

      (** The mapped-congruence helper either traps or establishes the corresponding
          accepted abstract unary exchange. *)
      Lemma checked_mapped state store locals kind lhs rhs witness used rest :
        used ++ witness :: rest = witnesses -> prepared store used ->
        checked
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [word (native_constructor_index kind); ext lhs; ext rhs; ext (supplied_claim witness)] ++
              [AI_basic (BI_call 35%N)])
          (fun final => let '(_, store', frame', instructions) := final in
            instructions = [] /\ frame' = Build_frame locals G.E.collector_instance /\
            prepared store' (used ++ [witness]) /\
            X.unary Equivalent Equivalent (X.mapped kind) [witness] lhs rhs = Some Equivalent).
      Proof.
        intros Order Prepared; pose proof (prepared_functions _ _ Prepared) as Functions.
        set (empty := VAL_ref (VAL_ref_null T_externref)).
        set (arguments := [word (native_constructor_index kind); ext lhs; ext rhs; ext (supplied_claim witness)]).
        eapply checked_call with (code := native_mapped)
          (parameters := [T_num T_i32; T_ref T_externref; T_ref T_externref; T_ref T_externref])
          (results := []) (address := 35%N) (defaults := [empty; empty]) (arguments := arguments)
          (dummy := []); try reflexivity.
        - rewrite Functions; reflexivity.
        - unfold native_mapped at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
          checked_pure.
          normalize_checked.
          let inside := match goal with
            |- checked (_, _, Build_frame ?inside G.E.collector_instance, _) _ => constr:(inside) end in
          checked_use (checked_endpoint state store inside (supplied_claim witness) true Functions).
          intros [[[s1 st1] f1] c1] [Store1 [Frame1 [a Code1]]]; subst st1 f1 c1.
          do 2 checked_pure.
          normalize_checked.
          let inside := match goal with
            |- checked (_, _, Build_frame ?inside G.E.collector_instance, _) _ => constr:(inside) end in
          checked_use (checked_endpoint s1 store inside (supplied_claim witness) false Functions).
          intros [[[s2 st2] f2] c2] [Store2 [Frame2 [b Code2]]]; subst st2 f2 c2.
          do 4 checked_pure.
          normalize_checked.
          let inside := match goal with
            |- checked (_, _, Build_frame ?inside G.E.collector_instance, _) _ => constr:(inside) end in
          checked_use_under [ext lhs] (checked_map s2 store inside kind a Functions).
          intros [[[s3 st3] f3] c3] [State3 [Store3 [Frame3 [x [ConstructLeft Code3]]]]]; subst s3 st3 f3 c3.
          normalize_checked.
          let inside := match goal with
            |- checked (_, _, Build_frame ?inside G.E.collector_instance, _) _ => constr:(inside) end in
          checked_use (checked_same s2 store inside lhs x Functions).
          intros [[[s4 st4] f4] c4] [Store4 [Frame4 [Code4 Left]]]; subst st4 f4 c4 lhs.
          do 3 checked_pure.
          normalize_checked.
          let inside := match goal with
            |- checked (_, _, Build_frame ?inside G.E.collector_instance, _) _ => constr:(inside) end in
          checked_use_under [ext rhs] (checked_map s4 store inside kind b Functions).
          intros [[[s5 st5] f5] c5] [State5 [Store5 [Frame5 [y [ConstructRight Code5]]]]]; subst s5 st5 f5 c5.
          normalize_checked.
          let inside := match goal with
            |- checked (_, _, Build_frame ?inside G.E.collector_instance, _) _ => constr:(inside) end in
          checked_use (checked_same s4 store inside rhs y Functions).
          intros [[[s6 st6] f6] c6] [Store6 [Frame6 [Code6 Right]]]; subst st6 f6 c6 rhs.
          do 3 checked_pure.
          normalize_checked.
          let inside := match goal with
            |- checked (_, _, Build_frame ?inside G.E.collector_instance, _) _ => constr:(inside) end in
          checked_use (checked_premise s6 store inside used witness rest 0 Equivalent a b Order Prepared eq_refl).
          intros [[[s7 st7] f7] c7] [Code7 [Frame7 [Prepared7 Read]]]; subst f7 c7.
          eapply checked_result; [apply rt_refl|].
          exists []; split; [left; reflexivity|]; split; [reflexivity|].
          do 2 (split; [reflexivity|]); split; [exact Prepared7|].
          cbn [X.unary]; rewrite Read; unfold X.mapped; rewrite ConstructLeft.
          cbn [X.D.EC.E.EP.E.H.obind]; rewrite ConstructRight.
          cbn -[X.API.handle_eqb].
          rewrite !(proj2 (X.API.handle_eqb_spec _ _) eq_refl); reflexivity.
      Qed.

      (** A checked void body lifts through its surrounding Wasm label. *)
      Lemma checked_void_label state store frame body post :
        checked (state, store, frame, body)
          (fun final => let '(_, _, _, instructions) := final in instructions = [] /\ post final) ->
        checked (state, store, frame, [AI_label 0 [] body]) post.
      Proof.
        intro Body.
        assert (Shape : in_context 1 (LH_rec [] 0 [] (LH_base [] []) []) (state, store, frame, body) =
          (state, store, frame, [AI_label 0 [] body])).
        { cbn [in_context lfill]; rewrite !seq.cat0s, !seq.cats0; reflexivity. }
        rewrite <- Shape; eapply checked_context with (initial := (state, store, frame, body))
          (context := LH_rec [] 0 [] (LH_base [] []) []) (before := fun final =>
            let '(_, _, _, instructions) := final in instructions = [] /\ post final); [exact Body|].
        intros [[[state' store'] frame'] instructions] [Empty Post]; subst instructions.
        normalize_checked.
        eapply checked_result with (final := (state', store', frame', []));
          [apply rt_step, r_simple, rs_label_const; reflexivity|exact Post].
      Qed.

      (** The type guard either traps or establishes the requested handle classification. *)
      Lemma checked_type_guard state store locals input expected :
        s_funcs store = s_funcs G.E.allocated_store -> expected < 4 ->
        checked
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [ext input] ++ to_e_list
              [BI_call 0%N; BI_const_num (VAL_int32 (wasm_nat expected));
               BI_relop T_i32 (Relop_i ROI_eq); BI_call 21%N])
          (fun final => let '(state', store', frame', instructions) := final in
            state' = state /\ store' = store /\ frame' = Build_frame locals G.E.collector_instance /\
            instructions = [] /\ G.E.H.W.H.value_type input = expected).
      Proof.
        intros Functions Expected.
        assert (TypeBound : (Z.of_nat (G.E.H.W.H.value_type input) < 2 ^ 32)%Z).
        { destruct input as [[[]|reference]|th|encoded]; cbn [G.E.H.W.H.value_type]; lia. }
        pose proof (wasm_nat_equal (G.E.H.W.H.value_type input) expected TypeBound ltac:(cbn; lia)) as Compared.
        change (app_relop (Relop_i ROI_eq)
          (VAL_int32 (wasm_nat (G.E.H.W.H.value_type input))) (VAL_int32 (wasm_nat expected)) =
          Nat.eqb (G.E.H.W.H.value_type input) expected) in Compared.
        assert (Run : reduce_trans
          (state, store, Build_frame locals G.E.collector_instance,
            v_to_e_list [ext input] ++ to_e_list
              [BI_call 0%N; BI_const_num (VAL_int32 (wasm_nat expected));
               BI_relop T_i32 (Relop_i ROI_eq); BI_call 21%N])
          (state, store, Build_frame locals G.E.collector_instance,
            if Nat.eqb (G.E.H.W.H.value_type input) expected then [] else [AI_trap])).
        { execute (native_value_import state store locals 0%N shell_get_type [ext input]
            [word (G.E.H.W.H.value_type input)] Functions eq_refl eq_refl eq_refl
            ltac:(unfold G.E.H.host_values, ext; rewrite to_handle_to_externref; reflexivity)).
          pose proof (native_relop state store locals (VAL_int32 (wasm_nat (G.E.H.W.H.value_type input)))
            (VAL_int32 (wasm_nat expected)) T_i32 (Relop_i ROI_eq) eq_refl) as CompareRun.
          rewrite Compared in CompareRun; execute CompareRun.
          exact (native_require state store locals (Nat.eqb (G.E.H.W.H.value_type input) expected) Functions). }
        destruct (Nat.eqb (G.E.H.W.H.value_type input) expected) eqn:Same.
        - eapply checked_result; [exact Run|]; repeat split; try reflexivity.
          apply Nat.eqb_eq; exact Same.
        - eapply checked_trap; exact Run.
      Qed.

      (** The leaf guard either traps or establishes the abstract leaf-evaluation
          predicate. *)
      Lemma checked_leaf_guard state store locals input :
        s_funcs store = s_funcs G.E.allocated_store -> lookup_N locals 1%N = Some (ext input) ->
        checked
          (state, store, Build_frame locals G.E.collector_instance,
            to_e_list [BI_local_get 1%N; BI_call 0%N; BI_const_num (VAL_int32 (wasm_nat 3));
              BI_relop T_i32 (Relop_i ROI_ne); BI_call 21%N;
              BI_local_get 1%N; BI_call 0%N; BI_testop T_i32 TO_eqz;
              BI_if (BT_valtype None)
                [BI_local_get 1%N; BI_call 1%N; BI_testop T_i32 TO_eqz; BI_call 21%N] []])
          (fun final => let '(state', store', frame', instructions) := final in
            state' = state /\ store' = store /\ frame' = Build_frame locals G.E.collector_instance /\
            instructions = [] /\ X.reflexive_eval input = true).
      Proof.
        intros Functions Local; destruct (X.reflexive_eval input) eqn:Leaf.
        - eapply checked_result; [eapply native_leaf_guard; eassumption|].
          repeat split; assumption || reflexivity.
        - destruct input as [[[blob|tree]|reference]|th|encoded]; cbn [X.reflexive_eval] in Leaf; try discriminate.
          + eapply checked_trap.
            execute (native_type_query state store locals 1%N (treeh tree) Functions Local).
            do 2 native_pure.
            execute (native_type_query state store locals 1%N (treeh tree) Functions Local).
            native_pure; apply G.E.collector_if_void with (condition := true) (accepted := false).
            execute (native_data_type_query state store locals 1%N (Object (TreeObj tree)) Functions Local).
            do 2 native_pure; apply rt_refl.
          + eapply checked_trap.
            execute (native_type_query state store locals 1%N (Encode encoded) Functions Local).
            do 2 native_pure.
            eapply G.E.collector_trap_suffix with (instructions := [AI_trap]); apply rt_refl.
      Qed.

      (** The strict-result guard either traps or establishes that the output is the lifted
          form of the supplied Data. *)
      Lemma checked_strict_result state store locals input result :
        s_funcs store = s_funcs G.E.allocated_store ->
        lookup_N locals 7%N = Some (ext input) -> lookup_N locals 2%N = Some (ext result) ->
        checked
          (state, store, Build_frame locals G.E.collector_instance,
            to_e_list [BI_local_get 7%N; BI_call 33%N;
              BI_local_get 7%N; BI_call 0%N; BI_testop T_i32 TO_eqz;
              BI_if (BT_valtype None)
                [BI_local_get 2%N; BI_local_get 7%N; BI_call 22%N]
                [BI_local_get 7%N; BI_local_get 2%N; BI_call 28%N]])
          (fun final => let '(_, store', frame', instructions) := final in
            store' = store /\ frame' = Build_frame locals G.E.collector_instance /\ instructions = [] /\
            exists data, input = Data data /\ result = lift input).
      Proof.
        intros Functions Input Result.
        checked_advance (local_get_run state store locals 7%N (ext input) Input).
        checked_use (checked_data state store locals input Functions).
        intros [[[state' store'] frame'] instructions] [State [Store [Frame [Code [data Data]]]]];
          subst state' store' frame' instructions input.
        checked_advance (native_type_query state store locals 7%N (Data data) Functions Input).
        destruct data as [object|reference]; do 2 checked_pure; apply checked_void_label.
        - checked_advance (local_get_run state store locals 2%N (ext result) Result).
          checked_advance_under [ext result]
            (local_get_run state store locals 7%N (ext (Data (Object object))) Input).
          checked_use (checked_same state store locals result (Data (Object object)) Functions).
          intros [[[state' store'] frame'] instructions] [Store [Frame [Code Same]]]; subst store' frame' instructions result.
          eapply checked_result; [apply rt_refl|]; split; [reflexivity|].
          do 3 (split; [reflexivity|]).
          exists (Object object); split; reflexivity.
        - checked_advance (local_get_run state store locals 7%N (ext (Data (Ref reference))) Input).
          checked_advance_under [ext (Data (Ref reference))]
            (local_get_run state store locals 2%N (ext result) Result).
          checked_use (checked_object state store locals (Data (Ref reference)) result Functions).
          intros [[[state' store'] frame'] instructions] [Store [Frame [Code [object [Object Reference]]]]];
            subst store' frame' instructions result.
          eapply checked_result; [apply rt_refl|]; split; [reflexivity|].
          do 3 (split; [reflexivity|]).
          exists (Ref reference); split; [reflexivity|].
          inversion Reference; subst reference; destruct object; reflexivity.
      Qed.
      (** An in-bounds indexed list suffix begins with the selected child followed by the
          next suffix. *)
      Lemma remaining_child {A : Type} (values : list A) index value :
        nth_error values index = Some value -> skipn index values = value :: skipn (S index) values.
      Proof.
        revert values; induction index as [|index IH]; intros [|first rest] Entry;
          cbn in Entry |- *; try discriminate.
        - inversion Entry; reflexivity.
        - apply IH; exact Entry.
      Qed.

      (** Branching to the native loop label restarts its body without changing state. *)
      Lemma checked_loop_restart code state store frame :
        reduce_trans
          (state, store, frame, G.E.collector_loop_state code [AI_basic (BI_br 0%N)])
          (state, store, frame, G.E.collector_loop_state code (to_e_list code)).
      Proof.
        eapply rt_trans with (y := (state, store, frame, [AI_label 0 [] (G.E.collector_loop_cont code)])).
        - apply G.E.collector_runs_label, rt_step, r_simple;
            eapply rs_br with (vs := []) (i := 0%nat) (lh := LH_base [] []); reflexivity.
        - apply G.E.collector_runs_label, rt_step;
            eapply r_loop with (vs := []) (n := 0%nat) (t1s := []) (t2s := []); reflexivity.
      Qed.

      Section TreeValidation.
        Variable lhs rhs coupons : TreeName.
        Variable kind : nat.
        Variable claim_kind : statement_kind.
        (** The numeric claim code denotes the intended abstract statement kind. *)
        Hypothesis Kind : Claims.Code.statement_kind kind = Some claim_kind.
        Let count := List.length (get_tree_raw lhs).
        (** The Tree-loop iteration count fits in an unsigned 32-bit integer. *)
        Hypothesis Count : (Z.of_nat count < 2 ^ 32)%Z.
        (** The right Tree contains exactly the shared iteration count of children. *)
        Hypothesis Right : List.length (get_tree_raw rhs) = count.
        (** The witness Tree contains exactly the ordered witnesses used by the abstract
            checker. *)
        Hypothesis Coupons : get_tree_raw coupons = witnesses.
        (** The ordered witness list has one entry per Tree-loop iteration. *)
        Hypothesis WitnessCount : List.length witnesses = count.
        Let arguments := [VAL_num (VAL_int64 (wasm_nat64 kind)); ext (treeh lhs);
          ext (treeh rhs); ext (treeh coupons)].
        Let locals index := arguments ++ [word count; word index].
        Let frame index := Build_frame (locals index) G.E.collector_instance.

        Definition loop_prepared store used := prepared store used /\
          tree_view store 7%N lhs /\ tree_view store 8%N rhs.

        (** A Tree-loop iteration either traps or validates the next child pair and
            preserves the loop invariant. *)
        Lemma checked_tree_iteration state store index used :
          index < count -> used ++ skipn index witnesses = witnesses -> loop_prepared store used ->
          checked (state, store, frame index, to_e_list native_tree_loop)
            (fun final => let '(_, store', frame', instructions) := final in
              frame' = frame (S index) /\ instructions = [AI_basic (BI_br 0%N)] /\
              exists witness left right,
                nth_error witnesses index = Some witness /\ nth_error (get_tree_raw lhs) index = Some left /\
                nth_error (get_tree_raw rhs) index = Some right /\
                loop_prepared store' (used ++ [witness]) /\ X.read claim_kind witness = Some (left, right)).
        Proof.
          intros Index Order [Prepared [LeftView RightView]].
          pose proof (prepared_functions _ _ Prepared) as Functions.
          assert (IndexBound : (Z.of_nat index < 2 ^ 32)%Z) by lia.
          destruct (nth_error witnesses index) as [witness|] eqn:Witness;
            [|apply nth_error_None in Witness; lia].
          destruct (nth_error (get_tree_raw lhs) index) as [left|] eqn:Left;
            [|apply nth_error_None in Left; unfold count in Index; lia].
          destruct (nth_error (get_tree_raw rhs) index) as [right|] eqn:RightField;
            [|apply nth_error_None in RightField; lia].
          assert (Order' : used ++ witness :: skipn (S index) witnesses = witnesses).
          { rewrite <- Order at 2; f_equal; symmetry; eapply remaining_child; exact Witness. }
          pose proof (counter_test_run state store (locals index) 5%N 4%N index count
            IndexBound Count eq_refl eq_refl) as Test.
          assert (Continue : Nat.eqb index count = false) by (apply Nat.eqb_neq; lia).
          rewrite Continue in Test; unfold frame at 1, native_tree_loop.
          checked_advance Test; do 2 checked_pure.
          checked_advance_under [VAL_num (VAL_int64 (wasm_nat64 kind))]
            (retained_claim_run state store (locals index) witnesses index witness IndexBound
              (prepared_retained _ _ Prepared) Witness).
          checked_pure.
          checked_advance_under [VAL_num (VAL_int64 (wasm_nat64 kind)); ext (supplied_claim witness)]
            (tree_view_run state store (locals index) lhs 7%N index left IndexBound LeftView Left).
          checked_pure.
          checked_advance_under [VAL_num (VAL_int64 (wasm_nat64 kind)); ext (supplied_claim witness); ext left]
            (tree_view_run state store (locals index) rhs 8%N index right IndexBound RightView RightField).
          checked_use (checked_premise_preserving state store (locals index) used witness
            (skipn (S index) witnesses) kind claim_kind left right
            (fun next => tree_view next 7%N lhs /\ tree_view next 8%N rhs)
            Order' Prepared Kind (conj LeftView RightView)
            ltac:(intros next_state next Ready; split;
              [eapply premise_ready_tree_view with (address := 7%N)
              |eapply premise_ready_tree_view with (address := 8%N)];
              try exact Ready; try reflexivity; try discriminate; assumption)).
          intros [[[state' store'] frame'] instructions] [Code [Frame [Prepared' [Views Read]]]];
            subst frame' instructions.
          checked_pure.
          assert (Increment : reduce_trans
            (state', store', frame index,
              [v_to_e (word index); AI_basic (BI_const_num (VAL_int32 (wasm_nat 1)));
               AI_basic (BI_binop T_i32 (Binop_i BOI_add))])
            (state', store', frame index, v_to_e_list [word (S index)])).
          { apply rt_step, r_simple, wasm_nat_increment_step; exact IndexBound. }
          checked_advance Increment.
          pose proof (local_set_run state' store' (locals index) 5%N (word (S index))
            ltac:(unfold locals, arguments; lazy; lia)) as Update.
          change (reduce_trans
            (state', store', frame index, [v_to_e (word (S index)); AI_basic (BI_local_set 5%N)])
            (state', store', frame (S index), [])) in Update.
          checked_advance Update.
          eapply checked_result; [apply rt_refl|].
          split; [reflexivity|]; split; [reflexivity|].
          exists witness, left, right; repeat first [assumption | reflexivity | split].
        Qed.

        (** The remaining Tree-loop iterations either trap or validate all remaining pairs
            while preserving the scratch tables. *)
        Lemma checked_tree_loop remaining index state store used :
          index + remaining = count -> used ++ skipn index witnesses = witnesses -> loop_prepared store used ->
          checked (state, store, frame index, G.E.collector_loop_state native_tree_loop (to_e_list native_tree_loop))
            (fun final => let '(_, store', _, instructions) := final in
              instructions = [] /\ loop_prepared store' witnesses /\
              X.children claim_kind (skipn index (get_tree_raw lhs))
                (skipn index (get_tree_raw rhs)) (skipn index witnesses) = true).
        Proof.
          revert index state store used; induction remaining as [|remaining IH];
            intros index state store used End Order Prepared.
          - assert (Index : index = count) by lia; subst index.
            assert (EmptyLeft : skipn count (get_tree_raw lhs) = []) by apply skipn_all.
            assert (EmptyRight : skipn count (get_tree_raw rhs) = []) by (rewrite <- Right; apply skipn_all).
            assert (EmptyWitnesses : skipn count witnesses = []) by (rewrite <- WitnessCount; apply skipn_all).
            rewrite EmptyWitnesses, List.app_nil_r in Order; subst used.
            pose proof (counter_test_run state store (locals count) 5%N 4%N count count
              Count Count eq_refl eq_refl) as Test.
            rewrite Nat.eqb_refl in Test.
            eapply checked_result with (final := (state, store, frame count, [])).
            + eapply G.E.collector_loop_exit with (accepted := true)
                (suffix := to_e_list (skipn 4 native_tree_loop)).
              change (reduce_trans (state, store, frame count,
                  to_e_list (firstn 4 native_tree_loop) ++ to_e_list (skipn 4 native_tree_loop))
                (state, store, frame count, AI_basic (BI_br 1%N) :: to_e_list (skipn 4 native_tree_loop))).
              eapply G.E.collector_runs_context with (prefix := [])
                (suffix := to_e_list (skipn 4 native_tree_loop))
                (instructions := to_e_list (firstn 4 native_tree_loop))
                (instructions' := [AI_basic (BI_br 1%N)]); exact Test.
            + split; [reflexivity|]; split; [exact Prepared|].
              rewrite EmptyLeft, EmptyRight, EmptyWitnesses; reflexivity.
          - eapply checked_context with (initial := (state, store, frame index, to_e_list native_tree_loop))
              (context := LH_rec [] 0 [] (LH_rec [] 0 (G.E.collector_loop_cont native_tree_loop)
                (LH_base [] []) []) [])
              (before := fun final => let '(_, store', frame', instructions) := final in
                frame' = frame (S index) /\ instructions = [AI_basic (BI_br 0%N)] /\
                exists witness left right,
                  nth_error witnesses index = Some witness /\ nth_error (get_tree_raw lhs) index = Some left /\
                  nth_error (get_tree_raw rhs) index = Some right /\
                  loop_prepared store' (used ++ [witness]) /\ X.read claim_kind witness = Some (left, right)).
            + apply checked_tree_iteration; [lia|exact Order|exact Prepared].
            + intros [[[state' store'] frame'] instructions] [Frame [Code [witness [left [right
                [Witness [Left [RightField [Prepared' Read]]]]]]]]]; subst frame' instructions.
              eapply checked_progress; [apply checked_loop_restart|].
              eapply checked_weaken.
              * apply IH with (used := used ++ [witness]); [lia| |exact Prepared'].
                rewrite <- Order at 2.
                rewrite <- List.app_assoc; cbn [List.app]; f_equal.
                symmetry; eapply remaining_child; exact Witness.
              * intros [[[state'' store''] frame''] instructions] [Code [Prepared'' Children]].
                split; [exact Code|]; split; [exact Prepared''|].
                rewrite (remaining_child (get_tree_raw lhs) index left Left),
                  (remaining_child (get_tree_raw rhs) index right RightField),
                  (remaining_child witnesses index witness Witness).
                cbn [X.children]; rewrite Read.
                rewrite !(proj2 (X.API.handle_eqb_spec _ _) eq_refl); cbn; exact Children.
        Qed.
      End TreeValidation.

      (** The native Tree rule either traps or agrees with the accepted abstract Tree
          exchange. *)
      Lemma checked_tree_rule state store caller kind claim_kind lhs rhs coupons :
        Claims.Code.statement_kind kind = Some claim_kind -> prepared store [] -> fresh_tree_views store ->
        get_tree_raw coupons = witnesses -> lengths_bounded 1 lhs -> lengths_bounded 1 rhs ->
        lengths_bounded 1 (treeh coupons) ->
        checked
          (state, store, Build_frame caller G.E.collector_instance,
            v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 kind)); ext lhs; ext rhs; ext (treeh coupons)] ++
              [AI_basic (BI_call 36%N)])
          (fun final => let '(_, store', frame', instructions) := final in
            instructions = [] /\ frame' = Build_frame caller G.E.collector_instance /\
            prepared store' witnesses /\ X.tree claim_kind witnesses lhs rhs = Some claim_kind).
      Proof.
        intros Kind Prepared Fresh Coupons LeftBound RightBound CouponBound.
        pose proof (prepared_functions _ _ Prepared) as Functions.
        set (arguments := [VAL_num (VAL_int64 (wasm_nat64 kind)); ext lhs; ext rhs; ext (treeh coupons)]).
        eapply checked_call with (code := native_tree_rule)
          (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
          (results := []) (address := 36%N) (defaults := [word 0; word 0])
          (arguments := arguments) (dummy := []); try reflexivity.
        - rewrite Functions; reflexivity.
        - unfold native_tree_rule at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
          checked_pure.
          normalize_checked.
          let inside := match goal with
            |- checked (_, _, Build_frame ?inside G.E.collector_instance, _) _ => constr:(inside) end in
          checked_use (checked_size state store inside lhs Functions LeftBound).
          intros [[[s1 st1] f1] c1] [State1 [Store1 [Frame1 [left [Left [Count [NativeCount Code1]]]]]]];
            subst s1 st1 f1 c1 lhs.
          do 3 checked_pure.
          normalize_checked.
          let inside := match goal with
            |- checked (_, _, Build_frame ?inside G.E.collector_instance, _) _ => constr:(inside) end in
          checked_use (checked_arity state store inside rhs (List.length (get_tree_raw left)) Functions RightBound Count).
          intros [[[s2 st2] f2] c2] [State2 [Store2 [Frame2 [Code2 [right [Right LengthRight]]]]]];
            subst s2 st2 f2 c2 rhs.
          do 2 checked_pure.
          normalize_checked.
          let inside := match goal with
            |- checked (_, _, Build_frame ?inside G.E.collector_instance, _) _ => constr:(inside) end in
          checked_use (checked_arity state store inside (treeh coupons) (List.length (get_tree_raw left))
            Functions CouponBound Count).
          intros [[[s3 st3] f3] c3] [State3 [Store3 [Frame3 [Code3 [actual [Actual LengthCoupons]]]]]];
            subst s3 st3 f3 c3; inversion Actual; subst actual.
          normalize_checked.
          lazymatch goal with
          | |- checked (_, _, Build_frame ?inside _, _) _ =>
            checked_advance (native_tag_query state store inside 1%N left Functions eq_refl);
            checked_advance_under [VAL_num (VAL_int32 (wasm_bool (tree_tag left)))]
              (native_tag_query state store inside 2%N right Functions eq_refl)
          end.
          destruct (Bool.bool_dec (tree_tag left) (tree_tag right)) as [Markers|Different].
          + {
            assert (WitnessCount : List.length witnesses = List.length (get_tree_raw left))
              by (rewrite <- Coupons; exact LengthCoupons).
            destruct (prepared_operand_views store left right [] Prepared Fresh NativeCount LengthRight)
              as [first [next [LeftAttached [RightAttached [NextPrepared [LeftView RightView]]]]]].
            assert (FirstFunctions : s_funcs first = s_funcs G.E.allocated_store).
            { rewrite (G.E.H.tree_attachment_result_functions _ _ _ _ _ LeftAttached); exact Functions. }
            rewrite <- Markers.
            let finish := ltac:(
              do 2 checked_pure;
              let inside := match goal with |- checked (_, _, Build_frame ?inside _, _) _ => constr:(inside) end in
              checked_pure;
              checked_advance (attach_tree_run state store inside left 7 first Functions LeftAttached);
              checked_pure;
              checked_advance (attach_tree_run state first inside right 8 next FirstFunctions RightAttached);
              eapply checked_progress; [apply G.E.collector_loop_enter|];
              eapply checked_weaken;
              [ eapply checked_tree_loop with (lhs := left) (rhs := right) (coupons := coupons)
                  (kind := kind) (claim_kind := claim_kind) (remaining := List.length (get_tree_raw left))
                  (index := 0) (used := []); try assumption; try reflexivity;
                  unfold loop_prepared; auto
              | intros [[[state' store'] frame'] instructions] [Code [Prepared' Children]];
                subst instructions; eapply ex_intro with (x := []);
                split; [left; reflexivity|]; do 3 (split; [reflexivity|]); split; [exact (proj1 Prepared')|];
                cbn [List.skipn] in Children;
                change ((if Bool.eqb (tree_tag left) (tree_tag right) &&
                  X.children claim_kind (get_tree_raw left) (get_tree_raw right) witnesses
                  then Some claim_kind else None) = Some claim_kind);
                rewrite Tag, <- Markers, Children; reflexivity ]) in
            destruct (tree_tag left) eqn:Tag; finish.
            }
          + destruct (tree_tag left), (tree_tag right); try contradiction;
              do 2 checked_pure; eapply checked_trap, G.E.collector_trap_suffix
                with (instructions := [AI_trap]); apply rt_refl.
      Qed.

      Theorem collector_reconstruction state store caller metadata callable opcode original fields coupons author body :
        get_tree_raw combination = [metadata; callable; opcode; treeh original; treeh fields; treeh coupons] ->
        tree_tag original = true -> get_tree_raw fields = author :: body ->
        get_tree_raw coupons = witnesses -> prepared store [] -> fresh_tree_views store ->
        lengths_bounded 1 (treeh original) -> lengths_bounded 1 (treeh fields) ->
        lengths_bounded 1 (treeh coupons) ->
        checked
          (state, store, Build_frame caller G.E.collector_instance,
            v_to_e_list [ext (treeh combination)] ++ [AI_basic (BI_call 40%N)])
          (fun final => let '(_, store', frame', instructions) := final in
            frame' = Build_frame caller G.E.collector_instance /\ prepared store' witnesses /\
            exists result, instructions = v_to_e_list [ext (treeh result)] /\
              result = create_tree_tagged true (get_tree_raw fields) /\
              X.tree Eval witnesses (treeh original) (treeh result) = Some Eval).
      Proof.
        intros Combination Tagged Fields Coupons Prepared Fresh OriginalBound FieldsBound CouponsBound.
        pose proof (prepared_functions _ _ Prepared) as Functions.
        set (empty := VAL_ref (VAL_ref_null T_externref)).
        set (tag := create_tree_tagged true (get_tree_raw fields)).
        assert (TagBound : lengths_bounded 1 (treeh tag)).
        { unfold tag; cbn [lengths_bounded G.E.H.W.H.HTreeObj];
            rewrite get_tree_raw_create_tree_tagged; exact FieldsBound. }
        eapply checked_call with (code := native_reconstruct) (parameters := [T_ref T_externref])
          (results := [T_ref T_externref]) (address := 40%N) (defaults := [empty; empty; empty; empty])
          (arguments := [ext (treeh combination)]) (dummy := [ext (treeh fields)]); try reflexivity.
        - rewrite Functions; reflexivity.
        - unfold native_reconstruct at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
          checked_pure.
          checked_advance (arity_run state store [ext (treeh combination); empty; empty; empty; empty]
            combination 6 Functions ltac:(rewrite Combination; reflexivity) ltac:(vm_compute; reflexivity)).
          checked_advance (tree_view_run state store [ext (treeh combination); empty; empty; empty; empty]
            combination 2%N 3 (treeh original) ltac:(vm_compute; reflexivity)
              (prepared_combination _ _ Prepared) ltac:(rewrite Combination; reflexivity)).
          checked_pure.
          checked_advance (native_tag_query state store
            [ext (treeh combination); ext (treeh original); empty; empty; empty]
            1%N original Functions eq_refl).
          rewrite Tagged; checked_pure.
          checked_advance (tree_view_run state store
            [ext (treeh combination); ext (treeh original); empty; empty; empty]
            combination 2%N 4 (treeh fields) ltac:(vm_compute; reflexivity)
              (prepared_combination _ _ Prepared) ltac:(rewrite Combination; reflexivity)).
          checked_pure.
          checked_advance (tree_view_run state store
            [ext (treeh combination); ext (treeh original); ext (treeh fields); empty; empty]
            combination 2%N 5 (treeh coupons) ltac:(vm_compute; reflexivity)
              (prepared_combination _ _ Prepared) ltac:(rewrite Combination; reflexivity)).
          do 2 checked_pure.
          let inside := match goal with |- checked (_, _, Build_frame ?inside _, _) _ => constr:(inside) end in
          assert (Create : reduce_trans
            (state, store, Build_frame inside G.E.collector_instance,
              v_to_e_list [ext (treeh fields)] ++ [AI_basic (BI_call 19%N)])
            (state, store, Build_frame inside G.E.collector_instance, v_to_e_list [ext (treeh tag)])).
          { eapply native_value_import with (operation := shell_create_tag_unchecked); try reflexivity.
            - exact Functions.
            - unfold tag; rewrite Fields; eapply G.E.H.unchecked_tag_preserves_fields;
                try reflexivity; exact Fields. }
          checked_advance Create; do 4 checked_pure.
          let inside := match goal with |- checked (_, _, Build_frame ?inside _, _) _ => constr:(inside) end in
          checked_use (checked_tree_rule state store inside 1 Eval (treeh original) (treeh tag) coupons
            eq_refl Prepared Fresh Coupons OriginalBound TagBound CouponsBound).
          intros [[[state' store'] frame'] instructions] [Code [Frame [Prepared' Checked]]];
            subst frame' instructions.
          checked_pure.
          eapply checked_result; [apply rt_refl|].
          exists [ext (treeh tag)]; split; [left; reflexivity|]; split; [reflexivity|].
          split; [reflexivity|]; split; [exact Prepared'|]; exists tag; repeat split; assumption || reflexivity.
      Qed.

      Definition validated rule lhs rhs finish (final : configuration) :=
        let '(_, store, _, instructions) := final in exists code claim_kind,
          Claims.Code.statement_kind code = Some claim_kind /\ prepared store witnesses /\
          X.check rule witnesses lhs rhs = Some claim_kind /\ finish code instructions.

      (** A validated branch return remains validated when lifted through its surrounding
          label and instruction suffix. *)
      Lemma checked_return_label rule lhs rhs state store frame body suffix :
        checked (state, store, frame, body)
          (validated rule lhs rhs (fun code instructions =>
            instructions = v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 code))] ++ [AI_basic BI_return])) ->
        checked (state, store, frame, [AI_label 0 [] body] ++ suffix)
          (validated rule lhs rhs (fun code instructions =>
            exists depth (context : lholed depth), instructions = lfill context
              (v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 code))] ++ [AI_basic BI_return]))).
      Proof.
        intro Body.
        assert (Shape : in_context 1 (LH_rec [] 0 [] (LH_base [] []) suffix) (state, store, frame, body) =
          (state, store, frame, [AI_label 0 [] body] ++ suffix)).
        { cbn [in_context lfill]; rewrite !seq.cat0s, !seq.cats0; reflexivity. }
        rewrite <- Shape; eapply checked_context with (initial := (state, store, frame, body))
          (context := LH_rec [] 0 [] (LH_base [] []) suffix); [exact Body|].
        intros [[[state' store'] frame'] instructions] [code [claim_kind [Kind [Prepared [Checked Code]]]]];
          subst instructions.
        eapply checked_result; [apply rt_refl|].
        exists code, claim_kind; split; [exact Kind|]; split; [exact Prepared|]; split; [exact Checked|].
        exists 1, (LH_rec [] 0 [] (LH_base [] []) suffix); reflexivity.
      Qed.

      (** The witness-arity block either traps or establishes the requested number of
          witness entries. *)
      Lemma checked_arity_block state store locals coupons expected :
        s_funcs store = s_funcs G.E.allocated_store -> lengths_bounded 1 (treeh coupons) ->
        (Z.of_nat expected < 2 ^ 32)%Z -> lookup_N locals 3%N = Some (ext (treeh coupons)) ->
        checked (state, store, Build_frame locals G.E.collector_instance,
          [AI_label 0 [] (to_e_list [BI_local_get 3%N; BI_const_num (VAL_int32 (wasm_nat expected)); BI_call 25%N])])
          (fun final => let '(state', store', frame', instructions) := final in
            state' = state /\ store' = store /\ frame' = Build_frame locals G.E.collector_instance /\
            instructions = [] /\ List.length (get_tree_raw coupons) = expected).
      Proof.
        intros Functions Bounds Expected Local; apply checked_void_label.
        checked_advance (local_get_run state store locals 3%N (ext (treeh coupons)) Local).
        checked_use (checked_arity state store locals (treeh coupons) expected Functions Bounds Expected).
        intros [[[state' store'] frame'] instructions] [State [Store [Frame [Code [actual [Actual Length]]]]]];
          subst state' store' frame' instructions; inversion Actual; subst actual.
        eapply checked_result; [apply rt_refl|].
        split; [reflexivity|]; repeat split; assumption || reflexivity.
      Qed.

      Ltac validate_constructs :=
        repeat match goal with
        | Construct : X.construct ?kind ?input = Some ?output |- _ =>
          tryif is_var input then destruct input as [[[blob|tree]|reference]|thunk|[thunk|thunk]] else idtac;
          cbn [X.construct] in Construct; try discriminate; inversion Construct; subst; clear Construct
        end.

      Ltac validate_logic :=
        first [assumption | idtac];
        try match goal with Shape : witnesses = ?values |- _ => rewrite Shape end;
        try assumption;
        validate_constructs;
        cbn [X.check X.unary X.binary X.force_results X.self X.leaf X.identification];
        repeat match goal with
        | Read : X.read ?kind ?witness = Some (?left, ?right) |- context [X.read ?kind ?witness] => rewrite Read
        end;
        cbn [X.D.EC.E.EP.E.H.obind X.D.EC.E.EP.E.H.omap];
        cbv beta iota zeta delta [X.think_application X.think_selection X.force_encode X.think_step X.think_data
          X.substitute_eval X.substitute_force X.transitive];
        rewrite ?(proj2 (X.API.handle_eqb_spec _ _) eq_refl);
        repeat match goal with
        | Leaf : X.reflexive_eval ?input = true |- context [X.reflexive_eval ?input] => rewrite Leaf
        end;
        cbn -[X.API.handle_eqb];
        rewrite ?(proj2 (X.API.handle_eqb_spec _ _) eq_refl);
        reflexivity.

      Ltac validate_finish code :=
        let claim_kind := eval hnf in (Claims.Code.statement_kind code) in
        lazymatch claim_kind with
        | Some ?claim_kind =>
          eapply checked_result; [apply rt_refl|];
          exists code, claim_kind; split; [reflexivity|]; split;
            [first [assumption | idtac]; try match goal with Shape : witnesses = ?values |- _ => rewrite Shape end;
              cbn [List.app] in *; assumption|];
          split; [validate_logic|first [reflexivity|left; reflexivity]]
        end.

      Ltac validate_posture store :=
        let Functions := fresh "Functions" in
        match goal with
        | Prepared : prepared store ?used |- _ =>
          pose proof (prepared_functions store used Prepared) as Functions
        end.

      Ltac validate_fixed_arity coupons expected Length :=
        match goal with
        | Fields : get_tree_raw coupons = witnesses |- _ => rewrite Fields in Length
        end;
        let size := eval vm_compute in expected in
        let inputs := fresh "inputs" in let Shape := fresh "Shape" in
        let LengthCopy := fresh "Length" in
        pose proof Length as LengthCopy;
        remember witnesses as inputs eqn:Shape in LengthCopy;
        let rec peel amount :=
          lazymatch amount with
          | O => destruct inputs as [|? inputs]; [|discriminate]
          | S ?remaining => destruct inputs as [|? inputs]; [discriminate|peel remaining]
          end in
        peel size; symmetry in Shape;
        match goal with Fields : get_tree_raw coupons = witnesses |- _ => rewrite Shape in Fields end.

      Ltac validate_premise state store locals code witness lhs rhs :=
        let claim_kind := eval hnf in (Claims.Code.statement_kind code) in
        lazymatch claim_kind with
        | Some ?claim_kind =>
          match goal with
          | Prepared : prepared store ?used |- _ =>
            let values := match goal with Shape : witnesses = ?values |- _ => constr:(values) end in
            let rest := eval cbn [List.skipn List.length List.app] in (skipn (S (List.length used)) values) in
            let Order := fresh "Order" in
            assert (Order : used ++ witness :: rest = witnesses) by
              (match goal with Shape : witnesses = _ |- _ => rewrite Shape; reflexivity end);
            checked_use (checked_premise state store locals used witness rest code claim_kind lhs rhs
              Order Prepared eq_refl);
            let state' := fresh "state" in let store' := fresh "store" in
            let frame' := fresh "frame" in let instructions := fresh "instructions" in
            let Code := fresh "Code" in let Frame := fresh "Frame" in
            let Prepared' := fresh "Prepared" in let Read := fresh "Read" in
            intros [[[state' store'] frame'] instructions] [Code [Frame [Prepared' Read]]];
            subst frame' instructions
          end
        end.

      Ltac validate_import state store locals stack index rest :=
        validate_posture store;
        let count := eval cbn [List.length] in (List.length stack) in
        let split n := eval cbn [firstn List.length List.app Nat.sub] in (firstn (count - n) stack) in
        let tail n := eval cbn [skipn List.length List.app Nat.sub] in (skipn (count - n) stack) in
        lazymatch goal with
        | Functions : s_funcs store = s_funcs G.E.allocated_store |- _ =>
          let endpoint left :=
            let operands := tail 1 in let prefix := split 1 in
            lazymatch operands with
            | [VAL_ref (VAL_ref_extern (to_externref ?witness))] =>
              checked_use_under prefix (checked_endpoint state store locals witness left Functions);
              let state' := fresh "state" in let store' := fresh "store" in let frame' := fresh "frame" in
              let instructions := fresh "instructions" in let Store := fresh "Store" in
              let Frame := fresh "Frame" in let field := fresh "field" in let Code := fresh "Code" in
              intros [[[state' store'] frame'] instructions] [Store [Frame [field Code]]];
              subst store' frame' instructions
            end in
          lazymatch index with
          | 22%N =>
            let operands := tail 2 in let prefix := split 2 in
            lazymatch operands with
            | [VAL_ref (VAL_ref_extern (to_externref ?lhs)); VAL_ref (VAL_ref_extern (to_externref ?rhs))] =>
              checked_use_under prefix (checked_same state store locals lhs rhs Functions);
              let state' := fresh "state" in let store' := fresh "store" in let frame' := fresh "frame" in
              let instructions := fresh "instructions" in let Store := fresh "Store" in
              let Frame := fresh "Frame" in let Code := fresh "Code" in let Same := fresh "Same" in
              intros [[[state' store'] frame'] instructions] [Store [Frame [Code Same]]];
              subst store' frame' instructions; try subst lhs; try subst rhs
            end
          | 30%N =>
            let operands := tail 4 in
            lazymatch operands with
            | [VAL_num (VAL_int64 ?bits); VAL_ref (VAL_ref_extern (to_externref (supplied_claim ?witness)));
                VAL_ref (VAL_ref_extern (to_externref ?lhs)); VAL_ref (VAL_ref_extern (to_externref ?rhs))] =>
              let code := eval vm_compute in (Wasm_int.nat_of_uint i64m bits) in
              validate_premise state store locals code witness lhs rhs
            end
          | 31%N => endpoint constr:(true)
          | 32%N => endpoint constr:(false)
          | 34%N =>
            let operands := tail 2 in let prefix := split 2 in
            lazymatch operands with
            | [VAL_num (VAL_int32 ?bits); VAL_ref (VAL_ref_extern (to_externref ?input))] =>
              let number := eval vm_compute in (Wasm_int.nat_of_uint i32m bits) in
              let kind := eval cbn in (nth number [X.Application; X.Selection; X.Identification; X.Strict;
                X.Shallow; X.BlobReference; X.TreeReference] X.Application) in
              checked_use_under prefix (checked_map state store locals kind input Functions);
              let state' := fresh "state" in let store' := fresh "store" in let frame' := fresh "frame" in
              let instructions := fresh "instructions" in let State := fresh "State" in let Store := fresh "Store" in
              let Frame := fresh "Frame" in let result := fresh "result" in
              let Construct := fresh "Construct" in let Code := fresh "Code" in
              intros [[[state' store'] frame'] instructions] [State [Store [Frame [result [Construct Code]]]]];
              subst state' store' frame' instructions
            end
          | 35%N =>
            let operands := tail 4 in
            lazymatch operands with
            | [VAL_num (VAL_int32 ?bits); VAL_ref (VAL_ref_extern (to_externref ?lhs));
                VAL_ref (VAL_ref_extern (to_externref ?rhs)); VAL_ref (VAL_ref_extern (to_externref (supplied_claim ?witness)))] =>
              let number := eval vm_compute in (Wasm_int.nat_of_uint i32m bits) in
              let kind := eval cbn in (nth number [X.Application; X.Selection; X.Identification; X.Strict;
                X.Shallow; X.BlobReference; X.TreeReference] X.Application) in
              match goal with
              | Prepared : prepared store ?used |- _ =>
                let values := match goal with Shape : witnesses = ?values |- _ => constr:(values) end in
                let rest := eval cbn [List.skipn List.length List.app] in (skipn (S (List.length used)) values) in
                checked_use (checked_mapped state store locals kind lhs rhs witness used rest
                  ltac:(match goal with Shape : witnesses = _ |- _ => rewrite Shape; reflexivity end) Prepared);
                let state' := fresh "state" in let store' := fresh "store" in let frame' := fresh "frame" in
                let instructions := fresh "instructions" in let Frame := fresh "Frame" in let Code := fresh "Code" in
                let Prepared' := fresh "Prepared" in let Checked := fresh "Checked" in
                intros [[[state' store'] frame'] instructions] [Code [Frame [Prepared' Checked]]];
                subst frame' instructions
              end
            end
          | 7%N =>
            let operands := tail 2 in let prefix := split 2 in
            lazymatch operands with
            | [VAL_ref (VAL_ref_extern (to_externref (treeh ?coupons))); VAL_num (VAL_int32 ?bits)] =>
              let number := eval vm_compute in (Wasm_int.nat_of_uint i32m bits) in
              match goal with
              | Fields : get_tree_raw coupons = ?values |- _ =>
                let field := eval cbn [nth_error] in (nth_error values number) in
                lazymatch field with
                | Some ?field => checked_advance_under prefix (child_run state store locals coupons number field
                    Functions ltac:(vm_compute; reflexivity) ltac:(rewrite Fields; reflexivity))
                end
              end
            end
          | 25%N =>
            let operands := tail 2 in
            lazymatch operands with
            | [VAL_ref (VAL_ref_extern (to_externref (treeh ?coupons))); VAL_num (VAL_int32 ?bits)] =>
              let expected := eval vm_compute in (Wasm_int.nat_of_uint i32m bits) in
              checked_use (checked_arity state store locals (treeh coupons) expected Functions
                ltac:(assumption) ltac:(vm_compute; reflexivity));
              let state' := fresh "state" in let store' := fresh "store" in let frame' := fresh "frame" in
              let instructions := fresh "instructions" in let State := fresh "State" in let Store := fresh "Store" in
              let Frame := fresh "Frame" in let Code := fresh "Code" in let actual := fresh "actual" in
              let Actual := fresh "Actual" in let Length := fresh "Length" in
              intros [[[state' store'] frame'] instructions] [State [Store [Frame [Code [actual [Actual Length]]]]]];
              subst state' store' frame' instructions; inversion Actual; subst actual;
              validate_fixed_arity coupons expected Length
            end
          | 33%N =>
            let operands := tail 1 in let prefix := split 1 in
            lazymatch operands with
            | [VAL_ref (VAL_ref_extern (to_externref ?input))] =>
              checked_use_under prefix (checked_data state store locals input Functions);
              let state' := fresh "state" in let store' := fresh "store" in let frame' := fresh "frame" in
              let instructions := fresh "instructions" in let State := fresh "State" in let Store := fresh "Store" in
              let Frame := fresh "Frame" in let Code := fresh "Code" in let data := fresh "data" in
              let Data := fresh "Data" in
              intros [[[state' store'] frame'] instructions] [State [Store [Frame [Code [data Data]]]]];
              subst state' store' frame' instructions input
            end
          | 27%N =>
            let operands := tail 1 in let prefix := split 1 in
            lazymatch operands with
            | [VAL_ref (VAL_ref_extern (to_externref ?input))] =>
              checked_use_under prefix (checked_lower state store locals input Functions);
              let state' := fresh "state" in let store' := fresh "store" in let frame' := fresh "frame" in
              let instructions := fresh "instructions" in let State := fresh "State" in let Store := fresh "Store" in
              let Frame := fresh "Frame" in let Code := fresh "Code" in let data := fresh "data" in
              let Data := fresh "Data" in
              intros [[[state' store'] frame'] instructions] [State [Store [Frame [Code [data Data]]]]];
              subst state' store' frame' instructions input
            end
          | 0%N =>
            lazymatch constr:((stack, rest)) with
            | ([VAL_ref (VAL_ref_extern (to_externref ?input))],
                AI_basic (BI_const_num (VAL_int32 ?bits)) :: AI_basic (BI_relop T_i32 (Relop_i ROI_eq)) ::
                  AI_basic (BI_call 21%N) :: _) =>
              let expected := eval vm_compute in (Wasm_int.nat_of_uint i32m bits) in
              checked_use (checked_type_guard state store locals input expected Functions ltac:(lia));
              let state' := fresh "state" in let store' := fresh "store" in let frame' := fresh "frame" in
              let instructions := fresh "instructions" in let State := fresh "State" in let Store := fresh "Store" in
              let Frame := fresh "Frame" in let Code := fresh "Code" in let KindTest := fresh "KindTest" in
              intros [[[state' store'] frame'] instructions] [State [Store [Frame [Code KindTest]]]];
              subst state' store' frame' instructions;
              destruct input as [[[blob|tree]|reference]|thunk|encoded]; cbn [G.E.H.W.H.value_type] in KindTest;
                try discriminate
            end
          | 16%N =>
            lazymatch stack with
            | [VAL_ref (VAL_ref_extern (to_externref ?lhs));
                VAL_ref (VAL_ref_extern (to_externref (Data (Ref ?reference))))] =>
              checked_advance_under [ext lhs] (native_value_import state store locals 16%N shell_identification
                [ext (Data (Ref reference))] [ext (Thunk (G.E.H.W.H.Identification reference))]
                Functions eq_refl eq_refl eq_refl
                ltac:(unfold G.E.H.host_values, ext; rewrite to_handle_to_externref; reflexivity))
            end
          end
        end.

      Ltac validate_retained state store locals stack :=
        let count := eval cbn [List.length] in (List.length stack) in
        let prefix := eval cbn [firstn List.length List.app Nat.sub] in (firstn (count - 1) stack) in
        let operands := eval cbn [skipn List.length List.app Nat.sub] in (skipn (count - 1) stack) in
        lazymatch operands with
        | [VAL_num (VAL_int32 ?bits)] =>
          let index := eval vm_compute in (Wasm_int.nat_of_uint i32m bits) in
          match goal with
          | Prepared : prepared store ?used, Shape : witnesses = ?values |- _ =>
            let entry := eval cbn [nth_error] in (nth_error values index) in
            lazymatch entry with
            | Some ?witness =>
              checked_advance_under prefix (retained_claim_run state store locals witnesses index witness
                ltac:(vm_compute; reflexivity) (prepared_retained _ _ Prepared)
                ltac:(rewrite Shape; reflexivity))
            end
          end
        end.

      Ltac validate_scan state store locals stack instructions :=
        lazymatch instructions with
        | ?head :: ?tail =>
          let converted := eval cbn [e_to_v_opt e_to_vref_opt option_map v_to_e vref_to_e word
            G.E.H.W.H.value_type G.E.H.W.extern_value] in (e_to_v_opt head) in
          lazymatch converted with
          | Some ?value =>
            let stack := eval cbn [List.app] in (stack ++ [value]) in
            validate_scan state store locals stack tail
          | None => lazymatch head with
            | AI_basic (BI_call ?index) => validate_import state store locals stack index tail
            | AI_basic (BI_table_get 9%N) => validate_retained state store locals stack
            | AI_basic BI_return =>
              lazymatch stack with
              | [VAL_num (VAL_int64 ?bits)] =>
                let code := eval vm_compute in (Wasm_int.nat_of_uint i64m bits) in validate_finish code
              end
            end
          end
        | [] => lazymatch stack with
          | [VAL_num (VAL_int64 ?bits)] =>
            let code := eval vm_compute in (Wasm_int.nat_of_uint i64m bits) in validate_finish code
          end
        end.

      Ltac validate_special :=
        lazymatch goal with
        | |- checked (?state, ?store, Build_frame ?locals G.E.collector_instance, ?instructions) _ =>
          let code := eval cbn [in_context lfill to_e_list v_to_e_list List.app seq.cat seq.map v_to_e vref_to_e] in instructions in
          validate_posture store;
          lazymatch goal with
          | Functions : s_funcs store = s_funcs G.E.allocated_store |- _ =>
            first [
              lazymatch code with
              | AI_basic (BI_local_get 1%N) :: AI_basic (BI_call 0%N) ::
                  AI_basic (BI_const_num (VAL_int32 ?bits)) ::
                  AI_basic (BI_relop T_i32 (Relop_i ROI_ne)) :: _ =>
                let found := local_value locals 1%N in lazymatch found with
                | Some (VAL_ref (VAL_ref_extern (to_externref ?input))) =>
                  checked_use (checked_leaf_guard state store locals input Functions eq_refl);
                  let state' := fresh "state" in let store' := fresh "store" in let frame' := fresh "frame" in
                  let instructions := fresh "instructions" in let State := fresh "State" in let Store := fresh "Store" in
                  let Frame := fresh "Frame" in let Code := fresh "Code" in let Leaf := fresh "Leaf" in
                  intros [[[state' store'] frame'] instructions] [State [Store [Frame [Code Leaf]]]];
                  subst state' store' frame' instructions
                end
              end
            | lazymatch code with
              | AI_basic (BI_local_get 7%N) :: AI_basic (BI_call 33%N) ::
                  AI_basic (BI_local_get 7%N) :: AI_basic (BI_call 0%N) :: _ =>
                let input_value := local_value locals 7%N in
                let result_value := local_value locals 2%N in
                lazymatch constr:((input_value, result_value)) with
                | (Some (VAL_ref (VAL_ref_extern (to_externref ?input))), Some (VAL_ref (VAL_ref_extern (to_externref ?result)))) =>
                  checked_use (checked_strict_result state store locals input result Functions eq_refl eq_refl);
                  let state' := fresh "state" in let store' := fresh "store" in let frame' := fresh "frame" in
                  let instructions := fresh "instructions" in let Store := fresh "Store" in
                  let Frame := fresh "Frame" in let Code := fresh "Code" in
                  let data := fresh "data" in let Input := fresh "Input" in let Result := fresh "Result" in
                  intros [[[state' store'] frame'] instructions] [Store [Frame [Code [data [Input Result]]]]];
                  subst store' frame' instructions input result
                end
              end
            | lazymatch code with
              | AI_label 0 [] ?body :: ?suffix =>
                let body := eval cbn [to_e_list seq.map] in body in
                lazymatch body with
                | [AI_basic (BI_local_get 3%N); AI_basic (BI_const_num (VAL_int32 ?bits)); AI_basic (BI_call 25%N)] =>
                  let expected := eval vm_compute in (Wasm_int.nat_of_uint i32m bits) in
                  let found := local_value locals 3%N in lazymatch found with
                  | Some (VAL_ref (VAL_ref_extern (to_externref (treeh ?coupons)))) =>
                    checked_use (checked_arity_block state store locals coupons expected Functions
                      ltac:(assumption) ltac:(vm_compute; reflexivity) eq_refl);
                    let state' := fresh "state" in let store' := fresh "store" in let frame' := fresh "frame" in
                    let instructions := fresh "instructions" in let State := fresh "State" in let Store := fresh "Store" in
                    let Frame := fresh "Frame" in let Code := fresh "Code" in let Length := fresh "Length" in
                    intros [[[state' store'] frame'] instructions] [State [Store [Frame [Code Length]]]];
                    subst state' store' frame' instructions; validate_fixed_arity coupons expected Length
                  end
                end
              end ]
          end
        end.

      Ltac validate_tree state store locals stack :=
        lazymatch stack with
        | [VAL_num (VAL_int64 ?bits); VAL_ref (VAL_ref_extern (to_externref ?lhs));
            VAL_ref (VAL_ref_extern (to_externref ?rhs));
            VAL_ref (VAL_ref_extern (to_externref (treeh ?coupons)))] =>
          let kind := eval vm_compute in (Wasm_int.nat_of_uint i64m bits) in
          let claim_kind := eval hnf in (Claims.Code.statement_kind kind) in
          lazymatch claim_kind with
          | Some ?claim_kind =>
            match goal with
            | Prepared : prepared store [] |- _ =>
              checked_use (checked_tree_rule state store locals kind claim_kind lhs rhs coupons eq_refl Prepared
                ltac:(assumption) ltac:(assumption) ltac:(assumption) ltac:(assumption) ltac:(assumption));
              let state' := fresh "state" in let store' := fresh "store" in let frame' := fresh "frame" in
              let instructions := fresh "instructions" in let Code := fresh "Code" in let Frame := fresh "Frame" in
              let Prepared' := fresh "Prepared" in let Checked := fresh "Checked" in
              intros [[[state' store'] frame'] instructions] [Code [Frame [Prepared' Checked]]];
              subst frame' instructions
            end
          end
        end.

      Ltac validate_tree_scan state store locals stack instructions :=
        lazymatch instructions with
        | ?head :: ?tail =>
          let converted := eval cbn [e_to_v_opt e_to_vref_opt option_map v_to_e vref_to_e word
            G.E.H.W.H.value_type G.E.H.W.extern_value] in (e_to_v_opt head) in
          lazymatch converted with
          | Some ?value =>
            let stack := eval cbn [List.app] in (stack ++ [value]) in
            validate_tree_scan state store locals stack tail
          | None => lazymatch head with
            | AI_basic (BI_call 36%N) => validate_tree state store locals stack
            end
          end
        end.

      Ltac validate :=
        normalize_checked;
        first [
          lazymatch goal with
          | |- checked (?state, ?store, ?frame, AI_trap :: ?suffix) _ =>
              eapply checked_trap, G.E.collector_trap_suffix with (instructions := [AI_trap]); apply rt_refl
          end
        | progress validate_special; validate
        | checked_pure; validate
        | lazymatch goal with
          | |- checked (?state, ?store, Build_frame ?locals G.E.collector_instance, ?instructions) _ =>
            let instructions := eval cbn [in_context lfill to_e_list v_to_e_list List.app seq.cat seq.map v_to_e vref_to_e
              G.E.H.W.H.HTreeObj G.E.H.W.H.HBlobObj] in instructions in
            first [validate_scan state store locals (@nil value) instructions; try validate
              |validate_tree_scan state store locals (@nil value) instructions; validate]
          end
        | lazymatch goal with
          | |- checked (?state, ?store, ?frame, AI_label 0 [] ?body :: ?suffix)
              (validated ?rule ?lhs ?rhs ?finish) =>
            eapply checked_weaken with
              (before := validated rule lhs rhs (fun code instructions =>
                exists depth (context : lholed depth), instructions = lfill context
                  (v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 code))] ++ [AI_basic BI_return])));
            [apply checked_return_label; validate|];
            let state' := fresh "state" in let store' := fresh "store" in let frame' := fresh "frame" in
            let instructions := fresh "instructions" in let code := fresh "code" in let claim_kind := fresh "claim_kind" in
            let Kind := fresh "Kind" in let Prepared := fresh "Prepared" in let Checked := fresh "Checked" in
            let Return := fresh "Return" in
            intros [[[state' store'] frame'] instructions] [code [claim_kind [Kind [Prepared [Checked Return]]]]];
            exists code, claim_kind; split; [exact Kind|]; split; [exact Prepared|]; split; [exact Checked|];
            first [right; exact Return|exact Return]
          end
        ].

      (** The native exchange dispatcher either traps or returns a claim kind accepted by
          the abstract checker with a prepared output store. *)
      Theorem collector_dispatch_validation state store caller rule_code rule lhs rhs coupons :
        X.decode_exchange rule_code = Some rule -> get_tree_raw coupons = witnesses ->
        prepared store [] -> fresh_tree_views store -> lengths_bounded 1 lhs -> lengths_bounded 1 rhs ->
        lengths_bounded 1 (treeh coupons) ->
        checked
          (state, store, Build_frame caller G.E.collector_instance,
            v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 rule_code)); ext lhs; ext rhs; ext (treeh coupons)] ++
              [AI_basic (BI_call 37%N)])
          (fun final => let '(_, store', frame', instructions) := final in
            frame' = Build_frame caller G.E.collector_instance /\ exists code claim_kind,
              instructions = v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 code))] /\
              Claims.Code.statement_kind code = Some claim_kind /\ prepared store' witnesses /\
              X.check rule witnesses lhs rhs = Some claim_kind).
      Proof.
        intros Decoded Fields Prepared Fresh LeftBound RightBound CouponBound.
        pose proof (prepared_functions _ _ Prepared) as Functions.
        set (empty := VAL_ref (VAL_ref_null T_externref)).
        set (arguments := [VAL_num (VAL_int64 (wasm_nat64 rule_code)); ext lhs; ext rhs; ext (treeh coupons)]).
        eapply checked_call with (code := native_exchange)
          (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
          (results := [T_num T_i64]) (address := 37%N) (defaults := [empty; empty; empty; empty])
          (arguments := arguments) (dummy := [VAL_num (VAL_int64 (wasm_nat64 0))]); try reflexivity.
        - rewrite Functions; reflexivity.
        - eapply checked_weaken with
            (before := validated rule lhs rhs (fun code instructions =>
              instructions = v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 code))] \/
              exists depth (context : lholed depth), instructions = lfill context
                (v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 code))] ++ [AI_basic BI_return]))).
          + unfold native_exchange at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
            do 27 (destruct rule_code as [|rule_code];
              [cbn [X.decode_exchange] in Decoded; inversion Decoded; subst rule; clear Decoded;
                solve [validate]|]); discriminate.
          + intros [[[state' store'] frame'] instructions] [code [claim_kind [Kind [Prepared' [Checked Return]]]]].
            exists [VAL_num (VAL_int64 (wasm_nat64 code))].
            split; [exact Return|]; split; [reflexivity|].
            split; [reflexivity|]; exists code, claim_kind; repeat first [assumption | reflexivity | split].
      Qed.

    Ltac unknown_scan compared state store locals stack instructions :=
      lazymatch instructions with
      | ?head :: ?tail =>
        let converted := eval cbn [e_to_v_opt e_to_vref_opt option_map v_to_e vref_to_e word G.E.H.W.H.value_type G.E.H.W.extern_value] in (e_to_v_opt head) in
        lazymatch converted with
        | Some ?value =>
          let stack := eval cbn [List.app] in (stack ++ [value]) in
          unknown_scan compared state store locals stack tail
        | None => lazymatch head with
          | AI_basic (BI_relop T_i64 (Relop_i ROI_eq)) =>
            let prefix := eval cbv beta iota zeta delta [firstn List.length List.app Nat.sub] in (firstn (List.length stack - 2) stack) in
            let pair := eval cbv beta iota zeta delta [skipn List.length List.app Nat.sub] in (skipn (List.length stack - 2) stack) in
            lazymatch pair with
            | [VAL_num ?lhs; VAL_num (VAL_int64 ?rhs)] =>
              let code := eval vm_compute in (Wasm_int.nat_of_uint i64m rhs) in
              let decoded := eval cbv in (X.decode_exchange code) in
              lazymatch decoded with Some ?exchange =>
                let Run := fresh "Run" in
                pose proof (native_relop state store locals lhs (VAL_int64 rhs) T_i64 (Relop_i ROI_eq) eq_refl) as Run;
                replace rhs with (wasm_nat64 code) in Run by reflexivity;
                rewrite (compared code exchange eq_refl) in Run;
                execute_under prefix Run; clear Run
              end
            end
          | AI_basic ?instruction => native_pure_at state store locals stack instruction
          end
        end
      end.
    Ltac unknown_pure compared :=
      lazymatch goal with
      | |- ?relation (?state, ?store, Build_frame ?locals G.E.collector_instance, ?instructions) _ =>
        let instructions := eval cbn [in_context lfill to_e_list v_to_e_list List.app seq.cat seq.map v_to_e vref_to_e G.E.H.W.H.HTreeObj G.E.H.W.H.HBlobObj] in instructions in
        unknown_scan compared state store locals (@nil value) instructions
      end.

      Ltac validate_unknown compared :=
        normalize_checked;
        first [
          lazymatch goal with
          | |- checked (_, _, _, AI_trap :: _) _ =>
            eapply checked_trap, G.E.collector_trap_suffix with (instructions := [AI_trap]); apply rt_refl
          end
        | eapply checked_progress; [unknown_pure compared; apply rt_refl|]; validate_unknown compared
        | progress validate_special; validate_unknown compared
        | lazymatch goal with
          | |- checked (?state, ?store, Build_frame ?locals G.E.collector_instance, ?instructions) _ =>
            let instructions := eval cbn [in_context lfill to_e_list v_to_e_list List.app seq.cat seq.map
              v_to_e vref_to_e G.E.H.W.H.HTreeObj G.E.H.W.H.HBlobObj] in instructions in
            validate_scan state store locals (@nil value) instructions; validate_unknown compared
          end ].

      Theorem collector_family_validation state store caller code selected_code rule lhs rhs coupons
          (Resolve : forall state store locals,
            s_funcs store = s_funcs G.E.allocated_store ->
            checked (state, store, Build_frame locals G.E.collector_instance,
              v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 code)); ext lhs] ++ [AI_basic (BI_call 43%N)])
              (fun final => final = (state, store, Build_frame locals G.E.collector_instance,
                v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 selected_code))]))) :
        X.decode_exchange selected_code = Some rule -> get_tree_raw coupons = witnesses ->
        prepared store [] -> fresh_tree_views store -> lengths_bounded 1 lhs -> lengths_bounded 1 rhs ->
        lengths_bounded 1 (treeh coupons) ->
        checked
          (state, store, Build_frame caller G.E.collector_instance,
            v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 code)); ext lhs; ext rhs; ext (treeh coupons)] ++
              [AI_basic (BI_call 44%N)])
          (fun final => let '(_, store', frame', instructions) := final in
            frame' = Build_frame caller G.E.collector_instance /\ exists result_code claim_kind,
              instructions = v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 result_code))] /\
              Claims.Code.statement_kind result_code = Some claim_kind /\ prepared store' witnesses /\
              X.check rule witnesses lhs rhs = Some claim_kind).
      Proof.
        intros Decoded Coupons Prepared Fresh LeftBound RightBound CouponsBound.
        pose proof (prepared_functions _ _ Prepared) as Functions.
        set (arguments := [VAL_num (VAL_int64 (wasm_nat64 code)); ext lhs; ext rhs; ext (treeh coupons)]).
        eapply checked_call with (code := native_family)
          (parameters := [T_num T_i64; T_ref T_externref; T_ref T_externref; T_ref T_externref])
          (results := [T_num T_i64]) (address := 44%N) (defaults := [])
          (arguments := arguments) (dummy := [VAL_num (VAL_int64 (wasm_nat64 0))]); try reflexivity.
        - rewrite Functions; reflexivity.
        - unfold native_family at 1; cbn [modfunc_body mod_funcs evaluation_module nth].
          do 2 checked_pure.
          checked_use (Resolve state store arguments Functions).
          intros [[[state' store'] frame'] instructions] Equal; inversion Equal; subst state' store' frame' instructions.
          do 3 checked_pure.
          checked_use (collector_dispatch_validation state store arguments selected_code rule lhs rhs coupons
            Decoded Coupons Prepared Fresh LeftBound RightBound CouponsBound).
          intros [[[state' store'] frame'] instructions]
            [Frame [result_code [claim_kind [Code [Kind [Prepared' Checked]]]]]];
            subst frame' instructions.
          eapply checked_result; [apply rt_refl|].
          exists [VAL_num (VAL_int64 (wasm_nat64 result_code))]; split; [left; reflexivity|].
          split; [reflexivity|]; split; [reflexivity|].
          exists result_code, claim_kind; auto.
      Qed.

      Lemma prepared_after_read store arguments next result used :
        prepared store used ->
        G.E.H.blob_read_result G.E.collector_instance store arguments = Some (next, result) ->
        prepared next used.
      Proof.
        intros Before Read; constructor.
        - eapply resources_after_read; [exact (prepared_resources _ _ Before)|exact Read].
        - eapply blob_read_retained_claims; [exact Read|exact (prepared_retained _ _ Before)].
      Qed.

      Lemma fresh_after_read store arguments next result :
        fresh_tree_views store ->
        G.E.H.blob_read_result G.E.collector_instance store arguments = Some (next, result) ->
        fresh_tree_views next.
      Proof.
        intros [Left Right] Read; split;
          eapply blob_read_capacity; [exact Read|exact Left|exact Read|exact Right].
      Qed.

      (** Replacing the store's tables by the same tables leaves the store unchanged. *)
      Lemma table_update_identity store : upd_s_table store (s_tables store) = store.
      Proof. destruct store; reflexivity. Qed.

      Section CertifiedExecution.
        Variable output_type assertion_type : table_type.
        (** The output table stores externrefs. *)
        Hypothesis output_element : tt_elem_type output_type = T_externref.
        (** The assertion table stores externrefs. *)
        Hypothesis assertion_element : tt_elem_type assertion_type = T_externref.
        (** The remaining two tables provide exactly the initialized output and assertion
            scratch slots. *)
        Hypothesis scratch_tables : other_tables =
          [Build_tableinst output_type (repeat (VAL_ref_null T_externref) 2);
           Build_tableinst assertion_type (repeat (VAL_ref_null T_externref) 3)].

        (** Under the host and resource contracts, the collector entry point traps or
            returns exactly a certificate accepted and issued by the abstract collector. *)
        Theorem collector_refinement state initial_store caller opcode code selected_code rule metadata callable lhs rhs coupons issuing_author
            (Resolve : forall state store locals,
              s_funcs store = s_funcs G.E.allocated_store ->
              checked (state, store, Build_frame locals G.E.collector_instance,
                v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 code)); ext lhs] ++ [AI_basic (BI_call 43%N)])
                (fun final => final = (state, store, Build_frame locals G.E.collector_instance,
                  v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 selected_code))]))) :
          get_tree_raw combination = [metadata; callable; blobh opcode; lhs; rhs; treeh coupons] ->
          decode_u64 (get_blob_data opcode) = Some code -> X.decode_exchange selected_code = Some rule ->
          get_tree_raw coupons = witnesses -> prepared initial_store [] -> fresh_tree_views initial_store ->
          lengths_bounded 1 lhs -> lengths_bounded 1 rhs -> lengths_bounded 1 (treeh coupons) ->
          G.E.H.Tables.T.A.author_at 1 (treeh combination) = Some issuing_author ->
          exists state' store' instructions, reduce_trans
            (state, initial_store, Build_frame caller G.E.collector_instance,
              v_to_e_list [ext (treeh combination)] ++ [AI_basic (BI_call 38%N)])
            (state', store', Build_frame caller G.E.collector_instance, instructions) /\
            (instructions = [AI_trap] \/ exists claim_kind tag claim,
              instructions = v_to_e_list [ext (treeh tag)] /\
              X.check rule witnesses lhs rhs = Some claim_kind /\
              X.Wire.encode claim_kind issuing_author lhs rhs = Some (treeh (create_tree [treeh tag; treeh claim]))).
        Proof.
          intros Fields Code Rule Coupons Prepared Fresh LeftBound RightBound CouponBound Author.
          pose proof (prepared_functions _ _ Prepared) as Functions.
          pose proof (memory_capacity_integer_reads initial_store
            (prepared_typing _ _ Prepared) (prepared_memory _ _ Prepared)) as Reads.
          destruct (Reads opcode ltac:(eapply decode_u64_width; exact Code))
            as [copied [page [Read [Memory Loaded]]]].
          pose proof (prepared_after_read _ _ _ _ _ Prepared Read) as AfterOpcode.
          pose proof (fresh_after_read _ _ _ _ Fresh Read) as AfterFresh.
          set (empty := VAL_ref (VAL_ref_null T_externref)).
          set (selected := [ext (treeh combination); ext lhs; ext rhs; VAL_num (VAL_int64 (wasm_nat64 0))]).
          pose proof (collector_entry_header state initial_store copied opcode code page
            metadata callable lhs rhs coupons Functions (prepared_combination _ _ Prepared)
            (prepared_combination _ _ AfterOpcode) Read (prepared_functions _ _ AfterOpcode)
            Memory Loaded Code Fields) as Header.
          destruct (collector_family_validation state copied selected code selected_code rule
            lhs rhs coupons Resolve Rule Coupons AfterOpcode AfterFresh LeftBound RightBound CouponBound)
            as [[[[state' store'] frame'] instructions] [Dispatch [Trap|Accepted]]].
          - subst instructions.
            assert (NativeRejection : reduce_trans
                (state, initial_store, Build_frame caller G.E.collector_instance,
                  v_to_e_list [ext (treeh combination)] ++ [AI_basic (BI_call 38%N)])
                (state', store', Build_frame caller G.E.collector_instance, [AI_trap])).
            { eapply G.E.collector_call with (code := evaluation_apply)
              (parameters := [T_ref T_externref]) (results := [T_ref T_externref])
              (address := 38%N) (defaults := [empty; empty; VAL_num (VAL_int64 (wasm_nat64 0))])
              (arguments := [ext (treeh combination)]) (values := [ext lhs])
              (accepted := false) (final_frame := frame'); try reflexivity.
              - rewrite Functions; reflexivity.
              - rewrite <- (firstn_skipn 16 (modfunc_body evaluation_apply)) at 1.
                unfold to_e_list; rewrite List.map_app.
                execute Header.
                cbn [evaluation_apply modfunc_body mod_funcs evaluation_module nth firstn skipn List.length List.app seq.map].
                eapply G.E.collector_trap_suffix with
                  (instructions := v_to_e_list [VAL_num (VAL_int64 (wasm_nat64 code)); ext lhs; ext rhs; ext (treeh coupons)] ++
                    [AI_basic (BI_call 44%N)])
                  (suffix := to_e_list (skipn 17 (modfunc_body evaluation_apply))).
                exact Dispatch.
            }
            exists state', store', [AI_trap]; auto.
          - destruct Accepted as [Frame [result_code [claim_kind [Result [Kind [Prepared' Checked]]]]]];
              subst frame' instructions.
            pose proof (prepared_functions _ _ Prepared') as FinalFunctions.
            pose proof (prepared_tables _ _ Prepared') as LastTables.
            pose proof (prepared_combination _ _ Prepared') as LastCombination.
            set (claim := issued_claim lhs rhs result_code).
            set (tag := issued_tag issuing_author lhs rhs result_code).
            assert (CodeBound : (Z.of_nat result_code < 2 ^ 64)%Z).
            { pose proof (Claims.Code.statement_code_bound _ _ (Claims.Code.statement_kind_roundtrip _ _ Kind)); cbn; lia. }
            assert (StoreReady : coupon_output_store store' output_type assertion_type
                lhs rhs issuing_author result_code 0 0 = store').
            { unfold coupon_output_store, G.E.H.Tables.copied_entries.
              change (upd_s_table store'
                ([Build_tableinst output_type (repeat (VAL_ref_null T_externref) 2);
                  Build_tableinst assertion_type (repeat (VAL_ref_null T_externref) 3)] ++
                 skipn 2 (s_tables store')) = store').
              rewrite <- scratch_tables, <- LastTables, firstn_skipn; apply table_update_identity. }
            assert (Selected : nth_error (get_tree_raw combination) 1 = Some issuing_author).
            { change (match nth_error (get_tree_raw combination) 1 with
                | Some child => Some child | None => None end = Some issuing_author) in Author.
              destruct (nth_error (get_tree_raw combination) 1) as [author|] eqn:Entry;
                inversion Author; subst; reflexivity. }
            assert (AuthorView : stab_elem store' G.E.collector_instance 2%N 1%N =
                Some (G.E.H.Tables.externref issuing_author)).
            { exact (LastCombination 1 issuing_author Selected). }
            pose proof (collector_output_executes store' output_type assertion_type lhs rhs issuing_author
              result_code state' CodeBound FinalFunctions output_element assertion_element Author AuthorView) as Output.
            rewrite StoreReady in Output.
            set (output_store := coupon_output_store store' output_type assertion_type
              lhs rhs issuing_author result_code 3 2).
            assert (ClaimIssued : X.Wire.encode claim_kind issuing_author lhs rhs =
                Some (treeh (create_tree [treeh tag; treeh claim]))).
            { eapply collector_output_encodes; exact Kind. }
            assert (NativeExecution : reduce_trans
                (state, initial_store, Build_frame caller G.E.collector_instance,
                  v_to_e_list [ext (treeh combination)] ++ [AI_basic (BI_call 38%N)])
                (state', output_store, Build_frame caller G.E.collector_instance,
                  if is_comparable issuing_author then v_to_e_list [ext (treeh tag)] else [AI_trap])).
            { eapply G.E.collector_call with (code := evaluation_apply)
                (parameters := [T_ref T_externref]) (results := [T_ref T_externref])
                (address := 38%N) (defaults := [empty; empty; VAL_num (VAL_int64 (wasm_nat64 0))])
                (arguments := [ext (treeh combination)]) (values := [ext (treeh tag)])
                (accepted := is_comparable issuing_author)
                (final_frame := Build_frame [ext (treeh combination); ext lhs; ext rhs;
                  VAL_num (VAL_int64 (wasm_nat64 result_code))] G.E.collector_instance); try reflexivity.
              - rewrite Functions; reflexivity.
              - rewrite <- (firstn_skipn 16 (modfunc_body evaluation_apply)) at 1.
                unfold to_e_list; rewrite List.map_app.
                execute Header.
                cbn [evaluation_apply modfunc_body mod_funcs evaluation_module nth firstn skipn List.length List.app seq.map].
                execute Dispatch.
                cbn [evaluation_apply modfunc_body mod_funcs evaluation_module nth firstn skipn List.app List.length seq.map].
                native_pure.
                exact Output.
            }
            exists state', output_store,
              (if is_comparable issuing_author then v_to_e_list [ext (treeh tag)] else [AI_trap]).
            split; [exact NativeExecution|].
            destruct (is_comparable issuing_author); [right; exists claim_kind, tag, claim; auto|left; reflexivity].
        Qed.
      End CertifiedExecution.
    End Refinement.
  End Execution.
  Ltac normalize_checked := cbn [in_context lfill List.app seq.cat v_to_e_list seq.map].
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

End WasmRefinement.
