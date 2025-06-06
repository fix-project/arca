(module
 (import "fixpoint" "create_blob_i64"       (func $create_blob_i64 (param i64) (result externref)))
 (import "fixpoint" "attach_blob"           (func $attach_blob (param i32) (param externref)))
 (import "fixpoint" "get_tree_entry"        (func $get_tree_entry (param externref) (param i32) (result externref)))
 (memory $mem_0 0)
 (memory $mem_1 0)
 (func (export "_fixpoint_apply") (param $encode externref) (result externref)
       (call $attach_blob
             (i32.const 0)
             (call $get_tree_entry
                   (local.get $encode)
                   (i32.const 2)))
       (call $attach_blob
             (i32.const 1)
             (call $get_tree_entry
                   (local.get $encode)
                   (i32.const 3)))
       (call $create_blob_i64
             (i64.add
               (i64.load
                 (memory $mem_0)
                 (i32.const 0))
               (i64.load
                 (memory $mem_1)
                 (i32.const 0))))))
