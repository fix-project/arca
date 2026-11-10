# Documentation/code mismatches

- `docs/fix.md` and `docs/fix.wat`: `get_cc` is not implemented. `return_cc`
  exposes a request and its continuation as a pair.
- `docs/flatware.md`: CPIO directory images are not implemented.

- `fix/proof/shell/ABI.v` and `fix/proof/collector/wasm`: the Wasm execution
  proofs still describe the externref ABI and Tree attachments to Wasm tables.
  They do not cover the descriptor-table ABI used by the Shell and collector.
