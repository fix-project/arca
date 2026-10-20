# Fix JSON

`fix json-import DOCUMENT [BINDINGS]` constructs a Fix value and prints its handle.
Use `-` for stdin. It does not evaluate the constructed graph; use `fix eval`
when needed. The repository must contain the `json` procedure label, included
in the bootstrap archive.

The document is a Fix expression:

```json
{
  "create-strict-encode": {
    "create-application-thunk": [
      "",
      {"binding": "add"},
      {"u64": 2},
      {"u64": 4}
    ]
  }
}
```

The optional environment supplies `add` as an existing Fix value.

For shared graphs, supply newline-delimited expressions. `{"node": N}` reuses
the expression on line N (zero-based); references must point to earlier lines.
The last expression is the result. Nodes share recipes without evaluating them.

```json
{"u64": 4}
[{"node": 0}, {"node": 0}]
```

Strings create UTF-8 Blobs; arrays create Trees. Other expressions are objects
with exactly one key:

| Key | Argument |
| --- | --- |
| `hex` | Hexadecimal Blob bytes |
| `u8`, `u16`, `u32`, `u64` | Unsigned integer encoded little-endian at that width |
| `binding` | Name from the environment |
| `node` | Earlier expression index |
| `create-ref` | Object expression |
| `create-identification-thunk` | Object or Ref expression |
| `create-application-thunk` | Tree Object or Ref expression |
| `create-selection-thunk` | Selection argument Tree Object or Ref expression |
| `create-strict-encode`, `create-shallow-encode` | Thunk expression |

The environment is a Tree of `(name Blob, value)` pairs, as for the human
parser. Names must be unique. Its values are evaluated as application arguments;
use node references to share pending Encodes without forcing them. Serialized
handle bytes cannot create handles or authorize access to objects.

Bare numbers, booleans, and null are rejected. Integers must fit the specified
width. Emit large integers exactly, without passing them through floating-point
numbers. Normal JSON string escaping, including surrogate pairs, is supported.
Serde's default JSON nesting limit applies.

The standalone deserializer takes `(metadata, procedure, JSON Blob[, environment Tree])`
and returns the constructed value. Omitting the environment supplies no external
bindings. JSON decoding uses `serde_json` with `alloc`
for bare Wasm. The human-readable parser remains a separate procedure.

During emission, a private externref Table retains each constructed node, so
shared subgraphs are constructed once. This does not evaluate their Thunks.
