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
| 0x0000007f_00000000 | 4 GiB | WebAssembly Memory 127       |
| 0x00000080_00000000 | 4 GiB | WebAssembly Table 0          |
| 0x00000081_00000000 | 4 GiB | WebAssembly Table 1          |
| ...                 | ...   | ...                          |
| 0x000000ff_00000000 | 4 GiB | WebAssembly Table 127        |
| 0x00000100_00000000 | 4 GiB | Fix Blob/Table 0             |
| 0x00000101_00000000 | 4 GiB | Fix Blob/Table 1             |
| ...                 | ...   | ...                          |
| 0x000001fe_00000000 | 4 GiB | Fix Blob/Table 254           |
| 0x000001ff_00000000 | 4 GiB | Fix runtime + wasm2c runtime |


## Memories and Tables

WebAssembly Fix uses memories and tables to make blobs and trees visible to
procedures. Fix-on-Arca supports up to 128 memories and tables. The same
memory/table can be reused multiple times, making this a limitation on the
number of _active_ mappings rather than the number of _total_ mappings.
Furthermore, it would be possible to scale up to an alternative 256 TiB layout,
increasing the limit to 16384 of each, if needed.

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

Similar logic applies to table lookups.

## Data Representations

Fix "data" (BlobData and TreeData) are represented as Arca tables containing 4
KiB pages; if a datum is larger than 1 GiB, it is represented as a tuple of up
to four 1 GiB tables.  The length and type of the data are stored in the Fix
handle; they cannot be deduced from the raw data representation in Arca.

The representation of a datum must not be perceptible to a Fix procedure.  That
is, if there are multiple valid ways to physically represent the same Fix
datum, a Fix procedure must not be able to tell them apart.

## Trust boundary

The Fix Shell itself is a trusted component of Fix.  The Fix procedure (compiled
from WebAssembly) is not.  The Fix Shell therefore must be designed and written
defensively to prevent a malicious Fix procedure from breaking Fix's safety
guarantees (notably, determinism).

## Interaction with Fix

A fragment of the Fix runtime is resident within the same Arca process as the
user program.  This includes the wasm2c runtime and the implementations of some
Fix API functions.

Many Fix API calls can be satisfied simply by flipping bits in a Handle.
Handles are represented within the runtime as 32-byte vectors, compatible with
AVX2 registers; the Fix procedure only sees a WebAssembly externref.

To satisfy more complicated Fix API calls (e.g., creating/attaching blobs and
trees), the in-process Fix runtime produces an "effect" to be handled by the
machine-level Fix runtime.  This uses Arca's continuation-capture machinery, so
it appears to the Arca process to be a blocking system call.

## Fix "Local Handles"

As a performance optimization, not every blob/tree creation is sent to the
machine-level runtime.  Instead, when a Fix procedure tries to create a
blob/tree from a WebAssembly memory/table, the in-process Fix runtime remaps
that blob/tree to a higher address (0xff00000000-0x1fe00000000) and returns a
"local handle".  The "name" in this handle is actually the virtual address of
the blob/tree; it is distinguished from a "true" handle by metadata bits within
the handle.  When a Fix procedure exits, the Fix Shell "upgrades" its result if
necessary by constructing a "real" equivalent using effects. In the worst case,
this may require a recursive traversal of the result.

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
