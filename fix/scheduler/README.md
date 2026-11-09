# Scheduler components

This library provides operations that a scheduler can call from its own loop. It does not choose an event loop, work placement, ownership, transport, or scheduling policy.

- `Request` describes a Fix statement and input.
- `EvaluationState::step` advances one evaluation using supplied dependency results, proof operations, and object lookup. It returns `Complete`, `Dependencies`, or `Primitive`. Apply, Select, and value-Eval primitives are returned for the caller to execute locally or send elsewhere.
- `Collector`, `CachedClaims`, and `Evidence` provide proof construction and claim lookup. `Evidence::admit` checks a returned coupon against the requested statement/input and accepted authority. Advice is never evidence. The host still verifies the final result.
- `effects` exposes callable invocation, storage, queues, and the current driver's worker services. Advice is delivered through an invocation callback; no central advice registry is imposed.
- `//fix/scheduler/dependencies` tracks a suspended computation's resolved and missing dependency slots. It does not run computations, send messages, or choose their location.

A caller can match the result of `state.step(...)`: store a completed proof, enqueue missing dependencies according to its own policy, or execute/offload a primitive request. The current core's loop owns all these choices. Using the evaluation component requires no scheduler trait or dispatch protocol.

`tests/primitives.rs` contains a complete small scheduler with its own stack and loop. It executes selection locally and sends application primitives through a queue to another worker, admitting their returned coupons before resuming evaluation. Its integration test compares semantic results with one and four workers.

Complete schedulers are examples: `examples/runtime` contains the existing work-sharing engine used by `programs/fix/scheduler.rs`; `examples/control` contains a centralized mailbox implementation and its own command protocol. Their loops and interfaces are not SDK requirements. These examples use the existing worker harness, whose native driver still creates kernel threads; direct integration into native per-core loops is separate work.

`Request::key` uses local handle identity. A network scheduler must choose canonical task identities, transfer the corresponding objects and certificates, and admit received proofs into the local store. `fix::stream` provides byte-stream transport, but this library specifies no network protocol.

The supplied `Canonicalize` policy records only available advised targets and does not acquire or evaluate data solely because of a hint. Its recommendation applies to the matching invocation coupon; `InputRetention` propagates that recommendation through input-preserving proof steps. Policy state lasts for one scheduler run. Source compilation and execution currently use separate runs, so hints from compilation do not carry into execution. Other handlers may ignore advice or implement a different policy.
