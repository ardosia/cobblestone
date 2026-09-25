# C002 design

## Primitive crate

`cobblestone-core` begins as an independently testable safe Rust crate. `RuntimeId` is explicit and nonzero. `Handle<T>` contains stable slot identity plus a nonzero generation; stale generations fail lookup. Removing an object increments the generation before reuse. If the generation cannot be incremented, the slot is retired permanently instead of wrapping.

`NativeBuffer` owns immutable bytes through reference-counted native storage. Constructing from PHP/native mutable input performs an explicit copy or ownership transfer chosen by the adapter; cloning the Rust value is cheap and does not duplicate bytes.

## PHP/Zend adapter

The first diagnostic adapter is isolated from the stable mechanism workspace in `native/cobblestone-core-php`. It uses `ext-php-rs = 0.15.15` as an implementation substrate because that release supports PHP 8.5. The dependency is not part of Cobblestone's public plugin API and may be replaced without changing gameplay APIs.

Linux builds the diagnostic adapter with the pinned Rust 1.98.0 toolchain. The current ext-php-rs Windows substrate requires Rust nightly for the unstable `vectorcall` ABI, so the Windows proof uses a separately pinned nightly toolchain only for this isolated adapter crate. The stable `cobblestone-core` workspace remains Rust 1.98.0 on both platforms.

CI installs exact PHP 8.5.11 thread-safe/ZTS builds on Linux and Windows and first asserts the runtime reports `PHP_ZTS`/`ZEND_THREAD_SAFE`.

The diagnostic extension exposes only proof surfaces:

- a positive internal runtime identity stored per PHP execution thread;
- creation/validation/release of opaque native probe tokens backed by the real generational `Arena`;
- stale/unknown probe release translated into a PHP exception instead of unchecked native access.

These functions are C002 diagnostics, not normal plugin APIs. Runtime IDs and raw handles remain implementation concepts.

The separate FFI panic-boundary task is not satisfied merely by ext-php-rs loading successfully. Cobblestone still needs an explicit, tested policy proving Rust panics cannot unwind across the Zend boundary.

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
