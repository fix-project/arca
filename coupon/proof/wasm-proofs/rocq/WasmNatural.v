From Stdlib Require Import List Arith NArith ZArith Lia.
From Wasm Require Import numerics datatypes operations opsem.
Import ListNotations.

(** Natural counters are represented by Wasm words. The bounds are explicit
    premises, replacing the original global tree/coupon size assumptions. *)
Definition wasm_nat n := Wasm_int.int_of_Z i32m (Z.of_nat n).

Lemma wasm_nat_unsigned n : (Z.of_nat n < 2 ^ 32)%Z ->
  Wasm_int.Int32.unsigned (wasm_nat n) = Z.of_nat n.
Proof.
  intro Bound; apply Wasm_int.Int32.unsigned_repr.
  change (0 <= Z.of_nat n <= 4294967295)%Z; cbn in Bound; lia.
Qed.

Lemma wasm_nat_signed n : (Z.of_nat n < 2 ^ 31)%Z ->
  Wasm_int.Int32.signed (wasm_nat n) = Z.of_nat n.
Proof.
  intro Bound; apply Wasm_int.Int32.signed_repr.
  change (-2147483648 <= Z.of_nat n <= 2147483647)%Z; cbn in Bound; lia.
Qed.

Lemma wasm_nat_to_nat n : (Z.of_nat n < 2 ^ 32)%Z ->
  Wasm_int.nat_of_uint i32m (wasm_nat n) = n.
Proof.
  intro Bound; change (Z.to_nat (Wasm_int.Int32.unsigned (wasm_nat n)) = n).
  rewrite (wasm_nat_unsigned n Bound); apply Nat2Z.id.
Qed.

Lemma wasm_nat_to_N n : (Z.of_nat n < 2 ^ 32)%Z ->
  Wasm_int.N_of_uint i32m (wasm_nat n) = N.of_nat n.
Proof.
  intro Bound; change (Z.to_N (Wasm_int.Int32.unsigned (wasm_nat n)) = N.of_nat n).
  rewrite (wasm_nat_unsigned n Bound); apply N2Z.inj.
  rewrite Z2N.id by lia; symmetry; apply nat_N_Z.
Qed.

Lemma wasm_nat_equal n m : (Z.of_nat n < 2 ^ 32)%Z -> (Z.of_nat m < 2 ^ 32)%Z ->
  Wasm_int.int_eq i32m (wasm_nat n) (wasm_nat m) = Nat.eqb n m.
Proof.
  intros NBound MBound; change (Wasm_int.Int32.eq (wasm_nat n) (wasm_nat m) = Nat.eqb n m).
  unfold Wasm_int.Int32.eq; rewrite (wasm_nat_unsigned n NBound), (wasm_nat_unsigned m MBound).
  destruct (Coqlib.zeq (Z.of_nat n) (Z.of_nat m)) as [Equal|Different].
  - symmetry; apply Nat.eqb_eq; lia.
  - symmetry; apply Nat.eqb_neq; lia.
Qed.

Lemma wasm_nat_lt_unsigned n m : (Z.of_nat n < 2 ^ 32)%Z -> (Z.of_nat m < 2 ^ 32)%Z ->
  Wasm_int.int_lt_u i32m (wasm_nat n) (wasm_nat m) = Nat.ltb n m.
Proof.
  intros NBound MBound; change (Wasm_int.Int32.ltu (wasm_nat n) (wasm_nat m) = Nat.ltb n m).
  unfold Wasm_int.Int32.ltu; rewrite (wasm_nat_unsigned n NBound), (wasm_nat_unsigned m MBound).
  destruct (Coqlib.zlt (Z.of_nat n) (Z.of_nat m)) as [Less|NotLess].
  - symmetry; apply Nat.ltb_lt; lia.
  - symmetry; apply Nat.ltb_ge; lia.
Qed.

