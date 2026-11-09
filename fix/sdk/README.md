# Fix SDK

`fix.h` declares the implemented Wasm imports from module `fix`. A `fix_handle`
is a native `__externref_t`, including objects, references, thunks, and
encodes. C clients export `_fix_apply`. Native handles can be function
parameters, return values, and locals; Clang does not permit handle fields in
ordinary structs or classes. The Rust SDK distinguishes existing Fix handles
from pending Fix values.

Combinations use field 0 for metadata, field 1 for the callable, and fields
2 onward for arguments. Callable Trees also place their callable in field 1.
Ordinary combinations currently use an empty Blob for metadata.

Raw bindings reserve no resources. Memory and Table arguments are actual Wasm
indices. Table indices include funcref Tables. Constructors retain their shell
type preconditions: references require Objects, identification requires Data,
application and selection require Tree Objects or TreeRefs, and strict/shallow
encodes require Thunks. Invalid arguments can trap. C exposes `eval`, `effect`,
`return_cc`, and `call_cc`; Rust exposes direct-style `eval`. `is_comparable` tests whether a
handle supports comparison without computation; `equals` compares comparable
values and traps otherwise.

## Wasm operations from C

`wasm.h` wraps Clang's native Table builtins. Callers declare their own Tables,
for example `static fix_handle values[0]`, and pass their actual Wasm indices
to Fix imports. The bindings allocate no scratch resources.

`fix_cc_wasm` compiles C and runs the Wasm postprocessor. Its `memories` and
`tables` arguments specify total counts, including existing declarations.
Additional Memories and externref Tables start empty.

Clients declare their operations using the macros in `wasm.h`; the header
provides no default declarations. These operations import from
`__wasm_postprocessor`. The postprocessor replaces those imports with native
Wasm function definitions. Function names specify resource indices; only
requested functions are added. Memory 0 remains the C main Memory. These
operations contain no Fix logic.

## Type queries

`get_type` returns Object=0, Ref=1, Thunk=2, or Encode=3. `get_data_type`
accepts only Objects and Refs and returns Blob=0 or Tree=1. `get_encode_type`
accepts only Encodes and returns Strict=0 or Shallow=1. Invalid query arguments
trap at the Wasm boundary.

Rust uses `ValueType`, `DataType`, and `EncodeType`, with `value_type()`,
`data_type()`, and `encode_type()` accessors. C uses `fix_value_type`,
`fix_data_type`, and `fix_encode_type`. Queries reveal no naming/storage
details, thunk construction method, or underlying data type of a Thunk or
Encode.

## Rust values

The `Value` trait describes a Fix value, existing or pending. Each constructor
and adaptor implements this interface directly: `Blob::create` produces
`CreateBlob`; Tree construction produces `CreateTree`; `reference`,
`identification`, `application`, `selection`, `strict`, and `shallow` produce
corresponding adaptors. Their methods follow the Fix operations and constructor
preconditions. `choose(condition, left, right)` retains either value. `erase()`
widens the static Fix type to `Any`; it does not erase the adaptor's concrete
type.

`Handle<T, P>` represents an existing Fix handle. `T` is its Fix type:
`Object<Blob>`, `Object<Tree>`, `Ref<Blob>`, `Ref<Tree>`, `Thunk`, `Encode`, or
`Any`. `P` describes a lookup from the input combination, whose location is
`Focus`. Native externrefs remain in the C bridge; handles retain lookup paths.
Constructed values retain their inputs in Rust memory and are interpreted when
the procedure returns. Static adaptors need no heap allocation.

`fix::invalidate_externrefs()` resets the procedure's handle arena while
preserving live references, Rust lookup paths, and pending values. Call it between
operations in long-running loops. Reference slots are reclaimed automatically
when their native function returns or unwinds.

`value.eval()` evaluates through the current continuation and returns
`Handle<Any, Capture>`. The owned capture remains valid across further evaluations
and explicit invalidation. Borrowed views cannot outlive it; dropping the owner
releases its entry for reuse. Captures are movable and do not implement `Clone`.
Their storage grows with the maximum number of live captures, rather than the
total number of evaluations. No Rust heap allocator is required.

Evaluation follows Fix's ordinary rules: bare Refs and Thunks remain opaque;
strict and shallow encodes request computation. For example:

```rust
let result = 41u64.reference().identification().strict().eval()?;
fix::invalidate_externrefs()?;
assert_eq!(result.read_u64()?, 41);
```

Evaluation, effects, and reclamation are supported inside construction and
comparison callbacks. Their C-stack externrefs remain rooted across capture.
The C bridge stores owned captures in its private Wasm Table 2 and reuse metadata
in Memory 2; Memory 1 and Table 1 remain scratch resources. The Shell does not
need to know the SDK's capture lifetimes.

The C import `fix_invalidate_externrefs(keep)` preserves `keep`, live locals and
globals, all externref Tables, and Blob and Tree attachments. Surviving local
objects are promoted and the rest of the handle arena is released. Storage is not
reclaimed. The returned reference denotes `keep`.

