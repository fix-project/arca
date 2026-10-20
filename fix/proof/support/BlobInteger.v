(** Relates eight-byte little-endian Blob integers to natural numbers and
    Wasm integer loads. *)

From Stdlib Require Import List ZArith Lia.
From compcert Require Import Integers Memdata.
From Wasm Require Import datatypes operations.
Local Transparent Archi.big_endian.

Definition encode_u64 n := Memdata.bytes_of_int 8 (Z.of_nat n).
Definition decode_u64 (bytes : list byte) :=
  if Nat.eqb (length bytes) 8
  then Some (Z.to_nat (Memdata.int_of_bytes bytes)) else None.

(** Encoding an integer with a requested byte width produces exactly that many bytes. *)
Lemma bytes_of_int_length n z : length (Memdata.bytes_of_int n z) = n.
Proof. revert z; induction n; intros; simpl; auto. Qed.

(** Decoding a byte sequence gives a nonnegative integer bounded by its byte width. *)
Lemma int_of_bytes_bound bytes :
  (0 <= Memdata.int_of_bytes bytes < 256 ^ Z.of_nat (length bytes))%Z.
Proof.
  induction bytes as [|b bytes IH]; [cbn; lia|].
  change (0 <= Byte.unsigned b + Memdata.int_of_bytes bytes * 256 <
    256 ^ Z.of_nat (S (length bytes)))%Z.
  rewrite Nat2Z.inj_succ, Z.pow_succ_r by lia.
  pose proof (Byte.unsigned_range b); change Byte.modulus with 256%Z in *; nia.
Qed.

(** An integer fitting the requested byte width survives byte encoding and decoding
    unchanged. *)
Lemma int_of_bytes_roundtrip n z :
  (0 <= z < 256 ^ Z.of_nat n)%Z ->
  Memdata.int_of_bytes (Memdata.bytes_of_int n z) = z.
Proof.
  revert z; induction n as [|n IH]; intros z Bound; [cbn in *; lia|].
  cbn [Memdata.bytes_of_int Memdata.int_of_bytes].
  rewrite Nat2Z.inj_succ, Z.pow_succ_r in Bound by lia.
  rewrite IH.
  - rewrite Byte.unsigned_repr_eq; change Byte.modulus with 256%Z.
    pose proof (Z.div_mod z 256 ltac:(lia)); lia.
  - split.
    + apply Z.div_pos; lia.
    + apply Z.div_lt_upper_bound; nia.
Qed.

(** Unsigned 64-bit encoding produces exactly eight bytes. *)
Lemma encode_u64_length n : length (encode_u64 n) = 8.
Proof. apply bytes_of_int_length. Qed.

(** Successful unsigned 64-bit decoding requires exactly eight bytes. *)
Lemma decode_u64_width bytes n : decode_u64 bytes = Some n -> length bytes = 8.
Proof.
  unfold decode_u64; destruct (Nat.eqb (length bytes) 8) eqn:Width;
    [intros _; apply Nat.eqb_eq; exact Width|discriminate].
Qed.

(** A successfully decoded unsigned 64-bit integer is less than two to the sixty-fourth
    power. *)
Lemma decode_u64_bound bytes n :
  decode_u64 bytes = Some n -> (Z.of_nat n < 2 ^ 64)%Z.
Proof.
  intro Decoded; pose proof (decode_u64_width bytes n Decoded) as Width.
  pose proof (int_of_bytes_bound bytes) as Bound.
  change (0 <= Memdata.int_of_bytes bytes < 256 ^ Z.of_nat (length bytes))%Z in Bound.
  rewrite Width in Bound.
  unfold decode_u64 in Decoded; rewrite Width in Decoded; injection Decoded as <-.
  rewrite Z2Nat.id by lia.
  exact (proj2 Bound).
Qed.

(** An unsigned 64-bit integer survives the Fix byte encoding round trip. *)
Lemma decode_encode_u64 n : (Z.of_nat n < 2 ^ 64)%Z ->
  decode_u64 (encode_u64 n) = Some n.
Proof.
  intro Bound; unfold decode_u64; rewrite encode_u64_length.
  unfold encode_u64; rewrite int_of_bytes_roundtrip by (cbn; lia).
  rewrite Nat2Z.id; reflexivity.
Qed.

(** Wasm i64 serialization agrees with Fix's unsigned 64-bit encoding. *)
Lemma serialise_i64_encode_u64 i :
  serialise_i64 i = encode_u64 (Z.to_nat (Wasm_int.Int64.unsigned i)).
Proof.
  unfold serialise_i64, Memdata.encode_int, Memdata.rev_if_be, Archi.big_endian.
  unfold encode_u64; rewrite Z2Nat.id by apply Wasm_int.Int64.unsigned_range.
  reflexivity.
Qed.

(** Decoding a serialized Wasm i64 recovers its unsigned integer value. *)
Lemma decode_serialise_i64 i :
  decode_u64 (serialise_i64 i) = Some (Z.to_nat (Wasm_int.Int64.unsigned i)).
Proof.
  rewrite serialise_i64_encode_u64; apply decode_encode_u64.
  pose proof (Wasm_int.Int64.unsigned_range i) as Bound.
  rewrite Z2Nat.id by lia; exact (proj2 Bound).
Qed.

(** Fix unsigned 64-bit decoding agrees with Wasm i64 deserialization. *)
Lemma wasm_deserialise_u64 bytes n : decode_u64 bytes = Some n ->
  wasm_deserialise bytes T_i64 = VAL_int64 (Wasm_int.Int64.repr (Z.of_nat n)).
Proof.
  unfold decode_u64; destruct (Nat.eqb (length bytes) 8); [|discriminate].
  intro Decoded; injection Decoded as <-.
  unfold wasm_deserialise, Memdata.decode_int, Memdata.rev_if_be, Archi.big_endian.
  rewrite Z2Nat.id by apply int_of_bytes_bound; reflexivity.
Qed.
