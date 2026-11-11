(module
  ;; Validates ordered coupon exchanges without evaluating their inputs.
  ;; Premises are authored by this collector or the kernel.
  (import "fix" "get_type" (func $type (param i32) (result i32)))
  (import "fix" "get_data_type" (func $data_type (param i32) (result i32)))
  (import "fix" "get_encode_type" (func $encode_type (param i32) (result i32)))
  (import "fix" "is_tag" (func $is_tag (param i32) (result i32)))
  (import "fix" "same_handle_bytes" (func $same_handle_bytes (param i32 i32) (result i32)))
  (import "fix" "equals" (func $equals (param i32 i32) (result i32)))
  (import "fix" "len" (func $len (param i32) (result i64)))
  (import "fix" "get_tree_element" (func $at (param i32 i32) (result i32)))

  (import "fix" "access_blob" (func $read_blob (param i32 i32 i32 i32) (result i32)))
  (import "fix" "create_tree" (func $create_tree (param i32 i32) (result i32)))
  (import "fix" "create_blob_i64" (func $integer (param i64) (result i32)))
  (import "fix" "create_ref" (func $create_ref (param i32) (result i32)))
  (import "fix" "create_tag" (func $create_tag (param i32) (result i32)))
  (import "fix" "create_application_thunk" (func $application (param i32) (result i32)))
  (import "fix" "create_selection_thunk" (func $selection (param i32) (result i32)))
  (import "fix" "create_identification_thunk" (func $identification (param i32) (result i32)))
  (import "fix" "create_strict_encode" (func $strict (param i32) (result i32)))
  (import "fix" "create_shallow_encode" (func $shallow (param i32) (result i32)))

  (import "fix" "create_tag_unchecked" (func $tag_unchecked (param i32) (result i32)))

  (import "fix" "get_thunk_type" (func $thunk_type (param i32) (result i32)))

  (import "fix" "is_value" (func $is_value (param i32) (result i32)))
  (import "fix" "clone" (func $clone (param i32) (result i32)))

  (memory $bytes 1)

  (func $require (param $condition i32)
    (if (i32.eqz (local.get $condition)) (then unreachable)))

  (func $same (param $a i32) (param $b i32)
    (call $require (call $same_handle_bytes (local.get $a) (local.get $b))))

  (func $tree (param $value i32)
    (call $require (i32.eqz (call $type (local.get $value))))
    (call $require (i32.eq (call $data_type (local.get $value)) (i32.const 1))))

  (func $size (param $value i32) (result i32)
    (call $tree (local.get $value))
    (call $require (i64.lt_u (call $len (local.get $value)) (i64.const 134217729)))
    (i32.wrap_i64 (call $len (local.get $value))))

  (func $arity (param $value i32) (param $length i32)
    (call $require (i32.eq (call $size (local.get $value)) (local.get $length))))

  (func $u64 (param $value i32) (result i64)
    (call $require (i32.eqz (call $type (local.get $value))))
    (call $require (i32.eqz (call $data_type (local.get $value))))
    (call $require (i64.eq (call $len (local.get $value)) (i64.const 8)))
    (call $require (i32.eq (call $read_blob (local.get $value) (i32.const 0) (i32.const 0) (i32.const 8)) (i32.const 8)))
    (i64.load (i32.const 0)))

  (func $lower (param $value i32) (result i32)
    (if (i32.eqz (call $type (local.get $value)))
      (then (return (call $create_ref (local.get $value)))))
    (call $require (i32.eq (call $type (local.get $value)) (i32.const 1)))
    (local.get $value))

  (func $object (param $reference i32) (param $value i32)
    (call $require (i32.eqz (call $type (local.get $value))))
    (call $same (local.get $reference) (call $create_ref (local.get $value))))

  ;; A witness [Tag [author, Ref(claim)], claim] authenticates its supplied assertion.
  (func $claim (param $witness i32) (result i32)
    (local $coupon i32) (local $claim i32)
    (call $arity (local.get $witness) (i32.const 2))
    (local.set $coupon (call $at (local.get $witness) (i32.const 0)))
    (local.set $claim (call $at (local.get $witness) (i32.const 1)))
    (call $require (call $is_tag (local.get $coupon)))
    (call $arity (local.get $coupon) (i32.const 2))
    (call $arity (local.get $claim) (i32.const 3))
    (call $object (call $at (local.get $coupon) (i32.const 1)) (local.get $claim))
    (local.get $claim))

  (func $premise (param $kind i64) (param $claim i32) (param $lhs i32) (param $rhs i32)
    (call $require (i64.eq (call $u64 (call $at (local.get $claim) (i32.const 0))) (local.get $kind)))
    (call $same (call $at (local.get $claim) (i32.const 1)) (local.get $lhs))
    (call $same (call $at (local.get $claim) (i32.const 2)) (local.get $rhs)))

  (func $left (param $claim i32) (result i32)
    (call $at (local.get $claim) (i32.const 1)))

  (func $right (param $claim i32) (result i32)
    (call $at (local.get $claim) (i32.const 2)))

  (func $data (param $value i32)
    (call $require (i32.lt_u (call $type (local.get $value)) (i32.const 2))))

  (func $map (param $constructor i32) (param $value i32) (result i32)
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
    (call $require (call $is_value (local.get $value)))
    (call $create_ref (local.get $value)))

  (func $mapped (param $constructor i32) (param $lhs i32) (param $rhs i32) (param $witness i32)
    (local $a i32) (local $b i32)
    (local.set $a (call $left (local.get $witness)))
    (local.set $b (call $right (local.get $witness)))
    (call $same (local.get $lhs) (call $map (local.get $constructor) (local.get $a)))
    (call $same (local.get $rhs) (call $map (local.get $constructor) (local.get $b)))
    (call $premise (i64.const 0) (local.get $witness) (local.get $a) (local.get $b)))

  (func $tree_rule (param $kind i64) (param $lhs i32) (param $rhs i32) (param $witnesses i32)
    (local $count i32) (local $index i32)
    (local.set $count (call $size (local.get $lhs)))
    (call $arity (local.get $rhs) (local.get $count))
    (call $arity (local.get $witnesses) (local.get $count))
    (call $require (i32.eq (call $is_tag (local.get $lhs)) (call $is_tag (local.get $rhs))))
    (block $done (loop $children
      (br_if $done (i32.eq (local.get $index) (local.get $count)))
      (call $premise (local.get $kind)
        (call $witness_claim (local.get $witnesses) (local.get $index))
        (call $at (local.get $lhs) (local.get $index))
        (call $at (local.get $rhs) (local.get $index)))
      (local.set $index (i32.add (local.get $index) (i32.const 1)))
      (br $children))))

  (func $rule (param $rule i64) (param $lhs i32) (param $rhs i32) (param $witnesses i32) (result i64)
    (local $p i32) (local $q i32) (local $a i32) (local $b i32)
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
      (local.set $p (call $witness_claim (local.get $witnesses) (i32.const 0)))
      (local.set $q (call $witness_claim (local.get $witnesses) (i32.const 1)))
      (local.set $a (call $right (local.get $p)))
      (local.set $b (call $right (local.get $q)))
      (call $premise (i64.const 3) (local.get $p) (local.get $lhs) (local.get $a))
      (call $premise (i64.const 3) (local.get $q) (local.get $rhs) (local.get $b))
      (call $premise (i64.const 0) (call $witness_claim (local.get $witnesses) (i32.const 2)) (call $lower (local.get $a)) (call $lower (local.get $b)))
      (return (i64.const 0))))
    (if (i32.or (i32.or (i64.eq (local.get $rule) (i64.const 2)) (i64.eq (local.get $rule) (i64.const 8)))
        (i32.or (i64.eq (local.get $rule) (i64.const 11)) (i32.or (i64.eq (local.get $rule) (i64.const 24))
          (i64.eq (local.get $rule) (i64.const 25)))))
      (then (call $arity (local.get $witnesses) (i32.const 2)))
      (else (call $arity (local.get $witnesses) (i32.const 1))))
    (local.set $p (call $witness_claim (local.get $witnesses) (i32.const 0)))
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
      (call $premise (i64.const 2) (local.tee $q (call $witness_claim (local.get $witnesses) (i32.const 1))) (local.get $b) (local.get $rhs))
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
      (call $premise (i64.const 0) (local.tee $q (call $witness_claim (local.get $witnesses) (i32.const 1))) (local.get $a) (local.get $lhs))
      (return (i64.const 1))))
    ;; Trans
    (if (i64.eq (local.get $rule) (i64.const 11)) (then
      (call $premise (i64.const 0) (local.get $p) (local.get $lhs) (local.get $b))
      (call $premise (i64.const 0) (local.tee $q (call $witness_claim (local.get $witnesses) (i32.const 1))) (local.get $b) (local.get $rhs))
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
      (call $premise (i64.const 3) (local.tee $q (call $witness_claim (local.get $witnesses) (i32.const 1))) (local.get $a) (local.get $rhs))
      (return (i64.const 3))))
    ;; ThinkSelection
    (if (i64.eq (local.get $rule) (i64.const 25)) (then
      (call $same (local.get $lhs) (call $map (i32.const 1) (local.get $a)))
      (call $premise (i64.const 1) (local.get $p) (local.get $a) (local.get $b))
      (call $premise (i64.const 6) (local.tee $q (call $witness_claim (local.get $witnesses) (i32.const 1))) (local.get $b) (local.get $rhs))
      (return (i64.const 4))))
    unreachable)

  (func $collect (param $combination i32) (result i32)
    (local $lhs i32) (local $rhs i32) (local $operation i32) (local $witnesses i32) (local $kind i64)
    (call $arity (local.get $combination) (i32.const 6))
    (local.set $lhs (call $at (local.get $combination) (i32.const 3)))
    (local.set $rhs (call $at (local.get $combination) (i32.const 4)))
    (local.set $operation (call $at (local.get $combination) (i32.const 2)))
    (local.set $witnesses (call $at (local.get $combination) (i32.const 5)))
    (local.set $kind (call $exchange (call $u64 (local.get $operation)) (local.get $lhs) (local.get $rhs) (local.get $witnesses)))
    (call $issue (local.get $combination) (local.get $lhs) (local.get $rhs) (local.get $kind)))

  (func $validate_witnesses (param $combination i32)
    (local $collector i32) (local $witnesses i32)
    (local $witness i32) (local $author i32)
    (local $count i32) (local $index i32)
    (local.set $collector (call $at (local.get $combination) (i32.const 1)))
    (local.set $witnesses (call $at (local.get $combination) (i32.const 5)))
    (local.set $count (call $size (local.get $witnesses)))
    (call $require (i32.le_u (local.get $count) (i32.const 1048576)))
    (block $done (loop $next
      (br_if $done (i32.eq (local.get $index) (local.get $count)))
      (local.set $witness (call $at (local.get $witnesses) (local.get $index)))
      (drop (call $claim (local.get $witness)))
      (local.set $author (call $at (call $at (local.get $witness) (i32.const 0)) (i32.const 0)))
      (block $accepted
        (if (i32.eqz (call $type (local.get $author))) (then
          (if (i32.eq (call $data_type (local.get $author)) (i32.const 1)) (then
            (br_if $accepted (i32.and (i32.eqz (call $is_tag (local.get $author)))
                                     (i64.eqz (call $len (local.get $author)))))))))
        (call $same (local.get $collector) (local.get $author)))

      (local.set $index (i32.add (local.get $index) (i32.const 1)))
      (br $next)))
     )

  (func $reconstruct (param $combination i32) (result i32)
    (local $lhs i32) (local $fields i32) (local $witnesses i32) (local $tag i32)
    (call $arity (local.get $combination) (i32.const 6))
    (local.set $lhs (call $at (local.get $combination) (i32.const 3)))
    (call $require (call $is_tag (local.get $lhs)))
    (local.set $fields (call $at (local.get $combination) (i32.const 4)))
    (local.set $witnesses (call $at (local.get $combination) (i32.const 5)))
    (local.set $tag (call $tag_unchecked (local.get $fields)))
    (call $tree_rule (i64.const 1) (local.get $lhs) (local.get $tag) (local.get $witnesses))

    (local.get $tag))

  (func $issue (param $combination i32) (param $lhs i32) (param $rhs i32) (param $kind i64) (result i32)
    (local $reference i32)
    (i32.store (i32.const 32) (call $integer (local.get $kind)))
    (i32.store (i32.const 36) (local.get $lhs))
    (i32.store (i32.const 40) (local.get $rhs))
    (local.set $reference (call $create_ref (call $create_tree (i32.const 32) (i32.const 3))))
    (i32.store (i32.const 16) (call $at (local.get $combination) (i32.const 1)))
    (i32.store (i32.const 20) (local.get $reference))
    (call $create_tag (call $create_tree (i32.const 16) (i32.const 2))))

  (func (export "_fix_apply") (param $combination i32) (result i32)
    (local $lhs i32) (local $rhs i32) (local $operation i32)
    (call $arity (local.get $combination) (i32.const 6))
    (call $validate_witnesses (local.get $combination))
    (local.set $lhs (call $at (local.get $combination) (i32.const 3)))
    (local.set $rhs (call $at (local.get $combination) (i32.const 4)))
    (local.set $operation (call $at (local.get $combination) (i32.const 2)))
    (if (i32.and
          (i32.and (i64.eq (call $u64 (local.get $operation)) (i64.const 5))
                   (call $is_tag (local.get $lhs)))
          (i32.eqz (call $is_tag (local.get $rhs))))
      (then
        (local.set $rhs (call $reconstruct (local.get $combination)))
        (return (call $issue (local.get $combination) (local.get $lhs) (local.get $rhs) (i64.const 1)))))
    (call $collect (local.get $combination)))

  (func $resolve (param $operation i64) (param $lhs i32) (result i64)
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

  (func $exchange (param $operation i64) (param $lhs i32) (param $rhs i32) (param $witnesses i32) (result i64)
    (call $rule (call $resolve (local.get $operation) (local.get $lhs))
      (local.get $lhs) (local.get $rhs) (local.get $witnesses)))

  (func $witness_claim (param $witnesses i32) (param $index i32) (result i32)
    (call $claim (call $at (local.get $witnesses) (local.get $index))))
)
