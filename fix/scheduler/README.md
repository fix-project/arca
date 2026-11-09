# Scheduler SDK

The SDK provides Rust bindings to existing interfaces:

- `shell` re-exports the Fix SDK's handles, constructors, inspection, attachment, and backend operations.
- `kernel` exposes callable execution and continuations, maps, queues, worker notifications, and streams through the current host interface. Apply and Invoke return either a result or an advice event with a continuation. Resumption and queue notifications are explicit.
- `collector` encodes the coupon collector's exchange request: `[metadata, collector, rule, lhs, rhs, witnesses]`. Each witness contains a coupon and its assertion Object. The collector checks supplied premises; it does not find dependencies or load references. `request` constructs a callable combination; `exchange` invokes it.
- `storage` exposes object identification, claim lookup, certificates, persistence, and acquisition metadata. `identify` returns `None` for an unavailable object without acquiring it.

The SDK contains no scheduler loop, task graph, evaluator, cache policy, or advice handler. Its operations use the existing host ABI; the worker bindings describe the current kernel-thread harness, not a new execution model. Local handles and continuation tokens are scoped to that host and are not network identities.

## Examples

`examples/serial.rs` evaluates dependencies inline through recursive calls. It has no dependency graph or work queue. `examples/parallel.rs` owns a shared dependency graph, ready queue, worker stacks, and notification policy. `//programs/fix:scheduler_elf` builds the parallel example used by the CLI. `//fix/scheduler:serial` builds the serial example.

The examples share their own proof, caching, advice, and acquisition code in `examples/runtime`. Their serial evaluator and parallel evaluation state machine are also example code, not SDK interfaces. Both retain the existing canonicalization-advice policy: only available targets are recorded, recommendations propagate through input-preserving proof steps, and policy state lasts for one run.

Runtime and CLI integration tests exercise both examples, including shared dependencies, repeated premises, applications, selection, and coupon verification.
