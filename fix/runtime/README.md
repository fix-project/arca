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
| `schedule [-j N] [--memoize=auto|always|never] [--trusted-tagger CALLABLE]… COMBINATION` | Run a Fix combination with kernel scheduler effects; persist and print its result |
| `visit [--paths] VALUE` | Print reachable stored Objects once each; `--paths` prints NUL-separated data, record, index, and explicitly selected label paths relative to the repository's parent; omit literal Blobs and do not evaluate thunks |

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
| `is-tag VALUE`, `is-comparable VALUE` | Predicate exit status |

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

`eval` and `think` are wrapper commands. Both accept `--scheduler CALLABLE` and
`--collector CALLABLE`, defaulting to `FIX_SCHEDULER`/`FIX_COLLECTOR` or repository
labels `scheduler`/`collector`. These callables need ordinary Runnable admission.
The wrappers construct `[metadata, scheduler, collector, input]`, run `schedule`
with a tagging grant for the collector alone, and print the certified result.
They check the returned coupon’s claim and reject assumptions other than the
configured collector.
The scheduler returns an Eval coupon by default; an eight-byte metadata Blob
containing 4 requests a Think coupon. `--memoize=auto` (the default) keeps
intermediate coupons in memory and persists only the final evaluation coupon. `always` persists intermediate coupons too;
`never` disables automatic persistence and disk cache lookup. In `never` mode the scheduler returns the result directly; generated coupons
are not published and the wrappers skip coupon verification.
`-j 1` uses the same userspace evaluator serially; multiple workers share a work
queue and help execute queued work while waiting for their child evaluations.

`eval --memoize=always` saves application coupons and reuses existing zero-author
application coupons for the same canonical combination. A hit returns the
existing certificate unchanged. Conditional coupons are indexed but not reused;
their assumptions must first be propagated through collector derivations.
Saved coupons retain their inputs and results, increasing repository size.

Stored Tags are indexed at
`.fix/tags/<field-1>/…/<field-8>/entries/<tag-handle>` using up to eight fields
after the authors. Each entry is a relative symlink to its Object record.
Prefix lookup checks the actual stored Tag; the index does not confer authority.
Repositories imported without index entries remain usable, but those Tags will
not be found until saved again.

Calling an ELF requires a Tag `[authors, ELF, "Runnable"]`; every author must
be trusted. Empty-author certificates are unconditional. Trusting an ELF as
an author does not permit calling it without a certificate.

Trusted author handles are read from `$XDG_CONFIG_HOME/fix/trust`, or
`~/.config/fix/trust` when `XDG_CONFIG_HOME` is unset. Each line contains one
canonical handle. `FIX_TRUST` selects another file. A missing default file
trusts no authors; an explicitly selected file must exist. Repository trust
files are never loaded automatically. To use an explicitly trusted bootstrap,
set `FIX_TRUST` to its `.fix/trust` file.

## Commands

The CLI embeds the VMM library and Fix kernel; it requires no separate VMM
executable or kernel file. Besides the plumbing above, it provides:

| Command | Input and result |
| --- | --- |
| `parse SCRIPT [-b NAME=EXPRESSION]...` | Construct a value from a script (`-` reads stdin) |
| `run SCRIPT [-b NAME=EXPRESSION]...` | Parse and evaluate a script |
| `compile EXPRESSION [UNITS]` | Evaluate `compile` applied to a Wasm module, yielding a Runnable or an Error Tag |
| `flatware-import PATH [NAME]` | UNIX file, directory, or symlink → Flatware descriptor |
| `flatware-export EXPRESSION PATH` | Flatware descriptor → new UNIX path |
| `export [--without-backing EXPRESSION]... EXPRESSION ARCHIVE [EXPRESSION...]` | Package values and their reachable records and data |
| `import ARCHIVE` | Unpack a trusted repository archive using system tar |

The [compiler wrapper](../../programs/fix/compile.rs) behind `compile`
authenticates its resources and phase state, checks command exit statuses, and
returns a `[author, ELF, "Runnable"]` Tag or an Error Tag containing the failed
Flatware result. Flatware import and export use the native library described in
[Flatware](../../flatware/README.md).

`create-blob FILE` reads a file; `create-blob -` reads raw stdin. `--string TEXT` constructs UTF-8 bytes;
`--u8 NUMBER`, `--u32 NUMBER`, and `--u64 NUMBER` construct unsigned little-endian
integers with checked bounds. `read-blob` writes raw bytes by default; matching
format flags read UTF-8 text or decimal integers of the specified exact width.

Construction and evaluation are separate: `parse` constructs a value and `run`
evaluates it. A bare Thunk is forced through `create-strict-encode` followed by
`eval`.
