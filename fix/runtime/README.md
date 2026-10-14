# Fix CLI

Run commands with `bazel run //fix -- COMMAND …`.
`init` initializes the current directory. Other repository commands look for
`.fix` in the current directory, then its parents, and use the nearest repository.
An invalid local `.fix` reports an error rather than selecting a parent.

Use `-C DIRECTORY` before the command to run from another directory, as in Git:
`bazel run //fix -- -C /path/to/project read-tree inputs`.
Repository discovery and relative file arguments use that directory; repeated
`-C` options are applied in order.

```sh
fix init
blob=$(fix create-blob input.bin)
tree=$(fix create-tree "$blob" -l inputs)
fix read-tree inputs
fix create-application-thunk inputs -l pending
fix resolve pending
```

Constructors print a canonical hexadecimal handle. Values can be full handles
or label names; hexadecimal handles take precedence. Unary commands read one
whitespace-separated value from stdin when their argument is `-`.

| Command | Input and result |
| --- | --- |
| `repository` | Print the absolute path to the active `.fix` directory |
| `think THUNK` | Perform one thunk reduction and store its returned value, without recursively evaluating that value |
| `visit VALUE` | Print reachable stored Objects once each; omit literal Blobs and do not evaluate thunks |
| `eval [-j N] VALUE` | Evaluate using serial (`-j 1`) or parallel evaluator; persist and print result |
| `create-blob FILE… \| --files-from MANIFEST \| --string TEXT \| --u8 NUMBER \| --u32 NUMBER \| --u64 NUMBER` | File bytes or formatted argument → Blob handle(s); manifests contain NUL-separated paths, `-` reads stdin |
| `create-tree [--bindings] VALUE… \| -` | Arguments, or stdin tokens → ordered Tree handle; `--bindings -` reads NUL-separated name/value pairs |
| `read-blob [--string \| --u8 \| --u32 \| --u64] VALUE` | Blob Object → bytes, text, or unsigned decimal integer |
| `read-tree [--labels] VALUE` | Tree Object → one child handle per line; `--labels` appends tab-separated, quoted matching labels |
| `create-ref VALUE` | Object → Ref |
| `create-identification-thunk VALUE` | Object or Ref → identification Thunk |
| `create-application-thunk VALUE` | Tree Object or Ref → application Thunk |
| `create-selection-thunk VALUE` | Tree Object or Ref → selection Thunk |
| `create-strict-encode VALUE` | Thunk → strict Encode |
| `create-shallow-encode VALUE` | Thunk → shallow Encode |
| `label list` | List defined labels in lexical order |
| `label get NAME` | Read a label |
| `label set NAME VALUE` | Create or replace a label |
| `resolve VALUE` | Print the canonical handle |
| `get-type VALUE` | `object`, `ref`, `thunk`, or `encode` |
| `get-data-type VALUE` | Object or Ref → `blob` or `tree` |
| `get-encode-type VALUE` | Encode → `strict` or `shallow` |
| `len VALUE` | Blob bytes or Tree elements |
| `is-tag VALUE`, `is-eq VALUE` | Predicate exit status |

Constructors accept `-l NAME`. Label names are single filename components,
excluding `.` and `..`. Inspection does not evaluate or force values.
Predicates exit 0 for true and 1 for false. Errors exit 2 and write diagnostics
to stderr; stdout contains only results.

## Repository and trust

`.fix/data/KEY` contains raw Blob bytes or canonical 32-byte Tree entries.
The key is the Object handle with bytes 30 and 31 replaced by `FF` (a lowercase
hexadecimal filename ending in `ffff`, not a Fix handle).
`.fix/objects/HANDLE` symlinks to `../data/KEY` authorize specific Objects;
matching data alone does not authorize another Object or Tag.
`.fix/labels/NAME` contains a hexadecimal value handle and newline.
Literal blobs are self-contained and need no files.

The CLI parses identifiers and formats output. `Repository::lookup` is the
trusted boundary that turns identifier bytes into authorized handles. Typed
Fix constructors build Refs, Thunks, and Encodes from those handles without
additional records. Host I/O supplies files and streams, without Fix logic.

