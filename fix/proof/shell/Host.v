From Wasm Require Import datatypes operations typing host properties type_preservation.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Support Require Import WasmNatural.
From FixProof.Shell Require Import Operations Descriptors ABI Memory TagCreation.
From Stdlib Require Import List String Arith NArith ZArith Bool Lia.
Import ListNotations.
Local Open Scope list_scope.

Module ShellHost (S : STORAGE) (P : PROGRAM S).
  Module D := Descriptors S.
  Module H := Handles S.
  Module Api := Operations S.
  Module T := TagCreation S P.
  Import S H.

  Definition capacity := N.to_nat native_table_limit.
  Lemma capacity_value : Z.of_nat capacity = 134217728%Z.
  Proof. unfold capacity; rewrite N_nat_Z; reflexivity. Qed.
  Local Opaque capacity.
  Definition i32_of_nat n := Wasm_int.int_of_Z i32m (Z.of_nat n).
  Definition nat_of_i32 n := Wasm_int.nat_of_uint i32m n.
  Definition word n := VAL_num (VAL_int32 (i32_of_nat n)).
  Definition boolean (b : bool) := word (if b then 1 else 0).

  Record state := State { slots : D.table; observations : nat }.
  Definition initial value := State [Some value] 0.
  Definition resolve state index := D.lookup (slots state) (nat_of_i32 index).
  Definition unchanged (world : state) (values : list value) := Some (world, values).
  Definition allocate state value :=
    match D.allocate capacity (slots state) value with
    | Some (index, next) => Some (State next (observations state), [word index])
    | None => None end.
  Definition transform state index operation :=
    obind (resolve state index) (fun value => obind (operation value) (allocate state)).

  Lemma word_roundtrip index : index < capacity -> nat_of_i32 (i32_of_nat index) = index.
  Proof.
    intro Bound; apply wasm_nat_to_nat; apply Nat2Z.inj_lt in Bound.
    rewrite capacity_value in Bound; lia.
  Qed.

  Lemma allocate_resolves before value after values : allocate before value = Some (after, values) ->
    exists index, values = [word index] /\ index < capacity /\
      D.lookup (slots before) index = None /\
      resolve after (i32_of_nat index) = Some value /\
      D.live_count (slots after) = Datatypes.S (D.live_count (slots before)) /\
      observations after = observations before.
  Proof.
    unfold allocate; destruct (D.allocate capacity (slots before) value) as [[index next]|] eqn:Allocated;
      [|discriminate].
    intro Returned; inversion Returned; subst.
    destruct (D.allocate_spec _ _ _ _ _ Allocated) as [Bound [Fresh [Present _]]].
    exists index; repeat split; try reflexivity; try assumption.
    - change (D.lookup next (nat_of_i32 (i32_of_nat index)) = Some value).
      rewrite word_roundtrip by exact Bound; exact Present.
    - unfold D.allocate in Allocated; destruct (D.insert (slots before) value) as [offset tail] eqn:Inserted.
      destruct (Nat.ltb offset capacity); inversion Allocated; subst; eapply D.insert_live; exact Inserted.
  Qed.

  Lemma allocate_retains before value after values index previous :
    allocate before value = Some (after, values) ->
    resolve before index = Some previous -> resolve after index = Some previous.
  Proof.
    unfold allocate; destruct (D.allocate capacity (slots before) value) as [[offset next]|] eqn:Allocated;
      [|discriminate].
    intros Returned Present; inversion Returned; subst.
    destruct (D.allocate_spec _ _ _ _ _ Allocated) as [_ [Fresh [_ [_ Other]]]].
    unfold resolve in Present.
    change (D.lookup next (nat_of_i32 index) = Some previous).
    rewrite Other; [exact Present|].
    intro Same; rewrite Same, Fresh in Present; discriminate.
  Qed.

  Lemma allocate_available before value : D.live_count (slots before) < capacity ->
    exists after values, allocate before value = Some (after, values).
  Proof.
    intro Room; destruct (D.allocate_available capacity (slots before) value Room) as [index [next Allocated]].
    unfold allocate; rewrite Allocated; eauto.
  Qed.

  Definition within_capacity world := List.length (slots world) <= capacity.

  Lemma allocate_capacity before value after values : within_capacity before ->
    allocate before value = Some (after, values) -> within_capacity after.
  Proof.
    unfold within_capacity, allocate.
    destruct (D.allocate capacity (slots before) value) as [[index next]|] eqn:Allocated;
      [|discriminate].
    intros Bound Returned; inversion Returned; subst; cbn.
    pose proof (D.allocate_spec _ _ _ _ _ Allocated) as [Fits _].
    unfold D.allocate in Allocated; destruct (D.insert (slots before) value) as [offset tail] eqn:Inserted.
    destruct (Nat.ltb offset capacity); inversion Allocated; subst.
    pose proof (D.insert_length _ _ _ _ Inserted); lia.
  Qed.

  Lemma initial_capacity value : within_capacity (initial value).
  Proof.
    change (1 <= capacity); apply Nat2Z.inj_le; rewrite capacity_value; cbn; lia.
  Qed.

  Lemma resolve_bound world index value : within_capacity world -> resolve world index = Some value ->
    nat_of_i32 index < capacity.
  Proof.
    intros Within Present; unfold resolve, D.lookup in Present.
    destruct (nth_error (slots world) (nat_of_i32 index)) as [entry|] eqn:Slot; [|discriminate].
    assert (Bound : nat_of_i32 index < List.length (slots world)).
    { apply (proj1 (List.nth_error_Some _ _)); rewrite Slot; discriminate. }
    unfold within_capacity in Within; lia.
  Qed.

  Definition release world index :=
    omap (fun next => State next (observations world)) (D.close (slots world) (nat_of_i32 index)).

  Lemma release_available before index value : resolve before index = Some value ->
    exists after, release before index = Some after.
  Proof.
    intro Present; unfold resolve in Present.
    destruct (D.close_available _ _ _ Present) as [next Closed].
    unfold release; rewrite Closed; cbn [omap]; eexists; reflexivity.
  Qed.

  Lemma release_resolves before index after : release before index = Some after ->
    resolve after index = None /\
    D.live_count (slots before) = Datatypes.S (D.live_count (slots after)) /\
    observations after = observations before.
  Proof.
    unfold release; destruct (D.close (slots before) (nat_of_i32 index)) as [next|] eqn:Closed;
      [|discriminate].
    intro Released; inversion Released; subst; repeat split; try reflexivity.
    - change (D.lookup next (nat_of_i32 index) = None); eapply D.close_lookup; exact Closed.
    - eapply D.close_live; exact Closed.
  Qed.

  Lemma release_retains before index after other : release before index = Some after ->
    nat_of_i32 other <> nat_of_i32 index -> resolve after other = resolve before other.
  Proof.
    unfold release; destruct (D.close (slots before) (nat_of_i32 index)) as [next|] eqn:Closed;
      [|discriminate].
    intros Released Different; inversion Released; subst.
    change (D.lookup next (nat_of_i32 other) = D.lookup (slots before) (nat_of_i32 other)).
    eapply D.close_other; eassumption.
  Qed.

  Lemma release_capacity before index after : within_capacity before ->
    release before index = Some after -> within_capacity after.
  Proof.
    unfold release, within_capacity; destruct (D.close (slots before) (nat_of_i32 index)) as [next|] eqn:Closed;
      [|discriminate].
    intros Bound Released; inversion Released; subst.
    change (List.length next <= capacity); rewrite (D.close_length _ _ _ Closed); exact Bound.
  Qed.

  Lemma release_twice before index after : release before index = Some after -> release after index = None.
  Proof.
    unfold release; destruct (D.close (slots before) (nat_of_i32 index)) as [next|] eqn:Closed;
      [|discriminate].
    intro Released; inversion Released; subst.
    change (omap (fun following => State following (observations before))
      (D.close next (nat_of_i32 index)) = None).
    rewrite (D.close_twice _ _ _ Closed); reflexivity.
  Qed.

  Definition data_type value := match value with
    | Data (Object (BlobObj _)) | Data (Ref (BlobRef _)) => Some 0
    | Data (Object (TreeObj _)) | Data (Ref (TreeRef _)) => Some 1
    | _ => None end.
  Definition encode_type value := match value with
    | Encode (Strict _) => Some 0 | Encode (Shallow _) => Some 1 | _ => None end.
  Definition thunk_type value := match value with
    | Thunk (Application _) | Encode (Strict (Application _)) | Encode (Shallow (Application _)) => Some 0
    | Thunk (Selection _) | Encode (Strict (Selection _)) | Encode (Shallow (Selection _)) => Some 1
    | Thunk (Identification _) | Encode (Strict (Identification _)) | Encode (Shallow (Identification _)) => Some 2
    | _ => None end.
  Definition reference value := match value with
    | Data (Object object) => Some (Data (Ref (data_reference (Object object)))) | _ => None end.
  Definition tree_thunk constructor value := match value with
    | Data (Object (TreeObj node)) | Data (Ref (TreeRef node)) => Some (Thunk (constructor node))
    | _ => None end.

  Section Calls.
    Context `{mem : BlockUpdateMemory}.
    Variable combination : TreeName.
    Variable instance : moduleinst.
    Variable privileged trusted_tagger : bool.
    Variable handle_bytes : nat -> handle -> option BlobName.

    Fixpoint read_handles memory descriptors pointer count : option (list handle) :=
      match count with
      | 0 => Some []
      | Datatypes.S rest =>
        obind (read_bytes memory pointer 4) (fun bytes =>
          match wasm_deserialise bytes T_i32 with
          | VAL_int32 index =>
            obind (D.lookup descriptors (nat_of_i32 index)) (fun value =>
              omap (cons value) (read_handles memory descriptors (pointer + 4)%N rest))
          | _ => None end)
      end.

    Definition memory_zero (store : store_record) :=
      obind (lookup_N (inst_mems instance) 0%N) (lookup_N (s_mems store)).

    Definition tree_from_memory state store pointer count :=
      if Nat.leb (nat_of_i32 count) capacity then
        obind (memory_zero store) (fun memory =>
          obind (read_handles memory (slots state) (Wasm_int.N_of_uint i32m pointer) (nat_of_i32 count))
            (fun fields => allocate state (HTreeObj (create_tree fields))))
      else None.

    Definition checked_tag value := match value with
      | Data (Object (TreeObj fields)) => omap HTreeObj (T.create_tag combination (get_tree_raw fields))
      | _ => None end.
    Definition unchecked_tag value :=
      if privileged && trusted_tagger then
        match value with
        | Data (Object (TreeObj fields)) => match get_tree_raw fields with
          | [] => None | author :: body => Some (HTreeObj (create_tree_tagged true (author :: body))) end
        | _ => None end
      else None.

    Definition length_result state length :=
      if Z.ltb (Z.of_nat length) (2 ^ 64)%Z then
        unchanged state [VAL_num (VAL_int64 (Wasm_int.int_of_Z i64m (Z.of_nat length)))]
      else None.

    Definition host_values state (store : store_record) operation arguments :=
      match operation, arguments with
      | shell_get_type, [VAL_num (VAL_int32 index)] =>
        obind (resolve state index) (fun value => unchanged state [word (value_type value)])
      | shell_get_data_type, [VAL_num (VAL_int32 index)] =>
        obind (resolve state index) (fun value => obind (data_type value) (fun typ => unchanged state [word typ]))
      | shell_get_encode_type, [VAL_num (VAL_int32 index)] =>
        obind (resolve state index) (fun value => obind (encode_type value) (fun typ => unchanged state [word typ]))
      | shell_get_thunk_type, [VAL_num (VAL_int32 index)] =>
        if privileged then obind (resolve state index)
          (fun value => obind (thunk_type value) (fun typ => unchanged state [word typ])) else None
      | shell_is_tag, [VAL_num (VAL_int32 index)] =>
        obind (resolve state index) (fun value => unchanged state [boolean (Api.is_tag value)])
      | shell_equals, [VAL_num (VAL_int32 lhs); VAL_num (VAL_int32 rhs)] =>
        obind (resolve state lhs) (fun left => obind (resolve state rhs) (fun right =>
          obind (T.O.compare_handles left right) (fun equal => unchanged state [boolean equal])))
      | shell_same_handle_bytes, [VAL_num (VAL_int32 lhs); VAL_num (VAL_int32 rhs)] =>
        if privileged then
          obind (resolve state lhs) (fun left => obind (resolve state rhs) (fun right =>
            obind (handle_bytes (observations state) left) (fun left_bytes =>
              obind (handle_bytes (Datatypes.S (observations state)) right) (fun right_bytes =>
                obind (T.O.compare_handles (HBlobObj left_bytes) (HBlobObj right_bytes))
                  (fun equal => unchanged (State (slots state) (observations state + 2)) [boolean equal])))))
        else None
      | shell_len, [VAL_num (VAL_int32 index)] =>
        obind (resolve state index) (fun value => match value with
          | Data data => length_result state (T.O.data_length data) | _ => None end)
      | shell_get_tree_element, [VAL_num (VAL_int32 index); VAL_num (VAL_int32 offset)] =>
        transform state index (fun value => Api.get_tree_data_api value (nat_of_i32 offset))
      | shell_create_blob_i64, [VAL_num (VAL_int64 value)] => allocate state (HBlobObj (create_blob (serialise_i64 value)))
      | shell_create_tree, [VAL_num (VAL_int32 pointer); VAL_num (VAL_int32 count)] => tree_from_memory state store pointer count
      | shell_create_ref, [VAL_num (VAL_int32 index)] => transform state index reference
      | shell_create_tag, [VAL_num (VAL_int32 index)] => transform state index checked_tag
      | shell_create_tag_unchecked, [VAL_num (VAL_int32 index)] => transform state index unchecked_tag
      | shell_application, [VAL_num (VAL_int32 index)] => transform state index (tree_thunk Application)
      | shell_selection, [VAL_num (VAL_int32 index)] => transform state index (tree_thunk Selection)
      | shell_identification, [VAL_num (VAL_int32 index)] => transform state index Api.create_identification_thunk_api
      | shell_strict, [VAL_num (VAL_int32 index)] => transform state index Api.create_strict_encode_api
      | shell_shallow, [VAL_num (VAL_int32 index)] => transform state index Api.create_shallow_encode_api
      | shell_clone, [VAL_num (VAL_int32 index)] => obind (resolve state index) (allocate state)
      | shell_close, [VAL_num (VAL_int32 index)] =>
        omap (fun next => (next, [])) (release state index)
      | _, _ => None
      end.

    Definition component_typed {A B} (typing : A -> option B) values :=
      forallb (fun value => match typing value with Some _ => true | None => false end) values.

    Lemma component_typed_correct {A B} (typing : A -> option B) values :
      component_typed typing values = true ->
      List.Forall (fun value => exists typ, typing value = Some typ) values.
    Proof.
      intro Typed; apply List.Forall_forall; intros value Present.
      unfold component_typed in Typed; rewrite List.forallb_forall in Typed.
      specialize (Typed value Present).
      destruct (typing value) eqn:TypeOf; [eauto|discriminate].
    Qed.

    Lemma component_typed_complete {A B} (typing : A -> option B) values :
      List.Forall (fun value => exists typ, typing value = Some typ) values ->
      component_typed typing values = true.
    Proof.
      intro Typed; unfold component_typed; apply List.forallb_forall.
      intros value Present; rewrite List.Forall_forall in Typed.
      destruct (Typed value Present) as [typ TypeOf]; rewrite TypeOf; reflexivity.
    Qed.

    (** The supported host fragment retains WasmCert's monotone resource limits. *)
    Definition supported_update (store : store_record) tables memories :=
      let next := upd_s_mem (upd_s_table store tables) memories in
      if (store_extension store next &&
          (component_typed (tableinst_typing next) tables &&
           component_typed (meminst_typing next) memories))%bool
      then Some next else None.

    Lemma supported_update_shape store tables memories next :
      supported_update store tables memories = Some next ->
      next = upd_s_mem (upd_s_table store tables) memories.
    Proof.
      unfold supported_update; destruct (_ && _)%bool;
        intro Updated; inversion Updated; reflexivity.
    Qed.

    Lemma supported_update_functions store tables memories next :
      supported_update store tables memories = Some next -> s_funcs next = s_funcs store.
    Proof. intro Updated; rewrite (supported_update_shape _ _ _ _ Updated); reflexivity. Qed.

    Lemma supported_update_correct store tables memories next :
      supported_update store tables memories = Some next ->
      store_extension store next = true /\
      (store_typing store -> store_typing next).
    Proof.
      unfold supported_update.
      destruct (store_extension store (upd_s_mem (upd_s_table store tables) memories) &&
        (component_typed (tableinst_typing (upd_s_mem (upd_s_table store tables) memories)) tables &&
         component_typed (meminst_typing (upd_s_mem (upd_s_table store tables) memories)) memories))%bool
        eqn:Allowed; [|discriminate].
      intro Updated; inversion Updated; subst next.
      apply Bool.andb_true_iff in Allowed as [Extension Components].
      apply Bool.andb_true_iff in Components as [Tables Memories].
      split; [exact Extension|].
      intro Typed; destruct store as [functions oldtables oldmemories globals elements data].
      destruct Typed as [Functions [OldTables [OldMemories [Globals [Elements Data]]]]].
      cbn [upd_s_mem upd_s_table store_typing].
      repeat split.
      - eapply store_extension_funcs_typing; eassumption.
      - apply component_typed_correct; exact Tables.
      - apply component_typed_correct; exact Memories.
      - eapply store_extension_globals_typing; eassumption.
      - eapply store_extension_elems_typing; eassumption.
      - eapply store_extension_datas_typing; eassumption.
    Qed.

    Lemma supported_update_accept store tables memories :
      store_extension store (upd_s_mem (upd_s_table store tables) memories) = true ->
      store_typing (upd_s_mem (upd_s_table store tables) memories) ->
      supported_update store tables memories =
        Some (upd_s_mem (upd_s_table store tables) memories).
    Proof.
      intros Extension Typed.
      change (store_typing (upd_s_mem (upd_s_table store tables) memories)) in Typed.
      destruct store as [functions oldtables oldmemories globals elements data].
      destruct Typed as [Functions [Tables [Memories [Globals [Elements Data]]]]].
      cbn in Tables, Memories.
      unfold supported_update; rewrite Extension.
      rewrite (component_typed_complete _ _ Tables), (component_typed_complete _ _ Memories).
      reflexivity.
    Qed.

    Definition replace_resource {A} (values : list A) index before after :=
      seq.set_nth before values (N.to_nat index) after.

    Lemma supported_memory_replacement store address before after typ :
      lookup_N (s_mems store) address = Some before ->
      let memories := replace_resource (s_mems store) address before after in
      let next := upd_s_mem (upd_s_table store (s_tables store)) memories in
      store_typing store -> store_extension store next = true ->
      meminst_typing next after = Some typ ->
      supported_update store (s_tables store) memories = Some next.
    Proof.
      intros Present memories next Typed Extension After.
      apply supported_update_accept; [exact Extension|].
      destruct store as [functions tables oldmemories globals elements data].
      destruct Typed as [Functions [Tables [Memories [Globals [Elements Data]]]]].
      cbn [store_typing upd_s_mem upd_s_table]; repeat split.
      - eapply store_extension_funcs_typing; eassumption.
      - eapply store_extension_tables_typing; eassumption.
      - unfold memories, replace_resource; eapply Forall_set.
        + eapply store_extension_mems_typing; eassumption.
        + exists typ; exact After.
        + apply (ssrbool.introT ssrnat.ltP).
          apply (proj1 (List.nth_error_Some _ _)); unfold lookup_N in Present;
            rewrite Present; discriminate.
      - eapply store_extension_globals_typing; eassumption.
      - eapply store_extension_elems_typing; eassumption.
      - eapply store_extension_datas_typing; eassumption.
    Qed.

    Lemma host_values_typing world store operation arguments next values :
      host_values world store operation arguments = Some (next, values) ->
      (match shell_signature operation with Tf _ results => values_typing store values results end) = true.
    Proof.
      intro Returned; destruct operation;
        repeat (cbv beta iota zeta delta [host_values transform unchanged length_result allocate
          tree_from_memory boolean word obind omap option_map] in Returned;
          match type of Returned with context [match ?value with _ => _ end] => destruct value eqn:? end);
        try discriminate; inversion Returned; subst; reflexivity.
    Qed.


    Definition blob_read_result world (store : store_record) arguments :=
      match arguments with
      | [VAL_num (VAL_int32 address); VAL_num (VAL_int32 offset);
         VAL_num (VAL_int32 destination); VAL_num (VAL_int32 length)] =>
        obind (resolve world address) (fun value =>
          match value, lookup_N (inst_mems instance) 0%N with
          | Data (Object (BlobObj blob)), Some memory_address =>
            let bytes := get_blob_data blob in
            if Nat.leb (nat_of_i32 offset) (List.length bytes) then
              let copied := firstn (nat_of_i32 length) (skipn (nat_of_i32 offset) bytes) in
              obind (lookup_N (s_mems store) memory_address) (fun before =>
                obind (write_bytes_meminst before (Wasm_int.N_of_uint i32m destination) copied) (fun after =>
                  omap (fun next => (next, [word (List.length copied)]))
                    (supported_update store (s_tables store)
                      (replace_resource (s_mems store) memory_address before after))))
            else None
          | _, _ => None end)
      | _ => None end.

    Lemma blob_read_result_correct world store arguments next values :
      blob_read_result world store arguments = Some (next, values) ->
      store_extension store next = true /\ (store_typing store -> store_typing next).
    Proof.
      unfold blob_read_result; intro Read.
      repeat (cbv beta iota zeta delta [obind omap] in Read;
        match type of Read with context [match ?value with _ => _ end] => destruct value eqn:? end);
        try discriminate; inversion Read; subst.
      eapply supported_update_correct; eassumption.
    Qed.

    Lemma blob_read_result_success world store index blob offset destination length address before after next :
      resolve world index = Some (HBlobObj blob) ->
      lookup_N (inst_mems instance) 0%N = Some address -> lookup_N (s_mems store) address = Some before ->
      nat_of_i32 offset <= List.length (get_blob_data blob) ->
      write_bytes_meminst before (Wasm_int.N_of_uint i32m destination)
        (firstn (nat_of_i32 length) (skipn (nat_of_i32 offset) (get_blob_data blob))) = Some after ->
      supported_update store (s_tables store)
        (replace_resource (s_mems store) address before after) = Some next ->
      blob_read_result world store [VAL_num (VAL_int32 index); VAL_num (VAL_int32 offset);
        VAL_num (VAL_int32 destination); VAL_num (VAL_int32 length)] =
        Some (next, [word (List.length
          (firstn (nat_of_i32 length) (skipn (nat_of_i32 offset) (get_blob_data blob))))]).
    Proof.
      intros Live Address Before Offset Copied Supported.
      unfold blob_read_result; rewrite Live; cbn [obind HBlobObj]; rewrite Address.
      assert (Fits : Nat.leb (nat_of_i32 offset) (List.length (get_blob_data blob)) = true)
        by (apply Nat.leb_le; exact Offset).
      rewrite Fits; cbn [obind]; rewrite Before; cbn [obind]; rewrite Copied; cbn [obind].
      rewrite Supported; reflexivity.
    Qed.

    Lemma blob_read_zero_bounded world store index blob before after :
      resolve world index = Some (HBlobObj blob) ->
      inst_mems instance = [0%N] -> s_mems store = [before] -> store_typing store ->
      List.length (get_blob_data blob) = 8 ->
      write_bytes_meminst before 0%N (get_blob_data blob) = Some after ->
      blob_read_result world store [VAL_num (VAL_int32 index); word 0; word 0; word 8] =
        Some (upd_s_mem store [after], [word 8]).
    Proof.
      intros Live Instance Memory Typed Width Copied.
      assert (Prefix : firstn 8 (get_blob_data blob) = get_blob_data blob) by (apply firstn_all2; lia).
      assert (Supported : supported_update store (s_tables store)
        (replace_resource (s_mems store) 0%N before after) = Some (upd_s_mem store [after])).
      { unfold replace_resource; rewrite Memory.
        change (supported_update store (s_tables store) [after] = Some (upd_s_mem store [after])).
        apply supported_update_accept.
        - change (store_extension store (upd_s_mem store [after]) = true).
          eapply blob_copy_store_extension; eassumption.
        - change (store_typing (upd_s_mem store [after])).
          eapply blob_copy_store_typing; eassumption. }
      pose proof (blob_read_result_success world store index blob (i32_of_nat 0) (i32_of_nat 0)
        (i32_of_nat 8) 0%N before after (upd_s_mem store [after]) Live) as Read.
      change (lookup_N (inst_mems instance) 0%N = Some 0%N ->
        lookup_N (s_mems store) 0%N = Some before -> 0 <= List.length (get_blob_data blob) ->
        write_bytes_meminst before 0%N (firstn 8 (get_blob_data blob)) = Some after ->
        supported_update store (s_tables store) (replace_resource (s_mems store) 0%N before after) =
          Some (upd_s_mem store [after]) ->
        blob_read_result world store [VAL_num (VAL_int32 index); word 0; word 0; word 8] =
          Some (upd_s_mem store [after], [word (List.length (firstn 8 (get_blob_data blob)))])) in Read.
      rewrite Prefix in Read; rewrite Width in Read.
      apply Read; try assumption; try (rewrite Instance; reflexivity); try (rewrite Memory; reflexivity); lia.
    Qed.

    Lemma blob_read_result_functions world store arguments next result :
      blob_read_result world store arguments = Some (next, result) ->
      s_funcs next = s_funcs store.
    Proof.
      unfold blob_read_result; intro Read.
      repeat (cbv beta iota zeta delta [obind omap] in Read;
        match type of Read with context [match ?value with _ => _ end] => destruct value eqn:? end);
        try discriminate; inversion Read; subst.
      eapply supported_update_functions; eassumption.
    Qed.

    Lemma blob_read_result_tables world store arguments next result :
      blob_read_result world store arguments = Some (next, result) ->
      s_tables next = s_tables store.
    Proof.
      unfold blob_read_result; intro Read.
      repeat (cbv beta iota zeta delta [obind omap] in Read;
        match type of Read with context [match ?value with _ => _ end] => destruct value eqn:? end);
        try discriminate; inversion Read; subst.
      match goal with
      | Updated : supported_update _ _ _ = Some _ |- _ =>
        rewrite (supported_update_shape _ _ _ _ Updated); reflexivity
      end.
    Qed.

    Lemma blob_read_result_memory_shape world store arguments next result :
      blob_read_result world store arguments = Some (next, result) ->
      exists address before after,
        lookup_N (inst_mems instance) 0%N = Some address /\
        lookup_N (s_mems store) address = Some before /\
        meminst_type after = meminst_type before /\
        mem_length after = mem_length before /\
        s_mems next = replace_resource (s_mems store) address before after.
    Proof.
      unfold blob_read_result; intro Read.
      repeat (cbv beta iota zeta delta [obind omap] in Read;
        match type of Read with context [match ?value with _ => _ end] => destruct value eqn:? end);
        try discriminate; inversion Read; subst.
      match goal with
      | Copied : write_bytes_meminst ?before ?destination ?bytes = Some ?after,
        Updated : supported_update _ _ _ = Some _ |- _ =>
          destruct (blob_copy_shape _ _ _ _ Copied) as [TypeOf Length];
          eexists; exists before, after; repeat split; try eassumption;
          rewrite (supported_update_shape _ _ _ _ Updated); reflexivity
      end.
    Qed.


    Definition candidate world store operation arguments :=
      if shell_function_eq_dec operation shell_read_blob then
        omap (fun '(next, values) => (world, next, values)) (blob_read_result world store arguments)
      else omap (fun '(next, values) => (next, store, values)) (host_values world store operation arguments).

    Lemma candidate_correct world store operation arguments next updated values :
      candidate world store operation arguments = Some (next, updated, values) ->
      store_extension store updated = true /\ (store_typing store -> store_typing updated).
    Proof.
      unfold candidate; destruct (shell_function_eq_dec operation shell_read_blob).
      - destruct (blob_read_result world store arguments) as [[changed returned]|] eqn:Read;
          cbn [omap]; intro Step; try discriminate; inversion Step; subst.
        eapply blob_read_result_correct; exact Read.
      - destruct (host_values world store operation arguments) as [[changed returned]|];
          cbn [omap]; intro Step; try discriminate; inversion Step; subst.
        split; [apply store_extension_same|auto].
    Qed.

    Definition host_result world store signature operation arguments :=
      if function_type_eq_dec signature (shell_signature operation) then
        match candidate world store operation arguments with
        | Some (next, updated, values) =>
          match signature with Tf _ results =>
            if values_typing updated values results then (next, updated, result_values values)
            else (world, store, result_trap) end
        | None => (world, store, result_trap) end
      else (world, store, result_trap).

    Lemma host_values_result world store operation arguments next values :
      operation <> shell_read_blob ->
      host_values world store operation arguments = Some (next, values) ->
      host_result world store (shell_signature operation) operation arguments =
        (next, store, result_values values).
    Proof.
      intros Other Returned; pose proof (host_values_typing _ _ _ _ _ _ Returned) as Typed.
      unfold host_result, candidate.
      destruct (function_type_eq_dec _ _); [|contradiction].
      destruct (shell_function_eq_dec operation shell_read_blob); [contradiction|].
      rewrite Returned; cbn [omap].
      destruct (shell_signature operation); rewrite Typed; reflexivity.
    Qed.

    Lemma host_result_extension world store signature operation arguments next updated result :
      host_result world store signature operation arguments = (next, updated, result) ->
      store_extension store updated = true /\ (store_typing store -> store_typing updated).
    Proof.
      unfold host_result; destruct (function_type_eq_dec signature (shell_signature operation));
        [|intro Step; inversion Step; subst; split; [apply store_extension_same|auto]].
      destruct (candidate world store operation arguments) as [[[changed returned] values]|] eqn:Candidate;
        [|intro Step; inversion Step; subst; split; [apply store_extension_same|auto]].
      destruct signature as [parameters results]; destruct (values_typing returned values results);
        intro Step; inversion Step; subst.
      - eapply candidate_correct; exact Candidate.
      - split; [apply store_extension_same|auto].
    Qed.

    Lemma host_result_types world store parameters results operation arguments next updated result :
      host_result world store (Tf parameters results) operation arguments = (next, updated, result) ->
      result_types_agree updated results result = true.
    Proof.
      unfold host_result; destruct (function_type_eq_dec (Tf parameters results) (shell_signature operation));
        [|intro Step; inversion Step; reflexivity].
      destruct (candidate world store operation arguments) as [[[changed returned] values]|];
        [|intro Step; inversion Step; reflexivity].
      destruct (values_typing returned values results) eqn:Typed;
        intro Step; inversion Step; subst; [exact Typed|reflexivity].
    Qed.

    Lemma host_values_capacity world store operation arguments next values :
      within_capacity world -> host_values world store operation arguments = Some (next, values) ->
      within_capacity next.
    Proof.
      intros Within Returned; destruct operation;
        repeat (cbv beta iota zeta delta [host_values transform unchanged length_result tree_from_memory
          obind omap option_map] in Returned;
          match type of Returned with context [match ?value with _ => _ end] => destruct value eqn:? end);
        try discriminate;
        try (eapply allocate_capacity; [exact Within|exact Returned]);
        inversion Returned; subst; try exact Within.
      all: match goal with
      | Closed : release _ _ = Some _ |- _ => eapply release_capacity; eassumption
      end.
    Qed.

    Lemma host_result_capacity world store signature operation arguments next updated result :
      within_capacity world -> host_result world store signature operation arguments = (next, updated, result) ->
      within_capacity next.
    Proof.
      intros Within; unfold host_result.
      destruct (function_type_eq_dec signature (shell_signature operation));
        [|intro Returned; inversion Returned; subst; exact Within].
      destruct (candidate world store operation arguments) as [[[changed returned] values]|] eqn:Candidate;
        [|intro Returned; inversion Returned; subst; exact Within].
      destruct signature as [parameters results]; destruct (values_typing returned values results);
        intro Returned; inversion Returned; subst; try exact Within.
      unfold candidate in Candidate; destruct (shell_function_eq_dec operation shell_read_blob).
      - destruct (blob_read_result world store arguments) as [[read_store outcome]|];
          cbn [omap] in Candidate; inversion Candidate; subst; exact Within.
      - destruct (host_values world store operation arguments) as [[value_world outcome]|] eqn:Values;
          cbn [omap] in Candidate; inversion Candidate; subst.
        eapply host_values_capacity; eassumption.
    Qed.

    Definition host_application_impl world store signature operation arguments next output :=
      let '(changed, updated, result) := host_result world store signature operation arguments in
      next = changed /\ output = Some (updated, result).

    #[export] Instance shell_host : @host shell_host_functions mem.
    Proof.
      refine {| host_state := state; host_application := host_application_impl |}.
      - intros world signature store operation arguments next updated result Applied.
        unfold host_application_impl in Applied.
        destruct (host_result world store signature operation arguments) as [[changed returned] values] eqn:Step.
        destruct Applied as [Next Output]; inversion Output; subst.
        exact (proj1 (host_result_extension _ _ _ _ _ _ _ _ Step)).
      - intros world signature store operation arguments next updated result Applied Typed.
        unfold host_application_impl in Applied.
        destruct (host_result world store signature operation arguments) as [[changed returned] values] eqn:Step.
        destruct Applied as [Next Output]; inversion Output; subst.
        eapply (proj2 (host_result_extension _ _ _ _ _ _ _ _ Step)); exact Typed.
      - intros world parameters results store operation arguments next updated result Typed Applied.
        unfold host_application_impl in Applied.
        destruct (host_result world store (Tf parameters results) operation arguments) as [[changed returned] values] eqn:Step.
        destruct Applied as [Next Output]; inversion Output; subst.
        eapply host_result_types; exact Step.
    Defined.

  End Calls.
End ShellHost.
