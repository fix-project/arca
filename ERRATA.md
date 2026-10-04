# Documentation/code mismatches

- Coupon uses `fixpoint` imports and `_fixpoint_apply`; the shell uses `fix`
  and `_fix_apply`. Coupon is excluded from `//:artifacts` and CI test targets
  pending migration.
- `docs/fix.md`: tags require the procedure in the author list; code requires
  it as the first Tree entry.
- `docs/fix.md`: procedure lookup recursively descends through Trees; code
  requires a Blob directly at `combination[0]`.
- `docs/fix.wat`: attachment signatures are `(index, handle) -> i32`; code uses
  `(handle, index) -> ()`.
- `docs/fix.wat`: `create_tag` is `(index, handle) -> i32`; code uses
  `(index) -> externref`.
- `docs/fix.wat`: `length`, `encode_strict`, and `encode_shallow` are named
  `len`, `create_strict_encode`, and `create_shallow_encode` in code.
- `docs/fix.wat`: `call_cc`, `get_cc`, and `eval`
  are declared but not implemented.
- `docs/fix-shell.md`: the layout provides 128 Memories and 128 Tables; code
  provides 64 Memories, 32 externref Tables, and 31 funcref Tables.
- `docs/fix-shell.md`: local-handle creation, remapping, and result upgrading
  are described but not implemented.
- `docs/flatware.md`: CPIO directories are described but not implemented.
- `docs/api.rs`: identification accepts `Handle<T>`; code accepts only
  Objects and Refs. The SDK uses `Handle` for existing Fix values and `Value`
  adaptors for pending values. Content reads use caller-provided buffers or
  owned vectors with `alloc`, rather than borrowed slices of attached resources.
