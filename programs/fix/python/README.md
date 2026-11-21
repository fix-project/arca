# Fix Python

The `//programs/fix/python` procedure takes a UTF-8 script Blob followed by Fix
arguments. Run it with `bazel run //programs/fix/python -- '@file("script.py")' '"hello"'`.

```python
import fix

(message,) = fix.argv
fix.exit(bytes(message).upper())
```

`fix.argv` is a tuple of Fix arguments in application order, including curried
arguments. It starts after the script Blob. `fix.arity(n)` exits with a partial
application when fewer than `n` arguments are available. Otherwise it returns
the full argument tuple.
`fix.combination()` gives access to the invocation Tree.

`fix.exit(value)` ends the script and returns a Fix result. Calling it without
an argument or reaching the end of the script returns an empty Blob. Each
invocation starts a Python VM. An exception that escapes the script returns
a `Fix.error` Tree with its traceback.

`fix.exit` and unannotated function returns use the same conversions as
`fix.call` arguments.
Python bytes, strings, integers, and booleans become Blobs. Lists, tuples,
and dictionaries become Trees, with the same conversions applied recursively
to their contents. Existing Fix values pass through unchanged.

Use `Blob(data)` with bytes, a string, an integer, or a boolean. Strings are
encoded as UTF-8. Integers use signed little-endian encoding with an 8-byte default.
Set `width=1`, `width=2`, `width=4`, or `width=8` to choose the length.
An integer outside the selected width's range raises `OverflowError`.
`bytes(blob)` reads a Blob's contents. `str(blob)` decodes UTF-8 and
raises `UnicodeDecodeError` on invalid input. `int(blob)` reads a signed
little-endian integer from a 1-, 2-, 4-, or 8-byte Blob and raises `ValueError`
for other lengths.
`Blob(True)` and `Blob(False)` default to one-byte `1` and `0`.
`bool(blob)` accepts zero and nonzero integer Blobs and the UTF-8 strings
`"true"`, `"True"`, `"false"`, and `"False"`. Other inputs raise `ValueError`.

Lifted functions convert arguments annotated with `bytes`, `str`, `int`, or
`bool` from Blobs. Return values with those
annotations become Blobs. Fix resolves Refs and Thunks for typed arguments in
a new application before entering the function. Annotate an argument as
`fix.Reference` to request shallow evaluation or `fix.Thunk` to receive the
unevaluated Thunk.

List and tuple annotations convert Fix Trees. Dictionary annotations use a
Tree of two-element key-value Trees. Type parameters such as `list[int]`,
`tuple[str, int]`, and `dict[str, list[int]]` convert members recursively.
Bare containers keep members as Fix values, with Blob keys decoded to
`bytes` for bare dictionaries. Use `fix.Tree(data)` to encode Python lists,
tuples, and dictionaries, and `list(tree)`, `tuple(tree)`, or `dict(tree)`
to read them.

Trees support direct indexing, slicing, and iteration. Indexing reads a child
without evaluating it, and slicing returns a Tree. Indexing a Blob returns
an integer byte, and slicing returns Python bytes.
Use `ref.select(index)` or `ref.select(start, stop)` on a Tree or Blob Ref
to build a selection Thunk. The two-argument form selects a half-open range.
On a Thunk, `thunk[index]` and `thunk.select(index)` both build a selection
Thunk targeting its result. Use a nonnegative integer index. A shallow encode
resolves the target to a Ref, and Fix checks the selection at evaluation time.
`thunk.select(start, stop)` and `thunk[start:stop]` select a half-open range.
Thunk slices require a stop, nonnegative bounds, and a step of one.
`fix.call(program, *args)` builds an application Thunk. It accepts Fix values
and converts Python bytes, strings, integers, booleans, lists, tuples, and
dictionaries to Blobs or Trees. Use
`thunk.strict().eval()` to evaluate it, or return an Encode for the caller
to evaluate. Calling `eval()` captures the interpreter's continuation.

Use `fix.advise("fix::title", "uppercase")` to title the current application in
the progress log. Advice keys are strings. Payloads can be bytes, Blobs,
strings, integers, or booleans and use the same encoding as `Blob(data)`.
`fix.effect(tree)` submits an effect and can capture a continuation when the
runtime defers it.

`fix.callable()` returns the script's Fix callable with no arguments bound.

```python
import fix

(count,) = fix.arity(1)
remaining = int(count)
if remaining <= 0:
    fix.exit("done")
fix.exit(fix.call(fix.callable(), remaining - 1).strict())
```

```python
import fix

@fix.function
def uppercase(message: bytes) -> bytes:
    return message.upper()

if __name__ == "__main__":
    fix.exit(uppercase)
```

`@fix.function` binds the declared name to a Fix callable Tree. It stores the
script and the top-level function name. Apply the Tree with `fix.call`.
A lifted invocation reloads the script under the module name `_fix_script`
before calling the function. Keep definitions at module scope and put entry
code under `if __name__ == "__main__"` so it only runs for a script invocation.
To pass the script arguments directly to the lifted function, use
`fix.exit(fix.call(uppercase, *fix.argv).strict())` in the entry block.

`flatware.run` builds an application Thunk. `flatware.call` is an alias.
Pass `stdin` and `preopens` as keyword arguments. A directory's contents are a
Fix Tree of child entries, the same format as a directory imported with
`@file`. `flatware.dirent(name, contents)` adds a name and metadata.
`flatware.file(name, contents)` creates a regular file entry. When the
contents are an Encode, pass `filetype=3` for a directory or `filetype=4`
for a regular file to `dirent`.

