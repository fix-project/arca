# Scheduler SDK

`fixscheduler` supplies evaluation, coupons, coordination, and transfer operations. Policies live in [programs/fix/schedulers](../../programs/fix/schedulers/README.md).

## Evaluator and scheduler

Each `Scheduler` is an evaluator. Its trait methods provide the Fix evaluation rules by default; policies override individual steps.

| Method | Direction and purpose |
|---|---|
| `eval(value)` | Evaluate a value. |
| `think(thunk)` | Perform one thunk step. |
| `force(thunk)` | Follow thunk results to an Object or Ref. |
| `apply(combination)` | Apply an evaluated combination. |
| `select(combination)` | Select from an evaluated combination. |
| `load(value)` | Obtain backing data. |
| `on_advice(advice)` | Execution → scheduler: observe advice without replying to its emitter. The default does nothing. |

Each evaluation method forwards to its `default_*` trait method. An override can call, for example, `self.default_think(thunk).await` to perform the default step. The default calls `self.eval`, `self.apply` or `self.select`, and `self.load`, so recursive dependencies return to the scheduler's overrides. Unmodified steps need no implementation. The default `load` identifies locally available data and returns an error when unavailable. Identification and Encode unwrapping inspect descriptors without loading backing. Eval loads Blob Objects while leaving Refs lazy; Select asks the scheduler to load its data target before accessing it. Policies can place selection near its existing data before calling the evaluator helper.

Steps are native `async fn` methods returning `Result<Value, Error>`. Pending dependencies suspend the Rust future; the returned Fix value is chosen by the scheduler. Evaluator helpers are generic over `Scheduler`, and recursive calls are boxed where Rust requires it. Defaults return evaluated values without producing coupons. A scheduler using the coupon-free default evaluator must be a trusted tagger when evaluating Tags that require reconstruction. The collector is configured by the current invocation. Schedulers keep their state and composed policies private and can borrow a shared dependency graph.

`collector` is a separate, optional API whose functions use the collector from the current invocation. `exchange` invokes a requested exchange; `request` builds the invocation for later use. Its `eval`, `think`, `force`, `apply`, and `select` helpers eagerly construct coupons and require dependency callbacks to return coupons too. Each helper accepts an explicit callback for retaining produced coupons, including intermediate exchanges. `validate` checks a coupon against the expected collector, operation, and input without involving a scheduler or graph. Existing policies explicitly choose these helpers. `collector::result` extracts the evaluated value from a coupon. The scheduler controls when to call these operations, including never.

`graph::Node` identifies work in the dependency graph by operation and input. It is used for deduplication, queueing, and placement; it is not a callback argument or a wrapper around coupons. `Graph::within` establishes the current parent only while polling a dependency future. Serial looks up coupons through the collector API and runs dependencies inline.

Serial checks available coupons and awaits missing dependencies in order. Parallel submits independent dependencies together. Their futures await shared results without restarting evaluation. Parallel submits tree children together and awaits their shared results with `try_join_all`; Serial evaluates them in order. `executor::run` supplies the event loop and returns the requested result; the program chooses final retention and calls `worker::finish`.

OnePass calls the collector's coupon-evaluation helpers while constructing its dependency graph: data acquisition inspects local availability, and applications and selections become leaves. It then assigns dependencies before parents and dispatches selected subgraphs. Hint propagates choices from parents to children and reconsiders parents afterward. The scheduler owns these traversals.

Schedulers implement `Scheduler` directly. A scheduler can compose another scheduler by calling its trait methods. Calling `self.default_eval(...).await` preserves the caller's overrides, while calling `other.eval(...).await` selects the other scheduler's behavior.

## Operations

