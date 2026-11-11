  Definition authorized collector witness := exists bundle tag claim author,
    witness = treeh bundle /\ get_tree_raw bundle = [treeh tag; treeh claim] /\
    get_tree_raw tag = [author; G.E.H.H.HTreeRef claim] /\ tree_tag tag = true /\
    List.length (get_tree_raw claim) = 3 /\ (author = collector \/ author = X.kernel_author).