```python
command = flatware.run(program, argv, stdin=b"hello")
result = flatware.Result(command.strict().eval())
```

`flatware.Result` reads `exit_status`, `stdout`, `stderr`, and `descriptors`
from the evaluated Tree. Return `command.strict()` to leave evaluation to
the caller. The integration test uses a 2 GiB Fix invocation limit when
evaluating inside Python.

RustPython provides Python built-ins, native modules such as `itertools`, and
the bootstrap modules needed for imports. The freestanding Wasm module uses
Rust's standard library internally and imports the Fix ABI.

## Workflows

```python
@fix.do
async def calculate(add, x, y):
    total = await fix.call(add, x, y)
    left, right = await fix.parallel(fix.call(add, total, x), fix.call(add, total, y))
    return fix.Tree([left, right])
```

Calling a workflow returns a Thunk. The decorator compiles each `await` into
a Fix dependency and a lifted stage. A stage receives the values it needs
in a fresh Python invocation. Independent operands in `fix.parallel` are
available to the Fix scheduler together, and their results keep input order.

Workflows use top-level `async def` functions with required positional
parameters and a final Fix value return. Use `if`, `for`, and `while` within
a stage, including early returns and loop `break` and `continue`. Put each
`await` at the top level of the function body, in its own statement or on the
right-hand side of an assignment. Local values carried across an await must
be Fix values.

```python
@flatware.do
async def build(unix, copy, cat):
    first = await unix.run(copy, ["copy", "/work/input", "/work/prepared"])
    assert first.exit_status == 0
    left, right = await unix.parallel(
        unix.run(copy, ["copy", "/work/prepared", "/work/left"]),
        unix.run(copy, ["copy", "/work/prepared", "/work/right"]),
    )
    assert left.exit_status == right.exit_status == 0
    output = await unix.run(cat, ["cat", "/work/left", "/work/right"])
    return output.stdout

work = fix.Tree([flatware.file("input", b"hello")])
environment = flatware.Environment(env={"LANG": "C"})
environment.mount("/work", work)
command = build(environment, copy_program, cat_program)
```

The result is a Tree containing the returned value and the updated environment.
Use `flatware.Environment.from_value(result[1])` to start another workflow with
that environment. Commands share environment variables and preopens, with
separate standard streams and exit status for each command. Use
`command.strict()` to pass the workflow to the caller for evaluation.

Use `unix.getenv(name, default=None)`, `unix.setenv(name, value)`, and
`unix.unsetenv(name)` to work with UTF-8 environment variables. Edits update the
current environment for subsequent commands and appear in the returned state.

```python
unix.setenv("LANG", "C")
unix.mount("/work", fix.Tree())
unix.mkdir("/work/output")
unix.write_file("/work/output/message", b"hello")
message = unix.read_file("/work/output/message")
names = unix.listdir("/work/output")
unix.remove("/work/output/message")
unix.remove("/work/output")
work = unix.unmount("/work")
```

Paths are absolute within the Flatware environment. Lookup normalizes
`.` and `..` lexically, uses the most specific preopen, and traverses
materialized native directories by name.
`read_file` returns the Fix value for the file contents. `listdir` returns
byte names, including child mounts, and defaults to the root directory.
`mount` accepts directory contents as a Tree, a Tree reference, or an Encode.
`remove` accepts files and empty native directories. File writes and changes
to directory membership use the environment's timestamp.

Parallel commands start from the same filesystem. The merge combines disjoint
edits and identical changes. It recurses into materialized native directories
and treats files and opaque descriptors as whole entries. Merged directories
recompute their entry counts and take the latest modification and change times
from the edited directories. An unresolved conflict ends the workflow with
`[Blob(b"Flatware.merge-conflict"), path, environment]`. The path is a Tree of
Blob components, and the environment is the state before the parallel group.
To resolve conflicts, use `@flatware.do(resolve=my_resolver)`. The resolver
receives `(path, base, changes)`, where the path is a tuple of byte strings,
the base is the original descriptor or `None`, and distinct changes are in
branch order. Return a matching descriptor or `None` to remove the entry.

## Schedulers

```python
import fix
from fix.scheduler import Scheduler, run


class Policy(Scheduler):
    def __init__(self):
        self.applications = 0

    async def apply(self, combination):
        result = await super().apply(combination)
        self.applications += 1
        return result


fix.exit(run(Policy()))
```

Override any of the async steps `eval`, `think`, `force`, `apply`, `select`, or
`load`. The base methods run the Rust evaluator. Recursive dependencies call
back into the policy's overrides. Use `await self.eval(value)` for a dependency
and `await super().eval(value)` to delegate the current step to Rust.
`on_advice(target, key, payload)` receives each advisory event synchronously as
Fix values.

Store the script as a Blob and label the partial application of the
interpreter to that Blob. With the interpreter already labeled `python`,
the commands below install the script and select it for local evaluation.

```sh
memory=$(fix create-blob --u64 1073741824)
script=$(fix create-blob scheduler.py)
scheduler=$(fix create-tree "$memory" python "$script")
fix label set python-policy "$scheduler"
fix --scheduler python-policy --memoize never eval '"hello"'
```

Python policies return Fix values and use `--memoize never`. The Rust
evaluator requires a trusted tagger when the scheduler reconstructs a Tag.
