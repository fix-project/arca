    Fixpoint lengths_bounded depth value := match depth with
      | 0 => True
      | Datatypes.S remaining => match value with
        | Data (Object (BlobObj blob)) => (Z.of_nat (List.length (get_blob_data blob)) < 2 ^ 64)%Z
        | Data (Object (TreeObj t)) =>
          (Z.of_nat (List.length (get_tree_raw t)) < 2 ^ 64)%Z /\
          Forall (lengths_bounded remaining) (get_tree_raw t)
        | _ => True end
      end.
