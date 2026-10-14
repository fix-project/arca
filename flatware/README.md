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

## Running programs

Run a standard WASIp1 command module:

```sh
bazel run //flatware:run -- --dir ./input::/work --stdin ./stdin.txt program.wasm argument
```

`--dir HOST[::GUEST]` snapshots a host directory as a guest preopen. Without
`::GUEST`, the guest path is the supplied host path. Multiple mounts must have
nonoverlapping guest paths. `--env NAME=VALUE` supplies an environment entry.
Runner options precede guest arguments; `--` separates guest arguments that
would otherwise be interpreted as runner options.

Input is buffered from `--stdin FILE`, or empty when omitted. Guest stdout and
stderr are written to the corresponding host streams. The runner returns the
guest's exit status. It writes returned directories to a fresh temporary
directory and prints `output directory: PATH` to stderr, including when the
guest exits unsuccessfully. Absolute guest paths become relative paths within
that directory. Source directories are not overwritten.

Snapshots support regular files, directories, and preserved symbolic links.
Flatware does not support following symbolic links. Special files are rejected.
Returned files retain contents and access/modification timestamps; host inode
identities, permissions, and hard links are not reproduced.

The runner uses the existing `wasm-merge`, `wasm2c`, compiler, and KVM toolchain.
The Fix shell shares a 4 GiB address region between its native executable and
heap. Only ELF segments and heap allocations are mapped; unused address space
does not consume memory. Module instances are allocated using wasm2c's reported
size and alignment.
The standalone utilities in `//programs/flatware` use precompiled modules through
the same runner.

## Archive conversion

The `cpio` program takes one argument: a `newc` or `crc` Blob expands into a
Flatware directory descriptor, and a directory descriptor serializes to `newc`.
It supports directories, regular files, and symlinks, including hard-link data
stored on a later archive entry. GNU `crc` checksums are verified for regular
files. Modification times retain whole-second precision; missing access and
change times are zero. Flatware does not represent ownership, permissions, or
hard-link identities. Export uses uid/gid zero and modes 0755 for directories,
0644 for files, and 0777 for symlinks.

`flatware-normalize` takes one descriptor, recursively resets all three timestamps
to zero and link counts to one, and preserves names, devices, types, sizes, and
file-content handles. Directory children are recursive Fix applications, allowing
parallel evaluation without a Rust allocator.

