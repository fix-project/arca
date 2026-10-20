(** Relates natural-number counters to Wasm words, with explicit bounds
    for arithmetic and indexing. *)

From Stdlib Require Import List Arith NArith ZArith Lia.
From Wasm Require Import numerics datatypes operations opsem.
Import ListNotations.

Definition wasm_nat n := Wasm_int.int_of_Z i32m (Z.of_nat n).

(** An in-range natural number encoded as a Wasm i32 has that unsigned value. *)
Lemma wasm_nat_unsigned n : (Z.of_nat n < 2 ^ 32)%Z ->
  Wasm_int.Int32.unsigned (wasm_nat n) = Z.of_nat n.
Proof.
  intro Bound; apply Wasm_int.Int32.unsigned_repr.
  change (0 <= Z.of_nat n <= 4294967295)%Z; cbn in Bound; lia.
Qed.

(** A natural number below the signed i32 bound has that signed Wasm value. *)
Lemma wasm_nat_signed n : (Z.of_nat n < 2 ^ 31)%Z ->
  Wasm_int.Int32.signed (wasm_nat n) = Z.of_nat n.
Proof.
  intro Bound; apply Wasm_int.Int32.signed_repr.
  change (-2147483648 <= Z.of_nat n <= 2147483647)%Z; cbn in Bound; lia.
Qed.

(** Converting an in-range encoded Wasm i32 back to a natural number recovers it. *)
Lemma wasm_nat_to_nat n : (Z.of_nat n < 2 ^ 32)%Z ->
  Wasm_int.nat_of_uint i32m (wasm_nat n) = n.
Proof.
  intro Bound; change (Z.to_nat (Wasm_int.Int32.unsigned (wasm_nat n)) = n).
  rewrite (wasm_nat_unsigned n Bound); apply Nat2Z.id.
Qed.

(** Converting an in-range encoded Wasm i32 to a binary natural recovers it. *)
Lemma wasm_nat_to_N n : (Z.of_nat n < 2 ^ 32)%Z ->
  Wasm_int.N_of_uint i32m (wasm_nat n) = N.of_nat n.
Proof.
  intro Bound; change (Z.to_N (Wasm_int.Int32.unsigned (wasm_nat n)) = N.of_nat n).
  rewrite (wasm_nat_unsigned n Bound); apply N2Z.inj.
  rewrite Z2N.id by lia; symmetry; apply nat_N_Z.
Qed.

(** Wasm equality agrees with equality of natural numbers within the stated i32 bounds. *)
Lemma wasm_nat_equal n m : (Z.of_nat n < 2 ^ 32)%Z -> (Z.of_nat m < 2 ^ 32)%Z ->
  Wasm_int.int_eq i32m (wasm_nat n) (wasm_nat m) = Nat.eqb n m.
Proof.
  intros NBound MBound; change (Wasm_int.Int32.eq (wasm_nat n) (wasm_nat m) = Nat.eqb n m).
  unfold Wasm_int.Int32.eq; rewrite (wasm_nat_unsigned n NBound), (wasm_nat_unsigned m MBound).
  destruct (Coqlib.zeq (Z.of_nat n) (Z.of_nat m)) as [Equal|Different].
  - symmetry; apply Nat.eqb_eq; lia.
  - symmetry; apply Nat.eqb_neq; lia.
Qed.

(** Signed Wasm comparison agrees with natural-number comparison within the signed i32
    bounds. *)
Lemma wasm_nat_ge_signed n m : (Z.of_nat n < 2 ^ 31)%Z -> (Z.of_nat m < 2 ^ 31)%Z ->
  Wasm_int.int_ge_s i32m (wasm_nat n) (wasm_nat m) = negb (Nat.ltb n m).
Proof.
  intros NBound MBound; change (negb (Wasm_int.Int32.lt (wasm_nat n) (wasm_nat m)) = negb (Nat.ltb n m)).
  f_equal; unfold Wasm_int.Int32.lt; rewrite (wasm_nat_signed n NBound), (wasm_nat_signed m MBound).
  destruct (Coqlib.zlt (Z.of_nat n) (Z.of_nat m)) as [Less|NotLess].
  - symmetry; apply Nat.ltb_lt; lia.
  - symmetry; apply Nat.ltb_ge; lia.
