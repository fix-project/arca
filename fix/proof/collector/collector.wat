(module
  ;; Validates ordered coupon exchanges without evaluating their inputs.
  ;; Issued claims retain the premise authors and gain the collector's identity.
  (import "fix" "get_type" (func $type (param externref) (result i32)))
  (import "fix" "get_data_type" (func $data_type (param externref) (result i32)))
  (import "fix" "get_encode_type" (func $encode_type (param externref) (result i32)))
  (import "fix" "is_tag" (func $is_tag (param externref) (result i32)))
  (import "fix" "unsound_handle_bytes" (func $handle_bytes (param externref) (result externref)))
  (import "fix" "equals" (func $equals (param externref externref) (result i32)))
  (import "fix" "len" (func $len (param externref) (result i64)))
  (import "fix" "get_tree_element" (func $at (param externref i32) (result externref)))
  (import "fix" "attach_blob" (func $attach_blob (param externref i32)))
  (import "fix" "create_tree" (func $create_tree (param i32 i32) (result externref)))
  (import "fix" "create_blob_i64" (func $integer (param i64) (result externref)))
  (import "fix" "create_ref" (func $create_ref (param externref) (result externref)))
  (import "fix" "create_tag" (func $create_tag (param i32 i32) (result externref)))
  (import "fix" "create_application_thunk" (func $application (param externref) (result externref)))
  (import "fix" "create_selection_thunk" (func $selection (param externref) (result externref)))
  (import "fix" "create_identification_thunk" (func $identification (param externref) (result externref)))
  (import "fix" "create_strict_encode" (func $strict (param externref) (result externref)))
  (import "fix" "create_shallow_encode" (func $shallow (param externref) (result externref)))

  (table $authors 0 externref)
  (table $output 2 externref)
  (table $assertion 3 externref)
  (memory $bytes 0)

  (func $require (param $condition i32)
    (if (i32.eqz (local.get $condition)) (then unreachable)))

  (func $same (param $a externref) (param $b externref)
    (call $require (call $equals (call $handle_bytes (local.get $a))
                                (call $handle_bytes (local.get $b)))))

  (func $tree (param $value externref)
    (call $require (i32.eqz (call $type (local.get $value))))
    (call $require (i32.eq (call $data_type (local.get $value)) (i32.const 1))))

  (func $size (param $value externref) (result i32)
    (call $tree (local.get $value))
    (call $require (i64.le_u (call $len (local.get $value)) (i64.const 4294967295)))
    (i32.wrap_i64 (call $len (local.get $value))))

  (func $arity (param $value externref) (param $length i32)
    (call $require (i32.eq (call $size (local.get $value)) (local.get $length))))

  (func $u64 (param $value externref) (result i64)
    (call $require (i32.eqz (call $type (local.get $value))))
    (call $require (i32.eqz (call $data_type (local.get $value))))
    (call $require (i64.eq (call $len (local.get $value)) (i64.const 8)))
    (call $attach_blob (local.get $value) (i32.const 0))
    (i64.load (i32.const 0)))

  (func $lower (param $value externref) (result externref)
    (if (i32.eqz (call $type (local.get $value)))
      (then (return (call $create_ref (local.get $value)))))
    (call $require (i32.eq (call $type (local.get $value)) (i32.const 1)))
    (local.get $value))

  (func $object (param $reference externref) (param $value externref)
    (call $require (i32.eqz (call $type (local.get $value))))
    (call $same (local.get $reference) (call $create_ref (local.get $value))))

  ;; A witness [Tag [authors, Ref(claim)], claim] authenticates its supplied assertion.
  (func $claim (param $witness externref) (result externref)
    (local $coupon externref) (local $claim externref)
    (call $arity (local.get $witness) (i32.const 2))
    (local.set $coupon (call $at (local.get $witness) (i32.const 0)))
    (local.set $claim (call $at (local.get $witness) (i32.const 1)))
    (call $require (call $is_tag (local.get $coupon)))
    (call $arity (local.get $coupon) (i32.const 2))
    (call $arity (local.get $claim) (i32.const 3))
    (call $object (call $at (local.get $coupon) (i32.const 1)) (local.get $claim))
    (local.get $claim))

  (func $premise (param $kind i64) (param $witness externref)
                 (param $lhs externref) (param $rhs externref)
    (local $coupon externref) (local $claim externref)
    (local $authors externref)
    (local $i i32) (local $offset i32) (local $length i32)
    (local.set $claim (call $claim (local.get $witness)))
    (local.set $coupon (call $at (local.get $witness) (i32.const 0)))
    (call $require (i64.eq (call $u64 (call $at (local.get $claim) (i32.const 0)))
                          (local.get $kind)))
    (call $same (call $at (local.get $claim) (i32.const 1)) (local.get $lhs))
    (call $same (call $at (local.get $claim) (i32.const 2)) (local.get $rhs))
    (local.set $authors (call $at (local.get $coupon) (i32.const 0)))
    (local.set $length (call $size (local.get $authors)))
    (local.set $offset (table.grow $authors (ref.null extern) (local.get $length)))
    (call $require (i32.ne (local.get $offset) (i32.const -1)))
    (block $done (loop $next
      (br_if $done (i32.eq (local.get $i) (local.get $length)))
      (table.set $authors (i32.add (local.get $offset) (local.get $i))
        (call $at (local.get $authors) (local.get $i)))
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br $next))))

  (func $left (param $witness externref) (result externref)
    (call $at (call $at (local.get $witness) (i32.const 1)) (i32.const 1)))

  (func $right (param $witness externref) (result externref)
    (call $at (call $at (local.get $witness) (i32.const 1)) (i32.const 2)))

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
    (block $done (loop $children
      (br_if $done (i32.eq (local.get $index) (local.get $count)))
      (call $premise (local.get $kind) (call $at (local.get $witnesses) (local.get $index))
        (call $at (local.get $lhs) (local.get $index)) (call $at (local.get $rhs) (local.get $index)))
      (local.set $index (i32.add (local.get $index) (i32.const 1)))
      (br $children))))

  (func $exchange (param $rule i64) (param $lhs externref) (param $rhs externref) (param $witnesses externref) (result i64)
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
      (local.set $p (call $at (local.get $witnesses) (i32.const 0)))
      (local.set $q (call $at (local.get $witnesses) (i32.const 1)))
      (local.set $a (call $right (local.get $p)))
      (local.set $b (call $right (local.get $q)))
      (call $premise (i64.const 3) (local.get $p) (local.get $lhs) (local.get $a))
      (call $premise (i64.const 3) (local.get $q) (local.get $rhs) (local.get $b))
      (call $premise (i64.const 0) (call $at (local.get $witnesses) (i32.const 2)) (local.get $a) (local.get $b))
      (return (i64.const 0))))
    (if (i32.or (i32.or (i64.eq (local.get $rule) (i64.const 2)) (i64.eq (local.get $rule) (i64.const 8)))
        (i32.or (i64.eq (local.get $rule) (i64.const 11)) (i32.or (i64.eq (local.get $rule) (i64.const 24))
          (i64.eq (local.get $rule) (i64.const 25)))))
      (then (call $arity (local.get $witnesses) (i32.const 2)))
      (else (call $arity (local.get $witnesses) (i32.const 1))))
    (local.set $p (call $at (local.get $witnesses) (i32.const 0)))
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
      (call $premise (i64.const 2) (call $at (local.get $witnesses) (i32.const 1)) (local.get $b) (local.get $rhs))
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
      (call $premise (i64.const 0) (call $at (local.get $witnesses) (i32.const 1)) (local.get $a) (local.get $lhs))
      (return (i64.const 1))))
    ;; Trans
    (if (i64.eq (local.get $rule) (i64.const 11)) (then
      (call $premise (i64.const 0) (local.get $p) (local.get $lhs) (local.get $b))
      (call $premise (i64.const 0) (call $at (local.get $witnesses) (i32.const 1)) (local.get $b) (local.get $rhs))
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
      (call $premise (i64.const 3) (call $at (local.get $witnesses) (i32.const 1)) (local.get $a) (local.get $rhs))
      (return (i64.const 3))))
    ;; ThinkSelection
    (if (i64.eq (local.get $rule) (i64.const 25)) (then
      (call $same (local.get $lhs) (call $map (i32.const 1) (local.get $a)))
      (call $premise (i64.const 1) (local.get $p) (local.get $a) (local.get $b))
      (call $premise (i64.const 6) (call $at (local.get $witnesses) (i32.const 1)) (local.get $b) (local.get $rhs))
      (return (i64.const 4))))
    unreachable)

  (func (export "_fix_apply") (param $combination externref) (result externref)
    (local $lhs externref) (local $rhs externref) (local $kind i64)
    (call $arity (local.get $combination) (i32.const 6))
    (local.set $lhs (call $at (local.get $combination) (i32.const 3)))
    (local.set $rhs (call $at (local.get $combination) (i32.const 4)))
    (local.set $kind (call $exchange (call $u64 (call $at (local.get $combination) (i32.const 2)))
      (local.get $lhs) (local.get $rhs) (call $at (local.get $combination) (i32.const 5))))
    (table.set $assertion (i32.const 0) (call $integer (local.get $kind)))
    (table.set $assertion (i32.const 1) (local.get $lhs))
    (table.set $assertion (i32.const 2) (local.get $rhs))
    (local.set $rhs (call $create_ref (call $create_tree (i32.const 2) (i32.const 3))))
    (table.set $output (i32.const 0) (call $create_tree (i32.const 0) (table.size $authors)))
    (table.set $output (i32.const 1) (local.get $rhs))
    (call $create_tag (i32.const 1) (i32.const 1)))
)
