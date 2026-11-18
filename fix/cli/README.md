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
The REPL reuses one VM, starting a new scheduler for each request.

Parsing constructs a Fix value. An outer encode requests evaluation; thunks and
Trees are otherwise retained as constructed. For example, `#(...)` constructs an
application thunk and `*#(...)` evaluates it. `fix eval --encode '#(...)'`
wraps the input Thunk in a strict Encode before evaluation. This also works for
identification and selection Thunks; a non-Thunk input reports a type error.

With Runnable programs labelled `fib` and `add`, recursive Fibonacci evaluation is:

```sh
bazel run //fix -- eval --encode '#("" @label("fib") 8u64 @label("add"))'
```

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

Named CLI arguments use `-b name=EXPRESSION`, for example:

```sh
fix -b answer='42u64' '$answer'
```

Bindings and the other global options (`-C`, `-l`, `-j`, `--memory`,
`--memoize`, `--scheduler`, `--collector`, `--profile`) may appear before or
after the command. Label names, archive paths, and URIs remain host operands.

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

Progress is written to stderr; stdout remains available for Fix values and
protocol data. Terminals display indicatif task rows. Redirected stderr gets
activity messages and elapsed-time updates while operations are running.

Transfers count the payload bytes actually read or written. Signing counts
Tags after all configured identities have signed each one. Archive operations
name their current activity, such as extraction or indexing, without inventing
an overall percentage. Evaluation displays task titles and completed/total
units from advice, plus the scheduler's actual completed Tree-child counts.
When totals or titles are unavailable, Fix shows an elapsed timer and a label
or canonical name. Work without a measured total has an activity line, not a
completion bar. Completing reported application units does not claim that the
invocation has returned.

Remote labels resolve without retrieving data. The scheduler reports loads
and evaluations with their locations. All status messages,
logs, and forwarded SSH stderr share the progress renderer. Diagnostic writes
are collected into complete lines; result lines suspend the renderer while
printing. Ephemeral rows are erased on completion or failure; completed named
steps and the final report remain.

The expression parser returns a one-element Tree containing its parsed value.
Panic and trap responses are reported as parser errors, with the source and
parser name. The envelope lets expressions contain literal error-shaped values
without treating those values as parser failures. Runtime failures include the
operation, input name, and runtime diagnostic alongside their exit status.

Evaluation keeps completed named computations as persistent steps, with their
operation, location, outcome, and elapsed duration. Labels and application task
titles name steps; an application without either uses its canonical Fix name.
Live application advice and multi-part Tree evaluation use progress displays
while running. Counts describe completed units rather than an estimate of time
remaining. Cancelled discovery attempts do not appear as completed steps.

Every configured remote appears during evaluation. A successful handshake marks
it available even when no evaluation is delegated to it. Configured but
uncontacted remotes and failed connections are distinguished from available
nodes with no evaluation work sent. File remotes are identified as storage.
Connected nodes report CPU and VM memory load on the existing connection,
including when unused. Unknown measurements remain unspecified.

The final per-node table reports applications, collector calls, CPU time, peak
worker count / available workers, peak queue length (`Qmax`), peak allocated VM
memory / VM capacity, and bytes sent and received. Units appear once in each column header; every row
uses that column's unit. CPU time is measured across the worker process's
threads, including its VM and host I/O work; live CPU load is its change between
samples, normalized by the worker count. Memory is the VM allocator's allocation,
not process RSS; its peak is the allocator's high-water mark. Transport counters
include protocol and status traffic. Counters accumulate from the first runtime
sample; memory peaks cover the VM lifetime. A long-running server retains its
runtime peaks across requests. Transfer byte bars remain suppressed during
evaluation.

`fix pull [REMOTE [SOURCE [DESTINATION]]]` and
`fix push REMOTE [SOURCE [DESTINATION]]` realize labels using the selected
scheduler. Both default to `--shallow` (a shallow Encode yielding a Ref).
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
fetching unavailable data.
