(module
 (import "fixpoint" "create_blob_i64"       (func $create_blob_i64 (param i64) (result externref)))
 (import "fixpoint" "attach_blob"           (func $attach_blob (param i32) (param externref)))
 (import "fixpoint" "get_tree_entry"        (func $get_tree_entry (param externref) (param i32) (result externref)))
 ;; memories intended for rw-usage
 (memory $mem_0 1)
 (memory $mem_1 0)
 (memory $mem_2 0)
 (func (export "_fixpoint_apply") (param $encode externref) (result externref)
       ;; getting an entry of a tree multiple times
       (call $get_tree_entry
             (local.get $encode)
             (i32.const 2))
       drop
       ;; grow rw-memory
       (memory.grow
             (memory $mem_0)
             (i32.const 0))
       drop
       (call $attach_blob
             (i32.const 1)
             (call $get_tree_entry
                   (local.get $encode)
                   (i32.const 2)))
       (call $attach_blob
             (i32.const 2)
             (call $get_tree_entry
                   (local.get $encode)
                   (i32.const 3)))
       ;; write to rw-memory
       (i64.store (memory $mem_0)
             (i32.const 0)
             (i64.add
               (i64.load
                 (memory $mem_1)
                 (i32.const 0))
               (i64.load
                 (memory $mem_2)
                 (i32.const 0))))
       (call $create_blob_i64
             (i64.load
               (memory $mem_0)
               (i32.const 0)))
 ))
