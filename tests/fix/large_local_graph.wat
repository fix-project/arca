(module
  (import "fix" "create_blob" (func $blob (param i32 i32) (result externref)))
  (import "fix" "create_tree" (func $tree (param i32 i32) (result externref)))
  (memory 1)
  (table 5000 externref)
  (func (export "_fix_apply") (param externref) (result externref)
    (local $i i32)
    (loop $next
      (i32.store (i32.const 0) (local.get $i))
      (table.set (local.get $i) (call $blob (i32.const 0) (i32.const 64)))
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br_if $next (i32.lt_u (local.get $i) (i32.const 5000))))
    (call $tree (i32.const 0) (i32.const 5000))))
