(module
  (import "fixpoint" "create_blob_i64"
    (func $create_blob_i64 (param i64) (result externref)))
  (import "fixpoint" "attach_blob"
    (func $attach_blob (param i32) (param externref)))
  (import "fixpoint" "attach_tree"
    (func $attach_tree (param i32) (param externref)))

  (memory $mem_0 1)
  (memory $mem_1 0)
  (memory $mem_2 0)
  (table $tab_0 0 externref)

  (func (export "_fixpoint_apply")
    (param $encode externref)
    (result externref)

    (local $counter i64)
    (local $spin i64)
    (local $left i64)
    (local $right i64)

    ;; Attach the combination tree.
    (call $attach_tree
      (i32.const 0)
      (local.get $encode))

    ;; Grow rw-memory by zero pages, preserving the original behavior.
    (memory.grow
      (memory $mem_0)
      (i32.const 0))
    drop

    ;; Attach the two input blobs.
    (call $attach_blob
      (i32.const 1)
      (table.get $tab_0 (i32.const 1)))
    (call $attach_blob
      (i32.const 2)
      (table.get $tab_0 (i32.const 2)))

    ;; Load both operands once.
    (local.set $left
      (i64.load
        (memory $mem_1)
        (i32.const 0)))

    (local.set $right
      (i64.load
        (memory $mem_2)
        (i32.const 0)))

    ;; Artificial CPU work before the addition.
    ;; Change this constant to tune how slow each addition is.
    (local.set $counter (i64.const 100000000))
    (local.set $spin (local.get $left))

    (block $spin_done
      (loop $spin_loop
        (br_if $spin_done
          (i64.eqz (local.get $counter)))

        (local.set $spin
          (i64.add
            (i64.xor
              (local.get $spin)
              (local.get $counter))
            (i64.const 6364136223846793005)))

        (local.set $counter
          (i64.sub
            (local.get $counter)
            (i64.const 1)))

        (br $spin_loop)))

    ;; Store the spin result so the work is observable.
    (i64.store
      (memory $mem_0)
      (i32.const 8)
      (local.get $spin))

    ;; Both branches return left + right.
    ;; The condition depends on the spin result.
    (i64.store
      (memory $mem_0)
      (i32.const 0)
      (if (result i64)
        (i64.eqz
          (i64.and
            (local.get $spin)
            (i64.const 1)))
        (then
          (i64.add
            (local.get $left)
            (local.get $right)))
        (else
          (i64.sub
            (local.get $left)
            (i64.sub
              (i64.const 0)
              (local.get $right))))))

    ;; Return the sum as a Fix blob.
    (call $create_blob_i64
      (i64.load
        (memory $mem_0)
        (i32.const 0))))
)