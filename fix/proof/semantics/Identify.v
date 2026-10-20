(** Defines identification as returning the supplied Ref unchanged and proves
    that it preserves relations between its inputs. *)

From FixProof.Semantics Require Import Handle.
Module Identify (S : STORAGE).
  Module H := Handles S.
  Import H.
  Definition identify d := Some (Data (Ref d)).
  (** Identification preserves the relation between its Ref operands. *)
  Lemma identify_X (X : handle -> handle -> Prop) d1 d2 :
    X (Data (Ref d1)) (Data (Ref d2)) -> rel_opt X (identify d1) (identify d2).
  Proof. exact (fun h => h). Qed.
End Identify.
