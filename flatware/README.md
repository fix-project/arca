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

Link a WASIp1 Wasm Blob with Flatware, compile it, then evaluate explicitly:

```sh
wasm=$(fix create-blob program.wasm)
linked=$(fix flatware-link "$wasm")
thunk=$(fix compile "$linked")
executable=$(fix eval "$(fix create-strict-encode "$thunk")")
fix label set program "$executable"
fix flatware-run program argument < stdin.txt
```

`fix flatware-link` attaches the WASIp1 adapter; `fix compile` translates the
supplied module unchanged and constructs pending compilation and ELF linking
operations using Python and the JSON deserializer. No compiler runs until evaluation.
Intermediate files stay in Flatware directory values. It uses repository labels
`wasm-merge`, `wasm2c`, `clang`, `path`, and `toolchain`.
`FIX_COMPILE_PARTITIONS` selects C partitions (default 8); evaluation parallelism
is independent. The merger rejects unresolved memory and table imports, keeping
the adapter's resource indices unchanged.

Import a UNIX directory, construct an invocation, evaluate it, and export its
returned directory:

```sh
directory=$(fix flatware-import ./input /work)
fds=$(fix create-tree "$directory")
thunk=$(FIX_FDS="$fds" fix flatware-application program argument < stdin.txt)
result=$(fix eval "$(fix create-strict-encode "$thunk")")
mapfile -t output < <(fix read-tree "$result")
mapfile -t descriptors < <(fix read-tree "${output[0]}")
fix flatware-export "${descriptors[3]}" ./result
```

`flatware-application` only constructs an application Thunk. `flatware-run`
evaluates that Thunk, forwards stdout/stderr, and exits with the guest's status.
Both read host stdin into the invocation. Optional settings are environment
variables: `FIX_FDS` supplies preopens, `FIX_ENV` supplies a Tree of `NAME=VALUE`
Blobs, and `FIX_ARGV0` overrides the process name. `FIX_JOBS` selects evaluator
parallelism for `flatware-run`; `FIX_RESULT_LABEL` preserves its result.
Seed and initial clock are zero.

`flatware-import PATH [NAME]` snapshots a file, directory, or symlink. Names
default to the input basename; timestamps are zero and link counts one.
Snapshots exclude `.fix`. Import packages the host entries with GNU `cpio`, then
composes the `cpio` and `flatware-normalize` Fix programs. Export applies `cpio`
and extracts its output into a new directory with system `cpio`.
`flatware-export DIRECTORY PATH` refuses an existing destination.

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

A callable Tree `(compose outer inner)` applies the inner procedure to its
arguments, consumes that result strictly, and returns the outer application's
Thunk. For example, import uses `(compose flatware-normalize cpio)`. Both programs
and `compose` are included as labels in the bootstrap repository.

The `toolchain` label contains `usr/` (sysroot and LLVM resource files) and
`share/fix/` (adapter/control Wasm, `wasm-rt.c`, shell headers, `memmap.ld`, and
`libfixshell.a`). Build the bootstrap with `bazel build //bootstrap`, or import
a prebuilt archive with `fix import PATH`.
