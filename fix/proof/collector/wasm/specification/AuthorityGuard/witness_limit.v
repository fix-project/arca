  Definition witness_limit : Z := 1048576.
  Definition witnesses_bounded witnesses := (Z.of_nat (List.length (get_tree_raw witnesses)) <= witness_limit)%Z.