Lemma wasm_nat_ge_signed n m : (Z.of_nat n < 2 ^ 31)%Z -> (Z.of_nat m < 2 ^ 31)%Z ->
  Wasm_int.int_ge_s i32m (wasm_nat n) (wasm_nat m) = negb (Nat.ltb n m).
Proof.
  intros NBound MBound; change (negb (Wasm_int.Int32.lt (wasm_nat n) (wasm_nat m)) = negb (Nat.ltb n m)).
  f_equal; unfold Wasm_int.Int32.lt; rewrite (wasm_nat_signed n NBound), (wasm_nat_signed m MBound).
  destruct (Coqlib.zlt (Z.of_nat n) (Z.of_nat m)) as [Less|NotLess].
  - symmetry; apply Nat.ltb_lt; lia.
  - symmetry; apply Nat.ltb_ge; lia.
Qed.

Lemma wasm_nat_increment n : (Z.of_nat n < 2 ^ 32)%Z ->
  Wasm_int.int_add i32m (wasm_nat n) (wasm_nat 1) = wasm_nat (S n).
Proof.
  intro Bound; change (Wasm_int.Int32.add (wasm_nat n) (wasm_nat 1) = wasm_nat (S n)).
  rewrite Wasm_int.Int32.add_unsigned, (wasm_nat_unsigned n Bound).
  rewrite (wasm_nat_unsigned 1 ltac:(cbn; lia)).
  change (Wasm_int.Int32.repr (Z.of_nat n + 1) = Wasm_int.Int32.repr (Z.of_nat (S n))).
  f_equal; lia.
Qed.

Lemma wasm_nat_ge_signed_stop n : (Z.of_nat n < 2 ^ 31)%Z ->
  Wasm_int.int_ge_s i32m (wasm_nat n) (wasm_nat n) = true.
Proof. intro Bound; rewrite (wasm_nat_ge_signed n n Bound Bound), Nat.ltb_irrefl; reflexivity. Qed.

Lemma wasm_nat_ge_signed_continue i n : (i < n)%nat -> (Z.of_nat n < 2 ^ 31)%Z ->
  Wasm_int.int_ge_s i32m (wasm_nat i) (wasm_nat n) = false.
Proof.
  intros Less Bound; rewrite wasm_nat_ge_signed by lia.
  assert (Nat.ltb i n = true) as Test by (apply Nat.ltb_lt; exact Less).
  rewrite Test; reflexivity.
Qed.

Lemma wasm_nat_ge_signed_step i n : (Z.of_nat i < 2 ^ 31)%Z -> (Z.of_nat n < 2 ^ 31)%Z ->
  reduce_simple
    [v_to_e (VAL_num (VAL_int32 (wasm_nat i))); v_to_e (VAL_num (VAL_int32 (wasm_nat n)));
     AI_basic (BI_relop T_i32 (Relop_i (ROI_ge SX_S)))]
    [v_to_e (VAL_num (VAL_int32 (wasm_bool (negb (Nat.ltb i n)))))].
Proof.
  intros IBound NBound; rewrite <- (wasm_nat_ge_signed i n IBound NBound).
  apply rs_relop; reflexivity.
Qed.

Lemma wasm_nat_increment_step i : (Z.of_nat i < 2 ^ 32)%Z ->
  reduce_simple
    [v_to_e (VAL_num (VAL_int32 (wasm_nat i))); v_to_e (VAL_num (VAL_int32 (wasm_nat 1)));
     AI_basic (BI_binop T_i32 (Binop_i BOI_add))]
    [v_to_e (VAL_num (VAL_int32 (wasm_nat (S i))))].
Proof.
  intro Bound; apply rs_binop_success; [reflexivity|].
  change (Some (VAL_int32 (Wasm_int.int_add i32m (wasm_nat i) (wasm_nat 1))) = Some (VAL_int32 (wasm_nat (S i)))).
  rewrite (wasm_nat_increment i Bound); reflexivity.
Qed.
