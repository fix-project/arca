(module
  (import "env" "increment" (func $increment (param i64) (result i64)))
  (func $next_three (param $value i64) (result i64)
    (call $increment (call $increment (call $increment (local.get $value)))))
)
