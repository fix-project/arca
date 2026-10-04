(module
  ;; Always available.
  (import "fix" "get_type"    (func $get_type (param $value externref) (result i32)))

  ;; Objects
  (import "fix" "create_blob" (func $create_blob (param $memory_index i32) (param $length i32) (result externref)))
  (import "fix" "attach_blob" (func $attach_blob (param $memory_index i32) (param externref) (result i32)))
  (import "fix" "detach_blob" (func $detach_blob (param $memory_index i32) (param $copy i32)))

  (import "fix" "create_tree" (func $create_tree (param $table_index i32) (param $length i32) (result externref)))
  (import "fix" "attach_tree" (func $attach_tree (param $table_index i32) (param externref) (result i32)))
  (import "fix" "detach_tree" (func $detach_tree (param $table_index i32) (param $copy i32)))

  (import "fix" "create_tag" (func $create_tag   (param $table_index i32) (param externref) (result i32)))

  ;; Refs
  (import "fix" "create_ref"  (func $create_ref (param $object externref) (result externref)))
  (import "fix" "length"      (func $length (param externref) (result i32)))
  (import "fix" "is_eq"       (func $equals (param externref) (result i32)))
  (import "fix" "equals"      (func $equals (param externref) (param externref) (result i32)))

  ;; Thunks
  (import "fix" "create_application_thunk"    (func $create_application_thunk (param $ref externref) (result externref)))
  (import "fix" "create_identification_thunk" (func $create_identification_thunk (param $ref externref) (result externref)))
  (import "fix" "create_selection_thunk"      (func $create_selection_thunk (param $ref externref) (result externref)))

  ;; Encodes
  (import "fix" "encode_strict"   (func $encode_strict  (param $thunk externref) (result externref)))
  (import "fix" "encode_shallow"  (func $encode_shallow (param $thunk externref) (result externref)))

  ;; Continuation Capture
  (import "fix" "call_cc"   (func $call_cc  (param $thunk externref) (result externref)))
  (import "fix" "get_cc"    (func $get_cc   (param $thunk externref) (result externref)))
  (import "fix" "eval"      (func $eval     (param $thunk externref) (result externref))))
