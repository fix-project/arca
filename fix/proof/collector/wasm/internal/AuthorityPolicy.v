Module WasmAuthorityPolicy.
Import ListNotations.
Local Open Scope list_scope.
  Import C S.
  Module Authority := WasmAuthorityValidation.
  Local Notation treeh := G.E.H.H.HTreeObj.

  Lemma authorized_policy collector witnesses :
    Forall (Authority.authorized collector) witnesses -> X.reconstruction_authority collector witnesses = true.
  Proof.
    intro Authorized; unfold X.reconstruction_authority; rewrite forallb_forall.
    rewrite Forall_forall in Authorized; intros witness Member.
    destruct (Authorized witness Member) as [bundle [tag [claim [author [Witness [Bundle [Tag [Tagged [Arity Allowed]]]]]]]]]; subst witness.
    unfold X.authenticated_by, X.Wire.author; cbn [G.E.H.H.HTreeObj]; rewrite Bundle; cbn [G.E.H.H.HTreeObj]; rewrite Tagged,Tag.
    destruct (get_tree_raw claim) as [|code [|lhs [|rhs [|extra rest]]]] eqn:Fields;
      cbn [List.length] in Arity; try discriminate.
    rewrite X.Wire.API.handle_eqb_refl.
    destruct Allowed as [Same|Kernel].
    - subst author; rewrite X.API.handle_eqb_refl; reflexivity.
    - subst author; rewrite X.API.handle_eqb_refl; apply Bool.orb_true_r.
  Qed.
End WasmAuthorityPolicy.
