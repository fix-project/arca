(module
  (import "fix" "advise" (func $advise (param i32 i32 i32 i32 i32)))
  (import "fix" "effect" (func $effect (param i32) (result i32)))
  (memory 1)
  (data (i32.const 0) "fix::progress")
  (data (i32.const 16) "before capture")
  (func
    (export "_fix_apply")
    (param $input i32)
    (result i32)
    (call $advise (local.get $input) (i32.const 0) (i32.const 13) (i32.const 16) (i32.const 14))
    (call $effect (local.get $input))))
