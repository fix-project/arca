# Fix on Wasm

`fix/api` provides the platform-independent `fix` crate: the `FixShell` and
`Invocation` interfaces and typed `Value`, `Blob`, `Tree`, `Object`, `Reference`,
`Thunk`, and `Encode` wrappers. `FixShell` uses static operations, like Arca's
`Runtime`. Values and constructors use the execution's single shell implicitly.
`fixcli::Shell`, `fixruntime::api::Shell`, and `fixwasm::backend::Shell` provide
Linux, kernel, and Wasm implementations. Their shell modules export concrete
`Blob`, `Tree`, and `Value` aliases.

```rust
#![no_std]
use fixwasm::{Error, backend::{Blob, Tree, Value}};

#[fixwasm::apply]
fn apply(combination: &Tree) -> Result<Value, Error> {
    let blob = Blob::from(42u64);
    let tree = Tree::new(&[blob.into()]);
    let child = Blob::try_from(tree.get(0)?.expect("one child"))?;
    let mut bytes = [0; 8];
    child.read_exact(0, &mut bytes)?;
    Ok(child.into())
}
```

Linux installs its shell with `Shell::open` and releases it with `close` or
`Drop`. The kernel installs its backend once with `Shell::initialize`. Wasm
entry points establish the current invocation automatically.

The core interface needs no allocator. `api_alloc` enables allocating helpers
such as `Blob::to_vec` and collecting values into a `Tree`. Linux enables this
feature; Wasm enables it with `sdk_alloc`.

Construction, metadata, references, and thunk/encode transformations are
infallible. `From` converts Rust scalars and bytes into Blobs; `TryFrom` checks
a Value's type. Trees have `iter`, borrowed and owned `IntoIterator`, and
`FromIterator` under `alloc`. Iterators yield `Result<Value, Error>` because
fetching a child can fail. `get` returns `Result<Option<Value>, Error>`.
`Blob::read` returns the byte count; `read_exact` requires the whole range.

Blobs and Trees implement `Default`, `Clone`, `Debug`, and `PartialEq`. Their
defaults are empty values. Equality follows Fix comparability; noncomparable
values and failed comparisons compare unequal. Blobs also implement `Eq`.
Value and Object default to the empty Blob. Tree iterators implement `Clone`,
`Debug`, `DoubleEndedIterator`, `ExactSizeIterator`, and `FusedIterator`.
`Copy`, ordering, and hashing are not provided for owned values.

## Descriptors and ownership

The Wasm ABI uses `i32` descriptor indices. The shell allocates the lowest free
slot. `close` clears a slot and makes it available for reuse. Invalid or double
close traps. A stale integer can refer to a later allocation of the same slot.
Tree creation copies handles from an array of descriptor indices in Memory 0;
it leaves the input slots open. Tree access allocates descriptors for the
returned children.

`include/fix.h` contains the C imports. C callers own returned descriptors and
release them with `fix_close`; `fix_clone` creates a separately owned slot.
`include/wasm.h` declares bulk-memory helpers synthesized by the postprocessor.

The `fixwasm` crate implements those imports without a C bridge. The typed
SDK wraps owned descriptors: `Drop` closes the slot and `Clone` allocates a
copy. `From` and checked `TryFrom` conversions integrate Fix values with Rust
values and types. The Wasm convenience API builds values eagerly; Trees use
slices, with iterator construction available under `alloc`.

Memory 0 belongs to Rust. Memory 1 is the Blob staging area. No Wasm tables are
reserved for Fix handles.

## Memories

A `Memory` exclusively owns a declared Wasm memory index. Constructing it from
an index is unsafe: the memory must be empty, and no other owner may access it.
Indices 0 and 1 are reserved by the SDK.

```rust
use fixwasm::Memory;

let mut memory = unsafe { Memory::from_index(2) }.initialize(1)?;
memory.write(0, &42u64.to_le_bytes())?;
let blob = memory.create_blob(8)?;
let memory = memory.detach().attach(&blob);
let mut bytes = [0; 8];
memory.read_into(0, &mut bytes)?;
let mut memory = memory.make_mut();
memory.write(0, &43u64.to_le_bytes())?;
```

`Memory<Unmapped>` can be initialized as `ReadWrite` or attached to a Blob as
`ReadOnly`. Both mapped states provide bulk reads. Writes, growth, and Blob
creation require `ReadWrite`. `make_mut` preserves the contents and enables
writes, with copying deferred until needed. Read-only length is the Blob's
length; writable length includes the full mapped pages. `detach` returns an
unmapped memory. Dropping any memory discards its mapping. A Blob created from
a writable memory remains unchanged by later writes.

Privileged code may use `attach_tree_bytes` to read packed handle encodings.
This requires the runtime's existing privilege checks.

## Derived helpers

The Wasm library also provides procedure entry macros, argument decoding,
Flatware file/stat codecs, advice, scheduler maps, and streams. The `alloc`
feature enables iterator-based helpers and importers. `std` and `git` enable
the corresponding host I/O and Git support.