- `Scheduler::default_*` methods implement Fix evaluation through overridable trait steps. Collector operations are separate from evaluation defaults.
- `Graph` coalesces requests and provides dependency, queue, and completion operations. Completing a request publishes an opaque Fix value and wakes dependents. Results need not be coupons and remain available independently of persistent memoization. `evaluate` coordinates inline dependency evaluation; `defer` submits a dependency and awaits its result. `submit` starts shared work independently of a waiter. The scheduler chooses which operation to use.
- `Map::named` opens a shared map by byte-string name within the scheduler run. `Queue::named` likewise opens a shared queue, requiring callers to agree on its capacity. Neither depends on allocation order. Maps provide get, batched membership checks, insert, remove, and versioned compare-exchange.
- `storage` exposes identification, canonicalization, retention, eviction, claim lookup, source metadata, and acquisition. `evict` releases a blob from RAM. Existing readers retain their bytes. A saved blob can be loaded again by canonical name. The scheduler is responsible for recovering unsaved data it needs again. `inventory` enumerates reachable backing objects and their sizes; it does not fetch missing objects.
- `remote` exposes import/export, missing-object negotiation, certificates, and signing independently of transport. `Cluster` discovers workers over bidirectional TCP and SSH connections. Reachable workers have stable slots within a cluster, with duplicate connections sharing one worker slot. Placement sees the wider graph; requests go to the selected route's immediate neighbor, which schedules them independently. Connecting, transferring, and receiving use the scheduler's I/O registrations. `submit_transfer` accepts a proposal with caller-selected backing data and TODOs; `submit` prepares that proposal using the same object export mechanism as retrieval. Backing is read only after the receiver accepts it. `PendingTransfer::receive` waits for a validated coupon, and `execute` combines submission and completion.

Canonicalization, persistence, movement, and replication are operations the scheduler calls. The SDK does not interpret advice or choose destinations, retention, retrieval order, traversal, or remote-failure behavior.

## Advice and execution

Advice is a synchronous, one-way notification. The scheduler receives the target, key, and payload through `on_advice`; it receives no continuation and supplies no reply to the application. The SDK returns automatically to the invocation after the callback. The runtime retains its native invocation during delivery rather than constructing a Fix continuation. There is no advice queue.

One scheduler instance runs on its own kernel thread. Application and collector invocations run asynchronously on a fixed pool of kernel threads; `-j` sets the pool size. The scheduler shares VM cores with these workers. Running guest code is not preempted when its future is dropped. Advice provides no cancellation or rescheduling operation. Multiple remote requests can be outstanding; dropping a pending evaluation closes its logical request channel but does not cancel the remote computation.

`Graph::default` creates an in-process graph using Rust maps, a queue, and wakers. Schedulers share it by reference. Persistent deletion is not exposed yet.

A scheduler receives `[memory_limit, scheduler, mode, collector, input]`. Modes include `eval`, `think`, `force`, `apply`, `select`, `todo` source management, and `serve` for a persistent TCP server. Local handles belong to their runtime; network transfers use canonical names and validated certificates.

The event loop returns the scheduler's chosen Fix value without interpreting it as a coupon. The existing coupon-producing programs explicitly choose final coupon retention and result extraction. Remote certified-evaluation requests still require a valid matching coupon.

## Serving requests

`fix serve` starts one VM running the configured scheduler in `serve` mode.
The SDK's server handles the transfer protocol and polls connections in the single scheduler instance. Requests call the scheduler's evaluation methods; schedulers using a graph
share it across requests. The certified-evaluation protocol requires
a matching coupon for each response.

Tasks in a scheduler instance share completion registrations through `io::Io`, which submits effects to kernel threads
and registers each pending future's Rust waker. After a kernel notification, `Io::wake_completed` drains
completed effect tokens and wakes their tasks. `executor::task::Task` polls a
future only initially or after its waker fires; self-wakes remain ready for the
next turn. The event loop snapshots the worker notification before draining
completions and checks task readiness before sleeping.

`Graph::wait` awaits an explicitly submitted request. Completing a graph operation
wakes its waiting Rust tasks directly.
All connections share the VM's storage, worker pool, and memory budget.
Evaluating schedulers also serve requests arriving over their outgoing connections.
A remote can therefore return work through its coordinator to reach another worker.
Names use a local remote alias when available, or a `!`-separated chain for an indirect route. Unnamed links use the runtime identity.
The protocol's synchronous and asynchronous APIs use the same implementation.

## Cancellation

Dropping a graph waiter removes its waker registration without cancelling the
shared computation. Executor-owned tasks keep that computation alive. Dropping an
owning evaluation future requeues its operation so other waiters can still make
progress; completed results and errors are shared. Dropping an I/O future removes
its registration and discards its eventual result, but does not imply interruption
of an already submitted kernel operation. No cancellation of a remote computation
is implied by closing its connection.
