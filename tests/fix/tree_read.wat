(module
  (import "fix" "get_tree_element" (func $get (param i32 i32) (result i32)))
  (func (export "_fix_apply") (param $combination i32) (result i32)
    (call $get (call $get (local.get $combination) (i32.const 2)) (i32.const 0))))
