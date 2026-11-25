# Fix CLI

The CLI uses the shared `fix` SDK (`fix/api`). Its `fixcli::Shell` implementation
provides one shell per Linux thread. Open it once with `Shell::open`; value
operations use it implicitly. Expressions use the SDK's typed values;
repository and process operations belong to the Linux implementation.

`fix` is one binary; `fix --help` lists its commands. Value operands of the
expression commands (a bare expression, `eval`, `think`, `compile`, `todo`,
`parse`, `run`, `export`, and `flatware-export`) are Fix expressions. Plumbing
commands (`create-*`, `read-*`, `resolve`, `label`, `visit`, `call`,
`schedule`, ...) take handles or label names, so they never parse:

```sh
fix '42u64'
fix read-blob --u64 "$(fix '42u64')"
fix -l doubled '@script("double.fix" 21u64)'
fix compile '@file("program.wasm")'
```

`fix label delete NAME` (also `fix label remove NAME`) deletes a local label
without removing its value or backing data. A missing label is an error.

`fix bootstrap SEED` creates a store at `$XDG_DATA_HOME/fix/bootstrap/.fix`
(default `~/.local/share/fix/bootstrap/.fix`), imports and signs
the seed's compiler, scheduler, collector, and parser, and trusts the compiler
and collector. `fix init` creates an empty `.fix` repository with a `bootstrap`
file remote pointing to that store. Run `fix pull bootstrap` to fetch its label
refs, then `fix remote labels bootstrap` to list them or
`fix pull bootstrap compile compile` to create a local label. It needs no seed
archive; backing data for a selected program is copied from the bootstrap store
or an available file remote when the program is persisted locally. The seed's `compile` splits each
Wasm module into one C translation unit per 128 KiB by default.
A home repository is optional and works like any other repository.

`fix config KEY`, `fix config KEY NAME`, `fix config --unset KEY`, and
`fix config --list` manage scheduler, collector, parser, and compiler selections.
Reads combine user and repository settings; writes default to the current
repository. Use `--global` for user settings or `--local` to read only this
repository's settings. `fix bootstrap SEED` records the
canonical handles of the selected bootstrap programs. Bare names resolve in the active
repository. Command-line
options override settings for one invocation. `fix trust` manages trust and
signing identities in `$XDG_CONFIG_HOME/fix` (default `~/.config/fix`).
`FIX_CONFIG_DIR` overrides that directory; `XDG_DATA_HOME` selects the
bootstrap store's parent data directory. A home repository can be added as an
ordinary file remote. Run `fix pull` for all configured remotes, or `fix pull REMOTE` to
select one, then
`fix remote labels REMOTE` to list them. File remotes supply backing data for
canonical handles when those values are saved in a project.
`//bootstrap:seed` is the seed archive for build actions.
Use `--unsigned` only for staging; normal bootstrap requires configured signing
identities.

`fix repl` reads low-level Fix expressions and prints their canonical handles.
Each input uses the same syntax and imports as a command-line expression.
Use `:{` and `:}` on separate lines for multiline input, and `:quit` or EOF to exit.
Bindings are local to each input; `-b NAME=EXPRESSION` supplies bindings to all inputs.
The REPL uses one repository session, scheduler, and collector across requests.

Parsing constructs a Fix value. An outer encode requests evaluation; thunks and
Trees are otherwise retained as constructed. For example, `#(...)` constructs an
application thunk and `*#(...)` evaluates it. `fix eval --encode '#(...)'`
wraps the input Thunk in a strict Encode before evaluation. This also works for
identification and selection Thunks; a non-Thunk input reports a type error.

With Runnable programs labelled `fib` and `add`, recursive Fibonacci evaluation is:

```sh
bazel run //fix -- eval --encode '#(536870912u64 @label("fib") 8u64 @label("add"))'
```

Handle operands, including `@handle`, accept an unambiguous hexadecimal prefix
with an optional two-digit type byte: `<prefix>[.<type>]`. For example,
`fix info ab12cd34.90` inspects the Ref named by that prefix. Exact labels take
precedence over prefixes. Omitting the type requires a unique full name;
Object and Ref forms of the same data are distinct matches. Abbreviations use
known object and shadow-label names without loading backing data. Expression
imports also search the bootstrap store and configured file sources.

The preprocessor runs on Linux. The parser runs in Fix. Imports are explicit:

