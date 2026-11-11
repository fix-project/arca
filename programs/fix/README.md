# Fix programs

`wat`, `c`, and `rust` each provide the same standalone programs:

| Program | Arguments | Result |
| --- | --- | --- |
| `blob` | None | `"hello, world"` |
| `add` | Two little-endian u64 Blobs | Their sum |

Run any language directly:

```sh
bazel run //programs/fix/wat:blob
bazel run //programs/fix/c:add -- 2u64 3u64
bazel run //programs/fix/rust:add -- 2u64 3u64
```

Each launcher uses a temporary seeded repository and compiles its Wasm through
the Fix compiler. It prints the canonical result handle and removes the temporary
repository on exit. Relative file arguments resolve from the calling directory.

Arguments are Fix expressions. Fibonacci bundles its addition dependency:

```sh
bazel run //programs/fix:fib -- 8u64
```

This recursively computes Fibonacci and returns the handle for `34`.

`_elf` targets build native ELF artifacts.

`identity`, `addblob`, `fib`, `map`, `mapreduce`, `count_words`, `bptree_get`,
and `bptree_get_n` are available directly under `//programs/fix`.
