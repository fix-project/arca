# Userspace schedulers

All scheduler programs use the `fixscheduler` SDK. Each policy implements `executor::Scheduler`, overriding the evaluation steps it needs.

| Program | Behavior |
|---|---|
| `serial` | Resolves dependencies locally with one active worker. |
| `parallel` | Resolves dependencies locally using all configured workers. |
| `distributed` | Delegates whole Eval/Think requests to the first reachable remote. With no execution remotes it runs locally; a failed configured delegation is an error. |
| `onepass` | Prefers cached results, then maximizes resident input and dependency-output bytes. Ties favor greater parallelism, then the local worker. |
| `hint` | The default scheduler. Starts with OnePass placement, pulls computations back when transferring their outputs would cost more, and spreads wide independent work with weak locality across available remote capacity. |

## Reading the examples

[Serial](serial.rs) checks memoized and persisted coupons, then explicitly calls the collector's corresponding coupon-evaluation helper. Its callbacks resolve dependencies immediately by recursive evaluation. [Parallel](parallel.rs) claims shared work, queues dependency evaluations, applications, and selections, and wakes their futures when coupons become available. Their execution policies differ even with one worker.

Both files explicitly choose memoization, registered-source retrieval order, and coupon retention. These choices are not defaults in the SDK. Each program's `run` function chooses its worker count and final-result retention.

[Distributed](distributed.rs) delegates whole Eval/Think requests to the first reachable remote and explicitly uses Parallel for local requests, retrieval, and retention.

[OnePass](onepass.rs) discovers available dependencies, interprets advice, assigns dependencies before parents, and dispatches work. It uses graph operations to coordinate local dependencies and explicitly calls collector helpers with its own evaluation methods and coupon retention callback. [Hint](hint.rs) calls OnePass's discovery and dispatch operations and adds backward propagation and redistribution to assignment. Each policy spells out its scheduling sequence in its own file.

The SDK supplies Fix evaluation semantics, coupon construction, input inventories, shared maps, and execution/transfer operations. It does not choose retrieval order, memoization, retention, traversal, or placement.

## Implementing a scheduler

Implement `Scheduler` directly. Each evaluation callback takes a Fix value and returns an optional Fix value. `load` returns backing data. Defaults evaluate without creating coupons; a scheduler can also choose a different result representation, such as coupon Tags. Every step has a default. An override can call it explicitly; recursive calls still use the scheduler's overrides:

```rust
fn think(&self, thunk: &Value) -> Result<Value, Error> {
    self.default_think(thunk)
}
```

Evaluation defaults require no collector. Coupon-producing policies explicitly call `collector::eval(self, input, &mut |coupon| self.retain(coupon))` and the corresponding helpers for their other steps. Arbitrary exchanges and deferred request construction are exposed by `collector::exchange` and `collector::request`.

For example, the [recomputation scheduler](tests/recompute.rs) uses the graph directly without consulting persisted claims. Its collector calls recurse through Recompute and supply a callback that discards produced coupons. Retrieval is a separate call to Parallel's `load`.

Register a program with `fix_scheduler(name = "NAME", modules = ["parallel.rs"])`. List the other scheduler files it uses in `modules`. The rule provides the Wasm entry point and allocator; `NAME::run` contains the program's launch and final-retention choices.

Each scheduling round uses a map from request to `(worker, locality score)`. Worker zero is local. OnePass prefers cached results, then the worker with the most resident bytes; ties favor usable parallelism, capped by advice when supplied, then local execution. Without a locality candidate, explicit parallelism advice chooses usable capacity; otherwise it stays local. Local scores count retained bytes; remote scores measure the advantage over local execution. Higher scores run first.

Hint starts with OnePass's assignments. It pulls back work when the estimated output outweighs the remote locality advantage, then reconsiders parents after their children. It retains twice the local parallelism and sends lower-score excess work round-robin to a prefix of remotes with sufficient capacity. Assignments are rebuilt as evaluation reveals more dependencies. Dispatch stops at a remotely assigned root, leaving its descendants to that worker.

These follow the original OnePass and Hint design without its pass-class hierarchy. Eval, Think, Force, and Select requests can be assigned remotely. Force and Encode wrappers follow a single dependency’s placement. Known descriptor sizes and Tree dependencies supply output estimates; unknown application output defaults to one byte. Inventories and cached-result advertisements are hints; imported coupons still require valid certificates and a matching collector, operation, and input.

## Advice and memory limits

Applications can use `fixwasm::advise(target, key, payload)` with these keys:

- `fix::output-size`: estimated output bytes.
- `fix::parallel`: estimated number of independent jobs. One means serial work; larger counts describe parallelism that will become available after expansion. Zero leaves parallelism unspecified.

Payloads are eight-byte little-endian unsigned integers. Advice can target an available combination before returning its application thunk. It does not change the value or its identity. Advice is delivered synchronously through `on_advice`; it carries no continuation or reply. Execution continues automatically after the callback. The supplied advice handler ignores unknown keys, malformed payloads, and unavailable targets.

`compile` advises the next compilation thunk about the independent C compilation jobs before they are generated. It uses the input Wasm size as a rough output-size estimate; these estimates do not affect evaluation results or progress totals.

Every application declares its memory limit as an eight-byte little-endian unsigned integer at combination index zero. The runtime rejects missing or malformed limits. Unprivileged execution charges conservative live memory independently of page sharing and copy-on-write. Exhaustion returns `[Blob("Fix.memory"), Blob(limit)]`; admission failure returns a retryable error without an Apply coupon. Privileged schedulers are exempt from execution limits. `kernel::worker::available_memory()` reports current unreserved memory. OnePass and Hint compare application limits against worker availability. See [memory accounting](../../../arca/kernel/MEMORY.md) for the accounting contract.

With equal input locality, a parallelism estimate favors a machine that can use more of the expected jobs. Hint keeps that capacity advantage when considering whether to pull work back locally. Without advice, OnePass uses actual input locality, cached results, and worker parallelism. Hint uses observed dependency fan-out. OnePass and Hint fall back to local execution when a remote fails. Unknown memory capacity does not exclude a worker.

## Remote workers

Bootstrap the coordinator and workers from the same seed and trust each other's
signing keys. The bootstrap store supplies the collector and local scheduler;
these do not need labels in each repository. The [SSH setup](../../../fix/remote/README.md#ssh-clone-and-distributed-evaluation)
shows how to build and label `distributed` in the coordinator and clone a worker
repository. An on-demand SSH worker first uses its bootstrapped scheduler to
receive the coordinator's selected scheduler, then runs with that selection.

For a prestarted TCP worker, run:

```sh
fix -C /path/to/worker serve --listen 127.0.0.1:9000 -j 4 --memory 16384
fix remote add worker tcp://worker.example:9000
```

The worker advertises inventory, cached Eval/Think results, parallelism, and its current unreserved memory, capped by its configured VM memory. Transfers use persistent connections, send only requested backing data, and supply certificates signed by keys the receiver trusts. The example buffers each transfer and limits frames and received backing bytes to 512 MiB. Other schedulers can choose different transports and acquisition policies.

Scheduler programs reserve an 8 MiB Wasm stack for recursive async evaluation.