Objects and labels are published atomically. Trusted backing data is not
rehashed during reads. The repository assumes its files are controlled by the
trusted store; manually editing them can invalidate that assumption. Atomic
publication does not promise durability across power loss.

Evaluation defaults to the host's available parallelism. Input objects are read
from the repository on demand; newly produced objects are persisted before the
result handle is printed.

## External commands

The host launcher runs `fix-COMMAND` from the nearest repository's
`.fix/commands` directory, then from `PATH`.
Core Fix API commands take precedence. External commands inherit the command
directory selected by `-C`; they run without starting the VM. `FIX` names the
executable for helpers that compose core operations. The CLI embeds the VMM
library and Fix kernel; it requires no separate VMM executable or kernel file.

The bundled helper commands require an initialized repository:

| Command | Input and result |
| --- | --- |
| `parse SCRIPT [BINDINGS]` | Construct a value using `parser`; batch-import file expansions (`-` reads stdin) |
| `run SCRIPT [BINDINGS]` | Parse and evaluate a script; `FIX_JOBS` sets parallelism |
| `json-import DOCUMENT [BINDINGS]` | Construct a value using the JSON deserializer (`-` reads stdin) |
| `compile WASM_VALUE` | Construct an ELF-producing Thunk from a native Fix Wasm module |
| `flatware-link WASM_VALUE` | Attach the WASIp1 adapter, producing a Wasm-producing Thunk |
| `flatware-import PATH [NAME]` | UNIX file, directory, or symlink → Flatware descriptor |
| `flatware-export VALUE PATH` | Flatware descriptor → UNIX filesystem |
| `flatware-application EXECUTABLE [ARGS…]` | Construct an application Thunk, reading host stdin |
| `flatware-run EXECUTABLE [ARGS…]` | Evaluate an invocation and forward streams and status |
| `export LABEL ARCHIVE.tar.gz [LABELS…]` | Package the specified labels and their reachable records and data |
| `import ARCHIVE.tar.gz` | Unpack a trusted repository archive using system tar |

Imports use timestamps zero and link counts one; entry names default to the
input basename. Directory snapshots exclude `.fix`. Extraction requires a
nonexistent destination. These helpers encode Flatware's directory format,
rather than adding filesystem semantics to Fix itself. See [Flatware](../../flatware/README.md).

The native CLI bundles its VMM and kernel. External helpers own their tools;
Bazel supplies a command directory on PATH, without compiler resource variables.
`parse` uses `fix-preprocess` from PATH. The graph helpers use Python 3 and its standard library.
[fix-compile](../cli/commands/fix-compile) constructs translation, compilation, and linking directly as JSON expressions.
`compile` uses the `wasm2c`, `clang`, `path`, and `toolchain` repository labels.
`flatware-link` uses `wasm-merge`, `path`, and `toolchain`; its merger must support
`--no-resource-imports`. Both accept a Wasm Blob or a Thunk producing one.
Bootstrap archives are trusted store contents. They can also be extracted over
an empty repository with `tar -xzf ARCHIVE.tar.gz`.

`create-blob FILE` reads a file; `create-blob -` reads raw stdin. `--string TEXT` constructs UTF-8 bytes;
`--u8 NUMBER`, `--u32 NUMBER`, and `--u64 NUMBER` construct unsigned little-endian
integers with checked bounds. `read-blob` writes raw bytes by default; matching
format flags read UTF-8 text or decimal integers of the specified exact width.

Construction and evaluation are separate:
`parse`, `json-import`, `compile`, `flatware-link`, and `flatware-application` construct values.
`run` and `flatware-run` wrap evaluation. A bare Thunk is forced through
`create-strict-encode` followed by `eval`.

Parser bindings are `(name, value)` pairs. Flatware settings use environment
variables: `FIX_ENV` is a Tree of `NAME=VALUE` Blobs, `FIX_FDS` contains preopens,
and `FIX_ARGV0` overrides the process name. `FIX_JOBS` controls evaluation;
`FIX_RESULT_LABEL` optionally labels the result of `flatware-run`.
