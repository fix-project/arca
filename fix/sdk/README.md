# Fix SDK

`fix.h` declares the implemented Wasm imports from module `fix`. A `fix_handle`
is a native `__externref_t`, including objects, references, thunks, and
encodes. C clients export `_fix_apply`. Native handles can be function
parameters, return values, and locals; Clang does not permit handle fields in
ordinary structs or classes. The Rust SDK distinguishes existing Fix handles
from pending Fix values.

Raw bindings reserve no resources. Memory and Table arguments are actual Wasm
indices. Table indices include funcref Tables. Constructors retain their shell
type preconditions: references require Objects, identification requires Data,
application and selection require Tree Objects or TreeRefs, and strict/shallow
encodes require Thunks. Invalid arguments can trap. Evaluation and
continuations are not supported bindings. `is_eq` checks equality eligibility;
`equals` compares eligible values and traps for non-Eq operands.

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
`Tree::create` also accepts borrowed or owned slices of values. Calling
`tag()` on a constructed Tree uses the same child storage to construct a Tag.

Rust's `alloc` feature enables Blob `read()` returning `Result<Vec<u8>, Error>`
and Tree `children()` returning a vector of child values. Existing-handle child
reads can fail and return `Result`; constructed Trees return their vector
directly. Keep the returned buffers for ordinary indexing and slicing.
`Blob::create(&bytes[1..4])` and `Tree::create(&children[1..])` explicitly
construct new Fix values from slices. These operations do not construct Fix
selection thunks. `read_into` and `read_u64` require no allocation.

Rust `u32` and `u64` represent little-endian Blobs. `Blob::create` accepts
borrowed slices, owned arrays, or owned buffers. Rust uses `len()` and
`is_empty()`. Reads and type, length, and tag queries inspect local value
memory or look up input handles; they do not create Fix objects.

```rust
use fix::{Blob, Error, Focus, Handle, Object, Tree, Value};

#[fix::apply]
fn apply(combination: &Handle<Object<Tree>, Focus>)
    -> Result<impl Value<Type = Object<Blob>>, Error>
{
    let left: Handle<Object<Blob>, _> = combination.get(1).try_into()?;
    let right: Handle<Object<Blob>, _> = combination.get(2).try_into()?;
    Ok(Blob::create(left.read_u64()?.wrapping_add(right.read_u64()?).to_le_bytes()))
}
```

Allocator-free recursive programs can return Fix application thunks that call
the same procedure with the next arguments. Allocating Rust clients can instead
use `Rc<dyn Value<Type = Any>>` for a finite recursive return type.

`is_eq` checks Fix equality eligibility; `equals` returns `NotEq` for
ineligible operands. The Rust SDK delegates equality to Fix, temporarily
materializing pending operands. Rust SDK handles and adaptors implement
`PartialEq` by forwarding to Fix equality: `==` returns false for non-Eq
operands; other comparison errors trap. They do not implement `Eq`, since
non-Eq values compare unequal to themselves.

The private Rust runtime in `src/rust.c` uses Memory 1 and externref Table 1
as scratch resources; function pointers use Table 0. The bridge retains the input combination and keeps
pending Tree children and equality operands in native call-frame locals.
Scratch resources are released on normal completion and returned errors.

Rust's `//fix/sdk` disables `alloc` and `//fix/sdk:sdk_alloc` enables it;
allocating clients provide a global allocator. `#[fix::apply]` exports
`_fix_apply` and interprets the returned value.

## File descriptors and stat

Rust `files` provides canonical descriptor and stat codecs. C filesystem codecs
belong to Flatware.
