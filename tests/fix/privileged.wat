(module
  (import "fix" "unsound_handle_bytes" (func $bytes (param externref) (result externref)))
  (func (export "_fix_apply") (param $input externref) (result externref)
    (call $bytes (local.get $input))))
