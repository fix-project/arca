(module
  (import "fix" "unsound_handle_bytes" (func $bytes (param i32) (result i32)))
  (import "fix" "same_handle_bytes" (func $same (param i32 i32) (result i32)))
  (import "fix" "return_cc" (func $return_cc (param i32) (result i32)))
  (func
    (export "_fix_apply")
    (param $input i32)
    (result i32)
    (local $reply i32)
    (if (i32.eqz (call $same (local.get $input) (local.get $input))) (then unreachable))
    (local.set $reply (call $return_cc (call $bytes (local.get $input))))
    (if (i32.eqz (call $same (local.get $reply) (local.get $reply))) (then unreachable))
    (if (call $same (local.get $input) (local.get $reply)) (then unreachable))
    (call $bytes (local.get $reply))))
