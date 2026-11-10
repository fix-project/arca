(module
  ;; Always available.
  (import "fix" "get_type"    (func $get_type (param $handle i32) (result i32)))
  (import "fix" "close"       (func $close (param $handle i32)))

  ;; Objects
  (import "fix" "create_blob" (func $create_blob (param $memory_index i32) (param $len i32) (result i32)))
  (import "fix" "attach_blob" (func $attach_blob (param $memory_index i32) (param $handle i32) (result i32)))
  (import "fix" "access_blob" (func $access_blob (param $handle i32) (param $offset i32) (param $ptr i32) (param $len i32) (result i32)))
  (import "fix" "detach_blob" (func $detach_blob (param $memory_index i32) (param $copy i32)))

  (import "fix" "create_tree" (func $create_tree (param $ptr i32) (param $len i32) (result i32)))
  (import "fix" "access_tree" (func $access_tree (param $handle i32) (param $offset i32) (param $addr i32) (param $len i32) (result i32)))

  (import "fix" "create_tag"  (func $create_tag   (param $handle i32) (result i32)))

  (import "fix" "get_data_type" (func $get_data_type (param $handle i32) (result i32)))

  ;; Refs
  (import "fix" "create_ref"    (func $create_ref (param $handle i32) (result i32)))
  (import "fix" "len"           (func $len (param $handle i32) (result i64)))
  (import "fix" "is_comparable" (func $is_comparable (param $handle i32) (result i32)))
  (import "fix" "equals"        (func $equals (param $handle0 i32) (param $handle1 i32) (result i32)))

  ;; Thunks
  (import "fix" "create_application_thunk"    (func $create_application_thunk (param $handle i32) (result i32)))
  (import "fix" "create_identification_thunk" (func $create_identification_thunk (param $handle i32) (result i32)))
  (import "fix" "create_selection_thunk"      (func $create_selection_thunk (param $handle i32) (result i32)))

  ;; Encodes
  (import "fix" "create_strict_encode"  (func $create_strict_encode  (param $handle i32) (result i32)))
  (import "fix" "create_shallow_encode" (func $create_shallow_encode (param $handle i32) (result i32)))
  (import "fix" "get_encode_type"       (func $get_encode_type (param $handle i32) (result i32)))

  ;; Continuation Capture
  (import "fix" "call_cc"   (func $call_cc   (param $handle i32) (result i32)))
  (import "fix" "get_cc"    (func $get_cc    (param $handle i32) (result i32)))
  (import "fix" "return_cc" (func $return_cc (param $handle i32) (result i32)))
  (import "fix" "eval"      (func $eval      (param $handle i32) (result i32))))
