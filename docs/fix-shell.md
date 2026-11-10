# Fix Shell

The Fix Shell is a partial implementation of a Fix runtime for the Arca kernel.
It implements the Fix API in terms of Arca system calls and "effects" to the
machine-level Fix runtime.

## Memory Map

The memory map for 512 GiB-sized procedures is:


| Start Address       | Size  | Description                  |
|---------------------|-------|------------------------------|
| 0x00000000_00000000 | 4 GiB | WebAssembly Memory 0         |
| 0x00000001_00000000 | 4 GiB | WebAssembly Memory 1         |
| ...                 | ...   | ...                          |
| 0x0000003f_00000000 | 4 GiB | WebAssembly Memory 63        |
| 0x00000040_00000000 | 4 GiB | Fix Blob/Tree 0              |
| 0x00000041_00000000 | 4 GiB | Fix Blob/Tree 1              |
| ...                 | ...   | ...                          |
| 0x0000007d_00000000 | 4 GiB | Fix Blob/Tree 61             |
| 0x0000007e_00000000 | 4 GiB | Handle Descriptor Table      |
| 0x0000007f_00000000 | 4 GiB | Fix runtime + wasm2c runtime |


## Memories

WebAssembly Fix uses memories to make blobs visible to procedures. Fix-on-Arca
supports up to 64 memories. The same memory can be reused multiple times,
making this a limitation on the number of _active_ mappings rather than the
number of _total_ mappings. Furthermore, it would be possible to scale up to an
alternative 256 TiB layout, increasing the limit to 16384 of each, if needed.

To allow for more efficient memory accesses, we give each memory a
statically-known virtual address. This allows us to compile an instruction like:

```wasm
i32.load (memory $mem) (local.get $off)
```

into a C statement like:

```c
(mem << 32) | ((uint32_t) off)
```

which, since `mem` is known at compile time, gets compiled to an efficient
x86-64 assembly sequence like:

```asm
mov     eax, $off
movabs  rcx, ($mem << 32)
mov     eax, dword ptr [rax + rcx]
```

## Data Representations

Fix "data" (BlobData and TreeData) are represented as Arca tables containing 4
KiB pages; if a datum is larger than 1 GiB, it is represented as a tuple of up
to four 1 GiB tables.  The length and type of the data are stored in the Fix
handle; they cannot be deduced from the raw data representation in Arca.

The representation of a datum must not be perceptible to a Fix procedure.  That
is, if there are multiple valid ways to physically represent the same Fix
datum, a Fix procedure must not be able to tell them apart.  This includes
under continuation capture, which means nondeterministic information must be
scrubbed from continuations.

## Trust boundary

The Fix Shell itself is a trusted component of Fix.  The Fix procedure (compiled
from WebAssembly) is not.  The Fix Shell therefore must be designed and written
defensively to prevent a malicious Fix procedure from breaking Fix's safety
guarantees (notably, determinism).

## Interaction with Fix

A fragment of the Fix runtime is resident within the same Arca process as the
user program.  This includes the wasm2c runtime and the implementations of some
Fix API functions.

Fix handles are not directly exposed to WebAssembly procedures.  They instead
refer to handles through indices in a descriptor table, similar to file
descriptors.  As Trees are simply memory regions containing handles, this
descriptor table can be quickly "detached" as a Tree when capturing
continuations.

Many Fix API calls can be satisfied simply by flipping bits in a Handle.
Handles are represented within the runtime as 32-byte vectors.

To satisfy more complicated Fix API calls (e.g., creating/attaching blobs), the
in-process Fix runtime produces an "effect" to be handled by the machine-level
Fix runtime.  This uses Arca's continuation-capture machinery, so it appears to
the Arca process to be a blocking system call.

## Fix "Local Handles"

As a performance optimization, not every blob/tree creation is sent to the
machine-level runtime immediately.  Instead, when a Fix procedure tries to
create a blob from a WebAssembly memory, the in-process Fix runtime remaps that
blob to a higher address (0x4000000000-0x7d00000000) and returns a "local
handle" (a similar process applies to trees).  The "name" in this handle is
actually the virtual address of the blob/tree; it is distinguished from a
"true" handle by metadata bits within the handle.  When a Fix procedure exits,
the Fix Shell "upgrades" its result if necessary by constructing a "real"
equivalent using effects. In the worst case, this may require a recursive
traversal of the result.

There are three types of handle in total:
- Canonical Handles: content-addressed, valid across all of space and time
- Machine Handles: index-addressed, valid for a particular boot of a machine
- Local Handles: virtually addressed, valid only within the invocation of a
  particular Fix procedure

The general invariant is that a tree referred to by a more-local handle may
contain less-local handles (a locally-named tree may refer to machine-named and
canonically-named data), while the opposite is not possible (a
canonically-named tree may *only* refer to other canonically-named data).

## Runtime Effects

- `create_blob`
- `create_tree`
- `get_blob`
- `get_tree`
