# Fix programs

`wat`, `c`, and `rust` each provide the same standalone programs:

| Program | Arguments | Result |
| --- | --- | --- |
| `blob` | None | `"hello, world"` |
| `add` | Two little-endian u64 Blobs | Their sum |
| `path` | Canonical directory descriptor, Tree of path components | File descriptor, or an empty Tree if absent |

Run any language directly:

```sh
bazel run //programs/fix/wat:blob
bazel run //programs/fix/c:add -- 2u64 3u64
bazel run //programs/fix/rust:add -- 2u64 3u64
```

Arguments are Fix expressions. `_elf` targets build procedures for use as
arguments to other programs:

```sh
bazel build //programs/fix/rust:add_elf
bazel run //programs/fix:fib -- 8u64 '@"bazel-bin/programs/fix/rust/add.elf"'
```

Integration tests live alongside each language's programs. `identity`, `addblob`,
`parser`, `fib`, `map`, `mapreduce`, `count_words`, `bptree_get`, and
`bptree_get_n` are available directly under `//programs/fix`.
