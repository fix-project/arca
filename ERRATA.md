# Documentation/code mismatches

- Combinations and callable Trees use field 0 for metadata and field 1 for
  the callable. Ordinary arguments begin at field 2.
- The bootstrap label `compile` names the compiler ELF, the sole trusted
  author. `fixpoint` names its self-authored certificate
  `C = Tag([compile], compile, "Runnable")`. The exported bootstrap contains
  no temporary zero-author Runnable certificate. Runtime callable resolution
  requires a Runnable certificate, including for the trusted compiler itself.

- Thinking a selection evaluates its entire argument Tree before selecting,
  as for an application. An exposed Object Tree target is evaluated recursively;
  a Ref target remains opaque.

- Fix integer operands, including selection indices and collector operation and
  statement codes, require exactly eight little-endian bytes. Blob data itself
  remains arbitrary bytes.

- The proof distinguishes computational equivalence from indistinguishability.
  The latter preserves comparability, but its full Shell substitution theorem
  remains unproved. Computational
  equivalence alone does not justify arbitrary substitution.
- Privileged applications can inspect handle representations and must not
  receive application coupons. They may run the collector or any nondeterministic
  scheduling helper. The kernel entry point returns an uncertified result.
- The planned scheduler effect interface provides identification (authorized
  handle lookup from bytes), application of Value combinations, selection,
  and privileged application. Native Rust evaluators still share the kernel's
  storage interface; the effect boundary is not implemented yet.
- The Shell provides `get_tree_element(Object, index)` for reading a Tree child
  without attachment, Ref loading, or evaluation. The collector uses this
  accessor to avoid shrinking attachments, which WasmCert's host contract forbids.
- Application claims describe the exact one-step result. Ordinary runtime
  execution requires a Value combination and returns a zero-author application
  coupon containing its result.
- Native canonicalization, Apply, and Select certificates, and collector outputs,
  use `Tag([authors, Ref([operation, input, result])])`. The scheduler supplies
  each premise as `[Tag, assertion Object]`; the collector authenticates the
  Object by recreating its Ref, without accessing storage or evaluating it.
  Referencing the assertion prevents Tag evaluation from rewriting its claim.
  The repository retains the ordinary Tag index and separately indexes locally
  available assertions under `.fix/claims`, using the same bounded-prefix layout.
  These index entries are hints; memoization checks the stored Tag and assertion.
- Storage-issued Select certificates have operation code `6u64` and describe
  selection without operand evaluation. Storage currently reads the full source;
  authenticated partial reads remain planned.
- The native object store issues zero-author canonicalization certificates;
  the operation is not yet exposed through a Fix import or covered by the proof.
- Tag evaluation, issuance, marker preservation, and conditional coupon validity
  are modeled by the proof. Native Tag creation and comparison remain trusted
  host interfaces. The Wasm proof connects all 26 exchanges to the abstract
  checker, including invalid-exchange and unsupported-code rejection, and
  connects exported Tags to conditional issuance. Resource and exact-comparison
  readiness requirements are explicit.
- Coupon preservation uses a stronger issuance contract: claims must follow
  from trusting their stable authors, without relying on unstable extra authors.
  The abstract preservation theorem retains this contract under Tag evaluation.
  Native issuer implementations remain trusted interfaces.
- The kernel's KVM clock conversion omits the 32-bit fixed-point shift after
  multiplying by `tsc_to_system_mul`, producing incorrect elapsed times.
- The collector requires privileged execution for exact handle-bit comparisons.
  Different encodings of the same Fix value can cause a derivation to be
  rejected. Success-path proofs require comparison readiness; native validation
  proves that a returned claim is checked and that invalid exchanges trap.
- `docs/fix.md`: Tags use an authors Tree as their first field, with all
  remaining fields application-defined. Creation adds the author selected by
  depth on the executing combination's callable field-1 path, preserving
  additional authors. Only comparable authors are deduplicated through Fix
  equality.
- `docs/fix.wat`: attachment signatures are `(index, handle) -> i32`; code uses
  `(handle, index) -> ()`.
- `docs/fix.wat`: `create_tag` is `(index, handle) -> i32`; code uses
  `(index, author_depth) -> externref`.
- `docs/fix.wat`: `length`, `encode_strict`, and `encode_shallow` are named
  `len`, `create_strict_encode`, and `create_shallow_encode` in code.
- `docs/fix.wat`: `len` returns `i64`, preserving the handle's full length,
  rather than `i32`. C uses `uint64_t`; Rust `Value::len` returns `u64`.
  Local memory and table sizes still require checked narrowing.
- `docs/fix.wat`: `get_cc` and `eval` are declared but not implemented.
- `docs/fix-shell.md`: the layout provides 128 Memories and 128 Tables; code
  provides 64 Memories and 62 Tables sharing one index space; one additional
  slot is reserved for saved handles.
- `docs/fix-shell.md`: local names refer to data addresses. The Arca shell uses
  addresses of handle cells in its reserved region instead; it resolves them
  through the runtime, keeping physical names out of captured native memory.
- `docs/flatware.md`: CPIO directories are described but not implemented.
- `docs/api.rs`: identification accepts `Handle<T>`; code accepts only
  Objects and Refs. The SDK uses `Handle` for existing Fix values and `Value`
  adaptors for pending values. Content reads use caller-provided buffers or
  owned vectors with `alloc`, rather than borrowed slices of attached resources.
- `docs/cli.sh`: the CLI uses `create-shallow-encode`, not `create-encode`.
  The persistent core commands and external shell helpers are documented in
  `fix/runtime/README.md`. `eval` consumes a handle; `run` consumes a script.
  File/directory helpers use Flatware descriptors, rather than unimplemented CPIO.
- Parallel evaluation starts jobs without reserving memory. Exhausting the VM's
  memory can abort evaluation rather than delay jobs until memory is available.
  Blocking allocation alone could deadlock if all running jobs hold memory and
  need more to finish. Job admission needs to account for peak memory use;
  estimates alone cannot guarantee that arbitrary programs will fit.
