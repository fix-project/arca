(module
  (import "fix" "get_tree_element" (func $get (param externref i32) (result externref)))
  (import "fix" "attach_blob" (func $attach (param externref i32)))
  (import "fix" "effect" (func $effect (param externref) (result externref)))
  (memory $control 0)
  (memory $payload 0)
  (func (export "_fix_apply") (param $combination externref) (result externref)
    (local $size i32) (local $offset i32) (local $epoch i32) (local $request externref)
    (call $attach (call $get (local.get $combination) (i32.const 2)) (i32.const 0))
    (local.set $size (i32.wrap_i64 (i64.load $control (i32.const 0))))
    (if (i32.eq (memory.grow $payload (i32.div_u (i32.add (local.get $size) (i32.const 65535)) (i32.const 65536))) (i32.const -1)) (then unreachable))
    (local.set $request (call $get (local.get $combination) (i32.const 4)))
    (loop $again
      (local.set $offset (i32.const 0))
      (block $done
        (loop $touch
          (br_if $done (i32.ge_u (local.get $offset) (local.get $size)))
          (if (local.get $epoch)
            (then
              (if (i32.ne (i32.load8_u $payload (local.get $offset))
                          (i32.and (i32.sub (local.get $epoch) (i32.const 1)) (i32.const 255)))
                (then unreachable))))
          (i32.store8 $payload (local.get $offset) (local.get $epoch))
          (local.set $offset (i32.add (local.get $offset) (i32.const 4096)))
          (br $touch)))
      (local.set $epoch (i32.add (local.get $epoch) (i32.const 1)))
      (drop (call $effect (local.get $request)))
      (br $again))
    unreachable))
