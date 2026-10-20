(** Models admission through Runnable Tags and proves that accepted
    certificates identify an executable whose authors are all trusted. *)

From Stdlib Require Import List Bool ZArith String Ascii.
From compcert Require Import Integers.
From FixProof.Semantics Require Import Handle ApplyTree.
Import ListNotations.

Module Runnable (S : STORAGE) (P : PROGRAM S).
  Module A := ApplyTree S P.
  Import A.H.

  Definition claim : list raw :=
    map (fun c => Byte.repr (Z.of_nat (nat_of_ascii c)))
      (list_ascii_of_string "Runnable"%string).

  Definition runnable trusted tag :=
    if tree_tag tag then
      match get_tree_raw tag with
      | [Data (Object (TreeObj authors)); Data (Object (BlobObj executable));
         Data (Object (BlobObj name))] =>
        if list_eq_dec Byte.eq_dec (get_blob_data name) claim then
          if forallb trusted (get_tree_raw authors) then Some executable else None
        else None
      | _ => None
      end
    else None.

  Definition certificate authors executable :=
    create_tree_tagged true
      [HTreeObj (create_tree authors); HBlobObj executable; HBlobObj (create_blob claim)].

  (** Runnable admission returns only the ELF named by a properly marked certificate whose
      authors are all trusted. *)
  Theorem runnable_sound trusted tag executable :
    runnable trusted tag = Some executable ->
    exists authors name,
      tree_tag tag = true /\
      get_tree_raw tag = [HTreeObj authors; HBlobObj executable; HBlobObj name] /\
      get_blob_data name = claim /\
      Forall (fun author => trusted author = true) (get_tree_raw authors).
  Proof.
    unfold runnable; destruct (tree_tag tag) eqn:Tagged; [|discriminate].
    remember (get_tree_raw tag) as fields eqn:Fields.
    destruct fields as [|[[[blob|authors]|reference]|thunk|encode] rest]; try discriminate.
    destruct rest as [|[[[code|tree]|reference]|thunk|encode] rest]; try discriminate.
    destruct rest as [|[[[name|tree]|reference]|thunk|encode] rest]; try discriminate.
    destruct rest; try discriminate.
    destruct (list_eq_dec Byte.eq_dec (get_blob_data name) claim); [|discriminate].
    destruct (forallb trusted (get_tree_raw authors)) eqn:Trusted; [|discriminate].
    intro Accepted; inversion Accepted; subst executable.
    exists authors, name; repeat split; try assumption.
    apply Forall_forall; rewrite forallb_forall in Trusted; exact Trusted.
  Qed.

  (** A constructed Runnable certificate is accepted exactly when every listed author is
      trusted. *)
  Lemma runnable_certificate trusted authors executable :
    runnable trusted (certificate authors executable) =
      if forallb trusted authors then Some executable else None.
  Proof.
    unfold runnable, certificate.
    rewrite tree_tag_create_tree_tagged, get_tree_raw_create_tree_tagged.
    cbn [HTreeObj HBlobObj]; rewrite get_blob_data_create_blob, get_tree_raw_create_tree.
    destruct (list_eq_dec Byte.eq_dec claim claim); [reflexivity|contradiction].
  Qed.

  (** A successfully admitted Runnable Tag resolves to the program decoded from its ELF
      bytes. *)
  Theorem runnable_program trusted tag executable :
    runnable trusted tag = Some executable ->
    A.get_tree_prog tag = Some (P.get_prog (get_blob_data executable)).
  Proof.
    intro Accepted; apply runnable_sound in Accepted as [authors [name [_ [Fields _]]]].
    rewrite A.get_tree_prog_equation, Fields; reflexivity.
  Qed.
End Runnable.
