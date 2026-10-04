# Wasm postprocessor

```
postprocess input.wasm output.wasm --memories 3 --tables 4
```

Counts are totals, including imported resources and all Table element types.
Omitting a count preserves it. Counts below the existing totals are errors.
Additional resources are empty, growable wasm32 Memories and externref Tables.
Existing resource indices remain unchanged.

Function imports from `__wasm_postprocessor` become defined Wasm functions.
Only imported operations are synthesized. Supported names and signatures are:

| Name | Parameters | Result |
| --- | --- | --- |
| `memory_copy_S_to_D` | destination offset, source offset, byte count | — |
| `memory_size_N` | — | page count |
| `memory_grow_N` | page count | previous page count or -1 |
| `memory_fill_N` | destination offset, byte, byte count | — |
| `table_copy_S_to_D` | destination offset, source offset, element count | — |
| `table_size_N` | — | element count |
| `table_grow_N` | reference, element count | previous element count or -1 |
| `table_fill_N` | destination offset, reference, element count | — |
| `table_get_N` | element offset | reference |
| `table_set_N` | element offset, reference | — |

`S`, `D`, and `N` are decimal Wasm resource indices. Integer parameters and
results are i32; references match the Table's element type. These operations
use 32-bit resource indices and offsets. Copies have Wasm's overlap-safe
semantics. Invalid names, signatures, indices, or resulting modules are errors.

`fix/sdk/include/wasm.h` provides declarations for C. For example,
`WASM_MEMORY(2)` declares `wasm_memory_size_2`, `wasm_memory_grow_2`, and
`wasm_memory_fill_2`; `WASM_MEMORY_COPY(0, 2)` declares
`wasm_memory_copy_0_to_2`. `WASM_TABLE(N)` declares externref Table operations,
and `WASM_TABLE_COPY(S, D)` declares copying. For caller-declared Tables,
prefer Clang's Table builtins.
