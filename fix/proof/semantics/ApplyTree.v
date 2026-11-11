(** Models program execution through a small handle-manipulation language
    and proves that its operations preserve the required handle relations. *)

From Stdlib Require Import List Arith Bool Lia Recdef.
From FixProof.Semantics Require Import Handle.
From FixProof.Shell Require Export Interface.
Import ListNotations.

Module ApplyTree (S : STORAGE) (P : PROGRAM S).
  Module H := Handles S.
  Import H P.
  Record state := State { hs : list handle; ds : list (list raw) }.
  Definition hpush s h := State (h :: hs s) (ds s).
  Definition dpush s d := State (hs s) (d :: ds s).
  Definition run_internal s := State (hs s) (internal (ds s)).
  (** Internal byte computation leaves the handle stack unchanged. *)
  Lemma run_internal_hs s : hs (run_internal s) = hs s.
  Proof. reflexivity. Qed.
  (** Equal byte stacks give equal byte stacks after internal computation. *)
  Lemma run_internal_ds_equiv s1 s2 : ds s1 = ds s2 -> ds (run_internal s1) = ds (run_internal s2).
  Proof. unfold run_internal; simpl; congruence. Qed.
  Inductive step_result := Continue : state -> step_result | Return : handle -> step_result.
  Definition handle_at s i := nth_error (hs s) i.
  Definition push_thunk s i (f : TreeName -> thunk) :=
    match handle_at s i with
    | Some (Data (Object (TreeObj t))) | Some (Data (Ref (TreeRef t))) =>
      Some (Continue (hpush s (Thunk (f t))))
    | _ => None
    end.
  Definition push_encode s i (f : thunk -> encode) :=
    match handle_at s i with
    | Some (Thunk th) => Some (Continue (hpush s (Encode (f th))))
    | _ => None
    end.
  Definition step instr s :=
    match instr with
    | OGetBlobData i =>
      match handle_at s i with
      | Some (Data (Object (BlobObj b))) => Some (Continue (dpush s (get_blob_data b)))
      | _ => None
      end
    | OGetTreeData i j =>
      match handle_at s i with
      | Some (Data (Object (TreeObj t))) =>
        if j <? get_tree_size t then Some (Continue (hpush s (get_tree_data t j))) else None
      | _ => None
      end
    | OCreateBlob i =>
      match nth_error (ds s) i with
      | Some d => Some (Continue (hpush s (HBlobObj (create_blob d))))
      | None => None
      end
    | OCreateTree xs =>
      if forallb (fun i => i <? length (hs s)) xs then
        Some (Continue (hpush s (HTreeObj (create_tree
          (map (fun i => nth i (hs s) (HBlobObj (create_blob []))) xs)))))
      else None
    | OCreateBlobRef i =>
      match handle_at s i with
      | Some (Data (Object (BlobObj b))) => Some (Continue (hpush s (HBlobRef b)))
      | _ => None
      end
    | OCreateTreeRef i =>
      match handle_at s i with
      | Some (Data (Object (TreeObj t))) => Some (Continue (hpush s (HTreeRef t)))
      | _ => None
      end
    | OCreateApplicationThunk i => push_thunk s i Application
    | OCreateIdentificationThunk i =>
      match handle_at s i with
      | Some (Data d) => Some (Continue (hpush s (Thunk (Identification (data_reference d)))))
      | _ => None
      end
    | OCreateSelectionThunk i => push_thunk s i Selection
    | OCreateStrictEncode i => push_encode s i Strict
    | OCreateShallowEncode i => push_encode s i Shallow
    | OGetType i =>
      match handle_at s i with
      | Some h => Some (Continue (dpush s (from_nat (value_type h))))
      | None => None
      end
    | OReturn i => omap Return (handle_at s i)
    | ORunInternal => Some (Continue (run_internal s))
    end.
  Fixpoint exec prog s :=
    match prog with
    | [] => None
    | instr :: rest =>
      match step instr s with
      | None => None
      | Some (Return r) => Some r
      | Some (Continue s') => exec rest s'
      end
    end.
  Definition state_init t := State [HTreeObj t] [].
  Fixpoint author_at depth h :=
    match depth with
    | 0 => Some h
    | S depth' =>
      match h with
      | Data (Object (TreeObj t)) =>
        match nth_error (get_tree_raw t) 1 with
        | Some child => author_at depth' child
        | None => None
        end
      | _ => None
      end
    end.
  Function get_tree_prog (t : TreeName) {wf tree_child t} : option (list op) :=
    match nth_error (get_tree_raw t) 1 with
    | Some (Data (Object (BlobObj b))) => Some (get_prog (get_blob_data b))
    | Some (Data (Object (TreeObj child))) => get_tree_prog child
    | _ => None
    end.
  Proof.
    - intros; unfold tree_child; eapply nth_error_In; eassumption.
    - exact wfp_tree_child.
  Defined.
  Definition apply_tree t := obind (get_tree_prog t) (fun prog => exec prog (state_init t)).
Load "fix/proof/semantics/internal/ApplyTree/Congruence".
End ApplyTree.
