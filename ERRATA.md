# Documentation/code mismatches

- `docs/fix.md`: callable lookup follows field 1, not field 0. Combinations
  contain metadata at field 0, the callable at field 1, and arguments from field 2.
- `docs/fix.md`: Tags contain an author in field 0 followed by application-defined
  fields, rather than a fixed subject/claim/author-list triple. Ordinary creation
  checks the author against the current combination and its callable prefixes.
- `docs/fix.wat`: `attach_blob` and `attach_tree` take `(handle, index)` and return
  nothing. `create_tag` takes only a table index and returns an externref.
- `docs/fix.wat`: the implemented imports are `len`, `create_strict_encode`, and
  `create_shallow_encode`, rather than `length`, `encode_strict`, and
  `encode_shallow`. `len` returns `i64`, not `i32`.
- `docs/fix.md` and `docs/fix.wat`: `get_cc` is not implemented. `return_cc`
  exposes a request and its continuation as a pair.
- `docs/fix-shell.md`: the Shell supports 32 Memories and 30 Tables, not 128 of
  each. Each mapping reserves 8 GiB for a 4 GiB payload.
- `docs/fix-shell.md` and `docs/handle.md`: local names identify handle cells,
  not the virtual addresses of Blob or Tree data.
- `docs/api.rs`: identification accepts only Objects and Refs. SDK content reads
  use caller-provided buffers or owned vectors rather than borrowed slices.
- `docs/flatware.md`: CPIO directory images are not implemented.
- `docs/cli.sh`: the shallow-encode command is `create-shallow-encode`, not
  `create-encode`. Current command syntax is in `fix/cli/README.md`.
