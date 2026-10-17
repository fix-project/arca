From FixProof Require Import Handle.
Module Identify (S : STORAGE).
  Module H := Handles S.
  Import H.
  Definition identify d := Some (Data d).
  Lemma identify_X (X : handle -> handle -> Prop) d1 d2 :
    X (Data d1) (Data d2) -> rel_opt X (identify d1) (identify d2).
  Proof. exact (fun h => h). Qed.
End Identify.
