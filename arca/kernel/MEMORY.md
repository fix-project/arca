# Fix memory accounting

Fix charges operations under a deterministic contract. The ledger records those
charges; it does not measure an executing program or inspect its page tables.
Physical sharing, CoW, huge pages, and zero-page optimizations give no discount.
Kernel and Shell implementations must fit their bookkeeping allowances.

**S** denotes a structural quantity, **B** an enforced bound, and **E** an
estimated allowance. The estimates below have not been proved to bound every
allocation. Deterministic arithmetic alone does not establish a conservative
memory bound.

## Capacity and admission

Capacity has four states: reserved-unused, reserved-used, unreserved-unused, and
unreserved-used. Only unreserved-unused capacity admits a new reservation.
A normal application's declared budget is reserved before execution. Its charges
move credit from reserved-unused to reserved-used within the application's own
ledger. The shared ledger accounts for total reservations and changes to the
unreserved pool. Ending the invocation returns unused credit. Retained results
keep their credit until released by storage.

Privileged applications have no enforced program quota. Their charges consume
unreserved-unused capacity and cannot consume another application's reservation.
When active reservations prevent a privileged charge, execution can wait for
them to finish. Application workers continue through advice and do not wait for
the scheduler to consume it.
Schedulers can query total capacity and current available capacity. Admission
failure is retryable and does not issue an Apply coupon.

The kernel's execution instrumentation is one signed byte-adjustment syscall.
It updates the invocation's counter and capacity account. Without an invocation
budget, ordinary Arca execution is unmetered. It does not charge Arca syscalls,
traverse descriptors, or compute a process footprint.

Sources: [capacity ledger](src/memory/accounting.rs), [reservation ownership](src/memory.rs),
[byte adjustment](src/types/function.rs).

## Charge arithmetic

All sizes are bytes. Let `P = 4096` and `L(n) = ceil(n/P)`.

| Symbol | Contract | Basis |
| --- | --- | --- |
| `A(n)` | Smallest power of two at least `max(P,n)` | Rounding **S**; minimum and allocation coverage **E**. |
| `M(n)` | `P * L(n) + H(0,n)` | Logical backing **S**, plus bookkeeping. |
| `H(a,n)` | `128 * L(n)` plus 16 KiB for each 2 MiB, 1 GiB, or 512 GiB range intersecting the mapping at address `a`, capped at `4P * L(n)` | Page and range counts **S**; per-page metadata, table budget, and sparse-mapping cap **E**. The invocation covers the root table. |
| `R` | 16384 | Storage-object record and promotion bookkeeping **E**. |
| `D` | 128 | Bookkeeping per Fix handle slot **E**. |
| `W` | 8192 | Working allowance per effect **E**. |
| `I` | `2 MiB + 16384` | Kernel-thread stack is 2 MiB **B**; other invocation and Shell working coverage **E**. |
| `O(n)` | `M(n) + R` | New independent backing and record; inherited **S/E**. |

Sources: [Fix contracts](../../fix/handle/src/accounting.rs),
[Shell ledger](../../fix/shell/src/memory.rs).

## Initialization and scratch memory

Initialization charges `I` and `M` for each declared native code, data, and stack
segment using its linked address. Segment addresses and lengths are linker constants, not observations of
resident pages. Overlapping segment boundaries may be charged twice. Generated
Wasm instance storage additionally charges `M(A(instance_size))`.

Each scratch/table/handle-arena region records its charged high-water extent.
These extents follow Shell allocation calls and survive continuation capture.
They contain no information obtained from page tables.

| Operation | Charge or refund | Basis |
| --- | --- | --- |
| First mapping beyond the charged extent | `P` per newly mapped page plus the increase in `H` over the extent | Backing and deterministic bookkeeping above. |
| Mapping a page within an already charged extent | `P` | Backing **S**; its bookkeeping remains charged. |
| Successful explicit unmap | Refund `P` per removed page | Backing release **S**; bookkeeping remains charged. |
| Failed unmap | No refund | The old mapping remains live. |
| Wasm memory allocation/growth | Apply the mapping contract to the new byte range | Declared pages and page size **S**; maximum size checked **B**. |
| Funcref table allocation/growth | Apply the mapping contract to its rounded element bytes | Element count/width **S**; maximum checked **B**. |
| Blob/Tree attachment | Release replaced backing, then apply the mapping contract to the attachment's logical byte length | Blob length or `32 * child_count` **S**. Attachment backing is charged in full. |
| Detach with copy | Keep the backing charge; the region becomes scratch state | No CoW discount and no release. |
| Detach without copy | Release backing as an explicit unmap | Bookkeeping persists. |

Source: [Wasm memory and table operations](../../fix/shell/src/rt.rs),
[attachment and detach](../../fix/shell/src/shell.rs).

## Fix handles and objects

