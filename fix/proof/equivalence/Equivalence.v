(** Defines the coinductive relations underlying computational equivalence
    and proves their structural and evaluation properties. *)

From Stdlib Require Import List.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Evaluation Require Import Properties.

Module Equivalence (S : STORAGE) (P : PROGRAM S).
  Module EP := EvaluationProperties S P.
  Import EP EP.E EP.E.H.
  Definition squash h :=
    match h with Encode (Strict th) | Encode (Shallow th) => Thunk th | _ => h end.

  (** R compares recursive behavior rather than counting reduction steps. It can relate
      computations whose next replies differ in how they defer the remaining work,
      provided the replies represent related eventual Objects. Coinduction permits
      recursive comparisons with no finite bound on their depth. The directed execution
      rule also admits replacing an Encode by its actual result; symmetry and finite
      composition are added separately in Closure.v. *)
  CoInductive R : handle -> handle -> Prop :=
  | RBlob : forall b1 b2, get_blob_data b1 = get_blob_data b2 -> R (HBlobObj b1) (HBlobObj b2)
  | RBlobRef : forall b1 b2, R (HBlobObj b1) (HBlobObj b2) -> R (HBlobRef b1) (HBlobRef b2)
  | RTreeNodes : forall t1 t2, tree_tag t1 = tree_tag t2 -> R_handles (get_tree_raw t1) (get_tree_raw t2) -> R (HTreeObj t1) (HTreeObj t2)
  | RTreeRef : forall t1 t2, R (HTreeObj t1) (HTreeObj t2) -> R (HTreeRef t1) (HTreeRef t2)
  | RThunkNone : forall t1 t2, think t1 = None -> think t2 = None -> R (Thunk t1) (Thunk t2)
  | RThunkSomeResData : forall t1 t2 d1 d2,
      think t1 = Some (Data d1) -> think t2 = Some (Data d2) ->
      R (lift (Data d1)) (lift (Data d2)) -> R (Thunk t1) (Thunk t2)
  | RThunkSomeResNotData : forall t1 t2 h1 h2,
      think t1 = Some h1 -> think t2 = Some h2 ->
      ~(exists d, h1 = Data d) -> ~(exists d, h2 = Data d) ->
      R (squash h1) (squash h2) -> R (Thunk t1) (Thunk t2)
  | RThunkSomeResEncodeData : forall t1 t2 e1 d2,
      think t1 = Some (Encode e1) -> think t2 = Some (Data d2) ->
      R (Encode (Strict (encode_to_thunk e1))) (lift (Data d2)) -> R (Thunk t1) (Thunk t2)
  | RThunkSomeResEncodeEncode : forall t1 t2 e1 e2,
      think t1 = Some (Encode e1) -> think t2 = Some (Encode e2) ->
      R (Encode (Strict (encode_to_thunk e1))) (Encode (Strict (encode_to_thunk e2))) -> R (Thunk t1) (Thunk t2)
  | RThinkSingleStepThunk : forall t1 t2, think t1 = Some (Thunk t2) -> R (Thunk t1) (Thunk t2)
  | RThunkSingleStepEncodeShallow : forall t1 t2, think t1 = Some (Encode (Shallow t2)) -> R (Thunk t1) (Thunk t2)
  | RThunkSingleStepEncodeStrict : forall t1 t2, think t1 = Some (Encode (Strict t2)) -> R (Thunk t1) (Thunk t2)
  | REvalStrictNone : forall t1 t2, execute (Strict t1) = None -> execute (Strict t2) = None ->
      R (Encode (Strict t1)) (Encode (Strict t2))
  | REvalShallowNone : forall t1 t2, execute (Shallow t1) = None -> execute (Shallow t2) = None ->
      R (Encode (Shallow t1)) (Encode (Shallow t2))
  | REvalSomeRes : forall e1 e2 r1 r2, execute e1 = Some r1 -> execute e2 = Some r2 -> R r1 r2 -> R (Encode e1) (Encode e2)
  | REvalStep : forall e r, execute e = Some r -> R (Encode e) r
  | RSelf : forall h, R h h
  with R_handles : list handle -> list handle -> Prop :=
  | RNil : R_handles nil nil
  | RCons : forall h1 h2 xs ys, R h1 h2 -> R_handles xs ys -> R_handles (h1 :: xs) (h2 :: ys).

Load "fix/proof/equivalence/machinery/Equivalence/Coinduction".
Load "fix/proof/equivalence/internal/Equivalence/Preservation".
End Equivalence.
