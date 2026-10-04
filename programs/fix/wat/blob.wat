(module
  (import "fix" "create_blob" (func $blob (param i32 i32) (result externref)))
  (memory 1)
  (data (i32.const 0) "hello, world")
  (func $apply (export "_fix_apply") (param $combination externref) (result externref)
    (call $blob (i32.const 0) (i32.const 12))))