Handle-arena capacity grows in blocks of 128 slots: one 4096-byte page of
32-byte handles (**S/B**). Each block charges `128D` and its mapping cost. The
initial arena page is covered by startup. Closing and reusing a slot do not grow
capacity or refund its backing or free-slot bookkeeping.

| Fix API operation | Additional charge | Lifetime and basis |
| --- | --- | --- |
| Create Blob of at most 30 bytes, including integer constructors | No backing charge beyond handle capacity | Inline representation **B**. |
| Create larger Blob of `n` bytes | `O(n)` | New independent backing, even when contents or physical pages are shared. |
| Create Tree of `n` children | `O(32n)` | Handle-array size **S**; overhead above **E**. |
| Create Ref, Thunk, Encode, or Tag; clone a handle | Handle-capacity charge only | These retain existing data rather than constructing a new backing. |
| Type, length, comparability, and other metadata queries | No fee beyond the invocation allowance | Fixed-size handle operations; allocation coverage **E**. |
| Tree element lookup | Handle capacity and runtime working allowance | No full-Tree copy is promised by the API. |
| Promotion of local objects | Transfer their existing `O` credit to storage for promoted nonempty backings | No recount or new full-backing charge. Unreachable backings, inline Blobs, and the empty Tree release their unused object credit. |
| Re-encoding local Trees during promotion | Temporary `M(32 * child_count)` per Tree in the promotion batch | The old backing can overlap the rewritten Tree. Release this working allowance when the old mappings are removed. |

An object's charge is determined when it is created. Promoting a Tree still
examines its contents to resolve local handles; accounting uses the recorded
logical lengths, not a scan of native mappings. Content deduplication gives no
program-quota refund.

Sources: [descriptor capacity](../../fix/shell/src/descriptors.rs),
[object creation](../../fix/shell/src/shell.rs),
[promotion](../../fix/runtime/src/runtime/arca/locals.rs).

## Temporary API work

| Operation | Additional working charge | Basis |
| --- | --- | --- |
| Effect, eval, call/return continuation | Shell `W`, plus runtime `W` while processing each effect | Request/response bookkeeping **E**. |
| Advice | With `C = A(key_length) + A(payload_length)`, Shell `W + C` and runtime `C + (2C + R) + (C + 4R)` | Runtime working buffers, observer buffers, and scheduler delivery backing. Lengths **S**, allocation coverage **E**. |
| Non-inline Blob read | Shell `W + A(requested_bytes)` and runtime `A(requested_bytes)` | Requested byte count **S**; buffer coverage **E**. |
| Blob attachment | Runtime `M(blob_length)`, in addition to the resulting Shell mapping charge | Temporary backing/transfer coverage **E**. |
| Tree attachment | Runtime `M(32 * child_count)`, plus resulting Shell mapping charge | Handle bytes **S**; temporary coverage **E**. |
| Tree element fetch | Runtime `P` | Temporary bookkeeping **E**. |

Shell temporary charges remain live while their API call is suspended and are
released when it returns. Runtime working charges end after processing the
request. Advice also charges for published data that observers may retain after
the effect returns. The invocation counts this allowance for each event,
independently of whether an observer is present. The reservation transfers the
corresponding credit to the advice observers and the scheduler. Advice is
recorded while the application runs and is delivered to the scheduler before
its result.

## Continuations

Internal Arca continuations transfer execution state while implementing a Fix
API call. They do not create an additional program-visible copy of every mapped
page and receive no separate full-process charge.

Program-visible capture charges the application 65536 bytes for the capture
record, `A(128) + R` per attachment record, and `A(32n) + R` for a nonempty list
of `n` attachments (**E**). The capture backing retains the credit for the
saved frame and the new metadata. The invocation releases `I` when it ends.
Read-only attachments refer to existing backing. Promoted local objects carry
their own backing credit.

The saved application budget records the live frame charges at capture, less
`I`. Each resumption uses that budget and pays its own `I`. Captured data retains
its credit for as long as the backing remains live.

Checkpoint encoding runs when the stored checkpoint is demanded. The capture
charge does not depend on traversing the page tables or estimating serialized
size.

Sources: [capture accounting](../../fix/runtime/src/runtime/arca.rs),
[capture and replay](../../fix/runtime/src/runtime/arca/continuation.rs),
[checkpoint encoding](../../fix/runtime/src/runtime/arca/checkpoint.rs).

## Remaining coverage gaps

The capacity ledger starts from total Buddy capacity and its initial used count,
then applies explicit credits. It does not intercept every kernel allocation.
The prototype still lacks proofs for the estimated allowances, initial image
and image-cache ownership, complete preflight of serialized continuation loading,
full-source materialization during sliced reads, and all shared kernel/host I/O
and certification allocations. Passing quota and replay tests does not prove
that every implementation allocation fits the contract.
