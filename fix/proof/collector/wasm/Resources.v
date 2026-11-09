From FixProof.Collector.wasm Require Import Backend Support.
From Wasm Require Import properties type_preservation.
Import ListNotations.
Local Open Scope list_scope.

Module WasmResources (C : COLLECTOR_CONTEXT).
  Import C S R.
  Module Support := WasmSupport C.
  Import Support.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.

  Local Notation ext := G.E.H.W.extern_value.
  Local Notation treeh := G.E.H.W.H.HTreeObj.

  Lemma collector_table_address index :
    index < 10 ->
    lookup_N (inst_tables G.E.collector_instance) (N.of_nat index) = Some (N.of_nat index).
  Proof.
    intro Bound; rewrite (proj1 G.E.collector_resource_indices).
    unfold lookup_N; rewrite Nat2N.id.
    do 10 (destruct index as [|index]; [reflexivity|]); lia.
  Qed.

  Lemma tree_attachment_available store node index capacity :
    store_typing store -> view_capacity store (N.of_nat index) capacity ->
    capacity <= List.length (get_tree_raw node) ->
    (Z.of_nat index < 2 ^ 32)%Z ->
    (N.of_nat (List.length (get_tree_raw node)) <= native_table_limit)%N ->
    exists next, G.E.H.tree_attachment_result G.E.collector_instance store
      [ext (treeh node); word index] = Some (next, result_values []).
  Proof.
    intros Typed [table [Entry [Element [Maximum [Minimum Size]]]]] Capacity Bound Limit.
    unfold stab in Entry.
    destruct (lookup_N (inst_tables G.E.collector_instance) (N.of_nat index))
      as [address|] eqn:Address; [|discriminate].
    change (lookup_N (s_tables store) address = Some table) in Entry.
    eexists; eapply G.E.H.tree_attachment_bounded with (address := address) (before := table).
    - rewrite (wasm_nat_to_N index Bound); exact Address.
    - exact Entry.
    - exact Typed.
    - exact Element.
    - exact Maximum.
    - eapply N.le_trans; [exact Minimum|].
      lia.
    - unfold tab_size; lia.
    - exact Limit.
  Qed.

  Lemma tree_attachment_capacity store node index capacity next :
    (Z.of_nat index < 2 ^ 32)%Z ->
    G.E.H.tree_attachment_result G.E.collector_instance store
      [ext (treeh node); word index] = Some (next, result_values []) ->
    view_capacity store (N.of_nat index) capacity ->
    view_capacity next (N.of_nat index) (List.length (get_tree_raw node)).
  Proof.
    intros Bound Attached [table [Entry [Element [Maximum [Minimum Size]]]]].
    destruct (G.E.H.tree_attachment_result_shape _ _ _ _ _ Attached)
      as [address [before [Address [Before Next]]]].
    rewrite (wasm_nat_to_N index Bound) in Address.
    unfold stab in Entry; rewrite Address in Entry.
    change (lookup_N (s_tables store) address = Some table) in Entry.
    rewrite Before in Entry; inversion Entry; subst table next.
    exists (G.E.H.Tables.tree_attachment before (get_tree_raw node)); split.
    - unfold stab; rewrite Address.
      change (lookup_N
        (G.E.H.replace_resource (s_tables store) address before
          (G.E.H.Tables.tree_attachment before (get_tree_raw node))) address =
        Some (G.E.H.Tables.tree_attachment before (get_tree_raw node))).
      unfold lookup_N, G.E.H.replace_resource; rewrite nth_error_set_eq; reflexivity.
    - split; [reflexivity|].
      split; [exact Maximum|].
      split; [apply N.le_refl|].
      change (List.length (map G.E.H.Tables.externref (get_tree_raw node)) <=
        List.length (get_tree_raw node)).
      rewrite List.length_map; lia.
  Qed.

  Lemma tree_attachment_view store node index next :
    (Z.of_nat index < 2 ^ 32)%Z ->
    G.E.H.tree_attachment_result G.E.collector_instance store
      [ext (treeh node); word index] = Some (next, result_values []) ->
    tree_view next (N.of_nat index) node.
  Proof.
    intros Bound Attached offset value Entry.
    pose proof (G.E.H.tree_attachment_result_elements _ _ _ _ _ _ _ Attached Entry) as Found.
    rewrite (wasm_nat_to_N index Bound) in Found; exact Found.
  Qed.

  Lemma tree_attachment_memory_capacity store node index next :
    G.E.H.tree_attachment_result G.E.collector_instance store
      [ext (treeh node); word index] = Some (next, result_values []) ->
    memory_capacity store -> memory_capacity next.
  Proof.
    intros Attached [page [Memory [TypeOf Length]]].
    destruct (G.E.H.tree_attachment_result_shape _ _ _ _ _ Attached)
      as [address [before [Address [Before Next]]]].
    subst next; exists page; repeat split; assumption.
  Qed.

  Lemma tree_attachment_other_capacity store node index next other capacity :
    index < 10 -> other < 10 -> index <> other ->
    G.E.H.tree_attachment_result G.E.collector_instance store
      [ext (treeh node); word index] = Some (next, result_values []) ->
    view_capacity store (N.of_nat other) capacity ->
    view_capacity next (N.of_nat other) capacity.
  Proof.
    intros Index Other Different Attached [table [Entry Shape]].
    exists table; split; [|exact Shape].
    rewrite <- Entry.
    eapply G.E.H.tree_attachment_result_other_view
      with (target := N.of_nat index) (address := N.of_nat other).
    - exact Attached.
    - rewrite (wasm_nat_to_N index ltac:(lia)); apply collector_table_address; exact Index.
    - apply collector_table_address; exact Other.
    - lia.
  Qed.

  Lemma tree_attachment_other_element store node index next other offset :
    index < 10 -> other < 10 -> index <> other ->
    G.E.H.tree_attachment_result G.E.collector_instance store
      [ext (treeh node); word index] = Some (next, result_values []) ->
    stab_elem next G.E.collector_instance (N.of_nat other) offset =
      stab_elem store G.E.collector_instance (N.of_nat other) offset.
  Proof.
    intros Index Other Different Attached.
    eapply G.E.H.tree_attachment_result_other_table
      with (target := N.of_nat index) (address := N.of_nat other).
    - exact Attached.
    - rewrite (wasm_nat_to_N index ltac:(lia)); apply collector_table_address; exact Index.
    - apply collector_table_address; exact Other.
    - lia.
  Qed.

  Lemma tree_attachment_other_view store node index next other viewed :
    index < 10 -> other < 10 -> index <> other ->
    G.E.H.tree_attachment_result G.E.collector_instance store
      [ext (treeh node); word index] = Some (next, result_values []) ->
    tree_view store (N.of_nat other) viewed -> tree_view next (N.of_nat other) viewed.
  Proof.
    intros Index Other Different Attached View offset value Entry.
    rewrite (tree_attachment_other_element _ _ _ _ _ _ Index Other Different Attached).
    apply View; exact Entry.
  Qed.

  Lemma tree_attachment_retained_claims store node index next witnesses :
    index < 10 -> index <> 9 ->
    G.E.H.tree_attachment_result G.E.collector_instance store
      [ext (treeh node); word index] = Some (next, result_values []) ->
    retained_claims store witnesses -> retained_claims next witnesses.
  Proof.
    intros Index Different Attached Retained offset witness Entry.
    change (stab_elem next G.E.collector_instance (N.of_nat 9) (N.of_nat offset) =
      Some (G.E.H.Tables.externref (supplied_claim witness))).
    rewrite (tree_attachment_other_element store node index next 9 (N.of_nat offset)
      Index ltac:(lia) Different Attached).
    apply Retained; exact Entry.
  Qed.

  Lemma firstn_set_nth_before {A} (values : list A) default value count index :
    count <= List.length values -> count <= index ->
    firstn count (seq.set_nth default values index value) = firstn count values.
  Proof.
    revert values index; induction count as [|count IH]; intros values index Size Bound;
      [reflexivity|].
    destruct values as [|head values]; [cbn in Size; lia|].
    destruct index as [|index]; [lia|].
    cbn; f_equal; apply IH; cbn in Size; lia.
  Qed.

  Lemma tree_attachment_first_two_tables store node index next :
    2 <= index -> index < 10 ->
    G.E.H.tree_attachment_result G.E.collector_instance store
      [ext (treeh node); word index] = Some (next, result_values []) ->
    firstn 2 (s_tables next) = firstn 2 (s_tables store).
  Proof.
    intros Lower Upper Attached.
    destruct (G.E.H.tree_attachment_result_shape _ _ _ _ _ Attached)
      as [address [before [Address [Before Next]]]].
    rewrite (wasm_nat_to_N index ltac:(lia)), (collector_table_address index Upper) in Address.
    inversion Address; subst address.
    assert (Size : index < List.length (s_tables store)).
    { apply (proj1 (List.nth_error_Some _ _)).
      unfold lookup_N in Before; rewrite Nat2N.id in Before; rewrite Before; discriminate. }
    subst next; cbn [upd_s_mem upd_s_table].
    unfold G.E.H.replace_resource; rewrite Nat2N.id.
    apply firstn_set_nth_before; lia.
  Qed.

  Definition table_replacement store index replacement :=
    upd_s_table store (seq.set_nth replacement (s_tables store) index replacement).

  Lemma table_replacement_extension store index before replacement :
    nth_error (s_tables store) index = Some before ->
    table_extension before replacement = true ->
    store_extension store (table_replacement store index replacement) = true.
  Proof.
    intros Present Extension.
    change ((((((component_extension func_extension (s_funcs store) (s_funcs store) &&
      component_extension table_extension (s_tables store)
        (seq.set_nth replacement (s_tables store) index replacement)) &&
      component_extension mem_extension (s_mems store) (s_mems store)) &&
      component_extension global_extension (s_globals store) (s_globals store)) &&
      component_extension elem_extension (s_elems store) (s_elems store)) &&
      component_extension data_extension (s_datas store) (s_datas store))%bool = true).
    rewrite (component_extension_same_refl (s_funcs store) func_extension_refl),
      (component_extension_same_refl (s_mems store) mem_extension_refl),
      (component_extension_same_refl (s_globals store) global_extension_refl),
      (component_extension_same_refl (s_elems store) elem_extension_refl),
      (component_extension_same_refl (s_datas store) data_extension_refl).
    repeat rewrite Bool.andb_true_r; cbn.
    apply component_extension_update with (x := before).
    - exact table_extension_refl.
    - exact Present.
    - exact Extension.
  Qed.

  Lemma table_replacement_typing store index before replacement typ :
    nth_error (s_tables store) index = Some before -> store_typing store ->
    store_extension store (table_replacement store index replacement) = true ->
    tableinst_typing (table_replacement store index replacement) replacement = Some typ ->
    store_typing (table_replacement store index replacement).
  Proof.
    intros Present Typed Extension After.
    destruct store as [functions oldtables memories globals elements data].
    destruct Typed as [Functions [Tables [Memories [Globals [Elements Data]]]]].
    cbn [table_replacement store_typing upd_s_table]; repeat split.
    - eapply store_extension_funcs_typing; eassumption.
    - eapply Forall_set.
      + eapply store_extension_tables_typing; eassumption.
      + exists typ; exact After.
      + apply (ssrbool.introT ssrnat.ltP).
        apply (proj1 (List.nth_error_Some _ _)); rewrite Present; discriminate.
    - eapply store_extension_mems_typing; eassumption.
    - eapply store_extension_globals_typing; eassumption.
    - eapply store_extension_elems_typing; eassumption.
    - eapply store_extension_datas_typing; eassumption.
  Qed.

  Lemma table_replacement_lookup store index replacement other :
    index < List.length (s_tables store) -> index <> other ->
    lookup_N (s_tables (table_replacement store index replacement)) (N.of_nat other) =
      lookup_N (s_tables store) (N.of_nat other).
  Proof.
    intros Bound Different.
    change (nth_error (seq.set_nth replacement (s_tables store) index replacement)
      (N.to_nat (N.of_nat other)) = nth_error (s_tables store) (N.to_nat (N.of_nat other))).
    rewrite Nat2N.id, nth_error_set_neq; [reflexivity|lia|].
    apply (ssrbool.introT ssrnat.ltP); exact Bound.
  Qed.

  Lemma table_replacement_capacity store index replacement other capacity :
    index < List.length (s_tables store) -> other < 10 -> index <> other ->
    view_capacity store (N.of_nat other) capacity ->
    view_capacity (table_replacement store index replacement) (N.of_nat other) capacity.
  Proof.
    intros Bound Other Different [table [Entry Shape]].
    exists table; split; [|exact Shape].
    unfold stab in *; rewrite (collector_table_address other Other) in *.
    rewrite (table_replacement_lookup _ _ _ _ Bound Different); exact Entry.
  Qed.

  Lemma table_replacement_element store index replacement other offset :
    index < List.length (s_tables store) -> other < 10 -> index <> other ->
    stab_elem (table_replacement store index replacement) G.E.collector_instance
      (N.of_nat other) offset = stab_elem store G.E.collector_instance (N.of_nat other) offset.
  Proof.
    intros Bound Other Different; unfold stab_elem.
    rewrite (collector_table_address other Other),
      (table_replacement_lookup _ _ _ _ Bound Different); reflexivity.
  Qed.

  Lemma table_replacement_view store index replacement other node :
    index < List.length (s_tables store) -> other < 10 -> index <> other ->
    tree_view store (N.of_nat other) node ->
    tree_view (table_replacement store index replacement) (N.of_nat other) node.
  Proof.
    intros Bound Other Different View offset value Entry.
    rewrite (table_replacement_element _ _ _ _ _ Bound Other Different).
    apply View; exact Entry.
  Qed.

  Lemma table_replacement_memory_capacity store index replacement :
    memory_capacity store -> memory_capacity (table_replacement store index replacement).
  Proof. unfold memory_capacity, table_replacement; cbn [upd_s_table]; auto. Qed.

  Lemma table_replacement_first_two_tables store index replacement :
    2 <= index -> index < List.length (s_tables store) ->
    firstn 2 (s_tables (table_replacement store index replacement)) = firstn 2 (s_tables store).
  Proof.
    intros Lower Upper; unfold table_replacement; cbn [upd_s_table].
    apply firstn_set_nth_before; lia.
  Qed.

  Lemma blob_read_capacity store arguments next result index capacity :
    G.E.H.blob_read_result G.E.collector_instance store arguments = Some (next, result) ->
    view_capacity store index capacity -> view_capacity next index capacity.
  Proof.
    intros Read Capacity; eapply view_capacity_same_tables; [|exact Capacity].
    eapply G.E.H.blob_read_result_tables; exact Read.
  Qed.

  Lemma blob_read_view store arguments next result index node :
    G.E.H.blob_read_result G.E.collector_instance store arguments = Some (next, result) ->
    tree_view store index node -> tree_view next index node.
  Proof.
    intros Read View; eapply tree_view_same_tables; [|exact View].
    eapply G.E.H.blob_read_result_tables; exact Read.
  Qed.

  Lemma blob_read_retained_claims store arguments next result witnesses :
    G.E.H.blob_read_result G.E.collector_instance store arguments = Some (next, result) ->
    retained_claims store witnesses -> retained_claims next witnesses.
  Proof.
    intros Read Retained; eapply retained_claims_same_tables; [|exact Retained].
    eapply G.E.H.blob_read_result_tables; exact Read.
  Qed.

  Lemma blob_read_memory_capacity store arguments next result :
    G.E.H.blob_read_result G.E.collector_instance store arguments = Some (next, result) ->
    memory_capacity store -> memory_capacity next.
  Proof.
    intros Read [page [Memory [TypeOf Length]]].
    destruct (G.E.H.blob_read_result_memory_shape _ _ _ _ _ Read)
      as [address [before [after [Address [Before [AfterType [AfterLength Memories]]]]]]].
    rewrite (proj2 G.E.collector_resource_indices) in Address.
    cbn [lookup_N] in Address; inversion Address; subst address.
    unfold lookup_N in Before; rewrite Memory in Before; cbn in Before;
      inversion Before; subst before.
    exists after; split.
    - rewrite Memories, Memory; reflexivity.
    - rewrite AfterType, AfterLength; auto.
  Qed.
End WasmResources.
