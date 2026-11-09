# Fix build rules

`fix_run` from `//build:fix.bzl` evaluates one Fix expression in a fresh
repository. It imports `archives` (repository archives such as `//fix/cli:precompiled`),
copies `data` files into the working directory under their given names, and
evaluates `expression` with named `bindings`, as `fix -b NAME=VALUE... eval
EXPRESSION` would. Its outputs are the result handle and an archive of the whole
repository without labels, including every coupon the evaluation persisted.
Importing that archive lets later evaluations reuse those results.

```python
fix_run(
    name = "compilations",
    expression = '@script("compile.fix")',
    bindings = {"compile": '@label("compile")'},
    data = {"compile.fix": "compile.fix", ":module": "module.wasm"},
    archives = ["//fix/cli:precompiled"],
)
```

`fix_repository` archives a whole repository made by `fix init SEED
--precompile`; `seed` defaults to `//bootstrap:seed`.

Both rules evaluate with the CLI named by `fix`, `//fix/cli:fix` by default;
`//bootstrap:compilations` uses a release build of it.

`--//build:fix-jobs=N` sets the default parallelism (0 uses all cores). Memory
defaults to 1 GiB per job, at least 16 GiB, rounded up to a power of two.
