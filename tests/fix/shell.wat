(module
  (import "fix" "detach_blob" (func $detach_blob (param i32 i32)))
  (import "fix" "detach_tree" (func $detach_tree (param i32 i32)))
  (import "fix" "attach_blob" (func $attach_blob (param externref i32)))
  (import "fix" "attach_tree" (func $attach_tree (param externref i32)))
  (import "fix" "create_blob" (func $create_blob (param i32 i32) (result externref)))
  (import "fix" "create_blob_i64" (func $create_blob_i64 (param i64) (result externref)))
  (import "fix" "create_tree" (func $create_tree (param i32 i32) (result externref)))
  (import "fix" "create_tag" (func $create_tag (param i32) (result externref)))
  (import "fix" "is_tag" (func $is_tag (param externref) (result i32)))
  (import "fix" "is_data" (func $is_data (param externref) (result i32)))
  (tag $large (param i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64 i64))
  (tag $small (param i32))
  (type $answer_type (func (result i64)))
  (memory $source 33 40)
  (memory $attached 0)
  (memory $bounded 1 2)
  (memory $empty 0)
  (table $combination 0 externref)
  (table $source_tree 2049 2050 externref)
  (table $attached_tree 0 externref)
  (table $tag 3 externref)
  (table $functions 1 2 funcref)
  (elem declare func $answer)
  (func $assert (param $condition i32)
    (if (i32.eqz (local.get $condition)) (then unreachable)))
  (func $assert_blob (param $blob externref)
    (call $attach_blob (local.get $blob) (i32.const 1))
    (call $assert (i64.eq (i64.load $attached (i32.const 0)) (i64.const 42))))
  (func $answer (type $answer_type) (result i64) (i64.const 42))
  (func $throw_large
    (throw $large (i64.const 1) (i64.const 2) (i64.const 3) (i64.const 4) (i64.const 5) (i64.const 6) (i64.const 7) (i64.const 8) (i64.const 9) (i64.const 10) (i64.const 11) (i64.const 12) (i64.const 13) (i64.const 14) (i64.const 15) (i64.const 16) (i64.const 17) (i64.const 18) (i64.const 19) (i64.const 20) (i64.const 21) (i64.const 22) (i64.const 23) (i64.const 24) (i64.const 25) (i64.const 26) (i64.const 27) (i64.const 28) (i64.const 29) (i64.const 30) (i64.const 31) (i64.const 32) (i64.const 33) (i64.const 34) (i64.const 35) (i64.const 36) (i64.const 37) (i64.const 38) (i64.const 39) (i64.const 40)))
  (func $throw_small (throw $small (i32.const 7)))
  (func $nested_small
    (try
      (do (call $throw_small) unreachable)
      (catch $small (call $assert (i32.eq (i32.const 7))))))
  (func $rethrow_large
    (try
      (do (call $throw_large) unreachable)
      (catch $large
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        drop
        (call $nested_small)
        (rethrow 0))))
  (func $check_exceptions
    (try
      (do (call $rethrow_large) unreachable)
      (catch $large
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        i64.add
        (call $assert (i64.eq (i64.const 820))))))
  (func (export "_fix_apply") (param $input externref) (result externref)
    (local $blob externref)
    (local $tree externref)
    (call $check_exceptions)
    (call $assert (call $is_data (ref.null extern)))
    (call $assert (ref.is_null (ref.null extern)))
    (call $assert (ref.is_null (call $create_blob (i32.const 0) (i32.const 0))))
    (call $attach_blob (ref.null extern) (i32.const 3))
    (call $assert (i32.eqz (memory.size $empty)))
    (call $attach_tree (call $create_tree (i32.const 3) (i32.const 3)) (i32.const 2))
    (call $assert (i32.eq (table.size $attached_tree) (i32.const 3)))
    (call $assert (call $is_data (table.get $attached_tree (i32.const 0))))
    (call $attach_tree (local.get $input) (i32.const 0))
    (call $attach_tree (local.get $input) (i32.const 0))
    (call $assert (i32.eq (table.size $combination) (i32.const 1)))
    (call $assert (i32.eq (memory.grow $bounded (i32.const 1)) (i32.const 1)))
    (call $assert (i32.eq (memory.grow $bounded (i32.const 0)) (i32.const 2)))
    (call $assert (i32.eq (memory.grow $bounded (i32.const 1)) (i32.const -1)))
    (call $assert (i32.eq (memory.grow $bounded (i32.const -1)) (i32.const -1)))
    (i64.store $source (i32.const 0) (i64.const 42))
    (local.set $blob (call $create_blob (i32.const 0) (i32.const 2162688)))
    (call $attach_blob (local.get $blob) (i32.const 1))
    (call $assert (i32.eq (memory.size $attached) (i32.const 33)))
    (call $assert (i64.eq (i64.load $attached (i32.const 0)) (i64.const 42)))
    (call $assert (i32.eq (memory.grow $attached (i32.const 1)) (i32.const -1)))
    (call $assert (i32.eq (memory.grow $attached (i32.const 0)) (i32.const 33)))
    (call $detach_blob (i32.const 1) (i32.const 1))
    (i64.store $attached (i32.const 0) (i64.const 99))
    (call $attach_blob (local.get $blob) (i32.const 1))
    (call $assert (i64.eq (i64.load $attached (i32.const 0)) (i64.const 42)))
    (local.set $blob (call $create_blob (i32.const 0) (i32.const 8)))
    (call $assert (i32.eqz (ref.is_null (local.get $blob))))
    (call $attach_blob (local.get $blob) (i32.const 1))
    (call $assert (i32.eq (memory.size $attached) (i32.const 1)))
    (call $assert (i64.eqz (i64.load $attached (i32.const 65528))))
    (call $detach_blob (i32.const 1) (i32.const 1))
    (call $assert (i32.eq (memory.grow $attached (i32.const 1)) (i32.const 1)))
    (call $assert (i64.eqz (i64.load $attached (i32.const 131064))))
    (call $assert (i64.eq (i64.load $attached (i32.const 0)) (i64.const 42)))
    (call $attach_blob (call $create_blob (i32.const 0) (i32.const 0)) (i32.const 3))
    (call $assert (i32.eqz (memory.size $empty)))
    (call $assert (i32.eq (memory.grow $empty (i32.const 1)) (i32.const -1)))
    (call $detach_blob (i32.const 3) (i32.const 0))
    (call $detach_blob (i32.const 3) (i32.const 0))
    (call $assert (i32.eqz (memory.size $empty)))
    (call $assert (i32.eqz (memory.grow $empty (i32.const 1))))
    (call $assert (i64.eqz (i64.load $empty (i32.const 0))))
    (table.fill $source_tree (i32.const 0) (local.get $blob) (i32.const 2049))
    (local.set $tree (call $create_tree (i32.const 1) (i32.const 2049)))
    (call $attach_tree (local.get $tree) (i32.const 2))
    (call $assert (i32.eq (table.grow $attached_tree (local.get $blob) (i32.const 1)) (i32.const -1)))
    (call $assert (i32.eq (table.grow $attached_tree (ref.null extern) (i32.const 0)) (i32.const 2049)))
    (call $detach_tree (i32.const 2) (i32.const 1))
    (table.set $attached_tree (i32.const 0) (ref.null extern))
    (call $attach_tree (local.get $tree) (i32.const 2))
    (call $assert_blob (table.get $attached_tree (i32.const 0)))
    (call $detach_tree (i32.const 2) (i32.const 1))
    (call $assert (i32.eq (table.grow $attached_tree (local.get $blob) (i32.const 1)) (i32.const 2049)))
    (call $assert_blob (table.get $attached_tree (i32.const 2048)))
    (call $assert_blob (table.get $attached_tree (i32.const 2049)))
    (call $assert (i32.eq (table.grow $source_tree (local.get $blob) (i32.const 1)) (i32.const 2049)))
    (call $assert (i32.eq (table.grow $source_tree (local.get $blob) (i32.const 1)) (i32.const -1)))
    (call $attach_tree (call $create_tree (i32.const 1) (i32.const 3)) (i32.const 2))
    (call $assert (i32.eq (table.size $attached_tree) (i32.const 3)))
    (call $detach_tree (i32.const 2) (i32.const 1))
    (call $assert (i32.eq (table.grow $attached_tree (local.get $blob) (i32.const 2047)) (i32.const 3)))
    (call $assert_blob (table.get $attached_tree (i32.const 2049)))
    (call $assert (i32.eq (table.grow $functions (ref.func $answer) (i32.const 1)) (i32.const 1)))
    (call $assert (i32.eq (table.grow $functions (ref.func $answer) (i32.const 0)) (i32.const 2)))
    (call $assert (i32.eq (table.grow $functions (ref.func $answer) (i32.const 1)) (i32.const -1)))
    (call $assert (i64.eq (call_indirect $functions (type $answer_type) (i32.const 1)) (i64.const 42)))
    (table.set $tag (i32.const 0) (table.get $combination (i32.const 0)))
    (table.set $tag (i32.const 1) (local.get $blob))
    (table.set $tag (i32.const 2) (local.get $blob))
    (local.set $tree (call $create_tag (i32.const 3)))
    (call $assert (call $is_tag (local.get $tree)))
    (call $attach_tree (local.get $tree) (i32.const 2))
    (call $assert (i32.eq (table.size $attached_tree) (i32.const 3)))
    (call $assert_blob (table.get $attached_tree (i32.const 1)))
    (call $detach_tree (i32.const 2) (i32.const 0))
    (call $assert (i32.eqz (table.size $attached_tree)))
    (call $assert (i32.eqz (table.grow $attached_tree (ref.null extern) (i32.const 1))))
    (call $assert (ref.is_null (table.get $attached_tree (i32.const 0))))
    (call $attach_tree (call $create_tree (i32.const 1) (i32.const 0)) (i32.const 2))
    (call $assert (i32.eqz (table.size $attached_tree)))
    (call $assert (i32.eq (table.grow $attached_tree (ref.null extern) (i32.const 1)) (i32.const -1)))
    (call $assert (i32.eqz (table.grow $attached_tree (ref.null extern) (i32.const 0))))
    (call $detach_tree (i32.const 2) (i32.const 1))
    (call $assert (i32.eqz (table.grow $attached_tree (ref.null extern) (i32.const 1))))
    (call $assert (ref.is_null (table.get $attached_tree (i32.const 0))))
    (call $create_blob_i64 (i64.const 42))))