| Form | Meaning |
| --- | --- |
| `@handle("HEX")` | Import a canonical name; Objects require locally admitted backing. |
| `@label("NAME")` | Resolve a repository label, including remote shadow labels such as `origin:program`, without retrieving data. |
| `@file("PATH")` | Import a file's contents as a Blob, or a directory's contents as a Tree of Flatware child descriptors. |
| `@script("PATH" EXPR ... name=EXPR ...)` | Instantiate a script. |
| `@todo("REF" "URI" ...)` | Declare a pinned Ref and its acquisition hints. |
| `@todo("URI")` | Fetch an input and report its URI and canonical Ref name to stderr. |

Host paths are relative to the file containing them. This includes imports in
nested scripts and expressions passed to an imported script. CLI expressions
use the current directory. Git directory imports use normalized Flatware trees;
HTTP(S) imports produce Blobs. Git URIs use
`git://HOST/REPOSITORY#REVISION[:PATH]`.

Scripts use `name = expression` for private local bindings and end with a result
expression. Positional arguments `$0`, `$1`, ... belong to that script invocation
and must be supplied explicitly. The highest referenced position determines its
arity. Named arguments such as `$compiler` inherit through imports, with local
overrides:

```fix
flags = ("-O2" "-g")
@script("build.fix" @file("main.c") compiler=$compiler flags=flags)
```

Argument expressions use the caller's scope. A child inherits named arguments,
but never its caller's positional arguments or private local variables.
Recursive imports and missing or excess positional arguments are errors.

`fix run FILE` parses and evaluates a Fix script by default. If the first line
starts with `#!`, Fix evaluates the expression on that line. In a shebang,
`$0` is a Blob containing the entire file, including the shebang. Use `*#(...)`
to apply an interpreter and choose its argument order explicitly.

```fix
#! *#($limit @label("python") $0 "fixed argument" $1)
```

Supply positional arguments as Fix expressions after the file name. A Fix
script receives them starting at `$0`. A shebang receives them starting at
`$1`. CLI bindings are available in both forms. Imports in a script or shebang
use the file's directory, and imports in CLI arguments use the current directory.
`fix run -` reads stdin and uses the current directory for relative imports.

`$limit` is the application budget in bytes supplied by `fix run --limit`.
It defaults to 512 MiB. The global `--memory` option sets the VM budget in MiB.
If a script returns a conventional Fix error Tree or an unhandled
`Fix.error` effect, `fix run` prints the diagnostic to stderr and prints the
result handle to stdout as usual.
For other results, stderr can show a preview of inline UTF-8 text, labels,
and small Trees. A 1-, 2-, 4-, or 8-byte Blob is also shown as a little-endian signed
decimal and unsigned hexadecimal integer, even when it decodes as text.
Previews use Fix sigils for Refs, Thunks, and Encodes.

Named CLI arguments use `-b name=EXPRESSION`, for example:

```sh
fix -b answer='42u64' '$answer'
```

Bindings and the other global options (`-C`, `-l`, `-j`, `--memory`,
`--memoize`, `--scheduler`, `--collector`, `--profile`) may appear before or
after the command. Label names, archive paths, and URIs remain host operands.

Set CPU and memory defaults for your account on this machine with
`fix config --global jobs 8` and `fix config --global memory 16384`.
Memory is in MiB; `jobs auto` uses all available CPUs and `jobs 0` disables
local applications. Repository settings override account defaults, and `-j`
and `--memory` override both. SSH workers read the remote machine account's
settings independently.

The library accesses storage directly. Evaluation publishes its input to the
repository and runs the embedded Fix kernel through Arca's VMM library. Each
invocation owns its VM and returns its result through the repository.

`--memoize=auto|always|never` selects coupon persistence. The default, `auto`,
persists the final evaluation coupon and coupons selected by the scheduler's
canonicalization policy. The supplied scheduler honors `fix::canonicalize` advice
for available targets without fetching data solely to satisfy a hint. `always`
persists all intermediate coupons. `never` disables automatic persistence and
disk-cache lookup. In `never` mode the scheduler returns the result directly;
generated coupons are not published and `eval` and `think` skip coupon verification.

## Serving remote evaluations

Start a TCP server for the current repository with:

```sh
fix -C /path/to/repository serve --listen 127.0.0.1:9000 -j 4
```

