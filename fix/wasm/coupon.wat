(module
 (import "fixpoint" "is_equal" (func $is_equal (param externref) (param externref) (result i32)))
 (import "fixpoint" "is_tag" (func $is_tag (param externref) (result i32)))
 (import "fixpoint" "attach_blob" (func $attach_blob (param i32) (param externref)))
 (import "fixpoint" "attach_tree" (func $attach_tree (param i32) (param externref)))
 (import "fixpoint" "create_blob_i32" (func $create_blob_i32 (param i32) (result externref)))
 (import "fixpoint" "create_tag" (func $create_tag (param i32) (result externref)))
 (import "fixpoint" "create_application_thunk" (func $create_application_thunk (param externref) (result externref)))
 (import "fixpoint" "create_strict_encode" (func $create_strict_encode (param externref) (result externref)))
 (import "fixpoint" "create_shallow_encode" (func $create_shallow_encode (param externref) (result externref)))
 (import "fixpoint" "is_blob_obj" (func $is_blob_obj (param externref) (result i32)))
 (import "fixpoint" "is_data" (func $is_data (param externref) (result i32)))
 (import "fixpoint" "is_object" (func $is_object (param externref) (result i32)))
 (table $encode 0 externref)
 (table $coupon_scratch 0 externref)
 (table $coupons 0 externref)
 (table $lhstree 0 externref)
 (table $rhstree 0 externref)
 (table $output_coupon_scratch 4 externref)
 (memory $mem_0 0)
 (memory $mem_1 0)
 (global $Eq i32 (i32.const 0))
 (global $Eval i32 (i32.const 1))
 (global $Apply i32 (i32.const 2))
 (global $Force i32 (i32.const 3))
 (global $Think i32 (i32.const 4))
 (global $Storage i32 (i32.const 5))
 (type $make_coupon_t (func (param externref externref) (result externref)))
 (func $is_coupon (param $tag externref) (param $type i32) (result i32) 
   (call $is_tag (local.get $tag))
   (if (result i32)
     (then
       ;; Attach the tag
       (call $attach_tree (i32.const 1) (local.get $tag))
       ;; Check if the tag was authored by us
       (call $is_equal (table.get $encode (i32.const 0)) (table.get $coupon_scratch (i32.const 0)))
       (if (result i32)
         (then
           ;; Check if the coupon type matches the input type
           (call $attach_blob (i32.const 1) (table.get $coupon_scratch (i32.const 1)))
           (i32.load (memory $mem_1) (i32.const 0))
           (local.get $type)
           i32.eq
           (if (result i32)
             (then (i32.const 1))
             (else (i32.const 0))
           )
         )
         (else (i32.const 0))
       )
     )
     (else (i32.const 0))
   )
 )
 (func $is_eq_coupon (param $tag externref) (result i32)
   (call $is_coupon (local.get $tag) (global.get $Eq))
 )
 (func $is_eval_coupon (param $tag externref) (result i32)
   (call $is_coupon (local.get $tag) (global.get $Eval))
 )
 (func $is_apply_coupon (param $tag externref) (result i32)
   (call $is_coupon (local.get $tag) (global.get $Apply))
 )
 (func $is_force_coupon (param $tag externref) (result i32)
   (call $is_coupon (local.get $tag) (global.get $Force))
 )
 (func $is_think_coupon (param $tag externref) (result i32)
   (call $is_coupon (local.get $tag) (global.get $Think))
 )
 (func $is_storage_coupon (param $tag externref) (result i32)
   (call $is_coupon (local.get $tag) (global.get $Storage))
 )
 (func $create_coupon (param $type i32) (param $lhs externref) (param $rhs externref) (result externref)
   (table.set $output_coupon_scratch (i32.const 0) (table.get $encode (i32.const 0)))
   (table.set $output_coupon_scratch (i32.const 1) (call $create_blob_i32 (local.get $type)))
   (table.set $output_coupon_scratch (i32.const 2) (local.get $lhs))
   (table.set $output_coupon_scratch (i32.const 3) (local.get $rhs))
   (call $create_tag (i32.const 5))
 )
 (func $create_eq_coupon (param $lhs externref) (param $rhs externref) (result externref)
   (call $create_coupon (global.get $Eq) (local.get $lhs) (local.get $rhs))
 )
 (func $create_eval_coupon (param $lhs externref) (param $rhs externref) (result externref)
   (call $create_coupon (global.get $Eval) (local.get $lhs) (local.get $rhs))
 )
 (func $create_force_coupon (param $lhs externref) (param $rhs externref) (result externref)
   (call $create_coupon (global.get $Force) (local.get $lhs) (local.get $rhs))
 )
 (func $create_think_coupon (param $lhs externref) (param $rhs externref) (result externref)
   (call $create_coupon (global.get $Think) (local.get $lhs) (local.get $rhs))
 )
 (func $get_coupon_lhs (param $coupon externref) (result externref)
   (call $attach_tree (i32.const 1) (local.get $coupon))
   (table.get $coupon_scratch (i32.const 2))
 )
 (func $get_coupon_rhs (param $coupon externref) (result externref)
   (call $attach_tree (i32.const 1) (local.get $coupon))
   (table.get $coupon_scratch (i32.const 3))
 )
 (func $attach_lhs_tree (param $lhs externref)
   (call $attach_tree (i32.const 3) (local.get $lhs))
 )
 (func $attach_rhs_tree (param $rhs externref)
   (call $attach_tree (i32.const 4) (local.get $rhs))
 )
 (func $get_tree_size_lhs (result i32)
    table.size $lhstree
 )
 (func $get_tree_size_rhs (result i32)
    table.size $rhstree
 )
 (func $get_tree_data_lhs (param $i i32) (result externref) 
    (table.get $lhstree (local.get $i))
 )
 (func $get_tree_data_rhs (param $i i32) (result externref)
    (table.get $rhstree (local.get $i))
 )
 (func $make_eq_tree_coupon (export "make_eq_tree_coupon") (param $lhs externref) (param $rhs externref) (result externref) (local $c externref) (local $size i32) (local $i i32)
   (local.set $size (table.size $coupons))
   ;; Check that all coupons are eq coupons
   (local.set $i (i32.const 0))
   (block $exit
     (loop $loop
       (local.get $i)
       (local.get $size)
       i32.ge_s
       br_if $exit

       (call $is_eq_coupon (table.get $coupons (local.get $i)))
       (if
         (then nop)
         (else unreachable)
       )

       (local.set $i (i32.add (local.get $i) (i32.const 1)))
       br $loop
     )
   )

   ;; Attach lhs tree and rhs tree
   (call $attach_lhs_tree (local.get $lhs))
   (call $attach_rhs_tree (local.get $rhs))

   ;; Check tree size of lhs and rhs
   (i32.eq (call $get_tree_size_lhs) (local.get $size))
   (if (result externref)
     (then
       (i32.eq (call $get_tree_size_rhs) (local.get $size))
       (if (result externref)
         (then
           ;; Check that each coupon corresponds to one pair of tree entries
           (local.set $i (i32.const 0))
           (block $exit
             (loop $loop
               (local.get $i)
               (local.get $size)
               i32.ge_s
               br_if $exit

               (local.set $c (table.get $coupons (local.get $i)))
               (call $is_equal (call $get_tree_data_lhs (local.get $i)) (call $get_coupon_lhs (local.get $c)))
               (if
                 (then
                   (call $is_equal (call $get_tree_data_rhs (local.get $i)) (call $get_coupon_rhs (local.get $c)))
                   (if
                     (then nop)
                     (else unreachable)
                   )
                 )
                 (else
                   unreachable
                 )
               )

               (local.set $i (i32.add (local.get $i) (i32.const 1)))
               br $loop
             )
           )
           (call $create_eq_coupon (local.get $lhs) (local.get $rhs))
         )
         (else
           unreachable
         )
       )
     )
     (else
       unreachable
     )
   ))
 (func $make_eval_tree_coupon (export "make_eval_tree_coupon") (param $lhs externref) (param $rhs externref) (result externref) (local $c externref) (local $size i32) (local $i i32)
   (local.set $size (table.size $coupons))
   ;; Check that all coupons are eval coupons
   (local.set $i (i32.const 0))
   (block $exit
     (loop $loop
       (local.get $i)
       (local.get $size)
       i32.ge_s
       br_if $exit

       (call $is_eval_coupon (table.get $coupons (local.get $i)))
       (if
         (then nop)
         (else unreachable)
       )

       (local.set $i (i32.add (local.get $i) (i32.const 1)))
       br $loop
     )
   )

   ;; Attach lhs tree and rhs tree
   (call $attach_lhs_tree (local.get $lhs))
   (call $attach_rhs_tree (local.get $rhs))

   ;; Check tree size of lhs and rhs
   (i32.eq (call $get_tree_size_lhs) (local.get $size))
   (if (result externref)
     (then
       (i32.eq (call $get_tree_size_rhs)  (local.get $size))
       (if (result externref)
         (then
           ;; Check that each coupon corresponds to one pair of tree entries
           (local.set $i (i32.const 0))
           (block $exit
             (loop $loop
               (local.get $i)
               (local.get $size)
               i32.ge_s
               br_if $exit

               (local.set $c (table.get $coupons (local.get $i)))
               (call $is_equal (call $get_tree_data_lhs (local.get $i)) (call $get_coupon_lhs (local.get $c)))
               (if
                 (then
                   (call $is_equal (call $get_tree_data_rhs (local.get $i)) (call $get_coupon_rhs (local.get $c)))
                   (if
                     (then nop)
                     (else unreachable)
                   )
                 )
                 (else
                   unreachable
                 )
               )

               (local.set $i (i32.add (local.get $i) (i32.const 1)))
               br $loop
             )
           )
           (call $create_eval_coupon (local.get $lhs) (local.get $rhs))
         )
         (else
           unreachable
         )
       )
     )
     (else
       unreachable
     )
   ))
 (func $make_force_result_eq_coupon (export "make_force_result_eq_coupon") (param $lhs externref) (param $rhs externref) (result externref) (local $f1 externref) (local $f2 externref) (local $e externref)
    (local.set $f1 (table.get $coupons (i32.const 0)))
    (local.set $f2 (table.get $coupons (i32.const 1)))
    (local.set $e (table.get $coupons (i32.const 2)))
    (call $is_force_coupon (local.get $f1))
    (if (result externref)
      (then
        (call $is_force_coupon (local.get $f2))
        (if (result externref)
          (then
            (call $is_eq_coupon (local.get $e))
            (if (result externref)
              (then
                (call $is_equal (call $get_coupon_rhs (local.get $f1)) (call $get_coupon_lhs (local.get $e)))
                (if (result externref)
                  (then
                    (call $is_equal (call $get_coupon_rhs (local.get $f2)) (call $get_coupon_rhs (local.get $e)))
                    (if (result externref)
                      (then
                        (call $is_equal (call $get_coupon_lhs (local.get $f1)) (local.get $lhs))
                        (if (result externref)
                        (then
                          (call $is_equal (call $get_coupon_lhs (local.get $f2)) (local.get $rhs))
                          (if (result externref)
                            (then
                              (call $create_eq_coupon (local.get $lhs) (local.get $rhs))
                            )
                            (else
                              unreachable
                            )
                          )
                        )
                        (else
                          unreachable
                        )
                      )
                    )
                    (else
                      unreachable
                    )
                  )
                )
                (else
                  unreachable
                )
              )
            )
            (else
              unreachable
            )
          )
        )
        (else
          unreachable
        )
      )
    )
    (else
      unreachable
    )
  ))
 (func $make_eval_eq_coupon (export "make_eval_eq_coupon") (param $lhs externref) (param $rhs externref) (result externref) (local $c1 externref) (local $c2 externref)
    (local.set $c1 (table.get $coupons (i32.const 0)))
    (local.set $c2 (table.get $coupons (i32.const 1)))
    (call $is_eval_coupon (local.get $c1))
    (if (result externref)
      (then
        (call $is_eq_coupon (local.get $c2))
        (if (result externref)
          (then
            (call $is_equal (call $get_coupon_lhs (local.get $c1)) (call $get_coupon_lhs (local.get $c2)))
            (if (result externref)
              (then
                (call $is_equal (call $get_coupon_rhs (local.get $c2)) (local.get $lhs))
                (if (result externref)
                  (then
                    (call $is_equal (call $get_coupon_rhs (local.get $c1)) (local.get $rhs))
                    (if (result externref)
                      (then
                        (call $create_eval_coupon (local.get $lhs) (local.get $rhs))
                      )
                      (else
                        unreachable
                      )
                    )
                  )
                  (else
                    unreachable
                  )
                )
              )
              (else
                unreachable
              )
            )
          )
          (else
            unreachable
          )
        )
      )
      (else
        unreachable
      )
    )
  )
 (func $make_think_application_coupon (export "make_think_application_coupon") (param $lhs externref) (param $rhs externref) (result externref) (local $c1 externref) (local $c2 externref)
    (local.set $c1 (table.get $coupons (i32.const 0)))
    (local.set $c2 (table.get $coupons (i32.const 1)))
    (call $is_eval_coupon (local.get $c1))
    (if (result externref)
      (then
        (call $is_apply_coupon (local.get $c2))
        (if (result externref)
          (then
            (call $is_equal (call $get_coupon_rhs (local.get $c1)) (call $get_coupon_lhs (local.get $c2)))
            (if (result externref)
              (then
                (call $is_equal (call $create_application_thunk (call $get_coupon_lhs (local.get $c1))) (local.get $lhs))
                (if (result externref)
                  (then
                    (call $is_equal (call $get_coupon_rhs (local.get $c2)) (local.get $rhs))
                    (if (result externref)
                    (then
                      (call $create_think_coupon (local.get $lhs) (local.get $rhs))
                    )
                    (else
                      unreachable
                    )
                  )
                )
                (else
                  unreachable
                )
              )
            )
            (else
              unreachable
            )
          )
        )
        (else
          unreachable
        )
      )
    )
    (else
      unreachable
    )
  ))
 (func $make_think_to_force_coupon (export "make_think_to_force_coupon") (param $lhs externref) (param $rhs externref) (result externref) (local $t externref)
    (local.set $t (table.get $coupons (i32.const 0)))
    (call $is_think_coupon (local.get $t))
    (if (result externref)
      (then
        (call $is_data (call $get_coupon_rhs (local.get $t)))
        (if (result externref)
          (then
            (call $is_equal (call $get_coupon_lhs (local.get $t)) (local.get $lhs))
            (if (result externref)
              (then
                (call $is_equal (call $get_coupon_rhs (local.get $t)) (local.get $rhs))
                (if (result externref)
                (then
                  (call $create_force_coupon (local.get $lhs) (local.get $rhs))
                )
                (else
                  unreachable
                )
              )
            )
            (else
              unreachable
            )
          )
        )
        (else
          unreachable
        )
      )
    )
    (else
      unreachable
    )
  ))
 (func $make_force_to_encode_strict_coupon (export "make_force_to_encode_strict_coupon") (param $lhs externref) (param $rhs externref) (result externref) (local $t externref)
    (local.set $t (table.get $coupons (i32.const 0)))
    (call $is_force_coupon (local.get $t))
    (if (result externref)
      (then
        (call $is_object (call $get_coupon_rhs (local.get $t)))
        (if (result externref)
          (then
            (call $is_equal (call $get_coupon_rhs (local.get $t)) (local.get $rhs))
            (if (result externref)
              (then
                (call $is_equal (call $create_strict_encode (call $get_coupon_lhs (local.get $t))) (local.get $lhs))
                (if (result externref)
                  (then
                    (call $create_eq_coupon (local.get $lhs) (local.get $rhs))
                  )
                  (else
                    unreachable
                  )
                )
              )
              (else
                unreachable
              )
            )
          )
          (else
            unreachable
          )
        )
      )
      (else
        unreachable
      )
    ))
(func $make_eval_blobobj_coupon (export "make_eval_blobobj_coupon") (param $lhs externref) (param $rhs externref) (result externref)
   (call $is_blob_obj (local.get $lhs))
   (if (result externref)
     (then
       (call $is_equal (local.get $lhs) (local.get $rhs))
       (if (result externref)
         (then
           (call $create_eval_coupon (local.get $lhs) (local.get $rhs))
         )
         (else
           unreachable
         )
       )
     )
     (else
       unreachable
     )
   ))
 (func $make_eq_application_coupon (export "make_eq_application_coupon") (param $lhs externref) (param $rhs externref) (result externref) (local $e externref)
   (local.set $e (table.get $coupons (i32.const 0)))
   (call $is_eq_coupon (local.get $e))
   (if (result externref)
     (then
       (local.get $lhs)
       (call $create_application_thunk (call $get_coupon_lhs (local.get $e)))
       (call $is_equal)
       (if (result externref)
         (then
           (local.get $rhs)
           (call $create_application_thunk (call $get_coupon_rhs (local.get $e)))
           (call $is_equal)
           (if (result externref)
             (then
               (call $create_eq_coupon (local.get $lhs) (local.get $rhs))
             )
             (else
               unreachable
             )
           )
         )
         (else
           unreachable
         )
       )
     )
     (else
       unreachable
     )
  ))
 (func $make_eq_encode_strict_coupon (export "make_eq_encode_strict_coupon") (param $lhs externref) (param $rhs externref) (result externref) (local $e externref)
   (local.set $e (table.get $coupons (i32.const 0)))
   (call $is_eq_coupon (local.get $e))
   (if (result externref)
     (then
       (call $is_equal (call $create_strict_encode (call $get_coupon_lhs (local.get $e))) (local.get $lhs))
       (if (result externref)
         (then
           (call $is_equal (call $create_strict_encode (call $get_coupon_rhs (local.get $e))) (local.get $rhs))
           (if (result externref)
             (then
               (call $create_eq_coupon (local.get $lhs) (local.get $rhs))
             )
             (else
               unreachable
             )
           )
         )
         (else
           unreachable
         )
       )
     )
     (else
       unreachable
     )
  ))
 (func $make_sym_coupon (export "make_sym_coupon") (param $lhs externref) (param $rhs externref) (result externref) (local $e externref)
   (local.set $e (table.get $coupons (i32.const 0)))
   (call $is_eq_coupon (local.get $e))
   (if (result externref)
     (then
       (call $is_equal (call $get_coupon_rhs (local.get $e)) (local.get $lhs))
       (if (result externref)
         (then
           (call $is_equal (call $get_coupon_lhs (local.get $e)) (local.get $rhs))
           (if (result externref)
             (then
               (call $create_eq_coupon (local.get $lhs) (local.get $rhs))
             )
             (else
               unreachable
             )
           )
         )
         (else
           unreachable
         )
       )
     )
     (else
       unreachable
     )
   ))
 (func $make_trans_coupon (export "make_trans_coupon") (param $lhs externref) (param $rhs externref) (result externref) (local $e1 externref) (local $e2 externref)
   (local.set $e1 (table.get $coupons (i32.const 0)))
   (local.set $e2 (table.get $coupons (i32.const 1)))
   (call $is_eq_coupon (local.get $e1))
   (if (result externref)
     (then
       (call $is_eq_coupon (local.get $e2))
       (if (result externref)
         (then
           (call $is_equal (call $get_coupon_rhs (local.get $e1)) (call $get_coupon_lhs (local.get $e2)))
           (if (result externref)
             (then
               (call $is_equal (local.get $lhs) (call $get_coupon_lhs (local.get $e1)))
               (if (result externref)
                 (then
                   (call $is_equal (local.get $rhs) (call $get_coupon_rhs (local.get $e2)))
                   (if (result externref)
                     (then
                       (call $create_eq_coupon (local.get $lhs) (local.get $rhs))
                     )
                     (else
                       unreachable
                     )
                   )
                 )
                 (else
                   unreachable
                 )
               )
             )
             (else
               unreachable
             )
           )
         )
         (else
           unreachable
         )
       )
     )
     (else
       unreachable
     )
  ))
 (func $make_self_coupon (export "make_self_coupon") (param $lhs externref) (param $rhs externref) (result externref)
    (call $is_equal (local.get $lhs) (local.get $rhs))
    (if (result externref)
      (then
        (call $create_eq_coupon (local.get $lhs) (local.get $rhs))
      )
      (else
        unreachable
      )
    ))
 (table $dispatch_table funcref (elem (ref.func $make_eq_tree_coupon)
                                      (ref.func $make_eq_application_coupon)
                                      (ref.func $make_force_result_eq_coupon)
                                      (ref.func $make_eq_encode_strict_coupon)
                                      (ref.func $make_think_application_coupon)
                                      (ref.func $make_think_to_force_coupon)
                                      (ref.func $make_force_to_encode_strict_coupon)
                                      (ref.func $make_eval_eq_coupon)
                                      (ref.func $make_eval_blobobj_coupon)
                                      (ref.func $make_eval_tree_coupon)
                                      (ref.func $make_sym_coupon)
                                      (ref.func $make_trans_coupon)
                                      (ref.func $make_self_coupon)))
 (func $make_coupon (export "make_coupon") (param $request i32) (param $lhs externref) (param $rhs externref) (result externref)
    local.get $request
    table.size $dispatch_table
    i32.lt_u
    if (result externref)
       local.get $lhs
       local.get $rhs
       local.get $request
       (call_indirect $dispatch_table (type $make_coupon_t))
    else
       unreachable
    end
    )
 (func (export "_fixpoint_apply") (param $combination externref) (result externref)
       ;; attach combination tree
       (call $attach_tree
             (i32.const 0)
             (local.get $combination))
       ;; attach coupons
       (call $attach_tree
             (i32.const 2)
             (table.get $encode (i32.const 2)))
       ;; attach request field
       (call $attach_blob
             (i32.const 1)
             (table.get $encode (i32.const 1)))
       (call $make_coupon
             (i32.load $mem_1 (i32.const 0))
             (table.get $encode (i32.const 3))
             (table.get $encode (i32.const 4))
       )
 )
 (export "coupons" (table $coupons))
)
