# Flatware implementation

Flatware is a freestanding WASIp1 adapter. It links standard command modules
with `wasm-merge`, retaining separate guest and adapter memories. The control
bridge copies guest buffers and catches the adapter's exit tag.

The adapter uses the C Fix bindings directly. It reads and constructs descriptor
Trees with one scratch Memory and Table. The original combination stays in native
externref locals; input parent and child indices locate preserved fields
after filesystem mutations. Filesystem operations and WASIp1 bindings are
implemented in C.

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
The standalone utilities in `//programs/flatware` use precompiled modules through
the same runner.
