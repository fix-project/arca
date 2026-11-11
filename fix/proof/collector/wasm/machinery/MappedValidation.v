Module WasmMappedValidation.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Constructors := WasmConstructors.
  Import Constructors Constructors.V Constructors.V.V Constructors.V.V.Support.
  Module Premises := WasmPremise.
  Module Resources := WasmResources.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Local Notation treeh := G.E.H.H.HTreeObj.
  Local Notation mode := A.native_constructor_index.

  Lemma checked_mapped state store locals kind lhs_index rhs_index claim_index lhs rhs claim code_value a b :
    s_funcs store = s_funcs G.E.allocated_store -> store_typing store -> Resources.memory_capacity store ->
    G.E.H.resolve state lhs_index = Some lhs -> G.E.H.resolve state rhs_index = Some rhs ->
    G.E.H.resolve state claim_index = Some (treeh claim) ->
    get_tree_raw claim = [code_value; a; b] -> lengths_bounded 2 (treeh claim) ->
    (forall observation value, exists bytes, handle_bytes observation value = Some bytes) ->
    G.E.H.D.live_count (G.E.H.slots state) + 7 <= G.E.H.capacity ->
    checked (state, store, Build_frame locals G.E.collector_instance,
      v_to_e_list [word (mode kind); descriptor lhs_index; descriptor rhs_index; descriptor claim_index] ++
        [AI_basic (BI_call 36%N)])
      (fun final => exists next updated code,
        final = (next, updated, Build_frame locals G.E.collector_instance, []) /\
        get_tree_raw claim = [G.E.H.H.HBlobObj code; a; b] /\ decode_u64 (get_blob_data code) = Some 0 /\
        X.construct kind a = Some lhs /\ X.construct kind b = Some rhs /\ scope state next 7 /\
        s_funcs updated = s_funcs store /\ store_typing updated /\ Resources.memory_capacity updated).
  Proof.
    intros Functions Typed Capacity LhsLive RhsLive ClaimLive Fields Bounds Bytes Room.
    set (arguments := [word (mode kind); descriptor lhs_index; descriptor rhs_index; descriptor claim_index]).
    set (initial := arguments ++ [word 0; word 0]).
    destruct (A.exchange_endpoint_executes state store initial claim_index claim a true Functions ClaimLive
      ltac:(rewrite Fields; reflexivity) ltac:(lia))
      as [first [a_slot [ARun [ALive [AFresh [Scope1 Obs1]]]]]].
    unfold A.Support.word, A.Support.descriptor in ARun.
    pose proof (proj2 Scope1) as Count1.
    set (locals1 := arguments ++ [word a_slot; word 0]).
    assert (Claim1 : G.E.H.resolve first claim_index = Some (treeh claim)) by (apply (proj1 Scope1); exact ClaimLive).
    destruct (A.exchange_endpoint_executes first store locals1 claim_index claim b false Functions Claim1
      ltac:(rewrite Fields; reflexivity) ltac:(lia))
      as [second [b_slot [BRun [BLive [BFresh [Scope2 Obs2]]]]]].
    unfold A.Support.word, A.Support.descriptor in BRun.
    pose proof (proj2 Scope2) as Count2.
    set (locals2 := arguments ++ [word a_slot; word b_slot]).
    assert (A2 : G.E.H.resolve second (wasm_nat a_slot) = Some a) by (apply (proj1 Scope2); exact ALive).
    assert (Scope02 : scope state second 2).
    { replace 2 with (1+1) by lia; eapply scope_trans; eassumption. }
    eapply checked_call with (code := native_mapped)
      (parameters := [T_num T_i32; T_num T_i32; T_num T_i32; T_num T_i32]) (results := [])
      (defaults := [word 0; word 0]) (address := 36%N) (arguments := arguments) (dummy := []); try reflexivity.
    - rewrite Functions; reflexivity.
    - lazymatch goal with |- checked _ ?post =>
        change (checked (state, store, Build_frame initial G.E.collector_instance,
          to_e_list (modfunc_body native_mapped)) post) end.
      rewrite native_mapped_body.
      checked_pure; checked_advance ARun; checked_pure.
      checked_pure; checked_advance BRun; checked_pure.
      do 3 checked_pure.
      checked_use_under [descriptor lhs_index] (checked_map second store locals2 (wasm_nat a_slot) kind a Functions A2 ltac:(lia)).
      intros final [third [left_copy [mapped_left [Final [ConstructA [MapALive [MapAFresh [Scope3 Obs3]]]]]]]];
        subst final; normalize_checked.
      set (locals3 := locals2).
      assert (Scope03 : scope state third 3).
      { replace 3 with (2+1) by lia; eapply scope_trans; eassumption. }
      assert (Lhs3 : G.E.H.resolve third lhs_index = Some lhs) by (apply (proj1 Scope03); exact LhsLive).
      checked_use (checked_same third store locals3 lhs_index (wasm_nat left_copy) lhs mapped_left
        Functions Lhs3 MapALive Bytes).
      intros final [Final SameLeft]; subst final mapped_left; normalize_checked.
      assert (Scope3Compared : scope second (compared third) 1) by exact Scope3.
      pose (fourth := compared third).
      assert (Scope4 : scope second fourth 1) by exact Scope3.
      pose proof (proj2 Scope4) as Count4.
      assert (B4 : G.E.H.resolve fourth (wasm_nat b_slot) = Some b) by (apply (proj1 Scope4); exact BLive).
      do 3 checked_pure.
      checked_use_under [descriptor rhs_index] (checked_map fourth store locals3 (wasm_nat b_slot) kind b Functions B4 ltac:(lia)).
      intros final [fifth [right_copy [mapped_right [Final [ConstructB [MapBLive [MapBFresh [Scope5 Obs5]]]]]]]];
        subst final; normalize_checked.
      set (inside := locals2).
      assert (Scope25 : scope second fifth 2).
      { replace 2 with (1+1) by lia; eapply scope_trans; eassumption. }
      assert (Rhs5 : G.E.H.resolve fifth rhs_index = Some rhs).
      { apply (proj1 Scope25), (proj1 Scope02); exact RhsLive. }
      checked_use (checked_same fifth store inside rhs_index (wasm_nat right_copy) rhs mapped_right
        Functions Rhs5 MapBLive Bytes).
      intros final [Final SameRight]; subst final mapped_right; normalize_checked.
      assert (Scope5Compared : scope fourth (compared fifth) 1) by exact Scope5.
      pose (sixth := compared fifth).
      assert (Scope6 : scope fourth sixth 1) by exact Scope5.
      assert (Scope26 : scope second sixth 2).
      { replace 2 with (1+1) by lia; eapply scope_trans; eassumption. }
      assert (Scope06 : scope state sixth 4).
      { replace 4 with (2+2) by lia; eapply scope_trans; eassumption. }
      pose proof (proj2 Scope06) as Count6.
      assert (Claim6 : G.E.H.resolve sixth claim_index = Some (treeh claim)) by (apply (proj1 Scope06); exact ClaimLive).
      assert (A6 : G.E.H.resolve sixth (wasm_nat a_slot) = Some a) by (apply (proj1 Scope26); exact A2).
      assert (B6 : G.E.H.resolve sixth (wasm_nat b_slot) = Some b) by (apply (proj1 Scope26); exact BLive).
      do 3 checked_pure.
      pose proof (Premises.checked_premise sixth store inside claim_index (wasm_nat a_slot) (wasm_nat b_slot)
        claim 0 a b Functions Typed Capacity Claim6 A6 B6 ltac:(rewrite Fields; reflexivity)
        Bounds ltac:(vm_compute; congruence) Bytes ltac:(lia)) as Premise.
      unfold Premises.Integers.V.V.Support.word, Premises.Integers.V.V.Support.descriptor in Premise.
      lazymatch type of Premise with Premises.Integers.V.V.checked ?initial ?post =>
        change (checked initial post) in Premise end.
      checked_use Premise.
      intros final [seventh [updated [code [Final [ClaimFields [Decoded [Scope7 [Obs7 [Functions7 [Typed7 Capacity7]]]]]]]]]];
        subst final; normalize_checked.
      assert (FunctionsNext : s_funcs updated = s_funcs G.E.allocated_store) by (rewrite Functions7; exact Functions).
      assert (Scope27 : scope second seventh 5).
      { replace 5 with (2+3) by lia; eapply scope_trans; eassumption. }
      assert (Scope07 : scope state seventh 7).
      { replace 7 with (2+5) by lia; eapply scope_trans; eassumption. }
      eapply checked_result; [apply rt_refl|].
      exists []; split; [left; reflexivity|]; split; [reflexivity|].
      exists seventh, updated, code; split; [reflexivity|]; split; [exact ClaimFields|]; split; [exact Decoded|];
        split; [exact ConstructA|]; split; [exact ConstructB|]; split; [exact Scope07|]; auto.
  Qed.
End WasmMappedValidation.
