### Commands

| Name | Arguments | Return value | Description |
|---|---|---|---|
| `create_blob` | `Int(num), String("text"), Path("path")` | `handle` | creates blob in the runtime from the given primitive values |
| `create_tree` | `handle, ...` | `handle` | creates a tree from one or more runtime handles |
| `get_blob` | `handle` | `blobData` | fetches blob payload from runtime |
| `get_tree` | `handle` | `treeData` | fetches tree payload from runtime |
| `apply` | `handle` | `handle` | applies the first member to all subsequent members as operands

Inline comments using `//`

### Usage

fix-cli -- <mock|hybrid> -- <commands...>
fix-cli -- <mock|hybrid> --path <file>

### Examples

- cargo run -p evaluator --bin fix-cli hybrid --path fix/evaluator/add_script.txt
- cargo run -p evaluator --bin fix-cli mock -- 'print(get_blob(apply(create_tree(create_blob(String("+")), create_blob(Int(1)), apply(create_tree(create_blob(Path("fix/evaluator/add.txt")), create_blob(Int(2)), create_blob(Int(3))))))))'

### Runtimes

- MemoryRuntime: Stores all blobs and trees in memory. Holds ownership so `get_blob()` and `get_tree()` only return references
- StorageRuntime: Writes all blobs and trees to ".fix/objects/". Holds no ownership so `get_blob()` and `get_tree()` return boxed bytes read from ".fix/objects/"
- HybridRuntime: Uses MemoryRuntime but supports "flush" and "flush_handle" operations to recursively write in-memory blobs and trees to ".fix/objects/"
- MockRuntime: Uses memory runtime and does naive addition for applications in `apply()`