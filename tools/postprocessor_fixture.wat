(module
  (import "__wasm_postprocessor" "memory_copy_0_to_2"
    (func $helper (param $destination i32) (param $source i32) (param $length i32)))
  (import "host" "initialize" (func $initialize))
  (table 1 funcref)
  (export "helper" (func $helper))
  (export "apply" (func $apply))
  (start $apply)
  (elem (i32.const 0) $apply)
  (func $apply (local $temporary i32)
    (block $body
      (call $initialize)
      (call $helper (i32.const 0) (i32.const 0) (i32.const 0))))
)
