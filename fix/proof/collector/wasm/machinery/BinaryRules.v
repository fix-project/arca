Module WasmBinaryRules.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Base := WasmRules.
  Import Base Base.Support.
  #[local] Existing Instance C.mem.
  #[local] Existing Instance C.collector_host.
  Definition at_observation state offset := G.E.H.State (G.E.H.slots state) (G.E.H.observations state + offset).
  Lemma ready_at before after offset left right :
    G.E.H.observations after = G.E.H.observations before + offset ->
    observed_same (at_observation before offset) left right -> observed_same after left right.
  Proof.
    intros Same Ready; eapply observed_comparison_observations; [|exact Ready]; exact Same.
  Qed.

  Inductive substitution_rule := Transitive | EvalSubstitution | ForceSubstitution.
  Definition substitution_code kind := match kind with Transitive => 11 | EvalSubstitution => 8 | ForceSubstitution => 24 end.
  Definition substitution_tail kind := match kind with Transitive => 122 | EvalSubstitution => 118 | ForceSubstitution => 130 end.
  Definition substitution_first_kind kind := match kind with EvalSubstitution => 1 | _ => 0 end.
  Definition substitution_second_kind kind := match kind with ForceSubstitution => 3 | _ => 0 end.
  Definition substitution_result kind := match kind with Transitive => 0 | EvalSubstitution => 1 | ForceSubstitution => 3 end.
  Definition substitution_left kind (lhs pivot : handle) := match kind with Transitive => lhs | _ => pivot end.
  Definition substitution_right kind (lhs rhs pivot : handle) := match kind with
    Transitive => pivot | EvalSubstitution => rhs | ForceSubstitution => lhs end.
  Definition substitution_last kind (lhs rhs : handle) := match kind with EvalSubstitution => lhs | _ => rhs end.
  Definition substitution_left_slot kind (lhs a : Wasm_int.Int32.int) := match kind with Transitive => lhs | _ => a end.
  Definition substitution_right_slot kind (lhs rhs b : Wasm_int.Int32.int) := match kind with
    Transitive => b | EvalSubstitution => rhs | ForceSubstitution => lhs end.
  Definition substitution_pivot_slot kind (a b : Wasm_int.Int32.int) := match kind with Transitive => b | _ => a end.
  Definition substitution_last_slot kind (lhs rhs : Wasm_int.Int32.int) := match kind with EvalSubstitution => lhs | _ => rhs end.

End WasmBinaryRules.
