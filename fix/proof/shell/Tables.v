(** Models externref tables used to construct Trees and Tags, and proves
    that construction preserves their supplied contents. *)

From Wasm Require Import datatypes operations properties typing.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Shell Require Import Externref TagCreation.
From Stdlib Require Import List Arith NArith Lia.
Import ListNotations.

Module ShellTables (S : STORAGE) (P : PROGRAM S) (R : EXTERNREFS S).
  Module H := Handles S.
  Module T := TagCreation S P.
  Import S H R.

  Definition externref value := VAL_ref_extern (to_externref value).
  Definition read_externref reference := match reference with
    | VAL_ref_extern address => Some (to_handle address)
    | _ => None end.
  Definition read_externrefs references := those (map read_externref references).

  (** Decoding encoded externrefs recovers their original Fix handles. *)
  Lemma read_externrefs_encode values :
    read_externrefs (map externref values) = Some values.
  Proof.
    unfold read_externrefs; apply those_spec.
    - rewrite !List.length_map; reflexivity.
    - intros index value Value; rewrite !List.nth_error_map, Value; cbn.
      unfold read_externref, externref; rewrite to_handle_to_externref; reflexivity.
  Qed.

  Definition tree_attachment before values :=
    Build_tableinst
      (Build_table_type
        (Build_limits (N.of_nat (List.length values))
          (lim_max (tt_limits (tableinst_type before)))) T_externref)
      (map externref values).

  Lemma tree_attachment_contents before values :
    tableinst_elem (tree_attachment before values) = map externref values.
  Proof. reflexivity. Qed.

  Lemma tree_attachment_prefix before values :
    read_externrefs (tableinst_elem (tree_attachment before values)) = Some values.
  Proof. apply read_externrefs_encode. Qed.

  Lemma tree_attachment_extension before values :
    tt_elem_type (tableinst_type before) = T_externref ->
    lim_max (tt_limits (tableinst_type before)) = None ->
    (lim_min (tt_limits (tableinst_type before)) <= N.of_nat (List.length values))%N ->
    tab_size before <= List.length values ->
    table_extension before (tree_attachment before values) = true.
  Proof.
    intros Element Maximum Minimum Size.
    assert (TypeExtension : table_type_extension (tableinst_type before)
      (tableinst_type (tree_attachment before values)) = true).
    { unfold table_type_extension, limits_extension, tree_attachment; cbn.
      rewrite Element, Maximum; cbn.
      rewrite Bool.andb_true_r; apply N.leb_le; exact Minimum. }
    unfold table_extension; rewrite TypeExtension; cbn.
    unfold tab_size, tree_attachment; cbn; rewrite List.length_map.
    apply (ssrbool.introT ssrnat.leP); exact Size.
  Qed.

  Lemma tree_attachment_typing {hfc : host_function_class} {mem : BlockUpdateMemory}
      (store : store_record) before values :
    lim_max (tt_limits (tableinst_type before)) = None ->
    (N.of_nat (List.length values) <= table_limit_bound)%N ->
    tableinst_typing store (tree_attachment before values) =
      Some (tableinst_type (tree_attachment before values)).
  Proof.
    intros Maximum Bound.
    unfold tableinst_typing, tree_attachment; cbn -[tabletype_valid].
    rewrite Maximum; unfold tabletype_valid, limit_valid_range; cbn -[N.leb].
    assert (Fits : N.leb (N.of_nat (List.length values)) table_limit_bound = true)
      by (apply N.leb_le; exact Bound).
    change (N.leb (N.of_nat (List.length values)) 4294967295%N = true) in Fits.
    rewrite Fits; cbn -[eqtype.eq_op ssrnat.N_eqb].
    rewrite List.length_map.
    assert (SameLength : ssrnat.N_eqb (N.of_nat (List.length values))
      (N.of_nat (List.length values)) = true).
    { destruct (N.of_nat (List.length values)); cbn [ssrnat.N_eqb];
        [reflexivity|apply Pos.eqb_refl]. }
    rewrite SameLength.
    assert (References : seq.all (fun reference => value_typing store (VAL_ref reference) (T_ref T_externref))
      (map externref values) = true).
    { clear Bound Fits SameLength.
      assert (ReferenceTyped : forall value,
        value_typing store (VAL_ref (externref value)) (T_ref T_externref) = true)
        by (intro value; reflexivity).
      induction values; cbn [List.map seq.all]; [reflexivity|].
      rewrite ReferenceTyped, IHvalues; reflexivity. }
    rewrite References; reflexivity.
  Qed.

  Definition table_prefix table length :=
    match tt_elem_type (tableinst_type table) with
    | T_externref =>
      if Nat.leb length (List.length (tableinst_elem table))
      then read_externrefs (firstn length (tableinst_elem table)) else None
    | _ => None end.

  (** The encoded prefix of an externref table decodes to its original handle list. *)
  Lemma table_prefix_encode table values unused :
    tt_elem_type (tableinst_type table) = T_externref ->
    tableinst_elem table = map externref values ++ unused ->
    table_prefix table (List.length values) = Some values.
  Proof.
    intros Element Contents; unfold table_prefix; rewrite Element, Contents.
    assert (Nat.leb (List.length values) (List.length (map externref values ++ unused)) = true)
      as Bound by (apply Nat.leb_le; rewrite List.length_app, List.length_map; lia).
    rewrite Bound.
    replace (List.length values) with (List.length (map externref values) + 0) at 1
      by (rewrite List.length_map; lia).
    rewrite firstn_app_2; cbn; rewrite List.app_nil_r; apply read_externrefs_encode.
  Qed.

  Definition copied_entries previous values index :=
    map externref (previous ++ firstn index values) ++
      repeat (VAL_ref_null T_externref) (List.length values - index).

  (** Copying entries changes their contents but preserves the allocated destination-table
      length. *)
  Lemma copied_entries_length previous values index : index <= List.length values ->
    List.length (copied_entries previous values index) = List.length previous + List.length values.
  Proof.
    intro Bound; unfold copied_entries.
    rewrite List.length_app, List.length_map, List.length_app, List.length_firstn, List.repeat_length,
      Nat.min_l by exact Bound; lia.
  Qed.

  (** Extending a list prefix by one appends exactly its next entry. *)
  Lemma firstn_next {A} (values : list A) index value :
    nth_error values index = Some value -> firstn (S index) values = firstn index values ++ [value].
  Proof.
    revert values; induction index; intros [|first values] Entry; cbn in *; try discriminate.
    - inversion Entry; reflexivity.
    - rewrite IHindex by exact Entry; reflexivity.
  Qed.

  (** Updating the entry immediately after a prefix preserves the prefix and remaining
      suffix. *)
  Lemma set_nth_after_prefix {A} (prefix suffix : list A) default before after :
    seq.set_nth default (prefix ++ before :: suffix) (List.length prefix) after = prefix ++ after :: suffix.
  Proof. induction prefix; cbn; [reflexivity|rewrite IHprefix; reflexivity]. Qed.

  (** Writing the next encoded handle advances the table-copy invariant by one entry. *)
  Lemma copied_entries_write previous values index value :
    nth_error values index = Some value ->
    seq.set_nth (externref value) (copied_entries previous values index)
      (List.length previous + index) (externref value) = copied_entries previous values (S index).
  Proof.
    intro Entry.
    assert (Bound : index < List.length values).
    { apply (proj1 (List.nth_error_Some values index)); rewrite Entry; discriminate. }
    assert (Prefix : List.length (map externref (previous ++ firstn index values)) =
      List.length previous + index).
    { rewrite List.length_map, List.length_app, List.length_firstn, Nat.min_l by lia; reflexivity. }
    unfold copied_entries.
    replace (List.length values - index) with (S (List.length values - S index)) by lia.
    cbn [List.repeat].
    rewrite <- Prefix, set_nth_after_prefix, (firstn_next _ _ _ Entry).
    repeat rewrite List.map_app; repeat rewrite <- List.app_assoc; reflexivity.
  Qed.

  (** Completing the table copy gives exactly the previous entries followed by the new
      handles. *)
  Lemma copied_entries_complete previous values :
    copied_entries previous values (List.length values) = map externref (previous ++ values).
  Proof.
    unfold copied_entries; rewrite firstn_all, Nat.sub_diag; cbn; apply List.app_nil_r.
  Qed.

  (** Growing the author table within its index bound supplies the initial table-copy
      state. *)
  Lemma grow_copied_entries previous values minimum :
    (N.of_nat (List.length previous + List.length values) < u32_bound)%N ->
    growtable
      (Build_tableinst (Build_table_type (Build_limits minimum None) T_externref) (map externref previous))
      (N.of_nat (List.length values)) (VAL_ref_null T_externref) =
    Some (Build_tableinst
      (Build_table_type (Build_limits (N.of_nat (List.length previous + List.length values)) None) T_externref)
      (copied_entries previous values 0)).
  Proof.
    intro Bound; unfold growtable, tab_size; cbn -[N.leb N.of_nat].
    rewrite List.length_map, <- Nat2N.inj_add.
    assert (Fits : N.leb u32_bound (N.of_nat (List.length previous + List.length values)) = false)
      by (apply N.leb_gt; exact Bound).
    rewrite Fits; unfold limit_valid_range; cbn -[N.leb N.of_nat].
    assert (Limit : N.leb (N.of_nat (List.length previous + List.length values)) table_limit_bound = true)
      by (apply N.leb_le; unfold table_limit_bound, u32_bound in *; lia).
    unfold table_limit_bound, u32_bound in Limit.
    cbn -[N.leb N.of_nat] in Limit.
    rewrite Limit; cbn -[N.of_nat].
    rewrite Nat2N.id; unfold copied_entries; cbn; rewrite List.app_nil_r, Nat.sub_0_r; reflexivity.
  Qed.

  Section Store.
    Context `{hfc : host_function_class} `{mem : BlockUpdateMemory}.

    Definition create_tree_from_table store instance index length :=
      obind (stab store instance index) (fun table =>
        omap create_tree (table_prefix table length)).

    Definition create_tag_from_table combination store instance index :=
      obind (stab store instance index) (fun table =>
        obind (table_prefix table (List.length (tableinst_elem table)))
          (T.create_tag combination)).

    (** Tree creation from a valid externref-table prefix constructs exactly the
        corresponding Fix Tree. *)
    Lemma create_tree_from_table_correct store instance index table values unused :
      stab store instance index = Some table ->
      tt_elem_type (tableinst_type table) = T_externref ->
      tableinst_elem table = map externref values ++ unused ->
      create_tree_from_table store instance index (List.length values) =
        Some (create_tree values).
    Proof.
      intros Table Element Contents; unfold create_tree_from_table; rewrite Table; cbn [obind].
      rewrite (table_prefix_encode _ _ _ Element Contents); reflexivity.
    Qed.

    (** Tag creation from a valid externref table agrees with abstract Tag creation. *)
    Lemma create_tag_from_table_correct combination store instance index table values :
      stab store instance index = Some table ->
      tt_elem_type (tableinst_type table) = T_externref ->
      tableinst_elem table = map externref values ->
      create_tag_from_table combination store instance index =
        T.create_tag combination values.
    Proof.
      intros Table Element Contents; unfold create_tag_from_table; rewrite Table; cbn [obind].
      rewrite Contents, List.length_map.
      rewrite (table_prefix_encode table values [] Element ltac:(rewrite Contents, List.app_nil_r; reflexivity)).
      reflexivity.
    Qed.
  End Store.
End ShellTables.
