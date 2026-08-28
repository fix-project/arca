(module
 (import "fixpoint" "create_blob_i64"       (func $create_blob_i64 (param i64) (result externref)))
 (import "fixpoint" "attach_blob"           (func $attach_blob (param externref) (param i32)))
 (import "fixpoint" "attach_tree"           (func $attach_tree (param externref) (param i32)))
 (memory $mem_0 1)
 (memory $mem_1 0)
 (memory $mem_2 0)
 (table $tab_0 0 externref)
 (func (export "_fixpoint_apply") (param $encode externref) (result externref)
       ;; attach combination tree
       (call $attach_tree
             (local.get $encode)
             (i32.const 1))
       ;; grow rw-memory
       (memory.grow
             (memory $mem_0)
             (i32.const 0))
       drop
       (call $attach_blob
             (table.get $tab_0 (i32.const 1))
             (i32.const 1))
       (call $attach_blob
             (table.get $tab_0 (i32.const 2))
             (i32.const 2))
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