Rust procedures receive `&mut Handle<Object<Tree>, Focus>` or a shared
reference for read-only access. `get(index)` borrows an existing Tree's child.
Both existing handles and pending Trees support `with(index, callback)`: it
checks the child and calls the callback immediately. Rust's mutable-handle form
retains an exclusive parent borrow. Pending values move into the child. No
callback is deferred, and no native root switching is needed.

Rust uses ordinary `&dyn Value<Type = T>` for borrowed type erasure and `Rc<dyn
Value<Type = T>>` for owned type erasure when `alloc` is enabled. Borrowed
sources must outlive their views.

Rust `TryFrom`/`TryInto` narrows existing handles to typed `Handle`s and
pending values to typed `Cast` adaptors. Type checks happen immediately.

Rust tuples of up to twelve values convert into `CreateTree` with `From`.
`TryInto` checks exact length and child types. Existing handles unpack into
child handles; constructed Trees and borrowed Tree trait objects unpack into
`Cast` adaptors. `unpack` also accepts an explicit tuple target type.

```rust
let pair = fix::tree![42u64, fix::tree![7u64]];
let view: &dyn Value<Type = Object<Tree>> = &pair;
let (number, children): (fix::Cast<Object<Blob>, _>, fix::Cast<Object<Tree>, _>) =
    view.try_into()?;
```

`iter()` traverses Tree children without allocation. `Tree::from_iter` accepts
cloneable iterators, including iterators of `Result<V, Error>`; children are
recomputed on access. `tree!` retains heterogeneous children by value.
`Tree::create` also accepts borrowed or owned slices of values.

`fields.tag()?` constructs a Tag with exactly the supplied fields. The first
field must be a comparable author: the current combination or a callable reached
by following field 1, either as an Object or a Ref. The Shell checks this identity
and preserves its representation. Other fields are application-defined.
The C operation is `fix_create_tag(table)`.

Rust's `alloc` feature enables Blob `read()` returning `Result<Vec<u8>, Error>`
and Tree `children()` returning a vector of child values. Existing-handle child
reads can fail and return `Result`; constructed Trees return their vector
directly. Keep the returned buffers for ordinary indexing and slicing.
`Blob::create(&bytes[1..4])` and `Tree::create(&children[1..])` explicitly
construct new Fix values from slices. These operations do not construct Fix
selection thunks. `read_into` and `read_u64` require no allocation.

Rust `u32` and `u64` represent little-endian Blobs; `bool` represents a one-byte
Blob containing 0 or 1. `Blob::create` accepts
borrowed slices, owned arrays, or owned buffers. Rust uses `len()` and
`is_empty()`. Reads and type, length, and tag queries inspect local value
memory or look up input handles; they do not create Fix objects.
`Value::len()` returns `Result<u64, Error>`; C's `fix_len` returns `uint64_t`.
Lengths count Blob bytes or Tree elements, including for Refs whose backing
data is unavailable. Local buffers and indices use `usize`; conversions from
Fix lengths are checked.

```rust
use fix::{Blob, Error, Focus, Handle, Object, Tree, Value};

#[fix::apply]
fn apply(combination: &Handle<Object<Tree>, Focus>)
    -> Result<impl Value<Type = Object<Blob>>, Error>
{
    let left: Handle<Object<Blob>, _> = combination.get(2).try_into()?;
    let right: Handle<Object<Blob>, _> = combination.get(3).try_into()?;
    Ok(Blob::create(left.read_u64()?.wrapping_add(right.read_u64()?).to_le_bytes()))
}
```

Allocator-free recursive programs can return Fix application thunks that call
the same procedure with the next arguments. Allocating Rust clients can instead
use `Rc<dyn Value<Type = Any>>` for a finite recursive return type.

`is_comparable` tests handle comparability, and `is_value` tests whether a handle
is fully evaluated (contains no Encode); `equals` returns `NotComparable`
for ineligible operands. The Rust SDK delegates equality to Fix, temporarily
materializing pending operands. Rust SDK handles and adaptors implement
`PartialEq` by forwarding to Fix equality: `==` returns false for non-comparable
operands; other comparison errors trap. They do not implement `Eq`, since
non-comparable values compare unequal to themselves.

The private Rust runtime in `src/rust.c` uses Memory 1 and externref Table 1
as scratch resources; function pointers use Table 0. The bridge retains the input combination and keeps
pending Tree children and equality operands in native call-frame locals.
Scratch resources are released on normal completion and returned errors.

Rust's `//fix/sdk` disables `alloc` and `//fix/sdk:sdk_alloc` enables it;
allocating clients provide a global allocator. `#[fix::apply]` exports
`_fix_apply` and interprets the returned value.

`#[fix::main]` provides an allocation-free SDK calling convention:

```rust
#[fix::main]
fn add(a: u64, b: u64) -> Result<u64, fix::Error> {
    a.checked_add(b).ok_or(fix::Error::OutOfBounds)
}
```

