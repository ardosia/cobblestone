# C002 design

## Primitive crate

`cobblestone-core` begins as an independently testable safe Rust crate. `RuntimeId` is explicit and nonzero. `Handle<T>` contains stable slot identity plus a nonzero generation; stale generations fail lookup. Removing an object increments the generation before reuse. If the generation cannot be incremented, the slot is retired permanently instead of wrapping.

`NativeBuffer` owns immutable bytes through reference-counted native storage. Constructing from PHP/native mutable input performs an explicit copy or ownership transfer chosen by the adapter; cloning the Rust value is cheap and does not duplicate bytes.

## PHP/Zend adapter

The adapter targets a controlled PHP 8.5 ZTS build. The mechanism crate stays independent of Zend. Entry points validate thread/runtime affinity, convert invalid/stale handles into safe PHP errors, and contain Rust panics before they can cross FFI.

Diagnostic proof APIs may expose runtime identity and primitive operations under an internal namespace, but runtime IDs/handles are not ordinary plugin concepts.

## Workers and completions

The first worker substrate is intentionally dependency-light and Zend-free. A fixed set of named native threads receives owned jobs from a bounded submission queue. Submission is nonblocking: saturation returns the original job as an explicit `Full` result instead of growing memory without bound.

Each accepted job has a stable `TaskId` and a cooperative `CancellationToken`. Cancellation observed before execution prevents the handler from running. Once execution begins, the handler owns its cancellation points by checking the token.

The completion queue is also bounded. Workers block when the owning side does not drain completions, propagating backpressure until the submission queue also saturates rather than creating an unbounded completion backlog.

Handler panics are contained inside the worker loop and become `Panicked` completions. This is internal worker isolation only; it does not satisfy the separate PHP/Zend FFI panic-boundary proof.

Graceful shutdown drops the sole submission sender, drains every already-accepted job, actively drains completions so workers cannot deadlock on a full completion queue, and then joins every native worker. The mechanism crate has no PHP/Zend dependency, so arbitrary Zend calls from these workers are structurally absent from this surface.

This first implementation uses the standard-library synchronous channels behind the Cobblestone-owned abstraction. It is not a performance claim or a commitment to the final queue implementation; queue/worker measurements decide whether the mechanism changes later.

## ABI

Sibling native extensions must not reach through private Rust layouts. If cross-extension calls are needed, C002 introduces only the smallest versioned ABI surface required by a real sibling integration. No speculative broad ABI is created.

## Measurement

Benchmarks are added with the mechanism they measure. Results record build/profile/target and exact revision. No throughput/player-count claims are accepted from microbenchmarks alone.
