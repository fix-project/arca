### Commands

| Name | Arguments | Return value | Description |
|---|---|---|---|
| `create_blob` | `Int(num), String("text"), Path("path")` | `handle` | creates blob in the runtime from the given primitive values |
| `create_tree` | `handle, ...` | `handle` | creates a tree from one or more runtime handles |
| `get_blob` | `handle` | `blobData` | fetches blob payload from runtime |
| `get_tree` | `handle` | `treeData` | fetches tree payload from runtime |
| `apply` | `handle` | `handle` | applies the handle as the combination tree |
| `eval` | `handle` | `handle` | evals the handle |
| `trade` | `(trade_type: String, coupons: handle, lhs: handle, rhs: handle)` | `handle` | makes a coupon trade |
| `print` | `blobData` or `treeData` | () | prints the data |
| `show_coupon` | `handle` (must be a coupon) | () | shows the content of a coupon |
| `coupon_lhs` | `handle` (must be a coupon) | `handle` | returns the left hand side of a coupon |
| `coupon_rhs` | `handle` (must be a coupon) | `handle` | returns the right hand side of a coupon |
| `handle` | `"text"` where `text` is the first 8 digits of the hex-encoded handle name | `handle` | loads a canonical handle from `.fix` |
| `tag` | `"text"` where `text` is the first 8 digits of the hex-encoded tag name | `handle` | loads a canonical tag from `.fix` |


Inline comments using `//`

### Usage

fix-cli -- <mock|hybrid> -- <commands...>
fix-cli -- <mock|hybrid> --path <file>

### Examples

- `cd fix; cargo run -p fix --target=x86_64-unknown-none -- -- --path evaluator/eval_script.txt`
- `cd fix; cargo run -p fix --target=x86_64-unknown-none -- -- --path evaluator/apply_script.txt`
- `cd fix; cargo run -p fix --target=x86_64-unknown-none -- -- --path evaluator/trade_script.txt`
- `cargo run -p evaluator --bin fix-cli hybrid ../target/x86_64-unknown-none/debug/fix -- \'eval(create_blob(Int(2)))\'`
- `cargo run -p evaluator --bin fix-cli hybrid ../target/x86_64-unknown-none/debug/fix -- \'a = create_blob(Int(2)); trade("EvalBlobObj", create_tree(), a, a)\'`
- `cargo run -p evaluator --bin fix-cli hybrid ../target/x86_64-unknown-none/debug/fix -- \'apply(create_tree(create_blob(Path("target/x86_64-unknown-none/addblob", create_blob(Int(2)), create_blob(Int(3))))))\'`
- `cargo run -p evaluator --bin fix-cli mock -- 'print(get_blob(coupon_rhs(apply(create_tree(create_blob(String("+")), create_blob(Int(1)), apply(create_tree(create_blob(Path("fix/evaluator/add.txt")), create_blob(Int(2)), create_blob(Int(3)))))))))'`

### Runtimes

- MemoryRuntime: Stores all blobs and trees in memory. Holds ownership so `create_blob()` and `create_tree()` only return references
- StorageRuntime: Writes all blobs and trees to ".fix/objects/". Holds no ownership so `create_blob()` and `create_tree()` return boxed bytes read from ".fix/objects/"
- HybridRuntime: Runs `eval`, `trade` and `apply` in an instance of Fix-over-Arca. Uses MemoryRuntime but supports "flush" and "flush_handle" operations to recursively write in-memory blobs and trees to ".fix/objects/"
- MockRuntime: Uses memory runtime and does naive addition for applications in `apply()`
