(** Specifies exact handle inspection in privileged execution, which issues no application coupon. *)

From FixProof.Storage Require Import Interface.

Module Type PRIVILEGED_HANDLES (S : STORAGE).
  Import S.
  Parameter handle_bytes : nat -> handle -> option BlobName.
  (** Equal privileged handle-byte observations imply exact abstract-handle identity. *)
  Parameter bytes_identify : forall i j lhs rhs lhs_bytes rhs_bytes,
    handle_bytes i lhs = Some lhs_bytes -> handle_bytes j rhs = Some rhs_bytes ->
    get_blob_data lhs_bytes = get_blob_data rhs_bytes -> lhs = rhs.
End PRIVILEGED_HANDLES.
