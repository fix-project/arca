(** Defines index and half-open range selection on Blobs and Trees, and
    proves congruence under the supplied relations on handles. *)

From Stdlib Require Import List ZArith Arith Bool Lia.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle.
Import ListNotations.
Module Select (S : STORAGE).
  Module H := Handles S.
  Import H.
  Definition list_slice {A} (l : list A) a b :=
    if (a <=? b) && (b <=? length l)
    then Some (firstn (b-a) (skipn a l)) else None.

Load "fix/proof/semantics/internal/Select/Slices".
  Definition tree_slice t x y := omap create_tree (list_slice (get_tree_raw t) x y).
  Definition blob_slice b x y := omap create_blob (list_slice (get_blob_data b) x y).
  Definition range_ref target a b := match target with
    | Data (Object (BlobObj blob)) | Data (Ref (BlobRef blob)) =>
      omap BlobRef (blob_slice blob a b)
    | Data (Object (TreeObj t)) | Data (Ref (TreeRef t)) =>
      omap TreeRef (tree_slice t a b)
    | _ => None
    end.
  Definition select_range target a b := omap (fun ref => Data (Ref ref)) (range_ref target a b).
  Definition select_index target i := match target with
    | Data (Object (TreeObj t)) | Data (Ref (TreeRef t)) =>
      nth_error (get_tree_raw t) i
    | _ => select_range target i (S i)
    end.
  Definition read_index h := match h with
    | Data (Object (BlobObj b)) => to_nat (get_blob_data b)
    | _ => None
    end.
Load "fix/proof/semantics/internal/Select/Indices".
  Definition select_arguments target indices := match indices with
    | [i] => obind (read_index i) (select_index target)
    | [a; b] => obind (read_index a) (fun start =>
        obind (read_index b) (select_range target start))
    | _ => None
    end.
  Definition select t := match get_tree_raw t with
    | target :: indices => select_arguments target indices
    | [] => None
    end.

Load "fix/proof/semantics/internal/Select/Congruence".
End Select.
