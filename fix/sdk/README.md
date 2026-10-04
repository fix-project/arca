# Fix SDK

`fix.h` declares implemented Wasm imports from module `fix`. A `fix_handle` is
a native `__externref_t`. It can be a parameter, return value, or local, but
Clang does not permit externref fields in ordinary structs or classes. C
clients can use these bindings directly and export `_fix_apply`.

Raw bindings reserve no resources. Memory and Table arguments are actual Wasm
indices, including funcref Tables. Constructors retain Fix's type
preconditions: references require Objects; identification requires an Object or
Ref; application and selection require Tree Objects or TreeRefs; strict and
shallow encodes require Thunks. Invalid inputs can trap. Evaluation and
continuations are not supported bindings.

`wasm.h` declares requested native Memory and Table operations through
`__wasm_postprocessor` imports. The postprocessor replaces those imports with
Wasm definitions. Callers declare their own Tables; Memory 0 is the C main
Memory. The raw bindings allocate no scratch resources.

## Rust values

`Value` describes an existing or pending Fix value. Constructors and adaptors
implement it directly. `Blob::create` returns `CreateBlob`, Tree construction
returns `CreateTree`, and the `reference`, `identification`, `application`,
`selection`, `strict`, and `shallow` methods return corresponding adaptors.
`choose(condition, left, right)` retains either value. `erase()` widens its
static Fix type to `Any`, preserving the concrete adaptor type.

`Handle<T, P>` represents an existing Fix handle. `T` is `Object<Blob>`,
`Object<Tree>`, `Ref<Blob>`, `Ref<Tree>`, `Thunk`, `Encode`, or `Any`. `P`
describes a lookup from the input combination, whose location is `Focus`.
Native externrefs remain in the C bridge; handles retain lookup paths. Pending
values retain their inputs in Rust memory and are interpreted when the
procedure returns. Static adaptors and input lookups need no heap allocation.

Procedures receive `&mut Handle<Object<Tree>, Focus>` or a shared reference for
read-only access. `get(index)` borrows an existing Tree's child. Both handles
and pending Trees support `with(index, callback)`, which checks the child and
calls the callback immediately. Mutable handles retain an exclusive parent
borrow. Pending values move into the child. No callbacks are deferred.

`TryFrom`/`TryInto` narrows existing handles to typed `Handle`s and pending
values to typed `Cast` adaptors. Checks happen immediately. Rust uses ordinary
`&dyn Value<Type = T>` for borrowed type erasure and `Rc<dyn Value<Type = T>>`
for owned type erasure. Borrowed sources must outlive their views.

`Blob::create` accepts borrowed slices, owned arrays, or owned buffers. Rust
`u32` and `u64` represent little-endian Blobs. `tree!` retains heterogeneous
children; `Tree::create` accepts borrowed or owned slices. Tuples of up to
twelve values convert into `CreateTree`. `TryInto` and `unpack` check exact
Tree length and child types, returning native handles or pending `Cast`
adaptors. Calling `tag()` on a constructed Tree uses the same child storage
to construct a Tag.

`iter()` traverses Tree children without allocation. `Tree::from_iter` accepts
cloneable iterators of values or `Result<V, Error>`, recomputing children on
access. Blob `read()` returns `Result<Vec<u8>, Error>`; Tree `children()`
returns a vector, in `Result` for existing handles. `read_into` and `read_u64`
allocate no buffers. Slicing those vectors reads Rust data; `Blob::create` and
`Tree::create` explicitly build new Fix values from slices.

`value_type`, `data_type`, and `encode_type` return `ValueType`, `DataType`,
and `EncodeType`. Reads and type, length, and tag queries inspect local value
memory or look up input handles, without constructing Fix objects.

Fix alone defines equality. `is_eq` checks eligibility; SDK `equals` returns
`NotEq` for ineligible operands. Comparisons temporarily materialize pending
values. SDK handles and adaptors implement `PartialEq` by forwarding to Fix:
non-Eq operands compare false, and other errors trap. They do not implement
`Eq`.

The Rust SDK uses `alloc`; clients provide a global allocator. Allocator-free
recursive programs can return Fix application thunks that call the same
procedure with the next arguments. Allocating clients can use `Rc<dyn
Value<Type = Any>>` for a finite recursive return type.

The private Rust runtime in `src/rust.c` uses scratch Memory 1 and externref Table 1; function pointers use
Table 0. It retains the input combination and keeps pending children and
equality operands in native call-frame locals. Resources are released on normal
completion and returned errors. `#[fix::apply]` exports `_fix_apply` and
interprets the returned value.

Rust `files` provides canonical descriptor and stat codecs.
