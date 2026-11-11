  (** A decoded selection index comes from an eight-byte Blob and fits in an unsigned
      64-bit integer. *)
  Lemma read_index_spec h n : read_index h = Some n ->
    exists b, h = HBlobObj b /\ get_blob_size b = 8 /\
      (Z.of_nat n < 2 ^ 64)%Z.
  Proof.
    destruct h as [[[b|t]|[b|t]]|th|e]; cbn [read_index]; try discriminate.
    intro Decoded; exists b; split; [reflexivity|]; split.
    - apply (decode_u64_width _ _ Decoded).
    - apply (decode_u64_bound _ _ Decoded).
  Qed.
