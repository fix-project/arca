(module
  (import "fix" "get_tree_element" (func $get (param i32 i32) (result i32)))
  (import "fix" "attach_blob" (func $attach (param i32 i32) (result i32)))
  (import "fix" "create_blob_i64" (func $blob (param i64) (result i32)))
  (memory $input 0)
  (func
    (export "_fix_apply")
    (param $combination i32)
    (result i32)
    (local $a i64)
    (drop (call $attach (i32.const 0) (call $get (local.get $combination) (i32.const 2))))
    (local.set $a (i64.load $input (i32.const 0)))
    (drop (call $attach (i32.const 0) (call $get (local.get $combination) (i32.const 3))))
    (call $blob (i64.add (local.get $a) (i64.load $input (i32.const 0))))))
