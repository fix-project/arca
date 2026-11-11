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

`fix init SEED` creates a repository from a seed archive and adds the expression
parser as a Wasm module.
Each module compiles with the seed's `compile`, which by default splits a
module into one C translation unit per 128 KiB of Wasm so that large modules
compile in parallel. Label `fix-parser` names the parser's compilation. Commands use this
label, and memoization reuses each compilation after its first evaluation.
`--precompile` evaluates them all during `init`. `//fix/cli:precompiled`
archives such a repository.

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
| `@handle("HEX")` | Import an exact canonical handle already admitted by the repository. |
| `@label("NAME")` | Resolve a repository label. |
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
fix -b compiler='@label("clang")' '@script("build.fix" @file("main.c"))'
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
