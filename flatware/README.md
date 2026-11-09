# Flatware implementation

Flatware is a freestanding WASIp1 adapter. It links standard command modules
with `wasm-merge`, retaining separate guest and adapter memories. The control
bridge copies guest buffers and catches the adapter's exit tag.

The WABT 1.0.42 translator supports legacy and modern Wasm exception handling.
Exception references retain upstream's 256-byte inline payload limit; legacy
exceptions continue to use dynamically sized payload storage.

The adapter uses the C Fix bindings directly. It reads and constructs descriptor
Trees with one scratch Memory and Table. One scoped externref global holds the
original combination; input parent and child indices locate preserved fields
after filesystem mutations. Names, metadata, directory entries, and file contents
load on demand. Output reuses unchanged descriptors and subtrees, rebuilding
only changed fields and their ancestors. WASI inode numbers are unique within
an invocation; they are assigned as nodes are discovered and are not serialized.
Descriptor metadata contains device, filetype, link count, size, atime, mtime,
and ctime, in that order. Filetype is one byte; other fields are eight-byte
little-endian integers. Filesystem operations
and WASIp1 bindings are implemented in C.
Reads preserve access time; explicit timestamp-setting operations can change it.

The adapter allocates its buffers with upstream dlmalloc 2.8.6. Its MORECORE hook
grows only the adapter memory in whole Wasm pages. The allocator does not use OS
services or manage the guest heap. Allocation failure traps.

Random bytes use the authors' MT19937-64 reference implementation, initialized
with the input's eight-byte little-endian seed. Each generated 64-bit word is
serialized little-endian. Partial words carry across calls, so request sizes do
not change the byte stream. Each invocation resets the generator and byte cursor.

`//tests/flatware:filesystem_test` covers filesystem mutations and capabilities, descriptor
flags, standard streams, reference PRNG outputs, and independent stream chunking.
Integration tests in `//tests/flatware` exercise the adapter and Rust WASIp1 standard
library programs in the VM, checking status, output, and returned files.

## Host files

`fix flatware-import PATH [NAME]` snapshots a file, directory, or symlink as a
descriptor named NAME (default: the input basename). Directory entries are
sorted by name and `.fix` entries are skipped. Device, link count, and all
three timestamps are normalized to zero, one, and zero; a directory's size is
its entry count. `fix flatware-export DESCRIPTOR PATH` writes a descriptor to a
new PATH. Ownership and permissions are not represented; exported directories
and files use the host's default modes. Both commands use the native library
`//flatware:tree`, which the bootstrap seed tool also uses to build `/toolchain`.