For SSH remotes, `fix serve --stdio` starts automatically on the remote machine.
Each initiating session generates an SSH creation ID. Repeating a creation request
with that ID joins the running repository session. A different creation ID cannot
start another runtime in the same repository. `fix --join serve --stdio` explicitly
joins the existing session without comparing creation IDs. TCP connections always
join the server at their endpoint; SSH creation IDs do not constrain TCP joins.

A session owns one runtime, scheduler, and collector. Its creator selects CPU,
memory, scheduler, collector, and memoization settings; joins use those settings. A private
account socket and repository lock prevent duplicate runtimes. The session stops
when its last client and network connection close.

The remote URL supplies the repository path. See the
[two-machine setup](../remote/README.md#ssh-clone-and-distributed-evaluation)
for cloning, trust, and the distributed scheduler.

The server uses the usual scheduler, collector, memory, remote, and signing
options, together with the repository's remotes and the user's trust settings.
It prints its bound address on stderr; port zero selects an available port.
All connections share one VM. The scheduler handles connections and evaluations;
I/O waits run on kernel threads without occupying evaluation workers. `--jobs`
and `--memory` configure the whole server. Transfers use the negotiated Fix protocol and
signed coupon certificates described in [remote runtimes](../remote/README.md).

## Progress

Progress is a line log on stderr. Named operations log their start and end.
End lines include elapsed time. Byte transfers with measured payloads also
report the byte count and average rate. Use `--summarize` for per-worker
statistics. Stdout carries Fix values and protocol data.

The expression parser returns a one-element Tree containing its parsed value.
Panic and trap responses are reported as parser errors, with the source and
parser name. The envelope lets expressions contain literal error-shaped values
without treating those values as parser failures. Runtime failures include the
operation, input name, and runtime diagnostic alongside their exit status.

`-j0` disables local application and collector workers while retaining a local
scheduler thread. Hint, OnePass, and Distributed can delegate evaluation to a
configured execution remote. Parsing also uses the selected scheduler in this
mode. A local-only policy reports an error if it needs to run an application.

Labels and application titles name evaluation events. A new application title
starts a new named phase. Load events show their destination and source, for
example `START local: Load origin:program (from origin)`. Local repository
reads and file remotes are attributed to the node reading them. Connections
report the remote name and its worker count or failure reason.

`fix pull` evaluates the selected labels together as one Tree, allowing the
scheduler to share and run dependencies concurrently. Label snapshots and
backing data use proposal/accept exchanges, so stored snapshots and existing
data need not be retransmitted.

With `--summarize`, the final per-node table reports applications, collector calls, CPU time, peak
worker count / available workers, peak queue length (`Qmax`), peak allocated VM
memory / VM capacity, and bytes sent and received. Units appear once in each column header; every row
uses that column's unit. CPU time is measured across the worker process's
threads, including its VM and host I/O work. Memory is the VM allocator's allocation,
not process RSS; its peak is the allocator's high-water mark. Transport counters
include protocol and status traffic. Counters accumulate from the first runtime
sample; memory peaks cover the VM lifetime. A long-running server retains its
runtime peaks across requests.

`fix pull [REMOTE [SOURCE [DESTINATION]]]` and
`fix push REMOTE [SOURCE [DESTINATION]]` realize labels using the selected
scheduler. Both default to `--shallow`: existing Refs are preserved, Objects
become Refs, and Thunks use a shallow Encode. A shallow pull of an Object or
Ref does not fetch its backing.
`--strict` yields an Object; `--exact` preserves the original value and loads
its backing without evaluating it. Configure defaults with `pull.mode` and
`push.mode`, locally or with `fix config --global`. See
[remote labels and transfer modes](../remote/README.md#remote-refs).

`fix info VALUE` analyzes a canonical name or label without evaluating it or
retrieving remote data. It reports representation, declared type and size,
comparability, local backing, matching labels, acquisition metadata, Tag
certificates and fields, Tags that directly mention the value, and recognized
coupon claims about it. Coupons identify their
operation, input, and result; Runnable certificates are distinguished from other
Tags. Object and Ref forms of a subject match. Inspection reports signature
status and verifies inspected Tag data without granting execution authority or
fetching unavailable data. Handle references use friendly display names by default;
the top-level `Name:` line always contains the complete canonical handle. The
global `--no-abbrev` flag selects full handles in info reports and progress output.
