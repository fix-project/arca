(module
  ;; Validates ordered coupon exchanges without evaluating their inputs.
  ;; Premises are authored by this collector or the kernel.
  (import "fix" "get_type" (func $type (param externref) (result i32)))
  (import "fix" "get_data_type" (func $data_type (param externref) (result i32)))
  (import "fix" "get_encode_type" (func $encode_type (param externref) (result i32)))
  (import "fix" "is_tag" (func $is_tag (param externref) (result i32)))
  (import "fix" "same_handle_bytes" (func $same_handle_bytes (param externref externref) (result i32)))
  (import "fix" "equals" (func $equals (param externref externref) (result i32)))
  (import "fix" "len" (func $len (param externref) (result i64)))
  (import "fix" "get_tree_element" (func $at (param externref i32) (result externref)))
  (import "fix" "attach_tree" (func $attach_tree (param externref i32)))
  (import "fix" "read_blob" (func $read_blob (param externref i32 i32 i32) (result i32)))
  (import "fix" "create_tree" (func $create_tree (param i32 i32) (result externref)))
  (import "fix" "create_blob_i64" (func $integer (param i64) (result externref)))
  (import "fix" "create_ref" (func $create_ref (param externref) (result externref)))
  (import "fix" "create_tag" (func $create_tag (param i32) (result externref)))
  (import "fix" "create_application_thunk" (func $application (param externref) (result externref)))
  (import "fix" "create_selection_thunk" (func $selection (param externref) (result externref)))
  (import "fix" "create_identification_thunk" (func $identification (param externref) (result externref)))
  (import "fix" "create_strict_encode" (func $strict (param externref) (result externref)))
  (import "fix" "create_shallow_encode" (func $shallow (param externref) (result externref)))

  (import "fix" "create_tag_unchecked" (func $tag_unchecked (param externref) (result externref)))

  (import "fix" "get_thunk_type" (func $thunk_type (param externref) (result i32)))

  (table $output 2 externref)
  (table $assertion 3 externref)
  (table $combination_view 0 externref)
  (table $witness_view 0 externref)
  (table $claim_view 0 externref)
  (table $witnesses_view 0 externref)
  (table $coupon_view 0 externref)
  (table $lhs_view 0 externref)
  (table $rhs_view 0 externref)
  (table $claims 0 externref)
  (memory $bytes 1)

  (func $require (param $condition i32)
    (if (i32.eqz (local.get $condition)) (then unreachable)))

  (func $same (param $a externref) (param $b externref)
    (call $require (call $same_handle_bytes (local.get $a) (local.get $b))))

  (func $tree (param $value externref)
    (call $require (i32.eqz (call $type (local.get $value))))
    (call $require (i32.eq (call $data_type (local.get $value)) (i32.const 1))))

  (func $size (param $value externref) (result i32)
    (call $tree (local.get $value))
    (call $require (i64.lt_u (call $len (local.get $value)) (i64.const 134217729)))
    (i32.wrap_i64 (call $len (local.get $value))))

  (func $arity (param $value externref) (param $length i32)
    (call $require (i32.eq (call $size (local.get $value)) (local.get $length))))

  (func $u64 (param $value externref) (result i64)
    (call $require (i32.eqz (call $type (local.get $value))))
    (call $require (i32.eqz (call $data_type (local.get $value))))
    (call $require (i64.eq (call $len (local.get $value)) (i64.const 8)))
    (call $require (i32.eq (call $read_blob (local.get $value) (i32.const 0) (i32.const 0) (i32.const 8)) (i32.const 8)))
    (i64.load (i32.const 0)))

  (func $lower (param $value externref) (result externref)
    (if (i32.eqz (call $type (local.get $value)))
      (then (return (call $create_ref (local.get $value)))))
    (call $require (i32.eq (call $type (local.get $value)) (i32.const 1)))
    (local.get $value))

  (func $object (param $reference externref) (param $value externref)
    (call $require (i32.eqz (call $type (local.get $value))))
    (call $same (local.get $reference) (call $create_ref (local.get $value))))

  ;; A witness [Tag [author, Ref(claim)], claim] authenticates its supplied assertion.
  (func $claim (param $witness externref) (result externref)
    (local $coupon externref) (local $claim externref)
    (call $arity (local.get $witness) (i32.const 2))
    (call $attach_tree (local.get $witness) (i32.const 3))
    (local.set $coupon (table.get $witness_view (i32.const 0)))
    (local.set $claim (table.get $witness_view (i32.const 1)))
    (call $require (call $is_tag (local.get $coupon)))
    (call $arity (local.get $coupon) (i32.const 2))
    (call $attach_tree (local.get $coupon) (i32.const 6))
    (call $arity (local.get $claim) (i32.const 3))
    (call $object (table.get $coupon_view (i32.const 1)) (local.get $claim))
    (local.get $claim))

  (func $premise (param $kind i64) (param $claim externref)
                 (param $lhs externref) (param $rhs externref)
    (call $attach_tree (local.get $claim) (i32.const 4))
    (call $require (i64.eq (call $u64 (table.get $claim_view (i32.const 0)))
                          (local.get $kind)))
    (call $same (table.get $claim_view (i32.const 1)) (local.get $lhs))
    (call $same (table.get $claim_view (i32.const 2)) (local.get $rhs)))

  (func $left (param $claim externref) (result externref)
    (call $at (local.get $claim) (i32.const 1)))

  (func $right (param $claim externref) (result externref)
    (call $at (local.get $claim) (i32.const 2)))

  (func $data (param $value externref)
    (call $require (i32.lt_u (call $type (local.get $value)) (i32.const 2))))

  (func $map (param $constructor i32) (param $value externref) (result externref)
    (if (i32.eq (local.get $constructor) (i32.const 0)) (then
      (call $tree (local.get $value))
      (return (call $application (local.get $value)))))
    (if (i32.eq (local.get $constructor) (i32.const 1)) (then
      (call $tree (local.get $value))
      (return (call $selection (local.get $value)))))
    (if (i32.eq (local.get $constructor) (i32.const 2)) (then
      (call $require (i32.eq (call $type (local.get $value)) (i32.const 1)))
      (return (call $identification (local.get $value)))))
    (if (i32.eq (local.get $constructor) (i32.const 3)) (then
      (call $require (i32.eq (call $type (local.get $value)) (i32.const 2)))
      (return (call $strict (local.get $value)))))
    (if (i32.eq (local.get $constructor) (i32.const 4)) (then
      (call $require (i32.eq (call $type (local.get $value)) (i32.const 2)))
      (return (call $shallow (local.get $value)))))
    (call $require (i32.eqz (call $type (local.get $value))))
    (if (i32.eq (local.get $constructor) (i32.const 5)) (then
      (call $require (i32.eqz (call $data_type (local.get $value)))))
      (else
        (call $require (i32.eq (local.get $constructor) (i32.const 6)))
        (call $require (i32.eq (call $data_type (local.get $value)) (i32.const 1)))))
    (call $create_ref (local.get $value)))

  (func $mapped (param $constructor i32) (param $lhs externref) (param $rhs externref) (param $witness externref)
    (local $a externref) (local $b externref)
    (local.set $a (call $left (local.get $witness)))
    (local.set $b (call $right (local.get $witness)))
    (call $same (local.get $lhs) (call $map (local.get $constructor) (local.get $a)))
    (call $same (local.get $rhs) (call $map (local.get $constructor) (local.get $b)))
    (call $premise (i64.const 0) (local.get $witness) (local.get $a) (local.get $b)))

  (func $tree_rule (param $kind i64) (param $lhs externref) (param $rhs externref) (param $witnesses externref)
    (local $count i32) (local $index i32)
    (local.set $count (call $size (local.get $lhs)))
    (call $arity (local.get $rhs) (local.get $count))
    (call $arity (local.get $witnesses) (local.get $count))
    (call $require (i32.eq (call $is_tag (local.get $lhs)) (call $is_tag (local.get $rhs))))
    (call $attach_tree (local.get $lhs) (i32.const 7))
    (call $attach_tree (local.get $rhs) (i32.const 8))
    (block $done (loop $children
      (br_if $done (i32.eq (local.get $index) (local.get $count)))
      (call $premise (local.get $kind) (table.get $claims (local.get $index))
        (table.get $lhs_view (local.get $index)) (table.get $rhs_view (local.get $index)))
      (local.set $index (i32.add (local.get $index) (i32.const 1)))
      (br $children))))

  (func $rule (param $rule i64) (param $lhs externref) (param $rhs externref) (param $witnesses externref) (result i64)
    (local $p externref) (local $q externref)
    (local $a externref) (local $b externref)
    ;; TreeEq
    (if (i64.eq (local.get $rule) (i64.const 0)) (then
      (call $tree_rule (i64.const 0) (local.get $lhs) (local.get $rhs) (local.get $witnesses))
      (return (i64.const 0))))
    ;; EvalTree
    (if (i64.eq (local.get $rule) (i64.const 6)) (then
      (call $tree_rule (i64.const 1) (local.get $lhs) (local.get $rhs) (local.get $witnesses))
      (return (i64.const 1))))
    ;; Self
    (if (i64.eq (local.get $rule) (i64.const 12)) (then
      (call $arity (local.get $witnesses) (i32.const 0))
      (call $same (local.get $lhs) (local.get $rhs))
      (return (i64.const 0))))
    ;; EvalLeaf
    (if (i64.eq (local.get $rule) (i64.const 5)) (then
      (call $arity (local.get $witnesses) (i32.const 0))
      (call $require (i32.ne (call $type (local.get $lhs)) (i32.const 3)))
      (if (i32.eqz (call $type (local.get $lhs))) (then
        (call $require (i32.eqz (call $data_type (local.get $lhs))))))
      (call $same (local.get $lhs) (local.get $rhs))
      (return (i64.const 1))))
    ;; ThinkIdentification
    (if (i64.eq (local.get $rule) (i64.const 26)) (then
      (call $arity (local.get $witnesses) (i32.const 0))
      (call $require (i32.eq (call $type (local.get $rhs)) (i32.const 1)))
      (call $same (local.get $lhs) (call $identification (local.get $rhs)))
      (return (i64.const 4))))
    ;; ForceResultEq
    (if (i64.eq (local.get $rule) (i64.const 1)) (then
      (call $arity (local.get $witnesses) (i32.const 3))
      (local.set $p (table.get $claims (i32.const 0)))
      (local.set $q (table.get $claims (i32.const 1)))
      (local.set $a (call $right (local.get $p)))
      (local.set $b (call $right (local.get $q)))
      (call $premise (i64.const 3) (local.get $p) (local.get $lhs) (local.get $a))
      (call $premise (i64.const 3) (local.get $q) (local.get $rhs) (local.get $b))
      (call $premise (i64.const 0) (table.get $claims (i32.const 2)) (local.get $a) (local.get $b))
      (return (i64.const 0))))
    (if (i32.or (i32.or (i64.eq (local.get $rule) (i64.const 2)) (i64.eq (local.get $rule) (i64.const 8)))
        (i32.or (i64.eq (local.get $rule) (i64.const 11)) (i32.or (i64.eq (local.get $rule) (i64.const 24))
          (i64.eq (local.get $rule) (i64.const 25)))))
      (then (call $arity (local.get $witnesses) (i32.const 2)))
      (else (call $arity (local.get $witnesses) (i32.const 1))))
    (local.set $p (table.get $claims (i32.const 0)))
    ;; EqApplication
    (if (i64.eq (local.get $rule) (i64.const 4)) (then
      (call $mapped (i32.const 0) (local.get $lhs) (local.get $rhs) (local.get $p))
      (return (i64.const 0))))
    ;; EqEncodeStrict
    (if (i64.eq (local.get $rule) (i64.const 9)) (then
      (call $mapped (i32.const 3) (local.get $lhs) (local.get $rhs) (local.get $p))
      (return (i64.const 0))))
    ;; EqSelection
    (if (i64.eq (local.get $rule) (i64.const 17)) (then
      (call $mapped (i32.const 1) (local.get $lhs) (local.get $rhs) (local.get $p))
      (return (i64.const 0))))
    ;; EqIdentification
    (if (i64.eq (local.get $rule) (i64.const 18)) (then
      (call $mapped (i32.const 2) (local.get $lhs) (local.get $rhs) (local.get $p))
      (return (i64.const 0))))
    ;; EqEncodeShallow
    (if (i64.eq (local.get $rule) (i64.const 19)) (then
      (call $mapped (i32.const 4) (local.get $lhs) (local.get $rhs) (local.get $p))
      (return (i64.const 0))))
    ;; TreeRefEq
    (if (i64.eq (local.get $rule) (i64.const 21)) (then
      (call $mapped (i32.const 6) (local.get $lhs) (local.get $rhs) (local.get $p))
      (return (i64.const 0))))
    ;; BlobRefEq
    (if (i64.eq (local.get $rule) (i64.const 22)) (then
      (call $mapped (i32.const 5) (local.get $lhs) (local.get $rhs) (local.get $p))
      (return (i64.const 0))))
    ;; ThinkToForce
    (if (i64.eq (local.get $rule) (i64.const 3)) (then
      (call $data (local.get $rhs))
      (call $premise (i64.const 4) (local.get $p) (local.get $lhs) (local.get $rhs))
      (return (i64.const 3))))
    ;; Sym
    (if (i64.eq (local.get $rule) (i64.const 10)) (then
      (call $premise (i64.const 0) (local.get $p) (local.get $rhs) (local.get $lhs))
      (return (i64.const 0))))
    ;; ThunkEq
    (if (i64.eq (local.get $rule) (i64.const 13)) (then
      (call $require (i32.eq (call $type (local.get $rhs)) (i32.const 2)))
      (call $premise (i64.const 4) (local.get $p) (local.get $lhs) (local.get $rhs))
      (return (i64.const 0))))
    ;; ThunkStrictEq
    (if (i64.eq (local.get $rule) (i64.const 14)) (then
      (call $premise (i64.const 4) (local.get $p) (local.get $lhs) (call $map (i32.const 3) (local.get $rhs)))
      (return (i64.const 0))))
    ;; ThunkShallowEq
    (if (i64.eq (local.get $rule) (i64.const 15)) (then
      (call $premise (i64.const 4) (local.get $p) (local.get $lhs) (call $map (i32.const 4) (local.get $rhs)))
      (return (i64.const 0))))
    ;; EvalResultEq
    (if (i64.eq (local.get $rule) (i64.const 20)) (then
      (call $premise (i64.const 1) (local.get $p) (local.get $lhs) (local.get $rhs))
      (return (i64.const 0))))
    (local.set $a (call $left (local.get $p)))
    (local.set $b (call $right (local.get $p)))
    ;; ThinkApplication
    (if (i64.eq (local.get $rule) (i64.const 2)) (then
      (call $same (local.get $lhs) (call $map (i32.const 0) (local.get $a)))
      (call $premise (i64.const 1) (local.get $p) (local.get $a) (local.get $b))
      (call $premise (i64.const 2) (table.get $claims (i32.const 1)) (local.get $b) (local.get $rhs))
      (return (i64.const 4))))
    ;; ForceToEncodeStrict
    (if (i64.eq (local.get $rule) (i64.const 7)) (then
      (call $same (local.get $lhs) (call $map (i32.const 3) (local.get $a)))
      (call $data (local.get $b))
      (if (i32.eqz (call $type (local.get $b)))
        (then (call $same (local.get $rhs) (local.get $b)))
        (else (call $object (local.get $b) (local.get $rhs))))
      (call $premise (i64.const 3) (local.get $p) (local.get $a) (local.get $b))
      (return (i64.const 0))))
    ;; EvalEq
    (if (i64.eq (local.get $rule) (i64.const 8)) (then
      (call $premise (i64.const 1) (local.get $p) (local.get $a) (local.get $rhs))
      (call $premise (i64.const 0) (table.get $claims (i32.const 1)) (local.get $a) (local.get $lhs))
      (return (i64.const 1))))
    ;; Trans
    (if (i64.eq (local.get $rule) (i64.const 11)) (then
      (call $premise (i64.const 0) (local.get $p) (local.get $lhs) (local.get $b))
      (call $premise (i64.const 0) (table.get $claims (i32.const 1)) (local.get $b) (local.get $rhs))
      (return (i64.const 0))))
    ;; ForceToEncodeShallow
    (if (i64.eq (local.get $rule) (i64.const 16)) (then
      (call $same (local.get $lhs) (call $map (i32.const 4) (local.get $a)))
      (call $same (local.get $rhs) (call $lower (local.get $b)))
      (call $premise (i64.const 3) (local.get $p) (local.get $a) (local.get $b))
      (return (i64.const 0))))
    ;; ForceEq
    (if (i64.eq (local.get $rule) (i64.const 24)) (then
      (call $premise (i64.const 0) (local.get $p) (local.get $a) (local.get $lhs))
      (call $premise (i64.const 3) (table.get $claims (i32.const 1)) (local.get $a) (local.get $rhs))
      (return (i64.const 3))))
    ;; ThinkSelection
    (if (i64.eq (local.get $rule) (i64.const 25)) (then
      (call $same (local.get $lhs) (call $map (i32.const 1) (local.get $a)))
      (call $premise (i64.const 1) (local.get $p) (local.get $a) (local.get $b))
      (call $premise (i64.const 6) (table.get $claims (i32.const 1)) (local.get $b) (local.get $rhs))
      (return (i64.const 4))))
    unreachable)

  (func $collect (param $combination externref) (result externref)
    (local $lhs externref) (local $rhs externref) (local $kind i64)
    (call $arity (local.get $combination) (i32.const 6))
    (local.set $lhs (table.get $combination_view (i32.const 3)))
    (local.set $rhs (table.get $combination_view (i32.const 4)))
    (local.set $kind (call $exchange (call $u64 (table.get $combination_view (i32.const 2)))
      (local.get $lhs) (local.get $rhs) (table.get $combination_view (i32.const 5))))
    (call $issue (local.get $combination) (local.get $lhs) (local.get $rhs) (local.get $kind)))

  (func $validate_witnesses (param $combination externref)
    (local $collector externref) (local $witnesses externref)
    (local $witness externref) (local $author externref)
    (local $count i32) (local $index i32)
    (local.set $collector (table.get $combination_view (i32.const 1)))
    (local.set $witnesses (table.get $combination_view (i32.const 5)))
    (local.set $count (call $size (local.get $witnesses)))
    (if (i32.lt_u (table.size $claims) (local.get $count)) (then
      (call $require (i32.ne
        (table.grow $claims (ref.null extern) (i32.sub (local.get $count) (table.size $claims)))
        (i32.const -1)))))
    (call $attach_tree (local.get $witnesses) (i32.const 5))
    (block $done (loop $next
      (br_if $done (i32.eq (local.get $index) (local.get $count)))
      (local.set $witness (table.get $witnesses_view (local.get $index)))
      (table.set $claims (local.get $index) (call $claim (local.get $witness)))
      (local.set $author (table.get $coupon_view (i32.const 0)))
      (block $accepted
        (if (i32.eqz (call $type (local.get $author))) (then
          (if (i32.eq (call $data_type (local.get $author)) (i32.const 1)) (then
            (br_if $accepted (i32.and (i32.eqz (call $is_tag (local.get $author)))
                                     (i64.eqz (call $len (local.get $author)))))))))
        (call $same (local.get $collector) (local.get $author)))
      (local.set $index (i32.add (local.get $index) (i32.const 1)))
      (br $next))))

  (func $reconstruct (param $combination externref) (result externref)
    (local $lhs externref) (local $fields externref) (local $witnesses externref) (local $tag externref)
    (call $arity (local.get $combination) (i32.const 6))
    (local.set $lhs (table.get $combination_view (i32.const 3)))
    (call $require (call $is_tag (local.get $lhs)))
    (local.set $fields (table.get $combination_view (i32.const 4)))
    (local.set $witnesses (table.get $combination_view (i32.const 5)))
    (local.set $tag (call $tag_unchecked (local.get $fields)))
    (call $tree_rule (i64.const 1) (local.get $lhs) (local.get $tag) (local.get $witnesses))
    (local.get $tag))

  (func $issue (param $combination externref) (param $lhs externref) (param $rhs externref) (param $kind i64) (result externref)
    (table.set $assertion (i32.const 0) (call $integer (local.get $kind)))
    (table.set $assertion (i32.const 1) (local.get $lhs))
    (table.set $assertion (i32.const 2) (local.get $rhs))
    (local.set $rhs (call $create_ref (call $create_tree (i32.const 1) (i32.const 3))))
    (table.set $output (i32.const 0) (table.get $combination_view (i32.const 1)))
    (table.set $output (i32.const 1) (local.get $rhs))
    (call $create_tag (i32.const 0)))

  (func (export "_fix_apply") (param $combination externref) (result externref)
    (local $lhs externref) (local $rhs externref)
    (call $arity (local.get $combination) (i32.const 6))
    (call $attach_tree (local.get $combination) (i32.const 2))
    (call $validate_witnesses (local.get $combination))
    (local.set $lhs (table.get $combination_view (i32.const 3)))
    (local.set $rhs (table.get $combination_view (i32.const 4)))
    (if (i32.and
          (i32.and
            (i64.eq (call $u64 (table.get $combination_view (i32.const 2))) (i64.const 5))
            (call $is_tag (local.get $lhs)))
          (i32.eqz (call $is_tag (local.get $rhs))))
      (then
        (local.set $rhs (call $reconstruct (local.get $combination)))
        (return (call $issue (local.get $combination) (local.get $lhs) (local.get $rhs) (i64.const 1)))))
    (call $collect (local.get $combination)))

  (func $resolve (param $operation i64) (param $lhs externref) (result i64)
    (local $type i32)
    (local.set $type (call $type (local.get $lhs)))
    (if (i64.eq (local.get $operation) (i64.const 0)) (then
      (if (i32.eqz (local.get $type)) (then
        (call $require (i32.eq (call $data_type (local.get $lhs)) (i32.const 1)))
        (return (i64.const 0))))
      (if (i32.eq (local.get $type) (i32.const 1)) (then
        (if (call $data_type (local.get $lhs))
          (then (return (i64.const 21)))
          (else (return (i64.const 22))))))
      (if (i32.eq (local.get $type) (i32.const 2)) (then
        (local.set $type (call $thunk_type (local.get $lhs)))
        (if (i32.eqz (local.get $type)) (then (return (i64.const 4))))
        (if (i32.eq (local.get $type) (i32.const 1)) (then (return (i64.const 17))))
        (return (i64.const 18))))
      (call $require (i32.eq (local.get $type) (i32.const 3)))
      (if (call $encode_type (local.get $lhs))
        (then (return (i64.const 19)))
        (else (return (i64.const 9))))))
    (if (i64.eq (local.get $operation) (i64.const 2)) (then
      (call $require (i32.eq (local.get $type) (i32.const 2)))
      (local.set $type (call $thunk_type (local.get $lhs)))
      (if (i32.eqz (local.get $type)) (then (return (i64.const 2))))
      (if (i32.eq (local.get $type) (i32.const 1)) (then (return (i64.const 25))))
      (return (i64.const 26))))
    (if (i64.eq (local.get $operation) (i64.const 5)) (then
      (if (i32.eqz (local.get $type)) (then
        (if (call $data_type (local.get $lhs)) (then (return (i64.const 6))))))
      (return (i64.const 5))))
    (if (i64.eq (local.get $operation) (i64.const 7)) (then
      (call $require (i32.eq (local.get $type) (i32.const 3)))
      (if (call $encode_type (local.get $lhs))
        (then (return (i64.const 16)))
        (else (return (i64.const 7))))))
    (call $require (i32.or (i64.eq (local.get $operation) (i64.const 1)) (i32.or (i64.eq (local.get $operation) (i64.const 3)) (i32.or (i64.eq (local.get $operation) (i64.const 8)) (i32.or (i64.eq (local.get $operation) (i64.const 10)) (i32.or (i64.eq (local.get $operation) (i64.const 11)) (i32.or (i64.eq (local.get $operation) (i64.const 12)) (i32.or (i64.eq (local.get $operation) (i64.const 13)) (i32.or (i64.eq (local.get $operation) (i64.const 14)) (i32.or (i64.eq (local.get $operation) (i64.const 15)) (i32.or (i64.eq (local.get $operation) (i64.const 20)) (i64.eq (local.get $operation) (i64.const 24)))))))))))))
    (return (local.get $operation)))

  (func $exchange (param $operation i64) (param $lhs externref) (param $rhs externref) (param $witnesses externref) (result i64)
    (call $rule (call $resolve (local.get $operation) (local.get $lhs))
      (local.get $lhs) (local.get $rhs) (local.get $witnesses)))

)
