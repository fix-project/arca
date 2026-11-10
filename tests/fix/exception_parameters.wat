(module
  (import "fix" "create_blob_i64" (func $blob (param i64) (result i32)))
  (global $base i32 (i32.const 40))
  (tag $exception (param i32))
  (func $calculate
    (param $unchanged i32)
    (param $changed i32)
    (result i32)
    (local $payload i32)
    (try
      (result i32)
      (do (local.set $changed (i32.add (local.get $changed) (i32.const 6))) (throw $exception (i32.const 2)))
      (catch $exception
        (local.set $payload)
        (i32.add (local.get $payload) (i32.add (local.get $unchanged) (local.get $changed))))))
  (func
    (export "_fix_apply")
    (param i32)
    (result i32)
    (call $blob (i64.extend_i32_u (call $calculate (global.get $base) (i32.const 1))))))
