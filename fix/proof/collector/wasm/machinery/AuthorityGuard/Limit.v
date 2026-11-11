  Lemma witness_limit_comparison count : (Z.of_nat count < 2 ^ 32)%Z ->
    app_relop (Relop_i (ROI_le SX_U)) (VAL_int32 (wasm_nat count))
      (VAL_int32 (Wasm_int.int_of_Z i32m 1048576)) = Z.leb (Z.of_nat count) witness_limit.
  Proof.
    intro Bound.
    change (negb (Wasm_int.Int32.ltu (Wasm_int.Int32.repr 1048576) (wasm_nat count)) =
      Z.leb (Z.of_nat count) witness_limit).
    unfold Wasm_int.Int32.ltu.
    rewrite wasm_nat_unsigned by exact Bound.
    rewrite Wasm_int.Int32.unsigned_repr by (vm_compute; intuition congruence).
    unfold witness_limit.
    destruct (Coqlib.zlt 1048576 (Z.of_nat count));
      destruct (Z.leb (Z.of_nat count) 1048576) eqn:Compared;
      try reflexivity; first [apply Z.leb_le in Compared|apply Z.leb_gt in Compared]; lia.
  Qed.

  Lemma witness_limit_run state store locals count : (Z.of_nat count < 2 ^ 32)%Z ->
    reduce_trans
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [word count; VAL_num (VAL_int32 (Wasm_int.int_of_Z i32m 1048576))] ++
          [AI_basic (BI_relop T_i32 (Relop_i (ROI_le SX_U)))])
      (state, store, Build_frame locals G.E.collector_instance,
        v_to_e_list [VAL_num (VAL_int32 (wasm_bool (Z.leb (Z.of_nat count) witness_limit))) ]).
  Proof.
    intro Bound; pose proof (native_relop state store locals (VAL_int32 (wasm_nat count))
      (VAL_int32 (Wasm_int.int_of_Z i32m 1048576)) T_i32 (Relop_i (ROI_le SX_U)) eq_refl) as Run.
    rewrite witness_limit_comparison in Run by exact Bound; exact Run.
  Qed.