It reads the fixed number of parameters into a stack array, returns an underapplied combination unchanged,
and applies excess arguments to the result. `Argument` converts each parameter:
its `Argument::EVALUATION` says how far the input is evaluated first.
`Handle<Any, Capture>` stays lazy. Object handles and scalar arguments use strict
encodes to evaluate inputs and resolve Refs. Ref handles use shallow encodes for
Thunks and take an Object as its Ref. `Evaluated` handles, an Object or a Ref,
use strict encodes for Thunks and keep Refs.
When evaluation is needed, the entrypoint returns a new application with encoded
arguments for the scheduler to evaluate; it does not capture a continuation.
`u64`, `u32`, and `bool` read Blob values. Booleans use one byte, 0 or 1. Functions return an SDK `Value`
or `Result<Value, Error>`. `#[fix::main(combination)]` also passes the original
combination as the first Rust parameter.

Returned errors produce the normal effect combination `["Fix.error", message]`.
This is an SDK convention. `#[fix::apply]` retains direct
access to the combination and its existing error behavior. `call`, `reapply`,
`visit_arguments`, and `apply_remaining` are also available without `alloc`.
`arguments` collects all arguments into a `Vec` and requires `alloc`.

## File descriptors and stat

Rust `files` provides canonical descriptor and stat codecs. C filesystem codecs
belong to Flatware.

## Scheduler importers

`stream::Stream` wraps the TCP and read-only URI stream effects. Streams belong
to the current worker and must be closed explicitly. `importers::blob(uri, limit)`
reads an HTTP(S) or local `file:` URI into a Blob, returning `None` on acquisition
failure or when the result exceeds the byte limit. These helpers require a
scheduler execution context; ordinary Fix evaluation does not handle their effects.

`//fix/sdk:sdk_import` additionally enables Git import through the memory-only
`//fix/importers/git` library. Git protocol and object decoding run in userspace
over the same TCP streams. This SDK variant requires a global allocator and
provides `std::io::Read` and `Write` implementations for `Stream`.

Git URIs have the form `git://HOST[:PORT]/REPO#REV:PATH`. The default port is
9418, the default revision is `HEAD`, and an empty path selects the root tree.
Revisions can be advertised refs, unambiguous branch/tag names, or full SHA-1
object IDs accepted by the server. Paths are relative and percent-encoded.
`importers::blob` selects a Blob; `importers::directory` selects a directory and
returns its contents as a Tree of normalized Flatware child descriptors. Local
`file:` directories are also supported. Children are
sorted by filename bytes, timestamps and device IDs are zero, and link counts
are one. Symlink contents are preserved; executable permissions are not represented.
Submodules become empty directories and are not fetched recursively.

`importers::fetch(uri)` infers Blob or directory from the source without a
known output size. `fix todo add URI` uses it to import the content, persist
the resulting object and its URI hint, and print the canonical handle.
`fix todo add VALUE URI…` registers hints for an existing value without fetching.

Git fetches a self-contained snapshot, shallow when supported by the server;
the Blob limit bounds the selected output, not the transferred pack. Only the
native `git://` transport and SHA-1 repositories are supported. Callers needing
a pack/decoding budget can use the backend's `fetch_with_limit` with a `Stream`.
Importers return ordinary Fix values. Schedulers control hints, persistence,
and comparison with the expected canonical name.

`fix_unsound_handle_bytes` returns a Blob containing the existing 32-byte
encoding of a kernel-resolved handle, without canonicalizing it. It requires
a privileged invocation and traps otherwise. Different encodings need not
denote different Fix values.

For repeated access, attach a value to a caller-owned Wasm resource:

```rust
let mut table = unsafe { fix::Table::from_index(3) };
let attached = table.attach(&tree)?;
let first = attached.get(0).read_u64()?;
let second = attached.get(1).capture()?;
```

`Memory::attach` provides a Blob view with `read_into`. Attached reads use Wasm
Table access or Memory copy operations. The existing handle `get` and
`read_into` methods perform one-off reads.

Declare the additional resources when postprocessing the Wasm module; indices
0–2 belong to the SDK. `from_index` requires exclusive ownership of the declared
resource. A view borrows its resource and detaches on drop. Capture an element to
retain it after the view is dropped. Separate resources support simultaneous
attachments and may remain attached across evaluation or reclamation.

`fix::advise(&value, key, payload)` emits a non-normative event without
forcing the value. Keys are UTF-8 strings and payloads are bytes. Conventional
keys are `fix::advice::TITLE`, `PROGRESS`, and `CANONICALIZE`.

```rust
fix::advise(&combination, fix::advice::TITLE, b"compile module")?;
fix::advise(&combination, fix::advice::CANONICALIZE, b"")?;
```

Schedulers choose how to handle or discard each event. Advice has no lookup
operation in ordinary Fix execution and is excluded from handles and coupons.
The runtime's `execute_observed` and `resume_observed` APIs expose events to
host callers; unobserved execution discards them.