Qed.

(** Wasm addition by one agrees with natural-number increment within the stated bound. *)
Lemma wasm_nat_increment n : (Z.of_nat n < 2 ^ 32)%Z ->
  Wasm_int.int_add i32m (wasm_nat n) (wasm_nat 1) = wasm_nat (S n).
Proof.
  intro Bound; change (Wasm_int.Int32.add (wasm_nat n) (wasm_nat 1) = wasm_nat (S n)).
  rewrite Wasm_int.Int32.add_unsigned, (wasm_nat_unsigned n Bound).
  rewrite (wasm_nat_unsigned 1 ltac:(cbn; lia)).
  change (Wasm_int.Int32.repr (Z.of_nat n + 1) = Wasm_int.Int32.repr (Z.of_nat (S n))).
  f_equal; lia.
Qed.

(** The signed loop-stop comparison agrees with the bounded natural-number stop condition. *)
Lemma wasm_nat_ge_signed_stop n : (Z.of_nat n < 2 ^ 31)%Z ->
  Wasm_int.int_ge_s i32m (wasm_nat n) (wasm_nat n) = true.
Proof. intro Bound; rewrite (wasm_nat_ge_signed n n Bound Bound), Nat.ltb_irrefl; reflexivity. Qed.

(** A bounded Wasm counter increment yields the encoding of the next natural number. *)
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

Definition wasm_nat64 n := Wasm_int.int_of_Z i64m (Z.of_nat n).

(** An in-range natural number encoded as a Wasm i64 has that unsigned value. *)
Lemma wasm_nat64_unsigned n : (Z.of_nat n < 2 ^ 64)%Z ->
  Wasm_int.Int64.unsigned (wasm_nat64 n) = Z.of_nat n.
Proof.
  intro Bound; apply Wasm_int.Int64.unsigned_repr.
  change (0 <= Z.of_nat n <= 18446744073709551615)%Z; cbn in Bound; lia.
Qed.

(** The modeled i64-to-i32 range test agrees with the unsigned 32-bit bound. *)
Lemma wasm_nat64_fits32 n : (Z.of_nat n < 2 ^ 64)%Z ->
  app_relop (Relop_i (ROI_le SX_U)) (VAL_int64 (wasm_nat64 n))
    (VAL_int64 (Wasm_int.int_of_Z i64m 4294967295%Z)) = Z.ltb (Z.of_nat n) (2 ^ 32)%Z.
Proof.
  intro Bound.
  change (negb (Wasm_int.Int64.ltu (Wasm_int.Int64.repr 4294967295) (wasm_nat64 n)) =
    Z.ltb (Z.of_nat n) (2 ^ 32)%Z).
  unfold Wasm_int.Int64.ltu.
  rewrite (wasm_nat64_unsigned n Bound).
  rewrite Wasm_int.Int64.unsigned_repr by (vm_compute; intuition congruence).
  destruct (Coqlib.zlt 4294967295 (Z.of_nat n)); destruct (Z.ltb (Z.of_nat n) (2 ^ 32)) eqn:Fits;
    try reflexivity; first [apply Z.ltb_lt in Fits|apply Z.ltb_ge in Fits]; cbn in Fits; lia.
Qed.

(** Wrapping an encoded unsigned i64 to i32 gives the same natural number's i32 encoding,
    including its modular reduction. *)
Lemma wasm_nat64_wrap n : (Z.of_nat n < 2 ^ 64)%Z ->
  eval_cvt T_i32 CVO_wrap None (VAL_int64 (wasm_nat64 n)) = Some (VAL_int32 (wasm_nat n)).
Proof.
  intro Bound.
  change (Some (VAL_int32 (Wasm_int.Int32.repr (Wasm_int.Int64.unsigned (wasm_nat64 n)))) =
    Some (VAL_int32 (wasm_nat n))).
  rewrite (wasm_nat64_unsigned n Bound); reflexivity.
Qed.
