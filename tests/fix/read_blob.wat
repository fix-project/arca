(module
  (import "fix" "create_blob" (func $__empty (param i32 i32) (result i32)))
  (import "fix" "access_blob" (func $read (param i32 i32 i32 i32) (result i32)))
  (import "fix" "get_tree_element" (func $get (param i32 i32) (result i32)))
  (import "fix" "create_blob" (func $blob (param i32 i32) (result i32)))
  (import "fix" "create_blob_i64" (func $literal (param i64) (result i32)))
  (import "fix" "effect" (func $effect (param i32) (result i32)))
  (memory 2)
  (func $assert (param $condition i32) (if (i32.eqz (local.get $condition)) (then unreachable)))
  (func $check_bytes
    (param $source i32)
    (param $destination i32)
    (param $length i32)
    (local $index i32)
    (loop $bytes
      (call $assert
        (i32.eq
          (i32.load8_u (i32.add (local.get $destination) (local.get $index)))
          (i32.rem_u (i32.add (local.get $source) (local.get $index)) (i32.const 251))))
      (local.set $index (i32.add (local.get $index) (i32.const 1)))
      (br_if $bytes (i32.lt_u (local.get $index) (local.get $length)))))
  (func $check
    (param $value i32)
    (memory.fill (i32.const 16382) (i32.const 255) (i32.const 4102))
    (call $assert
      (i32.eq
        (call $read (local.get $value) (i32.const 4093) (i32.const 16383) (i32.const 4100))
        (i32.const 4100)))
    (call $check_bytes (i32.const 4093) (i32.const 16383) (i32.const 4100))
    (call $assert (i32.eq (i32.load8_u (i32.const 16382)) (i32.const 255)))
    (call $assert (i32.eq (i32.load8_u (i32.const 20483)) (i32.const 255)))
    (memory.fill (i32.const 32767) (i32.const 255) (i32.const 16))
    (call $assert
      (i32.eq (call $read (local.get $value) (i32.const 8190) (i32.const 32767) (i32.const 16)) (i32.const 3)))
    (call $check_bytes (i32.const 8190) (i32.const 32767) (i32.const 3))
    (call $assert (i32.eq (i32.load8_u (i32.const 32770)) (i32.const 255)))
    (call $assert
      (i32.eqz (call $read (local.get $value) (i32.const 8193) (i32.const 131072) (i32.const -1))))
    (call $assert (i32.eqz (call $read (local.get $value) (i32.const 4093) (i32.const 131072) (i32.const 0))))
    (call $assert
      (i32.eq
        (call $read (local.get $value) (i32.const 0) (i32.const 65535) (i32.const 8193))
        (i32.const 8193)))
    (call $check_bytes (i32.const 0) (i32.const 65535) (i32.const 8193)))
  (func
    (export "_fix_apply")
    (param $combination i32)
    (result i32)
    (local $input i32)
    (local $local i32)
    (local $index i32)
    (local.set $input (call $get (local.get $combination) (i32.const 2)))
    (loop $initialize
      (i32.store8 (local.get $index) (i32.rem_u (local.get $index) (i32.const 251)))
      (local.set $index (i32.add (local.get $index) (i32.const 1)))
      (br_if $initialize (i32.lt_u (local.get $index) (i32.const 8193))))
    (local.set $local (call $blob (i32.const 0) (i32.const 8193)))
    (memory.fill (i32.const 0) (i32.const 255) (i32.const 8193))
    (drop (call $effect (call $__empty (i32.const 0) (i32.const 0))))
    (call $assert (i32.eqz (i32.load (i32.const 100000))))
    (i32.store (i32.const 100000) (i32.const 1))
    (call $check (local.get $input))
    (call $check (local.get $local))
    (call $assert
      (i32.eqz
        (call $read
          (call $__empty (i32.const 0) (i32.const 0))
          (i32.const 0)
          (i32.const 131072)
          (i32.const -1))))
    (call $assert
      (i32.eq
        (call $read
          (call $literal (i64.const 0x0807060504030201))
          (i32.const 2)
          (i32.const 131066)
          (i32.const -1))
        (i32.const 6)))
    (call $assert (i32.eq (i32.load (i32.const 131066)) (i32.const 0x06050403)))
    (call $assert (i32.eq (i32.load16_u (i32.const 131070)) (i32.const 0x0807)))
    (call $assert (i32.eq (memory.grow (i32.const 64)) (i32.const 2)))
    (call $assert
      (i32.eq
        (call $read (local.get $input) (i32.const 4093) (i32.const 2097150) (i32.const 4100))
        (i32.const 4100)))
    (call $check_bytes (i32.const 4093) (i32.const 2097150) (i32.const 4100))
    (call $assert (i32.eqz (i32.load8_u (i32.const 2097149))))
    (call $assert (i32.eqz (i32.load8_u (i32.const 2101250))))
    (call $blob (i32.const 0) (i32.const 131072))))
