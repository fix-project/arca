(** Defines claim kinds and exact abstract handle matching for the collector. *)

From Stdlib Require Import List ClassicalDescription.
From FixProof.Semantics Require Import Handle.

Inductive statement_kind := Force | Apply | Select | Think | Eval | Equivalent | Indistinguishable.

Module CouponMatching (S : STORAGE).
  Module H := Handles S.
  Import H.
  Definition handle_eqb (h1 h2 : handle) := if excluded_middle_informative (h1 = h2) then true else false.
  (** Claim kinds have decidable exact equality. *)
  Definition statement_kind_eq_dec (t1 t2 : statement_kind) : {t1=t2}+{t1<>t2}.
  Proof. decide equality. Defined.
  (** The exact-handle equality test returns true exactly for identical abstract handles. *)
  Lemma handle_eqb_spec h1 h2 : handle_eqb h1 h2 = true <-> h1 = h2.
  Proof. unfold handle_eqb; destruct (excluded_middle_informative _); intuition discriminate. Qed.
  (** The exact-handle equality test accepts a handle compared with itself. *)
  Lemma handle_eqb_refl h : handle_eqb h h = true.
  Proof. apply handle_eqb_spec; reflexivity. Qed.
End CouponMatching.
