# Fix build rules

Load constructors from `//build:fix.bzl`. They describe a graph using the
corresponding Fix operations. Bazel's `json.encode()` writes the graph;
`fix-json-import` constructs its Fix value using the in-Fix deserializer.
`fix_blob(src = ...)` imports only file contents. Large Blob inputs are imported
together and supplied as symbolic bindings. NDJSON node references share pending
subgraphs, including directory conversion, within a single deserializer call.
`fix_binding(value = NAME)` refers to a value supplied through the deserializer's
external bindings when importing the graph.

```python
fix_blob(name = "metadata", value = "")
fix_binding(name = "program", value = "program")
fix_tree(name = "combination", children = [":metadata", ":program", ":arguments"])
fix_application_thunk(name = "application", src = ":combination")
fix_strict_encode(name = "request", src = ":application")
fix_eval(name = "result", src = ":request", jobs = 8)
fix_read_blob(name = "file", src = ":result", out = "result.bin")
```

Execution rules use the default certified toolchain. Pass `toolchain = ":tools"`
to select another toolchain target. `fix_binding` selects its named values,
including the JSON deserializer and Flatware helpers; these names are lexical
bindings, not repository labels. Importing an ELF with `fix_blob` does not
certify it. Ordinary rule users do not handle serialized handles or repositories.

`fix_select_index(src = ..., index = ...)` and
`fix_select_range(src = ..., begin = ..., end = ...)` construct selection thunks
using an index or a half-open range. `fix_selection_thunk` accepts an argument
Tree for the lower-level constructor.

`fix_build` stores a graph without evaluating it. Use it to cache a shared
subgraph, such as an imported sysroot. `fix_eval` constructs its input graph and
then evaluates it in a separate action; its result can become another graph's
input. `fix_read_blob` reads a Blob value without implicit evaluation.
`jobs = 0` (the default) uses `--//build:fix-jobs=N`, which defaults to
available cores. The same setting supplies `FIX_JOBS` for `bazel run //fix`;
an explicit `fix eval -j N` overrides it. Installed Fix accepts `FIX_JOBS` and
otherwise uses available cores. `memory` is the VM
budget in MiB, defaults to 16384, and must be a power of two. Put independent
strict requests in a `fix_tree` and evaluate that Tree to let Fix schedule them.
Both budgets also accept labels of integer build settings. The `graph` output
group of `fix_eval` constructs its input without running that evaluation:
`bazel build TARGET --output_groups=graph`.

Fix actions share one Bazel resource slot, so Bazel runs them sequentially.
Parallel work belongs in a single evaluated Tree. Evaluation actions reserve
their worker count and VM memory budget for Bazel scheduling. `memory = 0`
budgets 1 GiB per worker, rounded up to a power of two with a 16 GiB minimum.

Fix actions reuse one persistent repository at
`${XDG_CACHE_HOME:-$HOME/.cache}/fix/bazel`. Override its location with
`--//build:fix-cache=/path`. Access is serialized across Bazel processes;
evaluation uses `--memoize`. Declared inputs are imported once, and all graph
references use canonical handles. No repository labels affect the build.

Reset the default cache with `bazel run //build:fix-cache-reset`.
For a custom location, use the same `--//build:fix-cache=/path` setting.
Reset waits for active actions.
The cache is a trusted object store; only trusted users may write it.

Bazel still caches ordinary output artifacts. Internally, stored values carry
their canonical handle and a minimum-repository archive, so an action can use
a Bazel cache hit even when the local Fix cache is empty. Imports publish
complete files through hard links; results already in the shared repository
are reused without copying. This storage layout is internal to the rules.

Load Flatware helpers from `//flatware:build.bzl`:

- `flatware_directory`: constructs pending directory conversion from `src`
  (optionally `subdir`) or file targets mapped to relative paths with `inputs`.
  `guest_path` defaults to `/work`.
- `flatware_files`: constructs a directory from Blob values mapped to filenames.
- `flatware_work`: selects a returned descriptor after checking the program's
  exit status; `fd` defaults to 3.
- `flatware_file`: looks up a relative `path` in a directory and selects its Blob.
- `flatware_application`: constructs a strict application request. Declare
  `executable`, `args`, `env`, and `mounts` (directory values to guest paths).
  `inputs` supplies files under `/work`; alternatively supply a directory value
  as `work`. Seed and initial clock are zero; stdin is empty.
- `flatware_outputs`: extracts `outs`, relative to the selected descriptor,
  from an evaluated result. `fd` defaults to 3. For a batch, `indices` selects
  the result Tree child for each output. Guest failures fail the action with
  their stdout and stderr. Each output is also available as an output group
  named by its path.

The rules require the same local KVM environment as the Fix CLI. Bazel supplies
the CLI and shell helpers; neither depends on Bazel when installed normally.
