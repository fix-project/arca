From Stdlib Require Import List Bool Arith Lia.
From FixProof Require Import Handle ApplyTree Coupon CouponApi.
Import ListNotations.

Module CouponConstructors (S : STORAGE) (P : PROGRAM S) (C : COUPON_STORAGE S).
  Module API := CouponApi S C.
  Module D := Coupon S P.
  Import D.EC.E.EP.E.H.

  (** Isabelle's proposition-valued Boolean becomes Prop. Constructor guards
      remain executable booleans; no decision procedure for coupon proofs is
      assumed. *)
  Definition relation_of (t : C.coupon_type) l r := match t with
    | C.Force => D.coupon_force l r | C.Storage => D.coupon_storage l r
    | C.Apply => D.coupon_apply l r | C.Slice => D.coupon_slice l r
    | C.Think => D.coupon_think l r | C.Eval => D.coupon_eval l r
    | C.Eq => D.coupon_eq l r end.
  Definition coupon_good c := match C.get_coupon_type c, C.get_coupon_lhs c, C.get_coupon_rhs c with
    | Some t, Some l, Some r => relation_of t l r | _, _, _ => False end.
  Definition read_coupon t c := if API.is_type t c then
    match C.get_coupon_lhs c, C.get_coupon_rhs c with
    | Some l, Some r => Some (l, r) | _, _ => None end else None.

  Lemma good_create t l r : relation_of t l r -> coupon_good (C.create_coupon t l r).
  Proof.
    unfold coupon_good; rewrite C.get_coupon_type_match, C.get_coupon_lhs_match, C.get_coupon_rhs_match; auto.
  Qed.
  Lemma good_type t c l r : coupon_good c -> C.get_coupon_type c = Some t ->
    C.get_coupon_lhs c = Some l -> C.get_coupon_rhs c = Some r -> relation_of t l r.
  Proof. unfold coupon_good; intros Good TypeTag Lhs Rhs; rewrite TypeTag, Lhs, Rhs in Good; exact Good. Qed.
  Lemma read_coupon_type t c l r : read_coupon t c = Some (l, r) ->
    C.get_coupon_type c = Some t /\ C.get_coupon_lhs c = Some l /\ C.get_coupon_rhs c = Some r.
  Proof.
    unfold read_coupon; destruct (API.is_type t c) eqn:TypeTag; [|discriminate].
    destruct (C.get_coupon_lhs c) as [l'|] eqn:Lhs, (C.get_coupon_rhs c) as [r'|] eqn:Rhs;
      try discriminate; intro Result; inversion Result; subst.
    split; [apply API.is_type_match; exact TypeTag|auto].
  Qed.
  Lemma read_coupon_good t c l r : coupon_good c -> read_coupon t c = Some (l, r) -> relation_of t l r.
  Proof.
    intros Good Read; destruct (read_coupon_type _ _ _ _ Read) as [TypeTag [Lhs Rhs]].
    eapply good_type; eassumption.
  Qed.
  Lemma type_lhs_exist t c : API.is_type t c = true -> exists l, C.get_coupon_lhs c = Some l.
  Proof.
    intro TypeTag; apply API.is_type_match in TypeTag.
    destruct (C.get_coupon_type_exists _ _ TypeTag) as [t' [l [r ->]]].
    exists l; apply C.get_coupon_lhs_match.
  Qed.
  Lemma type_rhs_exist t c : API.is_type t c = true -> exists r, C.get_coupon_rhs c = Some r.
  Proof.
    intro TypeTag; apply API.is_type_match in TypeTag.
    destruct (C.get_coupon_type_exists _ _ TypeTag) as [t' [l [r ->]]].
    exists r; apply C.get_coupon_rhs_match.
  Qed.
  Lemma eq_lhs_exist c : API.is_eq_coupon c = true -> exists l, C.get_coupon_lhs c = Some l.
  Proof. apply (type_lhs_exist C.Eq). Qed.
  Lemma eq_rhs_exist c : API.is_eq_coupon c = true -> exists r, C.get_coupon_rhs c = Some r.
  Proof. apply (type_rhs_exist C.Eq). Qed.
  Lemma eval_lhs_exist c : API.is_eval_coupon c = true -> exists l, C.get_coupon_lhs c = Some l.
  Proof. apply (type_lhs_exist C.Eval). Qed.
  Lemma eval_rhs_exist c : API.is_eval_coupon c = true -> exists r, C.get_coupon_rhs c = Some r.
  Proof. apply (type_rhs_exist C.Eval). Qed.
  Lemma think_lhs_exist c : API.is_think_coupon c = true -> exists l, C.get_coupon_lhs c = Some l.
  Proof. apply (type_lhs_exist C.Think). Qed.
  Lemma think_rhs_exist c : API.is_think_coupon c = true -> exists r, C.get_coupon_rhs c = Some r.
  Proof. apply (type_rhs_exist C.Think). Qed.
  Lemma apply_lhs_exist c : API.is_apply_coupon c = true -> exists l, C.get_coupon_lhs c = Some l.
  Proof. apply (type_lhs_exist C.Apply). Qed.
  Lemma apply_rhs_exist c : API.is_apply_coupon c = true -> exists r, C.get_coupon_rhs c = Some r.
  Proof. apply (type_rhs_exist C.Apply). Qed.
  Lemma force_lhs_exist c : API.is_force_coupon c = true -> exists l, C.get_coupon_lhs c = Some l.
  Proof. apply (type_lhs_exist C.Force). Qed.
  Lemma force_rhs_exist c : API.is_force_coupon c = true -> exists r, C.get_coupon_rhs c = Some r.
  Proof. apply (type_rhs_exist C.Force). Qed.
  Lemma good_eq c l r : coupon_good c -> API.is_eq_coupon c = true ->
    C.get_coupon_lhs c = Some l -> C.get_coupon_rhs c = Some r -> D.coupon_eq l r.
  Proof. intros Good TypeTag; apply (good_type C.Eq c l r Good); apply API.is_type_match; exact TypeTag. Qed.
  Lemma good_eval c l r : coupon_good c -> API.is_eval_coupon c = true ->
    C.get_coupon_lhs c = Some l -> C.get_coupon_rhs c = Some r -> D.coupon_eval l r.
  Proof. intros Good TypeTag; apply (good_type C.Eval c l r Good); apply API.is_type_match; exact TypeTag. Qed.
  Lemma good_apply c l r : coupon_good c -> API.is_apply_coupon c = true ->
    C.get_coupon_lhs c = Some l -> C.get_coupon_rhs c = Some r -> D.coupon_apply l r.
  Proof. intros Good TypeTag; apply (good_type C.Apply c l r Good); apply API.is_type_match; exact TypeTag. Qed.
  Lemma good_force c l r : coupon_good c -> API.is_force_coupon c = true ->
    C.get_coupon_lhs c = Some l -> C.get_coupon_rhs c = Some r -> D.coupon_force l r.
  Proof. intros Good TypeTag; apply (good_type C.Force c l r Good); apply API.is_type_match; exact TypeTag. Qed.

  Definition make_self_coupon (coupons : list handle) l r :=
    if API.is_equal l r then Some (C.create_coupon C.Eq l r) else None.
  Definition make_sym_coupon coupons l r := match coupons with
    | e :: _ => match read_coupon C.Eq e with
      | Some (el, er) => if API.is_equal er l && API.is_equal el r then Some (C.create_coupon C.Eq l r) else None
      | None => None end
    | nil => None end.
  Definition make_trans_coupon coupons l r := match coupons with
    | e1 :: e2 :: _ => match read_coupon C.Eq e1, read_coupon C.Eq e2 with
      | Some (e1l, e1r), Some (e2l, e2r) =>
        if API.is_equal e1r e2l && API.is_equal l e1l && API.is_equal r e2r
        then Some (C.create_coupon C.Eq l r) else None
      | _, _ => None end
    | _ => None end.
  Lemma make_self_coupon_good coupons l r c : make_self_coupon coupons l r = Some c -> coupon_good c.
  Proof.
    unfold make_self_coupon; destruct (API.is_equal l r) eqn:Check; [|discriminate].
    intro Result; inversion Result; subst c; apply API.is_equal_match in Check; subst r; apply good_create, D.CouponSelf.
  Qed.
  Lemma make_sym_coupon_good coupons l r c : Forall coupon_good coupons -> make_sym_coupon coupons l r = Some c -> coupon_good c.
  Proof.
    destruct coupons as [|e xs]; cbn [make_sym_coupon]; intros Goods Result; [discriminate|].
    destruct (read_coupon C.Eq e) as [[el er]|] eqn:Read; [|discriminate].
    destruct (API.is_equal er l && API.is_equal el r) eqn:Check; [|discriminate].
    inversion Result; subst c; apply andb_true_iff in Check as [Left Right].
    apply API.is_equal_match in Left; apply API.is_equal_match in Right; subst el er.
    apply good_create, D.CouponSym; exact (read_coupon_good _ _ _ _ (Forall_inv Goods) Read).
  Qed.
  Lemma make_trans_coupon_good coupons l r c : Forall coupon_good coupons -> make_trans_coupon coupons l r = Some c -> coupon_good c.
  Proof.
    destruct coupons as [|e1 [|e2 xs]]; cbn [make_trans_coupon]; intros Goods Result; try discriminate.
    destruct (read_coupon C.Eq e1) as [[e1l e1r]|] eqn:Read1,
      (read_coupon C.Eq e2) as [[e2l e2r]|] eqn:Read2; try discriminate.
    destruct (API.is_equal e1r e2l && API.is_equal l e1l && API.is_equal r e2r) eqn:Check; [|discriminate].
    inversion Result; subst c; apply andb_true_iff in Check as [Checks Right]; apply andb_true_iff in Checks as [Middle Left].
    apply API.is_equal_match in Middle; apply API.is_equal_match in Left; apply API.is_equal_match in Right; subst e1l e2l e2r.
    apply good_create; eapply D.CouponTrans.
    - exact (read_coupon_good _ _ _ _ (Forall_inv Goods) Read1).
    - exact (read_coupon_good _ _ _ _ (Forall_inv (Forall_inv_tail Goods)) Read2).
  Qed.
  Lemma application_api_some h v : API.create_application_thunk_api h = Some v ->
    exists t, h = HTreeObj t /\ v = Thunk (Application t).
  Proof.
    destruct h as [[[b|t]|ref]|th|e]; cbn [API.create_application_thunk_api]; try discriminate.
    intro Result; inversion Result; subst v; exists t; auto.
  Qed.
  Lemma strict_api_some h v : API.create_strict_encode_api h = Some v ->
    exists th, h = Thunk th /\ v = Encode (Strict th).
  Proof.
    destruct h as [d|th|e]; cbn [API.create_strict_encode_api]; try discriminate.
    intro Result; inversion Result; subst v; exists th; auto.
  Qed.
  Lemma shallow_api_some h v : API.create_shallow_encode_api h = Some v ->
    exists th, h = Thunk th /\ v = Encode (Shallow th).
  Proof.
    destruct h as [d|th|e]; cbn [API.create_shallow_encode_api]; try discriminate.
    intro Result; inversion Result; subst v; exists th; auto.
  Qed.
  Definition make_eq_mapped_coupon (convert : handle -> option handle) coupons l r := match coupons with
    | e :: _ => match read_coupon C.Eq e with
      | Some (el, er) => match convert el, convert er with
        | Some l', Some r' => if API.is_equal l l' && API.is_equal r r'
          then Some (C.create_coupon C.Eq l r) else None
        | _, _ => None end
      | None => None end
    | nil => None end.
  Definition make_eq_application_coupon := make_eq_mapped_coupon API.create_application_thunk_api.
  Definition make_eq_encode_strict_coupon := make_eq_mapped_coupon API.create_strict_encode_api.
  Definition make_eq_encode_shallow_coupon := make_eq_mapped_coupon API.create_shallow_encode_api.
  Lemma make_eq_mapped_coupon_good convert
      (Cong : forall h1 h2 v1 v2, D.coupon_eq h1 h2 -> convert h1 = Some v1 -> convert h2 = Some v2 -> D.coupon_eq v1 v2)
      coupons l r c : Forall coupon_good coupons -> make_eq_mapped_coupon convert coupons l r = Some c -> coupon_good c.
  Proof.
    destruct coupons as [|e xs]; cbn [make_eq_mapped_coupon]; intros Goods Result; [discriminate|].
    destruct (read_coupon C.Eq e) as [[el er]|] eqn:Read; [|discriminate].
    destruct (convert el) as [l'|] eqn:Lhs, (convert er) as [r'|] eqn:Rhs; try discriminate.
    destruct (API.is_equal l l' && API.is_equal r r') eqn:Check; [|discriminate].
    inversion Result; subst c; apply andb_true_iff in Check as [Left Right].
    apply API.is_equal_match in Left; apply API.is_equal_match in Right; subst l r.
    apply good_create; eapply Cong; [exact (read_coupon_good _ _ _ _ (Forall_inv Goods) Read)|exact Lhs|exact Rhs].
  Qed.
  Lemma make_eq_application_coupon_good coupons l r c : Forall coupon_good coupons ->
    make_eq_application_coupon coupons l r = Some c -> coupon_good c.
  Proof.
    apply make_eq_mapped_coupon_good; intros h1 h2 v1 v2 Related Left Right.
    destruct (application_api_some _ _ Left) as [t1 [-> ->]], (application_api_some _ _ Right) as [t2 [-> ->]].
    apply D.CouponEqApplication; exact Related.
  Qed.
  Lemma make_eq_encode_strict_coupon_good coupons l r c : Forall coupon_good coupons ->
    make_eq_encode_strict_coupon coupons l r = Some c -> coupon_good c.
  Proof.
    apply make_eq_mapped_coupon_good; intros h1 h2 v1 v2 Related Left Right.
    destruct (strict_api_some _ _ Left) as [th1 [-> ->]], (strict_api_some _ _ Right) as [th2 [-> ->]].
    apply D.CouponEqEncodeStrict; exact Related.
  Qed.
  Lemma make_eq_encode_shallow_coupon_good coupons l r c : Forall coupon_good coupons ->
    make_eq_encode_shallow_coupon coupons l r = Some c -> coupon_good c.
  Proof.
    apply make_eq_mapped_coupon_good; intros h1 h2 v1 v2 Related Left Right.
    destruct (shallow_api_some _ _ Left) as [th1 [-> ->]], (shallow_api_some _ _ Right) as [th2 [-> ->]].
    apply D.CouponEqEncodeShallow; exact Related.
  Qed.
  Definition make_eval_blob_coupon (coupons : list handle) l r :=
    if (get_type l =? 0) && API.is_equal l r then Some (C.create_coupon C.Eval l r) else None.
  Lemma make_eval_blob_coupon_good coupons l r c : make_eval_blob_coupon coupons l r = Some c -> coupon_good c.
  Proof.
    unfold make_eval_blob_coupon; destruct ((get_type l =? 0) && API.is_equal l r) eqn:Check; [|discriminate].
    intro Result; inversion Result; subst c; apply andb_true_iff in Check as [Tag Equal].
    apply Nat.eqb_eq in Tag; apply API.is_equal_match in Equal; subst r.
    destruct l as [[[b|t]|[b|t]]|th|e]; cbn [get_type] in Tag; try discriminate.
    apply good_create, D.CouponEvalBlobObj.
  Qed.
  Definition make_think_to_force_coupon coupons l r := match coupons with
    | t :: _ => match read_coupon C.Think t with
      | Some (tl, tr) => if ((get_type tr =? 0) || (get_type tr =? 1) || (get_type tr =? 2) || (get_type tr =? 3)) &&
          (API.is_equal tl l && API.is_equal tr r) then Some (C.create_coupon C.Force l r) else None
      | None => None end
    | nil => None end.
  Lemma make_think_to_force_coupon_good coupons l r c : Forall coupon_good coupons ->
    make_think_to_force_coupon coupons l r = Some c -> coupon_good c.
  Proof.
    destruct coupons as [|t xs]; cbn [make_think_to_force_coupon]; intros Goods Result; [discriminate|].
    destruct (read_coupon C.Think t) as [[tl tr]|] eqn:Read; [|discriminate].
    destruct (((get_type tr =? 0) || (get_type tr =? 1) || (get_type tr =? 2) || (get_type tr =? 3)) &&
      (API.is_equal tl l && API.is_equal tr r)) eqn:Check; [|discriminate].
    inversion Result; subst c; apply andb_true_iff in Check as [Tag Check]; apply andb_true_iff in Check as [Left Right].
    apply API.is_equal_match in Left; apply API.is_equal_match in Right; subst l r.
    pose proof (read_coupon_good _ _ _ _ (Forall_inv Goods) Read) as Think.
    destruct (D.coupon_think_sound _ _ Think) as [th [th' [Source Rest]]]; subst tl.
    destruct tr as [d|th''|e]; cbn [get_type] in Tag; try discriminate.
    apply good_create, D.CouponThinktoForce; exact Think.
  Qed.
  Definition make_force_to_encode_strict_coupon coupons l r := match coupons with
    | e :: _ => match read_coupon C.Force e with
      | Some (el, er) => match API.create_strict_encode_api el with
        | Some l' => if API.is_equal l' l && API.is_equal er r && ((get_type er =? 0) || (get_type er =? 1))
          then Some (C.create_coupon C.Eq l r) else None
        | None => None end
      | None => None end
    | nil => None end.
  Lemma make_force_to_encode_strict_coupon_good coupons l r c : Forall coupon_good coupons ->
    make_force_to_encode_strict_coupon coupons l r = Some c -> coupon_good c.
  Proof.
    destruct coupons as [|e xs]; cbn [make_force_to_encode_strict_coupon]; intros Goods Result; [discriminate|].
    destruct (read_coupon C.Force e) as [[el er]|] eqn:Read; [|discriminate].
    destruct (API.create_strict_encode_api el) as [l'|] eqn:Encode; [|discriminate].
    destruct (API.is_equal l' l && API.is_equal er r && ((get_type er =? 0) || (get_type er =? 1))) eqn:Check; [|discriminate].
    inversion Result; subst c; apply andb_true_iff in Check as [Checks Tag]; apply andb_true_iff in Checks as [Left Right].
    apply API.is_equal_match in Left; apply API.is_equal_match in Right; subst l r.
    destruct (strict_api_some _ _ Encode) as [th [-> ->]].
    pose proof (read_coupon_good _ _ _ _ (Forall_inv Goods) Read) as Force.
    assert (D.EC.E.EP.E.lift er = er) as Lift by (destruct er as [[[b|t]|[b|t]]|u|enc]; try reflexivity; discriminate Tag).
    apply good_create; rewrite <- Lift at 1; apply D.CouponForcetoEncodeStrict; exact Force.
  Qed.
  Definition make_force_result_eq_coupon coupons l r := match coupons with
    | f1 :: f2 :: e :: _ => match read_coupon C.Force f1, read_coupon C.Force f2, read_coupon C.Eq e with
      | Some (f1l, f1r), Some (f2l, f2r), Some (el, er) =>
        if API.is_equal f1r el && API.is_equal f2r er && API.is_equal f1l l && API.is_equal f2l r
        then Some (C.create_coupon C.Eq l r) else None
      | _, _, _ => None end
    | _ => None end.
  Lemma make_force_result_eq_coupon_good coupons l r c : Forall coupon_good coupons ->
    make_force_result_eq_coupon coupons l r = Some c -> coupon_good c.
  Proof.
    destruct coupons as [|f1 [|f2 [|e xs]]]; cbn [make_force_result_eq_coupon]; intros Goods Result; try discriminate.
    destruct (read_coupon C.Force f1) as [[f1l f1r]|] eqn:Read1,
      (read_coupon C.Force f2) as [[f2l f2r]|] eqn:Read2,
      (read_coupon C.Eq e) as [[el er]|] eqn:Read3; try discriminate.
    destruct (API.is_equal f1r el && API.is_equal f2r er && API.is_equal f1l l && API.is_equal f2l r) eqn:Check; [|discriminate].
    inversion Result; subst c; apply andb_true_iff in Check as [Checks Right]; apply andb_true_iff in Checks as [Checks Left];
      apply andb_true_iff in Checks as [First Second].
    apply API.is_equal_match in Right; apply API.is_equal_match in Left;
      apply API.is_equal_match in First; apply API.is_equal_match in Second; subst el er f1l f2l.
    apply good_create; eapply D.CouponForceResultEq.
    - exact (read_coupon_good _ _ _ _ (Forall_inv Goods) Read1).
    - exact (read_coupon_good _ _ _ _ (Forall_inv (Forall_inv_tail Goods)) Read2).
    - exact (read_coupon_good _ _ _ _ (Forall_inv (Forall_inv_tail (Forall_inv_tail Goods))) Read3).
  Qed.
  Definition make_think_application_coupon coupons l r := match coupons with
    | c1 :: c2 :: _ => match read_coupon C.Eval c1, read_coupon C.Apply c2 with
      | Some (evall, evalr), Some (applyl, applyr) => if API.is_equal evalr applyl then
        match API.create_application_thunk_api evall with
        | Some th => if API.is_equal th l && API.is_equal applyr r then Some (C.create_coupon C.Think l r) else None
        | None => None end else None
      | _, _ => None end
    | _ => None end.
  Lemma make_think_application_coupon_good coupons l r c : Forall coupon_good coupons ->
    make_think_application_coupon coupons l r = Some c -> coupon_good c.
  Proof.
    destruct coupons as [|c1 [|c2 xs]]; cbn [make_think_application_coupon]; intros Goods Result; try discriminate.
    destruct (read_coupon C.Eval c1) as [[evall evalr]|] eqn:Read1,
      (read_coupon C.Apply c2) as [[applyl applyr]|] eqn:Read2; try discriminate.
    destruct (API.is_equal evalr applyl) eqn:Middle; [|discriminate].
    destruct (API.create_application_thunk_api evall) as [th|] eqn:Create; [|discriminate].
    destruct (API.is_equal th l && API.is_equal applyr r) eqn:Check; [|discriminate].
    inversion Result; subst c; apply andb_true_iff in Check as [Left Right].
    apply API.is_equal_match in Middle; apply API.is_equal_match in Left; apply API.is_equal_match in Right; subst applyl l r.
    pose proof (read_coupon_good _ _ _ _ (Forall_inv Goods) Read1) as Eval;
      pose proof (read_coupon_good _ _ _ _ (Forall_inv (Forall_inv_tail Goods)) Read2) as Apply.
    destruct (application_api_some _ _ Create) as [t1 [-> ->]].
    destruct (D.coupon_apply_sound _ _ Apply) as [t2 [-> App]].
    apply good_create; eapply D.CouponThinkApplication; [exact Eval|exact Apply].
  Qed.
  Definition make_eval_eq_coupon coupons l r := match coupons with
    | c1 :: c2 :: _ => match read_coupon C.Eval c1, read_coupon C.Eq c2 with
      | Some (evall, evalr), Some (eql, eqr) =>
        if API.is_equal evall eql && API.is_equal eqr l && API.is_equal evalr r
        then Some (C.create_coupon C.Eval l r) else None
      | _, _ => None end
    | _ => None end.
  Lemma make_eval_eq_coupon_good coupons l r c : Forall coupon_good coupons ->
    make_eval_eq_coupon coupons l r = Some c -> coupon_good c.
  Proof.
    destruct coupons as [|c1 [|c2 xs]]; cbn [make_eval_eq_coupon]; intros Goods Result; try discriminate.
    destruct (read_coupon C.Eval c1) as [[evall evalr]|] eqn:Read1,
      (read_coupon C.Eq c2) as [[eql eqr]|] eqn:Read2; try discriminate.
    destruct (API.is_equal evall eql && API.is_equal eqr l && API.is_equal evalr r) eqn:Check; [|discriminate].
    inversion Result; subst c; apply andb_true_iff in Check as [Checks Right]; apply andb_true_iff in Checks as [Middle Left].
    apply API.is_equal_match in Middle; apply API.is_equal_match in Left; apply API.is_equal_match in Right; subst eql l r.
    apply good_create; eapply D.CouponEvalEq.
    - exact (read_coupon_good _ _ _ _ (Forall_inv Goods) Read1).
    - exact (read_coupon_good _ _ _ _ (Forall_inv (Forall_inv_tail Goods)) Read2).
  Qed.
  Definition has_tree_size h n := match API.get_tree_size_api h with
    | Some m => m =? n | None => false end.
  Definition tree_entry_match coupons l r i := match nth_error coupons i,
      API.get_tree_data_api l i, API.get_tree_data_api r i with
    | Some c, Some li, Some ri => match C.get_coupon_lhs c, C.get_coupon_rhs c with
      | Some cl, Some cr => API.is_equal li cl && API.is_equal ri cr | _, _ => false end
    | _, _, _ => false end.
  Definition make_tree_coupon tag coupons l r :=
    if forallb (API.is_type tag) coupons then
      if has_tree_size l (length coupons) && has_tree_size r (length coupons) then
        if forallb (tree_entry_match coupons l r) (seq 0 (length coupons))
          then Some (C.create_coupon tag l r) else None
      else None else None.
  Definition make_eq_tree_coupon := make_tree_coupon C.Eq.
  Definition make_eval_tree_coupon := make_tree_coupon C.Eval.
  Lemma has_tree_size_some h n : has_tree_size h n = true -> exists t, h = HTreeObj t /\ get_tree_size t = n.
  Proof.
    destruct h as [[[b|t]|ref]|th|e]; cbn [has_tree_size API.get_tree_size_api]; try discriminate.
    intro Size; apply Nat.eqb_eq in Size; exists t; split; [reflexivity|exact Size].
  Qed.
  Lemma tree_api_in_bounds t i : i < get_tree_size t -> API.get_tree_data_api (HTreeObj t) i = Some (get_tree_data t i).
  Proof.
    intro Bound; unfold API.get_tree_data_api, get_tree_data.
    apply nth_error_nth'; exact Bound.
  Qed.
  Lemma tree_entry_match_good tag coupons t1 t2 i : Forall coupon_good coupons ->
    forallb (API.is_type tag) coupons = true -> i < length coupons ->
    i < get_tree_size t1 -> i < get_tree_size t2 ->
    tree_entry_match coupons (HTreeObj t1) (HTreeObj t2) i = true ->
    relation_of tag (get_tree_data t1 i) (get_tree_data t2 i).
  Proof.
    intros Goods Types Bound B1 B2 Entry; unfold tree_entry_match in Entry;
      rewrite (tree_api_in_bounds _ _ B1), (tree_api_in_bounds _ _ B2) in Entry.
    destruct (nth_error coupons i) as [c|] eqn:Input; [|discriminate].
    destruct (C.get_coupon_lhs c) as [cl|] eqn:Lhs, (C.get_coupon_rhs c) as [cr|] eqn:Rhs; try discriminate.
    apply andb_true_iff in Entry as [Left Right]; apply API.is_equal_match in Left; apply API.is_equal_match in Right; subst cl cr.
    assert (In c coupons) as Member by (eapply nth_error_In; exact Input).
    rewrite Forall_forall in Goods; rewrite forallb_forall in Types.
    eapply good_type; [apply Goods; exact Member|apply API.is_type_match; apply Types; exact Member|exact Lhs|exact Rhs].
  Qed.
  Lemma Forall2_nth_bounds (Rel : handle -> handle -> Prop) xs ys da db : length xs = length ys ->
    (forall i, i < length xs -> Rel (nth i xs da) (nth i ys db)) -> Forall2 Rel xs ys.
  Proof.
    revert ys; induction xs as [|x xs IH]; intros [|y ys] Size Entries; try discriminate; [constructor|].
    constructor.
    - exact (Entries 0 ltac:(cbn; lia)).
    - apply IH; [cbn in Size; lia|].
      intros i Bound; exact (Entries (S i) ltac:(cbn; lia)).
  Qed.
  Lemma make_tree_coupon_good tag
      (Cong : forall t1 t2, Forall2 (relation_of tag) (get_tree_raw t1) (get_tree_raw t2) -> relation_of tag (HTreeObj t1) (HTreeObj t2))
      coupons l r c : Forall coupon_good coupons -> make_tree_coupon tag coupons l r = Some c -> coupon_good c.
  Proof.
    unfold make_tree_coupon; intros Goods Result.
    destruct (forallb (API.is_type tag) coupons) eqn:Types; [|discriminate].
    destruct (has_tree_size l (length coupons) && has_tree_size r (length coupons)) eqn:Sizes; [|discriminate].
    destruct (forallb (tree_entry_match coupons l r) (seq 0 (length coupons))) eqn:Entries; [|discriminate].
    inversion Result; subst c; apply andb_true_iff in Sizes as [SizeL SizeR].
    destruct (has_tree_size_some _ _ SizeL) as [t1 [-> Size1]], (has_tree_size_some _ _ SizeR) as [t2 [-> Size2]].
    apply good_create, Cong.
    apply (Forall2_nth_bounds _ _ _ (HBlobObj (create_blob nil)) (HBlobObj (create_blob nil))).
    - change (get_tree_size t1 = get_tree_size t2); congruence.
    - intros i Bound; change (relation_of tag (get_tree_data t1 i) (get_tree_data t2 i)).
      assert (i < length coupons) as InputBound by (change (i < get_tree_size t1) in Bound; lia).
      apply (tree_entry_match_good tag coupons t1 t2 i Goods Types InputBound).
      + exact Bound.
      + lia.
      + rewrite forallb_forall in Entries; apply Entries, in_seq; lia.
  Qed.
  Lemma make_eq_tree_coupon_good coupons l r c : Forall coupon_good coupons -> make_eq_tree_coupon coupons l r = Some c -> coupon_good c.
  Proof. apply make_tree_coupon_good; intros t1 t2 Related; apply D.CouponTreeEq; exact Related. Qed.
  Lemma make_eval_tree_coupon_good coupons l r c : Forall coupon_good coupons -> make_eval_tree_coupon coupons l r = Some c -> coupon_good c.
  Proof. apply make_tree_coupon_good; intros t1 t2 Related; apply D.CouponEvalTreeObj; exact Related. Qed.

  Inductive request := TreeEq | ForceResultEq | ThinkApplication | ThinkToForce | EqApplication |
    EvalBlobObj | EvalTreeObj | ForceToEncodeStrict | EvalEq | EqEncodeStrict | Sym | Trans | Self.
  Definition make_coupon req coupons l r := match req with
    | TreeEq => make_eq_tree_coupon coupons l r | ForceResultEq => make_force_result_eq_coupon coupons l r
    | ThinkApplication => make_think_application_coupon coupons l r | ThinkToForce => make_think_to_force_coupon coupons l r
    | EqApplication => make_eq_application_coupon coupons l r | EvalBlobObj => make_eval_blob_coupon coupons l r
    | EvalTreeObj => make_eval_tree_coupon coupons l r | ForceToEncodeStrict => make_force_to_encode_strict_coupon coupons l r
    | EvalEq => make_eval_eq_coupon coupons l r | EqEncodeStrict => make_eq_encode_strict_coupon coupons l r
    | Sym => make_sym_coupon coupons l r | Trans => make_trans_coupon coupons l r | Self => make_self_coupon coupons l r end.
  Theorem make_coupon_good req coupons l r c : Forall coupon_good coupons -> make_coupon req coupons l r = Some c -> coupon_good c.
  Proof.
    intros Goods Result; destruct req; cbn [make_coupon] in Result;
      eauto using make_eq_tree_coupon_good, make_force_result_eq_coupon_good, make_think_application_coupon_good,
        make_think_to_force_coupon_good, make_eq_application_coupon_good, make_eval_blob_coupon_good, make_eval_tree_coupon_good,
        make_force_to_encode_strict_coupon_good, make_eval_eq_coupon_good, make_eq_encode_strict_coupon_good,
        make_sym_coupon_good, make_trans_coupon_good, make_self_coupon_good.
  Qed.
End CouponConstructors.
