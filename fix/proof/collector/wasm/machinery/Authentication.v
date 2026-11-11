Module WasmAuthentication.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module V := WasmValidation.
  Import V V.V V.V.Support.
  Module Witnesses := WasmWitnesses.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Lemma pair_fields (values : list handle) : List.length values = 2 ->
    exists first second, values = [first; second].
  Proof.
    destruct values as [|first [|second [|third rest]]]; cbn; intro Length; try discriminate.
    exists first, second; reflexivity.
  Qed.

  Lemma checked_claim state store locals root witness :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state root = Some witness ->
    lengths_bounded 2 witness ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 4 <= G.E.H.capacity ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor root] ++ [AI_basic (BI_call 30%N)])
      (fun final => exists next output claim,
        final = (next, store, Build_frame locals G.E.collector_instance, v_to_e_list [word output]) /\
        Witnesses.authenticated_claim witness claim /\
        G.E.H.resolve next (wasm_nat output) = Some (treeh claim) /\
        G.E.H.resolve state (wasm_nat output) = None /\ scope state next 4 /\
        G.E.H.observations next = G.E.H.observations state + 2).
  Proof.
    intros Functions Root Bounds Bytes Room.
    set (initial := [descriptor root; word 0; word 0]).
    eapply checked_call with (code := evaluation_claim)
      (parameters := [T_num T_i32]) (results := [T_num T_i32]) (address := 30%N)
      (defaults := [word 0; word 0]) (arguments := [descriptor root]) (dummy := [word 0]); try reflexivity.
    - rewrite Functions; reflexivity.
    - lazymatch goal with |- checked _ ?post =>
        change (checked (state, store, Build_frame initial G.E.collector_instance,
          to_e_list (modfunc_body evaluation_claim)) post) end.
      rewrite evaluation_claim_body; checked_pure.
      checked_use (checked_arity state store initial root witness 2 Functions Root
        (lengths_bounded_weaken 1 witness Bounds) ltac:(vm_compute; congruence)).
      intros final [Final [bundle [Witness Length]]]; subst final witness; normalize_checked.
      destruct (pair_fields _ Length) as [coupon_value [claim_value Bundle]].
      destruct (child_run state store initial root bundle 0 coupon_value Functions Root
        ltac:(rewrite Bundle; reflexivity) ltac:(lia) ltac:(lia))
        as [first [coupon [CouponRun [CouponLive [CouponFresh [Scope1 Obs1]]]]]].
      checked_pure; checked_advance CouponRun; checked_pure.
      set (locals1 := [descriptor root; word coupon; word 0]).
      assert (Root1 : G.E.H.resolve first root = Some (treeh bundle)) by (apply (proj1 Scope1); exact Root).
      destruct (child_run first store locals1 root bundle 1 claim_value Functions Root1
        ltac:(rewrite Bundle; reflexivity) ltac:(lia) ltac:(destruct Scope1; lia))
        as [second [assertion [ClaimRun [ClaimLive [ClaimFresh [Scope2 Obs2]]]]]].
      checked_pure; checked_advance ClaimRun; checked_pure.
      set (locals2 := [descriptor root; word coupon; word assertion]).
      assert (Coupon2 : G.E.H.resolve second (wasm_nat coupon) = Some coupon_value)
        by (apply (proj1 Scope2); exact CouponLive).
      checked_pure.
      checked_advance (tag_run second store locals2 (wasm_nat coupon) coupon_value
        (G.E.H.Api.is_tag coupon_value) Functions Coupon2 eq_refl).
      destruct (G.E.H.Api.is_tag coupon_value) eqn:Tagged.
      2: { checked_pure; eapply checked_trap;
        eapply G.E.collector_trap_suffix with (instructions := [AI_trap]); apply rt_refl. }
      checked_pure.
      assert (CouponBound : lengths_bounded 1 coupon_value).
      { eapply lengths_bounded_child; [exact Bounds|rewrite Bundle; cbn; auto]. }
      checked_pure.
      checked_use (checked_arity second store locals2 (wasm_nat coupon) coupon_value 2
        Functions Coupon2 CouponBound ltac:(vm_compute; congruence)).
      intros final [Final [tag [Coupon TagLength]]]; subst final coupon_value; normalize_checked.
      assert (AssertionBound : lengths_bounded 1 claim_value).
      { eapply lengths_bounded_child; [exact Bounds|rewrite Bundle; cbn; auto]. }
      checked_pure.
      checked_use (checked_arity second store locals2 (wasm_nat assertion) claim_value 3
        Functions ClaimLive AssertionBound ltac:(vm_compute; congruence)).
      intros final [Final [claim [Claim ClaimLength]]]; subst final claim_value; normalize_checked.
      destruct (pair_fields _ TagLength) as [author [reference Tag]].
      destruct (child_run second store locals2 (wasm_nat coupon) tag 1 reference Functions Coupon2
        ltac:(rewrite Tag; reflexivity) ltac:(lia) ltac:(destruct Scope1; destruct Scope2; lia))
        as [third [reference_slot [RefRun [RefLive [RefFresh [Scope3 Obs3]]]]]].
      checked_pure; checked_advance RefRun.
      set (inside := locals2).
      assert (Claim3 : G.E.H.resolve third (wasm_nat assertion) = Some (treeh claim))
        by (apply (proj1 Scope3); exact ClaimLive).
      checked_pure.
      checked_use (checked_object third store inside (wasm_nat reference_slot) (wasm_nat assertion)
        reference (treeh claim) Functions RefLive Claim3 Bytes
        ltac:(destruct Scope1; destruct Scope2; destruct Scope3; lia)).
      intros final [fourth [object [Final [Scope4 [Obs4 [Object Reference]]]]]]; subst final.
      inversion Object; subst object; clear Object; normalize_checked.
      assert (Scoped : scope state fourth 4).
      { replace 4 with (1+(1+(1+1))) by lia.
        eapply scope_trans; [exact Scope1|]; eapply scope_trans; [exact Scope2|].
        eapply scope_trans; eassumption. }
      assert (ClaimNext : G.E.H.resolve fourth (wasm_nat assertion) = Some (treeh claim))
        by (apply (proj1 Scope4); exact Claim3).
      checked_pure.
      eapply checked_result; [apply rt_refl|].
      exists [word assertion]; split; [left; reflexivity|]; split; [reflexivity|].
      exists fourth, assertion, claim; split; [reflexivity|]; split.
      + exists bundle, tag, author; repeat split; try reflexivity; try exact Bundle; try exact ClaimLength.
        * rewrite Reference in Tag; exact Tag.
        * exact Tagged.
      + split; [exact ClaimNext|]; split.
        * eapply fresh_before; [exact (proj1 Scope1)|exact ClaimFresh].
        * split; [exact Scoped|]; rewrite Obs4, Obs3, Obs2, Obs1; reflexivity.
  Qed.

  Lemma checked_witness_claim state store locals root witnesses index witness :
    s_funcs store = s_funcs G.E.allocated_store -> G.E.H.resolve state root = Some (treeh witnesses) ->
    nth_error (get_tree_raw witnesses) index = Some witness -> (Z.of_nat index < 2 ^ 32)%Z ->
    lengths_bounded 2 witness ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 5 <= G.E.H.capacity ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list [descriptor root; word index] ++ [AI_basic (BI_call 46%N)])
      (fun final => exists next output claim,
        final = (next, store, Build_frame locals G.E.collector_instance, v_to_e_list [word output]) /\
        Witnesses.authenticated_claim witness claim /\
        G.E.H.resolve next (wasm_nat output) = Some (treeh claim) /\
        G.E.H.resolve state (wasm_nat output) = None /\ scope state next 5 /\
        G.E.H.observations next = G.E.H.observations state + 2).
  Proof.
    intros Functions Root Field Index Bounds Bytes Room.
    set (inside := [descriptor root; word index]).
    destruct (child_run state store inside root witnesses index witness Functions Root Field Index ltac:(lia))
      as [first [witness_slot [WitnessRun [WitnessLive [WitnessFresh [Scope1 Obs1]]]]]].
    eapply checked_call with (code := native_witness_claim)
      (parameters := [T_num T_i32; T_num T_i32]) (results := [T_num T_i32]) (address := 46%N)
      (defaults := []) (arguments := inside) (dummy := [word 0]); try reflexivity.
    - rewrite Functions; reflexivity.
    - lazymatch goal with |- checked _ ?post =>
        change (checked (state, store, Build_frame inside G.E.collector_instance,
          to_e_list (modfunc_body native_witness_claim)) post) end.
      rewrite native_witness_claim_body.
      do 2 checked_pure; checked_advance WitnessRun.
      checked_use (checked_claim first store inside (wasm_nat witness_slot) witness Functions WitnessLive Bounds Bytes
        ltac:(destruct Scope1; lia)).
      intros final [next [output [claim [Final [Authenticated [Live [Fresh [Scope2 Obs2]]]]]]]]; subst final.
      eapply checked_result; [apply rt_refl|].
      exists [word output]; split; [left; reflexivity|]; split; [reflexivity|].
      exists next, output, claim; split; [reflexivity|]; split; [exact Authenticated|]; split; [exact Live|]; split.
      + eapply fresh_before; [exact (proj1 Scope1)|exact Fresh].
      + split.
        * replace 5 with (1+4) by lia; eapply scope_trans; eassumption.
        * rewrite Obs2, Obs1; reflexivity.
  Qed.
End WasmAuthentication.
