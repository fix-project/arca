(module
  (import "fix" "create_blob_i64" (func $literal (param i64) (result i32)))
  (memory 1)
  (memory 1)
  (func (export "_fix_apply") (param i32) (result i32)
    (call $literal (i64.load (i32.const -4)))))
