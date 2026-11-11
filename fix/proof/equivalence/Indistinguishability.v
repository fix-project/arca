(** Models observable handle properties alongside computational equivalence.
    Proves that Comparable equivalents are indistinguishable and that the
    modeled observations are preserved. *)

From Stdlib Require Import List Bool ClassicalDescription.
From Hammer Require Import Tactics.
From FixProof.Semantics Require Import Handle ApplyTree.
From FixProof.Equivalence Require Import Closure.

Module Indistinguishability (S : STORAGE) (P : PROGRAM S).
  Module EC := EquivalenceClosure S P.
  Import EC EC.E EC.E.EP EC.E.EP.E EC.E.EP.E.H.

  Definition data_length d := match d with
    | Object (BlobObj b) | Ref (BlobRef b) => get_blob_size b
    | Object (TreeObj t) | Ref (TreeRef t) => get_tree_size t
    end.

  (** Computational equivalence justifies replacing computations, but an API observation
      may still see their packaging: an Encode and the Object it computes have different
      types. Matching observations separately records constructor, content, metadata, and
      the observations of future computation results. Indistinguishability requires both
      this observation agreement and computational equivalence. *)
  CoInductive same_observations : handle -> handle -> Prop :=
  | ObserveBlob : forall b1 b2,
      get_blob_data b1 = get_blob_data b2 ->
      same_observations (HBlobObj b1) (HBlobObj b2)
  | ObserveBlobRef : forall b1 b2,
      get_blob_data b1 = get_blob_data b2 ->
      same_observations (HBlobRef b1) (HBlobRef b2)
  | ObserveTree : forall t1 t2,
      tree_tag t1 = tree_tag t2 ->
      tree_comparable t1 = tree_comparable t2 ->
      observed_children (get_tree_raw t1) (get_tree_raw t2) ->
      same_observations (HTreeObj t1) (HTreeObj t2)
  | ObserveTreeRef : forall t1 t2,
      same_observations (HTreeObj t1) (HTreeObj t2) ->
      same_observations (HTreeRef t1) (HTreeRef t2)
  | ObserveThunk : forall th1 th2,
      observed_result (force th1) (force th2) ->
      same_observations (Thunk th1) (Thunk th2)
  | ObserveStrict : forall th1 th2,
      observed_result (execute (Strict th1)) (execute (Strict th2)) ->
      same_observations (Encode (Strict th1)) (Encode (Strict th2))
  | ObserveShallow : forall th1 th2,
      observed_result (execute (Shallow th1)) (execute (Shallow th2)) ->
      same_observations (Encode (Shallow th1)) (Encode (Shallow th2))
  with observed_children : list handle -> list handle -> Prop :=
  | ObserveNil : observed_children nil nil
  | ObserveCons : forall h1 h2 xs ys,
      same_observations h1 h2 ->
      observed_children xs ys ->
      observed_children (h1 :: xs) (h2 :: ys)
  with observed_result : option handle -> option handle -> Prop :=
  | ObserveNone : observed_result None None
  | ObserveSome : forall h1 h2,
      same_observations h1 h2 ->
      observed_result (Some h1) (Some h2).

  Definition indistinguishable h1 h2 := computational_equiv h1 h2 /\ same_observations h1 h2.

Load "fix/proof/equivalence/internal/Indistinguishability/Observations".
  Definition compare_handles left right :=
    if is_comparable left && is_comparable right then
      Some (if excluded_middle_informative (indistinguishable left right)
            then true else false)
    else None.

Load "fix/proof/equivalence/internal/Indistinguishability/Comparison".
  (** Replacing either operand by an indistinguishable handle leaves handle comparison
      unchanged.

      Indistinguishable replacements preserve comparability and the comparison predicate, by
      symmetry and transitivity. *)
Load "fix/proof/equivalence/machinery/Indistinguishability/compare_handles_congruence".
  Theorem compare_handles_congruence left left' right right' :
    indistinguishable left left' -> indistinguishable right right' ->
    compare_handles left right = compare_handles left' right'.
  Proof. apply compare_handles_congruence_proof. Qed.

Load "fix/proof/equivalence/internal/Indistinguishability/Structure".
  (** On Comparable handles, the computation logic and the observation logic agree. Thus
      restricting handle comparison to Comparable operands avoids identifying computations
      merely because their eventual results agree. *)
Load "fix/proof/equivalence/machinery/Indistinguishability/comparable_equivalence_indistinguishable".
  Theorem comparable_equivalence_indistinguishable left right :
    is_comparable left = true -> is_comparable right = true ->
    (computational_equiv left right <-> indistinguishable left right).
  Proof. apply comparable_equivalence_indistinguishable_proof. Qed.

Load "fix/proof/equivalence/internal/Indistinguishability/Values".
  (** Indistinguishable handles agree on whether they are Values.

      Value preservation in both directions follows from symmetry and preservation of the
      observed handle structure. *)
Load "fix/proof/equivalence/machinery/Indistinguishability/indistinguishable_values".
  Theorem indistinguishable_values h1 h2 : indistinguishable h1 h2 ->
    (value_handle h1 <-> value_handle h2).
  Proof. apply indistinguishable_values_proof. Qed.

Load "fix/proof/equivalence/internal/Indistinguishability/Index".
  (** Index selection on indistinguishable Data gives indistinguishable results or shared
      failure. *)
Load "fix/proof/equivalence/machinery/Indistinguishability/indistinguishable_select_index".
  Theorem indistinguishable_select_index d1 d2 i :
    indistinguishable (Data d1) (Data d2) ->
    rel_opt indistinguishable (Sl.select_index (Data d1) i)
      (Sl.select_index (Data d2) i).
  Proof. apply indistinguishable_select_index_proof. Qed.

Load "fix/proof/equivalence/internal/Indistinguishability/Range".
  (** Range selection on indistinguishable Data gives indistinguishable results or shared
      failure. *)
Load "fix/proof/equivalence/machinery/Indistinguishability/indistinguishable_select_range".
  Theorem indistinguishable_select_range d1 d2 a b :
    indistinguishable (Data d1) (Data d2) ->
    rel_opt indistinguishable (Sl.select_range (Data d1) a b)
      (Sl.select_range (Data d2) a b).
  Proof. apply indistinguishable_select_range_proof. Qed.

Load "fix/proof/equivalence/internal/Indistinguishability/Arguments".
  (** Selection on indistinguishable Data with pointwise indistinguishable operands gives
      indistinguishable results or shared failure. *)
Load "fix/proof/equivalence/machinery/Indistinguishability/indistinguishable_select_arguments".
  Theorem indistinguishable_select_arguments d1 d2 xs ys :
    indistinguishable (Data d1) (Data d2) -> Forall2 indistinguishable xs ys ->
    rel_opt indistinguishable (Sl.select_arguments (Data d1) xs)
      (Sl.select_arguments (Data d2) ys).
  Proof. apply indistinguishable_select_arguments_proof. Qed.
End Indistinguishability.
