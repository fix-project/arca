  (** R satisfies the structural laws and evaluator transport requirements. *)
  Lemma R_properties : relation_properties R.
  Proof.
    (* R′ already supports constructor congruence. Since R and R′ have now been shown to
        contain one another, those established congruence facts can be reused for R
        instead of reproving evaluation case by case. *)
    refine {| structural := {| blob_cong := blob_cong_R;
      tree_cong := tree_cong_R; tag_cong := tag_cong_R; tree_complete := tree_tagged_complete_R;
      blob_ref_cong := blob_ref_cong_R;
      tree_ref_cong := tree_ref_cong_R; blob_ref_complete := blob_ref_complete_R;
      tree_ref_complete := tree_ref_complete_R;
      preserve_tree_ref := R_preserve_tree_ref; preserve_blob_ref := R_preserve_blob_ref;
      preserve_tree := R_preserve_tree;
      preserve_blob := R_preserve_blob; preserve_thunk := R_preserve_thunk; X_self := RSelf |};
      preserve_tree_ref_rev := R_preserve_tree_ref_or_encode_rev;
      preserve_blob_ref_rev := R_preserve_blob_ref_or_encode_rev;
      preserve_tree_rev := R_preserve_tree_or_encode_rev;
      preserve_blob_rev := R_preserve_blob_or_encode_rev; encode_eval := R_encode_execute;
      execute_result := REvalStep;
      not_shallow_strict := R_not_shallow_strict; not_strict_shallow := R_not_strict_shallow;
      encode_reverse_absent := R_encode_execute_rev_does_not_exist |}.
    - intros t1 t2 Related; apply R'_impl_R, strict_encode_cong_R', R_to_R'; exact Related.
    - intros t1 t2 Related; apply R'_impl_R, shallow_encode_cong_R', R_to_R'; exact Related.
    - intros t1 t2 Related; apply R'_impl_R, application_thunk_cong_R', R_to_R'; exact Related.
    - intros t1 t2 Related; apply R'_impl_R, selection_thunk_cong_R', R_to_R'; exact Related.
    - intros d1 d2 Related; apply R'_impl_R, identification_thunk_cong_R', R_to_R'; exact Related.
    - intros t1 t2 Related; right; apply R_strict_encode_reasons; exact Related.
    - intros t1 t2 Related; right; apply R_shallow_encode_reasons; exact Related.
    - intros t1 t2 Related; unfold thunk_reasons; do 3 right; apply R_thunk_reasons; exact Related.
  Qed.
  (** One generating justification between inputs survives evaluation. This is the
      computational part of equivalence preservation: Closure.v needs only to join and
      reverse such local justifications afterward. *)
  Theorem eval_R h1 h2 : R h1 h2 -> rel_opt R (eval h1) (eval h2).
  Proof. apply (evals_X R R_properties). Qed.
  (** Related source Thunks agree on whether Force has a result, and successful results
      name related underlying Objects. The comparison is lifted so it remains usable by
      both strict and shallow consumers. *)
  Theorem force_R t1 t2 : R (Thunk t1) (Thunk t2) -> rel_opt (relaxed_X R) (force t1) (force t2).
  Proof. apply (forces_X R R_properties). Qed.
  (** A lowered handle cannot be an Object. *)
  Lemma lower_not_obj h h' : h = lower h' -> ~(exists o, h = Data (Object o)).
  Proof.
    intros -> [o E]; destruct h' as [[[b|t]|[b|t]]|th|[th|th]]; discriminate.
  Qed.
  (** A lifted handle cannot be a Ref. *)
  Lemma lift_not_ref h h' : h = lift h' -> ~(exists r, h = Data (Ref r)).
  Proof.
    intros -> [r E]; destruct h' as [[[b|t]|[b|t]]|th|[th|th]]; discriminate.
  Qed.
