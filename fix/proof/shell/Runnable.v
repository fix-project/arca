(** Models admission through single-author Runnable Tags. *)

From Stdlib Require Import List Bool ZArith String Ascii ClassicalDescription.
From compcert Require Import Integers.
From FixProof.Semantics Require Import Handle ApplyTree.
Import ListNotations.

Module Runnable (S : STORAGE) (P : PROGRAM S).
  Module A := ApplyTree S P.
  Import A.H.

  Definition claim : list raw :=
    map (fun c => Byte.repr (Z.of_nat (nat_of_ascii c)))
      (list_ascii_of_string "Runnable"%string).

  Definition kernel_author := HTreeObj (create_tree []).
  Definition author_trusted trusted author :=
    if excluded_middle_informative (author = kernel_author) then true else trusted author.

  Definition runnable trusted tag :=
    if tree_tag tag then
      match get_tree_raw tag with
      | [author; Data (Object (BlobObj executable)); Data (Object (BlobObj name))] =>
        if list_eq_dec Byte.eq_dec (get_blob_data name) claim then
          if author_trusted trusted author then Some executable else None
        else None
      | _ => None
      end
    else None.

  Definition certificate author executable :=
    create_tree_tagged true [author; HBlobObj executable; HBlobObj (create_blob claim)].

  Theorem runnable_sound trusted tag executable :
    runnable trusted tag = Some executable ->
    exists author name,
      tree_tag tag = true /\
      get_tree_raw tag = [author; HBlobObj executable; HBlobObj name] /\
      get_blob_data name = claim /\
      (author = kernel_author \/ trusted author = true).
  Proof.
    unfold runnable; destruct (tree_tag tag) eqn:Tagged; [|discriminate].
    destruct (get_tree_raw tag) as [|author rest] eqn:Fields; try discriminate.
    destruct rest as [|[[[code|tree]|reference]|thunk|encode] rest]; try discriminate.
    destruct rest as [|[[[name|tree]|reference]|thunk|encode] rest]; try discriminate.
    destruct rest; try discriminate.
    destruct (list_eq_dec Byte.eq_dec (get_blob_data name) claim); [|discriminate].
    destruct (author_trusted trusted author) eqn:Trusted; [|discriminate].
    intro Accepted; inversion Accepted; subst executable.
    exists author, name; repeat split; try assumption.
    unfold author_trusted in Trusted.
    destruct (excluded_middle_informative (author = kernel_author)); auto.
  Qed.

  Lemma runnable_certificate trusted author executable :
    runnable trusted (certificate author executable) =
      if author_trusted trusted author then Some executable else None.
  Proof.
    unfold runnable, certificate.
    rewrite tree_tag_create_tree_tagged, get_tree_raw_create_tree_tagged.
    cbn [HBlobObj]; rewrite get_blob_data_create_blob.
    destruct (list_eq_dec Byte.eq_dec claim claim); [reflexivity|contradiction].
  Qed.

  (** A successfully admitted Runnable Tag resolves to the program decoded from its ELF
      bytes. *)
  Theorem runnable_program trusted tag executable :
    runnable trusted tag = Some executable ->
    A.get_tree_prog tag = Some (P.get_prog (get_blob_data executable)).
  Proof.
    intro Accepted; apply runnable_sound in Accepted as [author [name [_ [Fields _]]]].
    rewrite A.get_tree_prog_equation, Fields; reflexivity.
  Qed.
End Runnable.
