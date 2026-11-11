# Documentation/code mismatches

- `docs/fix.md` and `docs/fix.wat`: `get_cc` is not implemented. `return_cc`
  exposes a request and its continuation as a pair.
- `docs/flatware.md`: CPIO directory images are not implemented.
- `docs/fix.wat`: the prototype also provides terminal `fix.exit`, with
  signature `(param i32)` and no result, for completing an invocation without
  unwinding. The Rust SDK uses it to return panic messages as an untagged Tree
  `[Blob("Rust.panic"), Blob(message)]`. Wasm traps return an untagged Tree
  `[Blob("Wasm.trap"), Blob(u32 trap_code in little-endian)]`.
- `docs/fix.md`: the prototype collector accepts at most 1,048,576
  witnesses per request. This bounds descriptor allocation without closing slots
  during an invocation.
